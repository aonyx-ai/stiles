use std::fmt;
use std::path::PathBuf;

use bon::Builder;

/// A file that `stiles check` could not read or analyze
///
/// The walk records the failure and moves on or stops, as the
/// [`ErrorRecovery`] decides. The command reports every failure after the
/// walk, on the standard error. There it stays out of the report that a
/// caller reads from the standard output.
///
/// [`ErrorRecovery`]: super::error_recovery::ErrorRecovery
#[derive(Debug, Builder)]
pub(crate) struct FileFailure {
    file: PathBuf,
    error: anyhow::Error,
}

impl fmt::Display for FileFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}: {:#}", self.file.display(), self.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_the_file_and_every_cause() {
        let error = anyhow::anyhow!("invalid UTF-8").context("failed to read the file");
        let failure = FileFailure::builder()
            .file(PathBuf::from("src/bad.rs"))
            .error(error)
            .build();

        let text = failure.to_string();

        assert_eq!(
            text,
            "error: src/bad.rs: failed to read the file: invalid UTF-8"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FileFailure>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FileFailure>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FileFailure>();
    }
}
