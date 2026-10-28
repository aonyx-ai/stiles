use rakko_action::{Access, Action, Context, Name, Outcome, action_name};

use crate::{HarnessError, step};

/// The language crates that a lint plugin depends on without their provider
const LANGUAGE_CRATES: [&str; 2] = ["stiles-rust", "stiles-typescript"];

/// The action that checks each language crate without its default features
///
/// A lint plugin depends on a language crate without its decoration
/// provider, because a rule needs the trait and the decoration types and not
/// the analysis stack behind the provider. The workspace always builds the
/// providers, so a type that leaks from a provider into the plugin's view
/// would go unnoticed without this check.
#[derive(Copy, Clone, Debug, Default)]
pub struct CheckFeatures;

impl Action for CheckFeatures {
    type Args = ();

    fn name(&self) -> Name {
        action_name!("check-features")
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
/// The run checks every language crate, and it stops at the first one that
/// does not build, because the report of that one is the finding.
///
/// # Errors
///
/// Returns the error of the step that could not finish: the resolution of
/// cargo or its run.
async fn drive(context: &Context) -> Result<Outcome, HarnessError> {
    for package in LANGUAGE_CRATES {
        let cargo = step::cargo(context).await?;

        let check =
            step::run(cargo.args(["check", "-p", package, "--no-default-features"])).await?;
        if !check.status().success() {
            return Ok(step::failed(
                &format!("checking {package} without its default features"),
                &check,
            ));
        }
    }

    Ok(Outcome::Passed { summary: None })
}
