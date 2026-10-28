use whisker_types::{Diagnostic, Severity};

/// The lowest diagnostic severity that makes `whisker check` fail
///
/// The default is [`FailureThreshold::Errors`]. The `--deny-warnings` flag
/// selects [`FailureThreshold::Warnings`].
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum FailureThreshold {
    /// Only [`Severity::Error`] diagnostics fail the run
    ///
    /// [`Severity::Error`]: whisker_types::Severity::Error
    Errors,
    /// [`Severity::Warn`] diagnostics fail the run alongside errors
    ///
    /// [`Severity::Warn`]: whisker_types::Severity::Warn
    Warnings,
}

impl FailureThreshold {
    /// Returns whether a diagnostic of this severity fails the run
    pub(crate) fn is_met_by(self, severity: Severity) -> bool {
        severity >= self.minimum_failing_severity()
    }

    /// Returns the lowest severity that fails the run under this threshold
    pub(crate) fn minimum_failing_severity(self) -> Severity {
        match self {
            Self::Errors => Severity::Error,
            Self::Warnings => Severity::Warn,
        }
    }

    /// Raises a failing diagnostic to error severity
    ///
    /// A diagnostic that meets the threshold gets [`Severity::Error`], so
    /// the rendered output matches the exit code. A diagnostic below the
    /// threshold comes back unchanged.
    ///
    /// [`Severity::Error`]: whisker_types::Severity::Error
    pub(crate) fn promote(self, diagnostic: Diagnostic) -> Diagnostic {
        match self.is_met_by(diagnostic.severity()) {
            true => diagnostic.with_severity(Severity::Error),
            false => diagnostic,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{Location, RuleId, Span, Suggestion};

    use super::*;

    /// Every severity, from the mildest to the most severe
    const SEVERITIES: [Severity; 4] = [
        Severity::Help,
        Severity::Info,
        Severity::Warn,
        Severity::Error,
    ];

    fn test_diagnostic(severity: Severity) -> Diagnostic {
        Diagnostic::builder()
            .rule_id(RuleId::new("lint.test"))
            .severity(severity)
            .message("test message")
            .span(
                Span::builder()
                    .file(PathBuf::from("test.rs"))
                    .start(0)
                    .end(10)
                    .build(),
            )
            .build()
    }

    #[test]
    fn is_met_by_with_errors_accepts_only_error() {
        let threshold = FailureThreshold::Errors;

        let met = SEVERITIES.map(|severity| threshold.is_met_by(severity));

        assert_eq!(met, [false, false, false, true]);
    }

    #[test]
    fn is_met_by_with_warnings_accepts_warn_and_error() {
        let threshold = FailureThreshold::Warnings;

        let met = SEVERITIES.map(|severity| threshold.is_met_by(severity));

        assert_eq!(met, [false, false, true, true]);
    }

    #[test]
    fn minimum_failing_severity_with_errors_returns_error() {
        let severity = FailureThreshold::Errors.minimum_failing_severity();

        assert_eq!(severity, Severity::Error);
    }

    #[test]
    fn minimum_failing_severity_with_warnings_returns_warn() {
        let severity = FailureThreshold::Warnings.minimum_failing_severity();

        assert_eq!(severity, Severity::Warn);
    }

    #[test]
    fn promote_with_errors_leaves_every_severity_untouched() {
        let severities = [
            Severity::Help,
            Severity::Info,
            Severity::Warn,
            Severity::Error,
        ];

        let promoted: Vec<Severity> = severities
            .into_iter()
            .map(|severity| {
                FailureThreshold::Errors
                    .promote(test_diagnostic(severity))
                    .severity()
            })
            .collect();

        assert_eq!(promoted, severities);
    }

    #[test]
    fn promote_with_warnings_leaves_help_and_info_untouched() {
        let severities = [Severity::Help, Severity::Info].map(|severity| {
            FailureThreshold::Warnings
                .promote(test_diagnostic(severity))
                .severity()
        });

        assert_eq!(severities, [Severity::Help, Severity::Info]);
    }

    #[test]
    fn promote_with_warnings_preserves_the_message() {
        let promoted = promoted_detailed_warning();

        assert_eq!(promoted.message(), "test message");
    }

    #[test]
    fn promote_with_warnings_preserves_the_origins() {
        let promoted = promoted_detailed_warning();

        assert_eq!(promoted.origins(), [origin()]);
    }

    #[test]
    fn promote_with_warnings_preserves_the_related_locations() {
        let promoted = promoted_detailed_warning();

        assert_eq!(promoted.related(), [related()]);
    }

    fn origin() -> Location {
        Location::builder()
            .span(
                Span::builder()
                    .file(PathBuf::from("other.rs"))
                    .start(1)
                    .end(2)
                    .build(),
            )
            .message("defined here")
            .build()
    }

    fn related() -> Location {
        Location::builder()
            .span(
                Span::builder()
                    .file(PathBuf::from("other.rs"))
                    .start(3)
                    .end(4)
                    .build(),
            )
            .message("also here")
            .build()
    }

    fn suggestion() -> Suggestion {
        Suggestion::builder()
            .span(
                Span::builder()
                    .file(PathBuf::from("test.rs"))
                    .start(0)
                    .end(1)
                    .build(),
            )
            .replacement("replacement")
            .message("try this")
            .build()
    }

    /// Returns a warning with an origin, a related location, and a
    /// suggestion, promoted under [`FailureThreshold::Warnings`]
    fn promoted_detailed_warning() -> Diagnostic {
        let diagnostic = test_diagnostic(Severity::Warn)
            .with_origin(origin())
            .with_related(related())
            .with_suggestion(suggestion());

        FailureThreshold::Warnings.promote(diagnostic)
    }

    #[test]
    fn promote_with_warnings_preserves_the_rule_id() {
        let promoted = promoted_detailed_warning();

        assert_eq!(promoted.rule_id(), RuleId::new("lint.test"));
    }

    #[test]
    fn promote_with_warnings_preserves_the_span() {
        let promoted = promoted_detailed_warning();

        assert_eq!(
            promoted.span(),
            &Span::builder()
                .file(PathBuf::from("test.rs"))
                .start(0)
                .end(10)
                .build()
        );
    }

    #[test]
    fn promote_with_warnings_preserves_the_suggestions() {
        let promoted = promoted_detailed_warning();

        assert_eq!(promoted.suggestions(), [suggestion()]);
    }

    #[test]
    fn promote_with_warnings_raises_warn_to_error() {
        let promoted = FailureThreshold::Warnings.promote(test_diagnostic(Severity::Warn));

        assert_eq!(promoted.severity(), Severity::Error);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FailureThreshold>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FailureThreshold>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FailureThreshold>();
    }
}
