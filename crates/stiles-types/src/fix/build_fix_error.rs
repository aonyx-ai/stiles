use std::ops::Range;
use std::path::PathBuf;

use thiserror::Error;

/// The reason a set of edits cannot form a [`Fix`]
///
/// Each variant is a mistake in the rule that built the fix. A fix is
/// applied without review, so stiles refuses an ambiguous one rather than
/// guessing what the rule meant.
///
/// [`Fix`]: crate::Fix
///
/// # Examples
///
/// ```
/// use stiles_types::BuildFixError;
///
/// let error = BuildFixError::NoEdits;
///
/// assert_eq!(error.to_string(), "the fix holds no edits");
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug, Error)]
pub enum BuildFixError {
    /// The fix holds no edits, so applying it would change nothing
    #[error("the fix holds no edits")]
    NoEdits,

    /// The edits of the fix lie in more than one file
    #[error(
        "the fix edits both {} and {}, but a fix edits one file",
        first.display(),
        other.display()
    )]
    SeveralFiles {
        /// The file of the edit that sorts first
        first: PathBuf,
        /// The file of an edit that lies elsewhere
        other: PathBuf,
    },

    /// Two edits of the fix overlap, so their result has no single order
    #[error(
        "the fix edits bytes {first:?} and {second:?} of {}, which overlap",
        file.display()
    )]
    OverlappingEdits {
        /// The file both edits lie in
        file: PathBuf,
        /// The byte range of the edit that sorts first
        first: Range<usize>,
        /// The byte range of the edit that overlaps it
        second: Range<usize>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_of_several_files_names_both_files() {
        let error = BuildFixError::SeveralFiles {
            first: PathBuf::from("a.rs"),
            other: PathBuf::from("b.rs"),
        };

        let message = error.to_string();

        assert_eq!(
            message,
            "the fix edits both a.rs and b.rs, but a fix edits one file"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<BuildFixError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<BuildFixError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<BuildFixError>();
    }
}
