/// The arguments of a run of the action
mod args;
/// The projects that the benchmarks check
mod corpus;
/// What the benchmarks measured, and the file that holds it
mod results;

use std::path::Path;

use rakko_action::{
    Access, Action, Context, FilePath, Finding, Location, Name, Outcome, Summary, action_name,
};
use rakko_tool::{Execution, Invocation};

pub use self::args::BenchArgs;
use self::corpus::{CONFIG, Project, RulesPin};
use self::results::Measurement;
use crate::{HarnessError, step};

/// The directory under the project root that holds the corpus and the results
const OUTPUT: &str = "target/bench";

/// The file under [`OUTPUT`] that holds the measures of every project
const RESULTS: &str = "results.json";

/// The action that measures stiles on large codebases
///
/// The run builds stiles in release mode, as a user installs it. For each
/// project of the corpus, it fetches the pinned commit and writes a
/// configuration of stiles into the checkout. The configuration names the
/// rules that this repository pins, built against this working tree. The
/// run then checks the project three ways:
///
/// 1. One run that warms up. It builds the rules and downloads what the
///    toolchain of the project needs, so neither reaches the numbers.
/// 2. One run with `--timings`, which records the time of each phase.
/// 3. The timed runs, under hyperfine, which record the wall time and the
///    peak memory of the whole process.
///
/// Each check passes `--keep-going`, so a file that stiles cannot cover
/// does not cut a run short. The findings of the rules do not matter here.
/// Stiles exits with 1 for findings and for an error that ends the run, so
/// the exit code cannot tell the two apart. The first two runs therefore
/// write a record of their phases, and a run counts as finished when its
/// record reaches the report.
///
/// The results land in `target/bench`: the export of hyperfine and the
/// record of the phases of each project, and `results.json` with the
/// measures of every project in the Bencher Metric Format.
///
/// The numbers mean little on a shared machine, whose runs vary by a third.
/// The action belongs on quiet hardware, so its name carries no prefix that
/// makes CI run it on every pull request.
#[derive(Copy, Clone, Debug, Default)]
pub struct Bench;

impl Action for Bench {
    type Args = BenchArgs;

    fn name(&self) -> Name {
        action_name!("bench")
    }

    fn access(&self, _context: &Context, _args: &Self::Args) -> Access {
        Access::Read
    }

    async fn run(&self, context: &Context, args: &Self::Args) -> Outcome {
        step::outcome(drive(context, args).await)
    }
}

/// Runs the action against the project of the context
///
/// # Errors
///
/// Returns the error of the step that could not finish: the resolution of
/// a tool, a command that did not start, or a file of the run.
async fn drive(context: &Context, args: &BenchArgs) -> Result<Outcome, HarnessError> {
    let root = context.root().get();
    let cargo = step::cargo(context).await?;
    let hyperfine = step::tool(context, "hyperfine").await?;

    let Some(pin) = RulesPin::load(root)? else {
        return Ok(refused(format!(
            "{CONFIG} names no git lint source with a rev, so the corpus has no rules to run"
        )));
    };

    let build = step::run(cargo.args(["build", "--release", "-p", "stiles"])).await?;
    if !build.status().success() {
        return Ok(step::failed("building stiles", &build));
    }

    let stiles = root.join("target/release/stiles");
    let output = root.join(OUTPUT);
    let runs = args.runs().to_string();
    let mut measurements = Vec::new();

    for project in args.projects() {
        let name = project.name;
        let directory = output.join(name);
        let checkout = directory.join("checkout");

        if let Some(failed) = project.fetch(&checkout).await? {
            return Ok(step::failed(&format!("fetching {name}"), &failed));
        }
        project.configure(&checkout, root, &pin)?;

        let check = || {
            Invocation::new(stiles.as_path())
                .args(["check", "--keep-going", project.path])
                .in_directory(checkout.as_path())
        };

        let warm_up_record = directory.join("warm-up.json");
        let warm_up = step::run(check().arg("--timings").arg(warm_up_record.as_path())).await?;
        if !finished(&warm_up, &warm_up_record)? {
            return Ok(step::failed(
                &format!("warming up stiles on {name}"),
                &warm_up,
            ));
        }

        let timings = directory.join("timings.json");
        let timed = step::run(check().arg("--timings").arg(timings.as_path())).await?;
        if !finished(&timed, &timings)? {
            return Ok(step::failed(
                &format!("timing the phases of stiles on {name}"),
                &timed,
            ));
        }

        let export = directory.join("hyperfine.json");
        let measured = step::run(
            hyperfine
                .clone()
                .args(["--runs", &runs, "--ignore-failure", "--command-name", name])
                .arg("--export-json")
                .arg(export.as_path())
                .arg(command(&stiles, &project))
                .in_directory(checkout.as_path()),
        )
        .await?;
        if !measured.status().success() {
            return Ok(step::failed(
                &format!("measuring stiles on {name}"),
                &measured,
            ));
        }

        measurements.push(Measurement::load(name, &export, &timings)?);
    }

    self::results::write(&output.join(RESULTS), &measurements)?;

    let summary = measurements
        .iter()
        .map(Measurement::summary)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Outcome::Passed {
        summary: Some(Summary::new(summary)),
    })
}

/// Returns whether a run of stiles finished, with or without findings
///
/// Stiles exits with 1 when the rules report something or a file stays
/// uncovered, and the corpus has both. Any other code means that the run
/// broke, such as 101 for a panic. An error that ends the run exits with 1
/// as well, and leaves a record of its phases that stops before the report.
///
/// # Errors
///
/// Returns an error when the record exists and cannot be read or parsed.
fn finished(execution: &Execution, record: &Path) -> Result<bool, HarnessError> {
    match execution.status().code() {
        Some(0 | 1) => self::results::reached_report(record),
        Some(_) | None => Ok(false),
    }
}

/// Returns the command line of a timed run, as hyperfine reads it
///
/// Hyperfine starts the command without a shell, and splits the line into
/// words the way a POSIX shell does. Each path therefore goes in quotes, so
/// a checkout under a directory with a space in its name stays one word.
fn command(stiles: &Path, project: &Project) -> String {
    format!(
        "{} check --keep-going {}",
        quote(&stiles.to_string_lossy()),
        quote(project.path)
    )
}

/// Returns a word in single quotes, as a POSIX shell reads it
///
/// A single quote cannot appear inside single quotes, so each one closes
/// the quotes, stands escaped on its own, and opens them again.
fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// Returns the outcome of a run that the configuration stopped
fn refused(reason: String) -> Outcome {
    let location =
        FilePath::try_from(CONFIG).map_or(Location::Project, |path| Location::File { path });

    Outcome::Failed {
        findings: vec![
            Finding::builder()
                .message(reason)
                .location(location)
                .build(),
        ],
        repairs: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    // An assertion in a test panics by design. A `# Panics` section on every
    // test would repeat that and give the reader no information.
    #![allow(clippy::missing_panics_doc, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn command_quotes_both_paths() {
        let project = Project::named("typescript").unwrap();

        let line = command(Path::new("/work/my stiles/stiles"), &project);

        assert_eq!(
            line,
            "'/work/my stiles/stiles' check --keep-going 'src/compiler'"
        );
    }

    #[test]
    fn quote_with_a_single_quote_escapes_it() {
        let word = quote("it's");

        assert_eq!(word, r"'it'\''s'");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Bench>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Bench>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Bench>();
    }
}
