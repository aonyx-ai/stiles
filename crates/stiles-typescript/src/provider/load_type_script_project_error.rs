use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// The reason [`TypeScriptDecorationProvider::load`] could not load a project
///
/// [`TypeScriptDecorationProvider::load`]: crate::TypeScriptDecorationProvider::load
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use stiles_typescript::LoadTypeScriptProjectError;
///
/// let error = LoadTypeScriptProjectError::MissingConfig {
///     path: PathBuf::from("/projects/app"),
/// };
///
/// assert_eq!(error.to_string(), "no tsconfig.json sits at or above /projects/app");
/// ```
#[derive(Debug, Error)]
pub enum LoadTypeScriptProjectError {
    /// The path has no absolute form, because the current directory is unreadable
    #[error("failed to resolve {} to an absolute path", path.display())]
    UnresolvablePath {
        /// The path the caller passed
        path: PathBuf,
        /// The failure to read the current directory
        source: io::Error,
    },

    /// No `tsconfig.json` sits at or above the path
    #[error("no {} sits at or above {}", super::CONFIG_FILE, path.display())]
    MissingConfig {
        /// The absolute path the search started from
        path: PathBuf,
    },

    /// A path is not valid UTF-8, and tsrs cannot address it
    #[error("{} is not valid UTF-8, and tsrs only addresses UTF-8 paths", path.display())]
    NotUtf8 {
        /// The path that is not valid UTF-8
        path: PathBuf,
    },

    /// tsrs refused the configuration file
    #[error("{} does not parse: {}", config.display(), messages.join("; "))]
    InvalidConfig {
        /// The configuration file
        config: PathBuf,
        /// What tsrs reported, one message for each problem
        messages: Vec<String>,
    },

    /// tsrs read no configuration from the file, and reported no problem
    #[error("tsrs read no configuration from {}", config.display())]
    EmptyConfig {
        /// The configuration file
        config: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_of_invalid_config_joins_the_messages() {
        let error = LoadTypeScriptProjectError::InvalidConfig {
            config: PathBuf::from("tsconfig.json"),
            messages: vec!["one".to_owned(), "two".to_owned()],
        };

        let message = error.to_string();

        assert_eq!(message, "tsconfig.json does not parse: one; two");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LoadTypeScriptProjectError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LoadTypeScriptProjectError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LoadTypeScriptProjectError>();
    }
}
