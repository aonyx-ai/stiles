use thiserror::Error;

use crate::{AttachFixError, BuildFixError, RuleId};

/// The reason a [`Collector`] could not build what a pass reported
///
/// A pass reports through its [`Sink`] one part at a time, and the host
/// builds each diagnostic from the parts once the pass returns. A fix gets
/// the checks that [`Fix::builder`] and [`Diagnostic::with_fix`] make
/// anywhere else, so the same errors say what went wrong. A call that names
/// a diagnostic the sink never started cannot fail at the boundary, so the
/// collector keeps it and reports it here.
///
/// # Examples
///
/// ```
/// use stiles_types::{CollectDiagnosticsError, RuleId};
///
/// let error = CollectDiagnosticsError::UnnamedFix {
///     rule: RuleId::new("lint.example"),
/// };
///
/// assert_eq!(
///     error.to_string(),
///     "a lint pass edited a fix for `lint.example` without naming the fix"
/// );
/// ```
///
/// [`Collector`]: crate::Collector
/// [`Diagnostic::with_fix`]: crate::Diagnostic::with_fix
/// [`Fix::builder`]: crate::Fix::builder
/// [`Sink`]: crate::Sink
#[derive(Clone, Eq, PartialEq, Hash, Debug, Error)]
pub enum CollectDiagnosticsError {
    /// The fix of a diagnostic did not fit the diagnostic
    #[error("failed to attach a fix to a diagnostic of `{rule}`")]
    AttachFix {
        /// The rule of the diagnostic
        rule: RuleId,
        /// Why the fix did not fit
        source: AttachFixError,
    },

    /// The edits of a fix did not form one
    #[error("failed to build a fix for a diagnostic of `{rule}`")]
    BuildFix {
        /// The rule of the diagnostic
        rule: RuleId,
        /// Why the edits did not form a fix
        source: BuildFixError,
    },

    /// A pass added to a diagnostic that its sink never started
    #[error("a lint pass added to diagnostic {handle}, but its sink started only {started}")]
    UnknownHandle {
        /// The handle the pass named
        handle: usize,
        /// How many diagnostics the sink had started by then
        started: usize,
    },
    /// A pass reported a diagnostic with a severity this stiles does not know
    ///
    /// A plugin built against a newer stiles may name a severity that this
    /// one lacks. The host refuses the report rather than guess its weight.
    #[error(
        "a lint pass reported `{rule}` with severity byte {byte}, which this stiles does not know"
    )]
    UnknownSeverity {
        /// The rule of the diagnostic
        rule: RuleId,
        /// The byte the pass sent
        byte: u8,
    },

    /// A pass edited a fix and never gave it a message
    #[error("a lint pass edited a fix for `{rule}` without naming the fix")]
    UnnamedFix {
        /// The rule of the diagnostic
        rule: RuleId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CollectDiagnosticsError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CollectDiagnosticsError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CollectDiagnosticsError>();
    }
}
