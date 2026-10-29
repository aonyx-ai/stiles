mod check_outcome;
mod error_recovery;
mod failure_threshold;
mod file_failure;
mod finding;
mod grammar;
mod providers;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clawless::exit::Exit;
use clawless::prelude::*;
use stiles_core::{Lint, Pipeline, RunPipelineError};
use stiles_types::{Diagnostic, RuleOptions, UncoveredFile};

use self::check_outcome::CheckOutcome;
use self::error_recovery::{ErrorRecovery, Walk};
use self::failure_threshold::FailureThreshold;
use self::file_failure::FileFailure;
use self::finding::Finding;
use self::grammar::Grammar;
use self::providers::Providers;
use super::closing_text::ClosingText;
use crate::config::StilesConfig;
use crate::custom_lints::CustomLints;
use crate::discovery::{Discovery, WalkErrorPolicy};

/// Run stiles lints against a project
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Path to the target project directory
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Continue checking after encountering errors
    #[arg(long)]
    keep_going: bool,

    /// Treat warnings as errors
    #[arg(long)]
    deny_warnings: bool,
}

/// Builds the lint passes the CLI runs
///
/// Stiles links no rules of its own. Every rule is a plugin the project
/// names in its configuration, which stiles builds and loads at the start
/// of a run, so a rule that is not configured does not run however
/// complete its own tests are. Nothing is enabled by default, and the
/// configuration is the whole answer to what a check does.
fn create_lint_passes(
    custom_lints: &CustomLints,
    options: &RuleOptions,
) -> anyhow::Result<Vec<Lint<'static>>> {
    custom_lints.instantiate(options)
}

/// The distinct remedies for the files this run could not analyze
///
/// The run collects remedies during the walk and prints each one once.
/// Per-file printing would repeat the same sentence for every skipped file
/// and hide the diagnostics. Each remedy comes from a [`CoverageGap`], so a
/// run that trips two kinds of gap prints both fixes.
///
/// [`CoverageGap`]: stiles_types::CoverageGap
#[derive(Clone, Eq, PartialEq, Debug, Default)]
struct CoverageRemedies {
    remedies: Vec<&'static str>,
}

impl CoverageRemedies {
    /// Records the remedy for every gap reported against one file
    ///
    /// The method drops duplicate remedies, so the run prints at most one
    /// line per [`CoverageGap`] variant.
    ///
    /// [`CoverageGap`]: stiles_types::CoverageGap
    fn record(&mut self, uncovered: &UncoveredFile) {
        for (_provider, gap) in uncovered.gaps() {
            let remedy = gap.help();
            if !self.remedies.contains(&remedy) {
                self.remedies.push(remedy);
            }
        }
    }

    /// Reports each remedy once, after the per-file errors
    ///
    /// The per-file error names the file and the reason; this names the fix.
    /// A remedy is a message, so `--quiet` hides it.
    ///
    /// # Errors
    ///
    /// Returns an error if the presenter has stopped listening.
    async fn report(&self, context: &Context) -> anyhow::Result<()> {
        for remedy in &self.remedies {
            context
                .output()
                .message(format!("help: {remedy}"))
                .await
                .context("failed to report a remedy")?;
        }

        Ok(())
    }
}

#[command]
pub async fn check(args: CheckArgs, context: Context) -> CommandResult<Exit> {
    let mut closing = ClosingText::default();

    match run(args, &context, &mut closing).await {
        Ok(code) => Ok(closing.into_exit(code)),
        Err(error) => Ok(closing.into_failure(&error)),
    }
}

