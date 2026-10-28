use std::io;
use std::path::PathBuf;

use codespan_reporting::files;
use thiserror::Error;

/// The reason [`render`] could not print the diagnostics
///
/// [`render`]: crate::render
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use stiles_reporting::PrintDiagnosticsError;
///
/// let error = PrintDiagnosticsError::UnreadableSource {
///     path: PathBuf::from("src/lib.rs"),
///     source: std::io::Error::from(std::io::ErrorKind::NotFound),
/// };
///
/// assert_eq!(error.to_string(), "failed to read src/lib.rs");
/// ```
#[derive(Debug, Error)]
pub enum PrintDiagnosticsError {
    /// The file a diagnostic points into could not be read
    #[error("failed to read {}", path.display())]
    UnreadableSource {
        /// The file the diagnostic points into
        path: PathBuf,
        /// The failure of the read
        source: io::Error,
    },

    /// Codespan could not lay out a diagnostic or write it
    #[error("failed to emit a diagnostic")]
    Unemitted {
        /// The failure that codespan reported
        source: files::Error,
    },

    /// The standard error refused the rendered text
    #[error("failed to flush the standard error")]
    Unflushed {
        /// The failure of the flush
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PrintDiagnosticsError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PrintDiagnosticsError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PrintDiagnosticsError>();
    }
}
