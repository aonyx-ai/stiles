use std::io::Error as IoError;
use std::path::PathBuf;

use rakko_cargo::ResolveToolchainError;
use rakko_tool::{ResolveToolError, RunCommandError};
use thiserror::Error;

/// An error that stops a run of an action before it has an answer
///
/// A failed build or a failed check is no error: it travels as a finding in
/// the outcome of the run. The variants below describe a run that could not
/// reach its question at all.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// A program of the run did not start, or its output could not be read
    #[error("failed to run {program}")]
    Unavailable {
        /// The program that the run tried to start
        program: String,
        /// The cause of the failure
        source: RunCommandError,
    },

    /// A file or a directory of the run could not be written
    #[error("failed to write {}", path.display())]
    Unwritable {
        /// The path that the run tried to write
        path: PathBuf,
        /// The cause of the failure
        source: IoError,
    },

    /// Mise reported no cargo for the project
    ///
    /// The action installs nothing, so the run stops here.
    #[error("failed to resolve cargo")]
    UnresolvedTool {
        /// The cause of the failure
        source: ResolveToolError,
    },

    /// Mise reported no toolchain for a channel that the run selects
    #[error("failed to resolve a Rust toolchain")]
    UnresolvedToolchain {
        /// The cause of the failure
        source: ResolveToolchainError,
    },
}
