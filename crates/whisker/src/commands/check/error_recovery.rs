use std::path::Path;

use super::file_failure::FileFailure;

/// Whether the walk continues after a file fails
///
/// A per-file failure never propagates out of the walk. The run keeps the
/// diagnostics it already collected and only decides whether to visit the
/// remaining files.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum Walk {
    /// The walk moves on to the next file
    Continue,
    /// The walk stops, and the run reports what it already collected
    Stop,
}

/// What `whisker check` does with a file it cannot read or analyze
///
/// A file can fail in two ways: the bytes are not valid UTF-8, or the
/// pipeline rejects the source. Both paths route through this type, so one
/// place decides how a per-file failure affects the run.
///
/// Both modes record the failure, and the command reports every recorded
/// failure and fails the run. A file that fails produces no diagnostics, so
/// nothing else would keep the exit code non-zero. The modes differ only in
/// whether the walk visits the remaining files.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum ErrorRecovery {
    /// The first failing file ends the walk
    Abort,
    /// The run reports the failing file and the walk continues
    Continue,
}

impl ErrorRecovery {
    /// Records a failure encountered while processing a single file
    ///
    /// The method adds the failure to `failures`. The command reports them
    /// after the walk and turns any of them into a non-zero exit code.
    ///
    /// Returns whether the caller should keep walking.
    pub(crate) fn record(
        self,
        failures: &mut Vec<FileFailure>,
        file: &Path,
        error: anyhow::Error,
    ) -> Walk {
        failures.push(
            FileFailure::builder()
                .file(file.to_path_buf())
                .error(error)
                .build(),
        );

        match self {
            Self::Abort => Walk::Stop,
            Self::Continue => Walk::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_error() -> anyhow::Error {
        anyhow::anyhow!("something broke")
    }

    #[test]
    fn record_with_abort_records_the_failure() {
        let mut failures = Vec::new();

        ErrorRecovery::Abort.record(&mut failures, Path::new("bad.rs"), test_error());

        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn record_with_abort_stops_the_walk() {
        let mut failures = Vec::new();

        let walk = ErrorRecovery::Abort.record(&mut failures, Path::new("bad.rs"), test_error());

        assert_eq!(walk, Walk::Stop);
    }

    #[test]
    fn record_with_continue_keeps_an_earlier_failure() {
        let mut failures = Vec::new();
        ErrorRecovery::Continue.record(&mut failures, Path::new("first.rs"), test_error());

        ErrorRecovery::Continue.record(&mut failures, Path::new("second.rs"), test_error());

        assert_eq!(failures.len(), 2);
    }

    #[test]
    fn record_with_continue_keeps_walking() {
        let mut failures = Vec::new();

        let walk = ErrorRecovery::Continue.record(&mut failures, Path::new("bad.rs"), test_error());

        assert_eq!(walk, Walk::Continue);
    }

    #[test]
    fn record_with_continue_records_the_failure() {
        let mut failures = Vec::new();

        ErrorRecovery::Continue.record(&mut failures, Path::new("bad.rs"), test_error());

        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ErrorRecovery>();
        assert_send::<Walk>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ErrorRecovery>();
        assert_sync::<Walk>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ErrorRecovery>();
        assert_unpin::<Walk>();
    }
}
