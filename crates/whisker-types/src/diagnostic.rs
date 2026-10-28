use bon::bon;
use stabby::{string, vec};

use crate::{Location, RuleId, Severity, Span, Suggestion};

/// A diagnostic emitted by a lint rule
///
/// Diagnostics are the primary output of the linting pipeline. Each one
/// identifies a specific issue found in source code, with a severity, a
/// primary span, and optional supplementary information like origin
/// locations, related locations, and suggested fixes.
///
/// A diagnostic is what a plugin hands back across the plugin boundary.
/// Stabby lays it out, and its strings and lists are stabby's `String` and
/// `Vec`. The message is copied in from what a caller passes, because
/// stabby's `String` cannot adopt an allocation std made.
/// A plugin allocates each diagnostic and whisker frees it, which is sound
/// because every allocation stabby makes carries its own release function.
/// The severity is one byte and sits last, so it puts no padding between
/// the pointer-sized fields before it.
#[stabby::stabby]
#[derive(Clone, Debug)]
pub struct Diagnostic {
    rule_id: RuleId,
    message: string::String,
    span: Span,
    origins: vec::Vec<Location>,
    related: vec::Vec<Location>,
    suggestions: vec::Vec<Suggestion>,
    severity: Severity,
}

#[bon]
impl Diagnostic {
    /// Creates a new diagnostic with the required fields
    #[builder(start_fn = builder)]
    pub fn new(
        rule_id: RuleId,
        severity: Severity,
        #[builder(into)] message: String,
        span: Span,
    ) -> Self {
        Self {
            rule_id,
            message: string::String::from(message.as_str()),
            span,
            origins: vec::Vec::new(),
            related: vec::Vec::new(),
            suggestions: vec::Vec::new(),
            severity,
        }
    }

    /// Adds an origin location to this diagnostic
    pub fn with_origin(mut self, origin: Location) -> Self {
        self.origins.push(origin);
        self
    }

    /// Adds a related location to this diagnostic
    pub fn with_related(mut self, related: Location) -> Self {
        self.related.push(related);
        self
    }

    /// Adds a suggested fix to this diagnostic
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Replaces the severity of this diagnostic
    ///
    /// The other `with_*` methods append. This one replaces, because a
    /// diagnostic has exactly one severity.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// use whisker_types::{Diagnostic, RuleId, Severity, Span};
    ///
    /// let diagnostic = Diagnostic::new(
    ///     RuleId::new("lint.bool-param"),
    ///     Severity::Warn,
    ///     "parameter has type `bool`".into(),
    ///     Span::new(PathBuf::from("src/lib.rs"), 0, 10),
    /// );
    ///
    /// let denied = diagnostic.with_severity(Severity::Error);
    ///
    /// assert_eq!(denied.severity(), Severity::Error);
    /// ```
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Returns the rule that produced this diagnostic
    pub fn rule_id(&self) -> RuleId {
        self.rule_id
    }

    /// Returns the severity of this diagnostic
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the human-readable diagnostic message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the primary source span
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns origin locations that explain where something was defined
    pub fn origins(&self) -> &[Location] {
        &self.origins
    }

    /// Returns related locations that provide additional context
    pub fn related(&self) -> &[Location] {
        &self.related
    }

    /// Returns suggested source code fixes
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn test_diagnostic() -> Diagnostic {
        Diagnostic::new(
            RuleId::new("lint.test"),
            Severity::Warn,
            "test message".into(),
            Span::new(PathBuf::from("test.rs"), 0, 10),
        )
    }

    #[test]
    fn clone_copies_the_message() {
        let diag = test_diagnostic().with_origin(origin());

        let copy = diag.clone();

        assert_eq!(copy.message(), diag.message());
    }

    #[test]
    fn clone_copies_the_origins() {
        let diag = test_diagnostic().with_origin(origin());

        let copy = diag.clone();

        assert_eq!(copy.origins(), diag.origins());
    }

    #[test]
    fn clone_copies_the_rule_id() {
        let diag = test_diagnostic().with_origin(origin());

        let copy = diag.clone();

        assert_eq!(copy.rule_id(), diag.rule_id());
    }

    /// Returns a location that a test attaches as an origin
    fn origin() -> Location {
        Location::new(
            Span::new(PathBuf::from("other.rs"), 5, 15),
            "defined here".into(),
        )
    }

    #[test]
    fn clone_copies_the_span() {
        let diag = test_diagnostic().with_origin(origin());

        let copy = diag.clone();

        assert_eq!(copy.span(), diag.span());
    }

    #[test]
    fn message_returns_the_message() {
        let diag = test_diagnostic();

        assert_eq!(diag.message(), "test message");
    }

    #[test]
    fn new_diagnostic_has_no_origins() {
        let diag = test_diagnostic();

        assert!(diag.origins().is_empty());
    }

    #[test]
    fn new_diagnostic_has_no_related_locations() {
        let diag = test_diagnostic();

        assert!(diag.related().is_empty());
    }

    #[test]
    fn new_diagnostic_has_no_suggestions() {
        let diag = test_diagnostic();

        assert!(diag.suggestions().is_empty());
    }

    #[test]
    fn rule_id_returns_the_rule_id() {
        let diag = test_diagnostic();

        assert_eq!(diag.rule_id(), RuleId::new("lint.test"));
    }

