use rakko_action::{Context, Finding, Location, Outcome};
use rakko_tool::{Execution, Invocation, Tool, ToolName};

use crate::HarnessError;

/// Returns a command that runs the cargo that mise installed for the project
///
/// The command runs in the project root.
///
/// # Errors
///
/// Returns [`UnresolvedTool`][unresolved] when mise reports no cargo.
///
/// [unresolved]: HarnessError::UnresolvedTool
pub(crate) async fn cargo(context: &Context) -> Result<Invocation, HarnessError> {
    let cargo = Tool::resolve(ToolName::new("cargo"), context.root().clone())
        .await
        .map_err(|source| HarnessError::UnresolvedTool { source })?;

    Ok(cargo.invocation().in_directory(context.root().get()))
}

/// Runs one command of an action and waits for it to end
///
/// # Errors
///
/// Returns [`Unavailable`][unavailable] when the command does not start.
///
/// [unavailable]: HarnessError::Unavailable
pub(crate) async fn run(invocation: Invocation) -> Result<Execution, HarnessError> {
    invocation
        .run()
        .await
        .map_err(|source| HarnessError::Unavailable {
            program: invocation.program().to_string(),
            source,
        })
}

/// Returns the outcome of a run that failed at one command
///
/// The tools that the actions of this crate start report in prose, not in a
/// format that names a file. So the finding belongs to the project, and its
/// message carries what the command wrote, after a line that says what the
/// command was for.
pub(crate) fn failed(purpose: &str, execution: &Execution) -> Outcome {
    let stdout = execution.stdout().to_string_lossy();
    let stderr = execution.stderr().to_string_lossy();
    let output = [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    Outcome::Failed {
        findings: vec![
            Finding::builder()
                .message(format!("{purpose} failed:\n{output}"))
                .location(Location::Project)
                .build(),
        ],
        repairs: Vec::new(),
    }
}

/// Returns the outcome of a run that a step answered
///
/// A step answers with an outcome or stops with an error, and the error
/// becomes the outcome that reports it.
pub(crate) fn outcome(result: Result<Outcome, HarnessError>) -> Outcome {
    result.unwrap_or_else(|error| Outcome::Errored {
        source: Box::new(error),
    })
}
