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
use self::corpus::{CONFIG, RulesPin};
use self::results::Measurement;
use crate::{HarnessError, step};

/// The directory under the project root that holds the corpus and the results
const OUTPUT: &str = "target/bench";

/// The file under [`OUTPUT`] that holds the measures of every project
const RESULTS: &str = "results.json";

/// The action that prepares large codebases for the benchmarks of stiles
///
/// The run builds stiles in release mode, as a user installs it. For each
/// project of the corpus, it fetches the pinned commit and writes a
/// configuration of stiles into the checkout. The configuration names the
/// rules that this repository pins, built against this working tree. The
/// run then checks the project twice:
///
/// 1. One run that warms up. It builds the rules and downloads what the
///    toolchain of the project needs, so neither reaches the numbers.
/// 2. One run with `--timings`, which records the time of each phase. It
///    runs cargo offline. Stiles builds the rules against this working
///    tree, so cargo resolves their dependencies again on every run, and
///    online it would ask the index of crates.io each time. The warm-up has
///    already downloaded everything that the resolution needs.
///
/// The action times nothing more. CodSpeed times the checks of the prepared
/// projects, on hardware quiet enough for the numbers to mean something.
///
/// Each check passes `--keep-going`, so a file that stiles cannot cover
/// does not cut a run short. The findings of the rules do not matter here.
/// Stiles exits with 1 for findings and for an error that ends the run, so
/// the exit code cannot tell the two apart. The first two runs therefore
/// write a record of their phases, and a run counts as finished when its
/// record reaches the report.
///
/// The results land in `target/bench`: the record of the phases of each
/// project, and `results.json` with the phases of every project in the
/// Bencher Metric Format. The action takes minutes per project, so its name
/// carries no prefix that makes CI run it on every pull request.
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
        let timed = step::run(
            check()
                .arg("--timings")
                .arg(timings.as_path())
                .env("CARGO_NET_OFFLINE", "true"),
        )
        .await?;
        if !finished(&timed, &timings)? {
            return Ok(step::failed(
                &format!("timing the phases of stiles on {name}"),
                &timed,
            ));
        }

        measurements.push(Measurement::load(name, &timings)?);
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
