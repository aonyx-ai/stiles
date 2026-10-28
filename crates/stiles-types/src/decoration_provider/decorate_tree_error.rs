use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// The failure of a provider's toolchain while it decorated a tree
///
/// A provider that knows nothing about a file declines it with a
/// [`Coverage::NotCovered`] verdict, and that is not an error. This error
/// is for the toolchain itself, which stops the run on the file.
///
/// [`Coverage::NotCovered`]: crate::Coverage::NotCovered
///
/// # Examples
///
/// ```
/// use stiles_types::DecorateTreeError;
///
/// let error = DecorateTreeError::PoisonedDatabase;
///
/// assert_eq!(
///     error.to_string(),
///     "the provider's database is poisoned, because a thread panicked while it held the lock"
/// );
/// ```
#[derive(Debug, Error)]
pub enum DecorateTreeError {
    /// The file of the tree has no absolute path
    #[error("failed to resolve {} to an absolute path", path.display())]
    UnresolvablePath {
        /// The path of the tree, as the tree names it
        path: PathBuf,
        /// The failure to read the current directory
        source: io::Error,
    },

    /// A thread panicked while it held the lock of the provider's database
    #[error(
        "the provider's database is poisoned, because a thread panicked while it held the lock"
    )]
    PoisonedDatabase,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_of_unresolvable_path_names_the_path() {
        let error = DecorateTreeError::UnresolvablePath {
            path: PathBuf::from("src/lib.rs"),
            source: io::Error::other("no current directory"),
        };

        let message = error.to_string();

        assert_eq!(message, "failed to resolve src/lib.rs to an absolute path");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecorateTreeError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecorateTreeError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorateTreeError>();
    }
}
