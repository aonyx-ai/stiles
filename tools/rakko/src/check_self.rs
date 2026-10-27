use rakko_action::{Access, Action, Context, Name, Outcome, action_name};
use rakko_tool::Invocation;

use crate::{HarnessError, step};

/// The action that checks this repository with the rules it configures
///
/// This is the end-to-end run: whisker resolves every lint source that
/// `.config/whisker.toml` names, builds it, completes the handshake, and
/// reports on its own sources. It was once an integration test, but a test
/// that reaches the network belongs in an action that a person can choose to
/// run, not in `cargo test`.
///
/// The run builds whisker in release mode, as a user installs it, and then
/// runs the binary at the project root.
#[derive(Copy, Clone, Debug, Default)]
pub struct CheckSelf;

impl Action for CheckSelf {
    type Args = ();

    fn name(&self) -> Name {
        action_name!("check-self")
    }

    fn access(&self, _context: &Context, _args: &Self::Args) -> Access {
        Access::Read
    }

    async fn run(&self, context: &Context, _args: &Self::Args) -> Outcome {
        step::outcome(drive(context).await)
    }
}

/// Runs the action against the project of the context
///
/// # Errors
///
/// Returns the error of the step that could not finish: the resolution of
/// cargo, its run, or the run of whisker.
async fn drive(context: &Context) -> Result<Outcome, HarnessError> {
    let cargo = step::cargo(context).await?;

    let build = step::run(cargo.args(["build", "--release", "-p", "whisker"])).await?;
    if !build.status().success() {
        return Ok(step::failed("building whisker", &build));
    }

    let whisker = context.root().get().join("target/release/whisker");
    let check = step::run(
        Invocation::new(whisker)
            .args(["check", "."])
            .in_directory(context.root().get()),
    )
    .await?;
    if !check.status().success() {
        return Ok(step::failed(
            "checking this repository with whisker",
            &check,
        ));
    }

    Ok(Outcome::Passed { summary: None })
}
