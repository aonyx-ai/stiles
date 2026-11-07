use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// The reason a Rust provider cannot take the changed text of files
///
/// # Examples
///
/// ```
/// use stiles_rust::ChangeRustFilesError;
///
/// let error = ChangeRustFilesError::PoisonedDatabase;
///
/// assert_eq!(
///     error.to_string(),
///     "the provider's database is poisoned, because a thread panicked while it held the lock"
/// );
/// ```
#[derive(Debug, Error)]
pub enum ChangeRustFilesError {
    /// The path of a changed file cannot be made absolute
    #[error("failed to resolve {} to an absolute path", path.display())]
    UnresolvablePath {
        /// The path of the change, as the caller named it
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
        let error = ChangeRustFilesError::UnresolvablePath {
            path: PathBuf::from("src/lib.rs"),
            source: io::Error::other("no current directory"),
        };

        let message = error.to_string();

        assert_eq!(message, "failed to resolve src/lib.rs to an absolute path");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ChangeRustFilesError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ChangeRustFilesError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ChangeRustFilesError>();
    }
}