/// Runs the check, and leaves its errors and notices in `closing`
///
/// The diagnostics go to the presenter as the walk finds them. The errors
/// of the files the walk could not check, and the notices of the lint
/// loading, go into `closing`. The command returns them in its exit, so
/// they reach the standard error after the report. An error that ends the
/// run comes back to the command, which puts it after them.
///
/// # Errors
///
/// Returns an error if the run cannot start or cannot report a result.
async fn run(
    args: CheckArgs,
    context: &Context,
    closing: &mut ClosingText,
) -> anyhow::Result<ExitCode> {
    let CheckArgs {
        path,
        keep_going,
        deny_warnings,
    } = args;

    let threshold = match deny_warnings {
        true => FailureThreshold::Warnings,
        false => FailureThreshold::Errors,
    };

    let recovery = match keep_going {
        true => ErrorRecovery::Continue,
        false => ErrorRecovery::Abort,
    };

    anyhow::ensure!(path.exists(), "{} does not exist", path.display());

    let config = StilesConfig::load(&path).context("failed to load the stiles configuration")?;

    let mut notices = Vec::new();
    let custom_lints = CustomLints::load()
        .config(&config)
        .notices(&mut notices)
        .call();
    closing.extend(notices);
    let custom_lints = custom_lints.context("failed to load the project's custom lints")?;

    let declared = custom_lints.declared();

    config
        .rules()
        .validate(&declared)
        .context("failed to read the project's [rules]")?;

    config
        .options()
        .validate(&declared)
        .context("failed to read the project's [rules.options]")?;

    let on_error = match keep_going {
        true => WalkErrorPolicy::ReportAndContinue,
        false => WalkErrorPolicy::Fail,
    };

    let discovery = Discovery::run()
        .path(&path)
        .config(&config)
        .on_error(on_error)
        .call()
        .context("failed to discover source files")?;

    let mut outcome = CheckOutcome::Success;

    for error in discovery.errors() {
        closing.push(format_args!("error: {error:#}"));
        outcome = CheckOutcome::Failure;
    }

    let files = discovery.files();

    anyhow::ensure!(
        !files.is_empty(),
        "stiles analyzed no files under {}: either nothing there is written in a language \
         stiles has a grammar for, or the ignore files and configured patterns excluded all of \
         it",
        path.display()
    );

    let providers = Providers::load().path(&path).files(files).call()?;
    let mut pipelines: HashMap<Grammar, Pipeline> = HashMap::new();

    let mut all_diagnostics = Vec::new();
    let mut sources: HashMap<Arc<Path>, String> = HashMap::new();
    let mut remedies = CoverageRemedies::default();
    let mut failures: Vec<FileFailure> = Vec::new();

    for file in files {
        context
            .output()
            .detail(format!("checking {}", file.display()))
            .await
            .context("failed to report progress")?;

        let source = match std::fs::read_to_string(file) {
            Ok(source) => source,
            Err(e) => match recovery.record(&mut failures, file, anyhow::Error::new(e)) {
                Walk::Continue => continue,
                Walk::Stop => break,
            },
        };

        let mut passes = create_lint_passes(&custom_lints, config.options())
            .context("failed to configure the custom lints")?;

        let Some(grammar) = Grammar::of(file) else {
            let error = anyhow::anyhow!("stiles has no grammar for {}", file.display());
            match recovery.record(&mut failures, file, error) {
                Walk::Continue => continue,
                Walk::Stop => break,
            }
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

        match pipeline.run_on_source(&source, file, &providers, &mut passes) {
            Ok(diagnostics) => {
                let diagnostics: Vec<_> = diagnostics
                    .into_iter()
                    .filter(|diagnostic| config.rules().admits(&diagnostic.rule_id()))
                    .collect();

                if !diagnostics.is_empty() {
                    let arc_path: Arc<Path> = file.clone().into();
                    sources.insert(arc_path, source);
                    all_diagnostics.extend(diagnostics);
                }
            }
            Err(error) => {
                if let RunPipelineError::Uncovered { source } = &error {
                    remedies.record(source);
                }

                match recovery.record(&mut failures, file, anyhow::Error::new(error)) {
                    Walk::Continue => {}
                    Walk::Stop => break,
                }
            }
        }
    }

    for failure in failures {
        closing.push(failure);
        outcome = CheckOutcome::Failure;
    }

    remedies.report(context).await?;

    let all_diagnostics: Vec<Diagnostic> = all_diagnostics
        .into_iter()
        .map(|diagnostic| threshold.promote(diagnostic))
        .collect();

    for diagnostic in &all_diagnostics {
        let finding = Finding::builder()
            .diagnostic(diagnostic)
            .sources(&sources)
            .build()?;
        context
            .output()
            .artifact(finding)
            .await
            .context("failed to report a diagnostic")?;
    }

    let outcome = outcome.combine(
        CheckOutcome::from_diagnostics()
            .diagnostics(&all_diagnostics)
            .threshold(threshold)
            .call(),
    );

    match outcome {
        CheckOutcome::Failure => Ok(ExitCode::FAILURE),
        CheckOutcome::Success => Ok(ExitCode::SUCCESS),
    }
}

#[cfg(test)]
mod tests {
    use stiles_types::{CoverageGap, ProviderName};

    use super::*;

    fn root() -> Arc<Path> {
        Arc::from(PathBuf::from("/ws"))
    }

    #[test]
    fn record_with_no_gaps_collects_no_remedy() {
        let uncovered = UncoveredFile::builder()
            .file(PathBuf::from("stray.rs"))
            .gaps(Vec::new())
            .build();
        let mut remedies = CoverageRemedies::default();

        remedies.record(&uncovered);

        assert!(remedies.remedies.is_empty());
    }

    #[test]
    fn record_with_repeated_gap_keeps_one_remedy() {
        let gap = CoverageGap::Unreachable { root: root() };
        let first = UncoveredFile::builder()
            .file(PathBuf::from("one.rs"))
            .gaps(vec![(ProviderName::new("rust"), gap.clone())])
            .build();
        let second = UncoveredFile::builder()
            .file(PathBuf::from("two.rs"))
            .gaps(vec![(ProviderName::new("rust"), gap)])
            .build();
        let mut remedies = CoverageRemedies::default();

        remedies.record(&first);
        remedies.record(&second);

        assert_eq!(
            remedies.remedies,
            [CoverageGap::Unreachable { root: root() }.help()]
        );
    }

    #[test]
    fn record_with_two_kinds_of_gap_keeps_both_remedies() {
        let uncovered = UncoveredFile::builder()
            .file(PathBuf::from("stray.rs"))
            .gaps(vec![
                (
                    ProviderName::new("rust"),
                    CoverageGap::Unreachable { root: root() },
                ),
                (ProviderName::new("other"), CoverageGap::StaleSource),
            ])
            .build();
        let mut remedies = CoverageRemedies::default();

        remedies.record(&uncovered);

        assert_eq!(
            remedies.remedies,
            [
                CoverageGap::Unreachable { root: root() }.help(),
                CoverageGap::StaleSource.help(),
            ]
        );
    }

    #[test]
    fn trait_send_check_args() {
        fn assert_send<T: Send>() {}
        assert_send::<CheckArgs>();
    }

    #[test]
    fn trait_send_coverage_remedies() {
        fn assert_send<T: Send>() {}
        assert_send::<CoverageRemedies>();
    }

    #[test]
    fn trait_sync_check_args() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CheckArgs>();
    }

    #[test]
    fn trait_sync_coverage_remedies() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CoverageRemedies>();
    }

    #[test]
    fn trait_unpin_check_args() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CheckArgs>();
    }

    #[test]
    fn trait_unpin_coverage_remedies() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CoverageRemedies>();
    }
}
