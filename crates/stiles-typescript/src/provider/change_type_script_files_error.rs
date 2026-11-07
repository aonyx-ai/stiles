use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// The reason a TypeScript provider cannot take the changed text of files
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use stiles_typescript::ChangeTypeScriptFilesError;
///
/// let error = ChangeTypeScriptFilesError::NotUtf8 { path: PathBuf::from("src/a.ts") };
///
/// assert_eq!(
///     error.to_string(),
///     "src/a.ts is not valid UTF-8, and tsrs only addresses UTF-8 paths"
/// );
/// ```
#[derive(Debug, Error)]
pub enum ChangeTypeScriptFilesError {
    /// The path of a changed file cannot be made absolute
    #[error("failed to resolve {} to an absolute path", path.display())]
    UnresolvablePath {
        /// The path of the change, as the caller named it
        path: PathBuf,
        /// The failure to read the current directory
        source: io::Error,
    },

    /// A path is not valid UTF-8, and tsrs addresses files by UTF-8 paths
    #[error("{} is not valid UTF-8, and tsrs only addresses UTF-8 paths", path.display())]
    NotUtf8 {
        /// The path that is not valid UTF-8
        path: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_of_unresolvable_path_names_the_path() {
        let error = ChangeTypeScriptFilesError::UnresolvablePath {
            path: PathBuf::from("src/a.ts"),
            source: io::Error::other("no current directory"),
        };

        let message = error.to_string();

        assert_eq!(message, "failed to resolve src/a.ts to an absolute path");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ChangeTypeScriptFilesError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ChangeTypeScriptFilesError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ChangeTypeScriptFilesError>();
    }
}
