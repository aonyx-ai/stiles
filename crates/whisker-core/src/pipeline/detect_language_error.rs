use std::path::PathBuf;

use thiserror::Error;

/// The reason the extension of a file names no language
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// use whisker_core::{DetectLanguageError, detect_language};
///
/// let error = detect_language(Path::new("Makefile")).expect_err("no extension");
///
/// assert_eq!(error.to_string(), "Makefile has no extension that names a language");
/// ```
#[derive(Clone, Eq, PartialEq, Debug, Error)]
pub enum DetectLanguageError {
    /// The file has no extension, or one that is not valid UTF-8
    #[error("{} has no extension that names a language", path.display())]
    MissingExtension {
        /// The file whose language was asked for
        path: PathBuf,
    },

    /// No language that whisker parses uses the extension
    #[error("no supported language uses the extension `.{extension}`")]
    UnsupportedExtension {
        /// The extension, without its dot
        extension: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DetectLanguageError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DetectLanguageError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DetectLanguageError>();
    }
}