    #[test]
    fn severity_returns_the_severity() {
        let diag = test_diagnostic();

        assert_eq!(diag.severity(), Severity::Warn);
    }

    #[test]
    fn span_returns_the_span() {
        let diag = test_diagnostic();

        assert_eq!(diag.span(), &Span::new(PathBuf::from("test.rs"), 0, 10));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Diagnostic>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Diagnostic>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Diagnostic>();
    }

    #[test]
    fn with_origin_adds_origin() {
        let diag = test_diagnostic().with_origin(origin());

        assert_eq!(diag.origins(), [origin()]);
    }

    #[test]
    fn with_related_adds_related() {
        let related = Location::new(
            Span::new(PathBuf::from("other.rs"), 5, 15),
            "also used here".into(),
        );

        let diag = test_diagnostic().with_related(related.clone());

        assert_eq!(diag.related(), [related]);
    }

    #[test]
    fn with_severity_preserves_the_message() {
        let diag = test_diagnostic()
            .with_origin(origin())
            .with_severity(Severity::Error);

        assert_eq!(diag.message(), "test message");
    }

    #[test]
    fn with_severity_preserves_the_origins() {
        let diag = test_diagnostic()
            .with_origin(origin())
            .with_severity(Severity::Error);

        assert_eq!(diag.origins(), [origin()]);
    }

    #[test]
    fn with_severity_preserves_the_rule_id() {
        let diag = test_diagnostic()
            .with_origin(origin())
            .with_severity(Severity::Error);

        assert_eq!(diag.rule_id(), RuleId::new("lint.test"));
    }

    #[test]
    fn with_severity_preserves_the_span() {
        let diag = test_diagnostic()
            .with_origin(origin())
            .with_severity(Severity::Error);

        assert_eq!(diag.span(), &Span::new(PathBuf::from("test.rs"), 0, 10));
    }

    #[test]
    fn with_severity_replaces_severity() {
        let diag = test_diagnostic().with_severity(Severity::Error);

        assert_eq!(diag.severity(), Severity::Error);
    }

    #[test]
    fn with_suggestion_adds_suggestion() {
        let suggestion = Suggestion::new(
            Span::new(PathBuf::from("test.rs"), 0, 10),
            "replacement".into(),
            "try this".into(),
        );

        let diag = test_diagnostic().with_suggestion(suggestion.clone());

        assert_eq!(diag.suggestions(), [suggestion]);
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        fn arb_severity() -> impl Strategy<Value = Severity> {
            prop_oneof![
                Just(Severity::Help),
                Just(Severity::Info),
                Just(Severity::Warn),
                Just(Severity::Error),
            ]
        }

        fn arb_span() -> impl Strategy<Value = Span> {
            (0..=1000usize, 0..=1000usize).prop_map(|(start, delta)| {
                Span::new(PathBuf::from("test.rs"), start, start + delta)
            })
        }

        proptest! {
            #[test]
            fn new_roundtrips_fields(
                severity in arb_severity(),
                message in "\\PC{0,50}",
                span in arb_span(),
            ) {
                let diag = Diagnostic::new(
                    RuleId::new("lint.prop"),
                    severity,
                    message.clone(),
                    span.clone(),
                );

                prop_assert_eq!(diag.rule_id(), RuleId::new("lint.prop"));
                prop_assert_eq!(diag.severity(), severity);
                prop_assert_eq!(diag.message(), message);
                prop_assert_eq!(diag.span(), &span);
            }

            #[test]
            fn new_starts_with_empty_collections(
                severity in arb_severity(),
            ) {
                let diag = Diagnostic::new(
                    RuleId::new("lint.prop"),
                    severity,
                    "msg".into(),
                    Span::new(PathBuf::from("f.rs"), 0, 1),
                );

                prop_assert!(diag.origins().is_empty());
                prop_assert!(diag.related().is_empty());
                prop_assert!(diag.suggestions().is_empty());
            }

            #[test]
            fn with_origin_accumulates(count in 1..=10usize) {
                let mut diag = test_diagnostic();
                for i in 0..count {
                    let origin = Location::new(
                        Span::new(PathBuf::from("f.rs"), i, i + 1),
                        format!("origin {i}"),
                    );
                    diag = diag.with_origin(origin);
                }
                prop_assert_eq!(diag.origins().len(), count);
            }

            #[test]
            fn with_related_accumulates(count in 1..=10usize) {
                let mut diag = test_diagnostic();
                for i in 0..count {
                    let related = Location::new(
                        Span::new(PathBuf::from("f.rs"), i, i + 1),
                        format!("related {i}"),
                    );
                    diag = diag.with_related(related);
                }
                prop_assert_eq!(diag.related().len(), count);
            }

            #[test]
            fn with_suggestion_accumulates(count in 1..=10usize) {
                let mut diag = test_diagnostic();
                for i in 0..count {
                    let suggestion = Suggestion::new(
                        Span::new(PathBuf::from("f.rs"), i, i + 1),
                        format!("fix {i}"),
                        format!("suggestion {i}"),
                    );
                    diag = diag.with_suggestion(suggestion);
                }
                prop_assert_eq!(diag.suggestions().len(), count);
            }
        }
    }
}
