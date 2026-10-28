use rakko_action::{Access, Action, Context, Name, Outcome, action_name};
use rakko_tool::Invocation;

use crate::{HarnessError, step};

/// The checkout of tsrs that the bundle comes from
const SOURCE: &str = "vendor/tsrs";

/// The directory of the generated crate, which a run replaces
const BUNDLE: &str = "crates/stiles-tsrs";

/// The name of the generated crate
const NAME: &str = "stiles-tsrs";

/// The tsrs crates that stiles imports
///
/// The generator adds every tsrs crate that these depend on, so the list
/// names only the crates that stiles-typescript names in its code.
const ROOTS: [&str; 8] = [
    "tsrs_ast",
    "tsrs_astnav",
    "tsrs_checker",
    "tsrs_compiler",
    "tsrs_core",
    "tsrs_module",
    "tsrs_tsoptions",
    "tsrs_vfs",
];

/// The action that generates `crates/stiles-tsrs` from the tsrs submodule
///
/// tsrs publishes no crates, and crates.io refuses a crate that depends on a
/// repository. The generator turns each tsrs crate that stiles uses into a
/// module of one crate, which can publish. A bump of tsrs moves the
/// submodule and runs this action.
#[derive(Copy, Clone, Debug, Default)]
pub struct BundleTsrs;

/// The action that checks `crates/stiles-tsrs` against a fresh run
///
/// A hand edit to the bundle would vanish at the next bump, so the check
/// fails on any byte that a fresh run does not produce. A file that the run
/// generates and nobody committed fails it too, even when an ignore pattern
/// hides the file from Git.
#[derive(Copy, Clone, Debug, Default)]
pub struct CheckBundleTsrs;

impl Action for BundleTsrs {
    type Args = ();

    fn name(&self) -> Name {
        action_name!("bundle-tsrs")
    }

    fn access(&self, _context: &Context, _args: &Self::Args) -> Access {
        Access::Write
    }

    async fn run(&self, context: &Context, _args: &Self::Args) -> Outcome {
        step::outcome(generate(context).await)
    }
}

impl Action for CheckBundleTsrs {
    type Args = ();

    fn name(&self) -> Name {
        action_name!("check-bundle-tsrs")
    }

    fn access(&self, _context: &Context, _args: &Self::Args) -> Access {
        Access::Read
    }

    async fn run(&self, context: &Context, _args: &Self::Args) -> Outcome {
        step::outcome(check(context).await)
    }
}

/// Runs the generator against the submodule
///
/// # Errors
///
/// Returns the error of the step that could not finish: the resolution of
/// cargo or its run.
async fn generate(context: &Context) -> Result<Outcome, HarnessError> {
    if !context
        .root()
        .get()
        .join(SOURCE)
        .join("Cargo.toml")
        .is_file()
    {
        return Ok(missing_submodule());
    }

    let cargo = step::cargo(context).await?;
    let run = step::run(
        cargo
            .args([
                "run",
                "--locked",
                "-p",
                "bundle-tsrs",
                "--",
                SOURCE,
                BUNDLE,
                NAME,
            ])
            .args(ROOTS),
    )
    .await?;
    if !run.status().success() {
        return Ok(step::failed("generating the tsrs bundle", &run));
    }

    Ok(Outcome::Passed { summary: None })
}

/// Tests the generator, runs it, and compares the result with the commit
///
/// The search for files that nobody committed leaves out
/// `--exclude-standard`, so a file that an ignore pattern hides from every
/// commit fails the check too.
///
/// # Errors
///
/// Returns the error of the step that could not finish: the resolution of
/// cargo, or the run of cargo or git.
async fn check(context: &Context) -> Result<Outcome, HarnessError> {
    if !context
        .root()
        .get()
        .join(SOURCE)
        .join("Cargo.toml")
        .is_file()
    {
        return Ok(missing_submodule());
    }

    let cargo = step::cargo(context).await?;
    let test = step::run(cargo.args(["test", "--locked", "-p", "bundle-tsrs"])).await?;
    if !test.status().success() {
        return Ok(step::failed(
            "testing the generator of the tsrs bundle",
            &test,
        ));
    }

    let generated = generate(context).await?;
    let Outcome::Passed { .. } = generated else {
        return Ok(generated);
    };

    let diff = step::run(git(context).args(["diff", "--exit-code", "--", BUNDLE])).await?;
    if !diff.status().success() {
        return Ok(step::failed(
            "comparing the tsrs bundle with a fresh run (run `bundle-tsrs` and commit the result)",
            &diff,
        ));
    }

    let untracked = step::run(git(context).args(["ls-files", "--others", "--", BUNDLE])).await?;
    if !untracked.stdout().is_empty() {
        return Ok(step::failed(
            "finding files that the generator writes and nobody committed",
            &untracked,
        ));
    }

    Ok(Outcome::Passed { summary: None })
}

/// Returns a git command in the project root
fn git(context: &Context) -> Invocation {
    Invocation::new("git").in_directory(context.root().get())
}

/// Returns the outcome of a run without the submodule
///
/// A checkout clones no submodule unless it is asked to, so the finding names
/// the command that fetches it.
fn missing_submodule() -> Outcome {
    Outcome::Failed {
        findings: vec![
            rakko_action::Finding::builder()
                .message(format!(
                    "{SOURCE} is missing; run `git submodule update --init {SOURCE}`"
                ))
                .location(rakko_action::Location::Project)
                .build(),
        ],
        repairs: Vec::new(),
    }
}
