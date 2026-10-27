use std::io;
use std::path::PathBuf;

use thiserror::Error;
use whisker_types::{DecorateTreeError, ProviderName, UncoveredFile};

use crate::WalkLintsError;

/// The reason a pipeline produced no diagnostics for a file
///
/// Every variant ends the run on that one file. A caller that checks many
/// files decides whether the next one runs.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use whisker_core::RunPipelineError;
/// use whisker_types::UncoveredFile;
///
/// let error = RunPipelineError::Uncovered {
///     source: UncoveredFile::builder()
///         .file(PathBuf::from("stray.rs"))
///         .gaps(Vec::new())
///         .build(),
/// };
///
/// let RunPipelineError::Uncovered { source } = &error else {
///     unreachable!();
/// };
/// assert!(source.gaps().is_empty());
/// ```
#[derive(Debug, Error)]
pub enum RunPipelineError {
    /// The file could not be read as UTF-8 text
    #[error("failed to read {}", path.display())]
    UnreadableSource {
        /// The file the pipeline was asked to check
        path: PathBuf,
        /// The failure of the read
        source: io::Error,
    },

    /// Tree-sitter returned no tree for the source
    #[error("tree-sitter produced no syntax tree for the source")]
    NoSyntaxTree,

    /// The toolchain of a provider malfunctioned
    #[error("failed to run the `{provider}` decoration provider")]
    ProviderFailed {
        /// The provider whose toolchain malfunctioned
        provider: ProviderName,
        /// The malfunction the provider reported
        source: DecorateTreeError,
    },

    /// No provider claimed the file, so semantic rules cannot run on it
    #[error(transparent)]
    Uncovered {
        /// The file, with each provider's reason for declining it
        source: UncoveredFile,
    },

    /// A lint pass stopped the walk
    #[error("failed to run the lint passes")]
    LintsFailed {
        /// The pass's fault, with the node it was checking
        source: WalkLintsError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RunPipelineError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RunPipelineError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RunPipelineError>();
    }
}
