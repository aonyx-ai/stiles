use std::path::PathBuf;

use thiserror::Error;

/// The reason a [`Fix`] cannot be attached to a [`Diagnostic`]
///
/// Each variant is a mistake in the rule that built the diagnostic.
///
/// [`Diagnostic`]: crate::Diagnostic
/// [`Fix`]: crate::Fix
///
/// # Examples
///
/// ```
/// use stiles_types::AttachFixError;
///
/// let error = AttachFixError::SecondFix;
///
/// assert_eq!(error.to_string(), "the diagnostic already holds a fix");
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug, Error)]
pub enum AttachFixError {
    /// The fix edits another file than the one the diagnostic reports on
    #[error(
        "the fix edits {}, but the diagnostic reports on {}",
        fix.display(),
        diagnostic.display()
    )]
    OtherFile {
        /// The file of the span of the diagnostic
        diagnostic: PathBuf,
        /// The file that the fix edits
        fix: PathBuf,
    },

    /// The diagnostic already holds a fix
    #[error("the diagnostic already holds a fix")]
    SecondFix,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_of_other_file_names_both_files() {
        let error = AttachFixError::OtherFile {
            diagnostic: PathBuf::from("a.rs"),
            fix: PathBuf::from("b.rs"),
        };

        let message = error.to_string();

        assert_eq!(
            message,
            "the fix edits b.rs, but the diagnostic reports on a.rs"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AttachFixError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AttachFixError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AttachFixError>();
    }
}
