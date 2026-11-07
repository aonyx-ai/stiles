use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::Context as _;
use bon::bon;
use stiles_core::{Lint, PHASE_TARGET, Pipeline, RunPipelineError};
use stiles_types::{Diagnostic, RuleOptions};

use super::grammar::Grammar;
use super::providers::Providers;
use crate::custom_lints::CustomLints;

/// What checking one file produced, before the run reports it
///
/// The files of a run are checked on every core, in no fixed order. The run
/// reports each check afterwards, in the order of the files. The report of a
/// run therefore does not depend on which thread finished first.
#[derive(Debug)]
pub(super) enum FileCheck {
    /// The file could not be read
    Unreadable {
        /// The error of the read
        source: std::io::Error,
    },

    /// No grammar of stiles parses the file
    NoGrammar,

    /// The pipeline ran on the file
    Checked {
        /// The text of the file, kept only when the passes reported on it
        ///
        /// A report quotes the lines of a diagnostic, so it needs the text.
        /// A file without diagnostics drops its text right away, so a run
        /// does not hold every file of a large project at once.
        text: Option<String>,
        /// The diagnostics of the file, or why the pipeline stopped on it
        result: Result<Vec<Diagnostic>, RunPipelineError>,
    },
}

/// Checks a list of files on every core of the machine
///
/// Each worker takes the next file that no worker has taken, until none is
/// left. A worker keeps one pipeline per grammar, because a parser holds
/// state that only one thread may use. It builds a fresh set of passes for
/// every file, as a sequential run does, because a pass holds state.
#[derive(Debug)]
pub(super) struct FileChecks;

#[bon]
impl FileChecks {
    /// Checks every file of `files`, and returns the checks in their order
    ///
    /// A file in `texts` is checked with the text it maps to, and any other
    /// file with its text on disk. `stiles check --fix` changes files in
    /// memory, and checks the changed text before it writes anything.
    ///
    /// # Errors
    ///
    /// Returns an error if the passes cannot be configured, if a pipeline
    /// cannot be built, or if a worker thread panics. An error stops every
    /// worker at its next file.
    #[builder]
    pub(super) fn run(
        files: &[PathBuf],
        texts: &HashMap<PathBuf, String>,
        providers: &Providers,
        custom_lints: &CustomLints,
        options: &RuleOptions,
    ) -> anyhow::Result<Vec<FileCheck>> {
        let workers = std::thread::available_parallelism()
            .map_or(1, NonZeroUsize::get)
            .min(files.len())
            .max(1);
        let next = AtomicUsize::new(0);
        let stopped = AtomicBool::new(false);

        let worker = || -> anyhow::Result<Vec<(usize, FileCheck)>> {
            let mut pipelines: HashMap<Grammar, Pipeline> = HashMap::new();
            let mut checks = Vec::new();

            while !stopped.load(Ordering::Relaxed) {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(file) = files.get(index) else { break };

                let check = check_file()
                    .file(file)
                    .maybe_text(texts.get(file))
                    .pipelines(&mut pipelines)
                    .providers(providers)
                    .custom_lints(custom_lints)
                    .options(options)
                    .call();
                match check {
                    Ok(check) => checks.push((index, check)),
                    Err(error) => {
                        stopped.store(true, Ordering::Relaxed);
                        return Err(error);
                    }
                }
            }

            Ok(checks)
        };

        let mut checks: Vec<(usize, FileCheck)> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers).map(|_| scope.spawn(worker)).collect();
            let mut checks = Vec::with_capacity(files.len());
            for handle in handles {
                let Ok(worker_checks) = handle.join() else {
                    anyhow::bail!("a thread that checks files panicked");
                };
                checks.extend(worker_checks.context("failed to check a file")?);
            }
            Ok(checks)
        })
        .context("failed to check the files")?;

        checks.sort_by_key(|(index, _)| *index);
        Ok(checks.into_iter().map(|(_, check)| check).collect())
    }
}

/// Builds the lint passes the CLI runs
///
/// Stiles links no rules of its own. Every rule is a plugin that the
/// project names in its configuration, and stiles builds and loads it at
/// the start of a run. A rule that is not configured does not run, however
/// complete its own tests are. Nothing is enabled by default, and the
/// configuration is the whole answer to what a check does.
fn create_lint_passes(
    custom_lints: &CustomLints,
    options: &RuleOptions,
) -> anyhow::Result<Vec<Lint<'static>>> {
    custom_lints.instantiate(options)
}

/// Checks one file on the current thread
///
/// The file holds `text` when the caller passes one, and its text on disk
/// otherwise.
///
/// # Errors
///
/// Returns an error if the passes cannot be configured, or if the pipeline
/// of the grammar of the file cannot be built. A failure of the file itself
/// is a [`FileCheck`], not an error.
#[bon::builder]
fn check_file(
    file: &PathBuf,
    text: Option<&String>,
    pipelines: &mut HashMap<Grammar, Pipeline>,
    providers: &Providers,
    custom_lints: &CustomLints,
    options: &RuleOptions,
) -> anyhow::Result<FileCheck> {
    let text = match text {
        Some(text) => text.clone(),
        None => match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(source) => return Ok(FileCheck::Unreadable { source }),
        },
    };

    let mut passes = tracing::info_span!(target: PHASE_TARGET, "configure")
        .in_scope(|| create_lint_passes(custom_lints, options))
        .context("failed to configure the custom lints")?;

    let Some(grammar) = Grammar::of(file) else {
        return Ok(FileCheck::NoGrammar);
    };
    let pipeline = match pipelines.entry(grammar) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => entry.insert(
            grammar
                .pipeline()
                .context("failed to initialize pipeline")?,
        ),
    };
    let providers = providers.for_file(file, grammar.language());

    let result = pipeline.run_on_source(&text, file, &providers, &mut passes);
    let text = match &result {
        Ok(diagnostics) if !diagnostics.is_empty() => Some(text),
        Ok(_) | Err(_) => None,
    };

    Ok(FileCheck::Checked { text, result })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_file_check() {
        fn assert_send<T: Send>() {}
        assert_send::<FileCheck>();
    }

    #[test]
    fn trait_send_file_checks() {
        fn assert_send<T: Send>() {}
        assert_send::<FileChecks>();
    }

    #[test]
    fn trait_sync_file_check() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FileCheck>();
    }

    #[test]
    fn trait_sync_file_checks() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FileChecks>();
    }

    #[test]
    fn trait_unpin_file_check() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FileCheck>();
    }

    #[test]
    fn trait_unpin_file_checks() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FileChecks>();
    }
}
