use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// The reason [`RustDecorationProvider::load`] could not load a workspace
///
/// rust-analyzer reports its own failures as [`anyhow::Error`], so the
/// variants that wrap one of its steps hold that error as their cause.
///
/// [`RustDecorationProvider::load`]: crate::RustDecorationProvider::load
/// [`anyhow::Error`]: anyhow::Error
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use stiles_rust::LoadRustWorkspaceError;
///
/// let error = LoadRustWorkspaceError::NotUtf8 {
///     path: PathBuf::from("/projects/app"),
/// };
///
/// assert!(error.to_string().starts_with("/projects/app is not valid UTF-8"));
/// ```
#[derive(Debug, Error)]
pub enum LoadRustWorkspaceError {
    /// The current directory, which anchors a relative root, is unreadable
    #[error("failed to read the current directory")]
    UnreadableCurrentDirectory {
        /// The failure of the read
        source: io::Error,
    },

    /// The root is not valid UTF-8, and rust-analyzer cannot address it
    #[error(
        "{} is not valid UTF-8, and rust-analyzer only addresses UTF-8 paths",
        path.display()
    )]
    NotUtf8 {
        /// The root, joined to the current directory
        path: PathBuf,
    },

    /// No Cargo manifest sits at or above the root
    #[error("failed to discover a Cargo project at {}", root.display())]
    Undiscovered {
        /// The root the caller passed
        root: PathBuf,
        /// The failure that rust-analyzer reported
        source: anyhow::Error,
    },

    /// Cargo could not resolve the workspace of the manifest
    #[error("failed to resolve the Cargo workspace discovered from {}", root.display())]
    Unresolved {
        /// The root the caller passed
        root: PathBuf,
        /// The failure that rust-analyzer reported
        source: anyhow::Error,
    },

    /// The build scripts of the workspace did not run
    #[error("failed to run build scripts for the workspace")]
    BuildScriptsFailed {
        /// The failure that rust-analyzer reported
        source: anyhow::Error,
    },

    /// rust-analyzer could not build its database of the workspace
    #[error("failed to build the analysis database for the workspace")]
    DatabaseFailed {
        /// The failure that rust-analyzer reported
        source: anyhow::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LoadRustWorkspaceError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LoadRustWorkspaceError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LoadRustWorkspaceError>();
    }
}
