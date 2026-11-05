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

    /// A file that the run reads could not be read
    #[error("failed to read {}", path.display())]
    Unreadable {
        /// The path that the run tried to read
        path: PathBuf,
        /// The cause of the failure
        source: IoError,
    },

    /// An export of hyperfine holds no result
    #[error("{} holds no result", path.display())]
    EmptyExport {
        /// The export that the run read
        path: PathBuf,
    },

    /// A JSON file that a tool of the run wrote holds something else
    #[error("failed to parse {} as JSON", path.display())]
    UnparsableJson {
        /// The file that the run read
        path: PathBuf,
        /// The cause of the failure
        source: serde_json::Error,
    },

    /// A TOML file that the run reads holds something else
    #[error("failed to parse {} as TOML", path.display())]
    UnparsableToml {
        /// The file that the run read
        path: PathBuf,
        /// The cause of the failure
        source: toml::de::Error,
    },

    /// A value that the run writes as TOML could not be serialized
    #[error("failed to serialize {} as TOML", path.display())]
    UnserializableToml {
        /// The file that the run was about to write
        path: PathBuf,
        /// The cause of the failure
        source: toml::ser::Error,
    },

    /// A value that the run writes as JSON could not be serialized
    #[error("failed to serialize {} as JSON", path.display())]
    UnserializableJson {
        /// The file that the run was about to write
        path: PathBuf,
        /// The cause of the failure
        source: serde_json::Error,
    },

    /// Mise reported no program of a tool that the run needs
    ///
    /// The action installs nothing, so the run stops here.
    #[error("failed to resolve {tool}")]
    UnresolvedTool {
        /// The name of the tool that mise did not report
        tool: &'static str,
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
