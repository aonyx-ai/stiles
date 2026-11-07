mod attach_fix_error;

use bon::bon;
use stabby::{option, string, vec};

pub use self::attach_fix_error::AttachFixError;
use crate::{Fix, Location, RuleId, Severity, Span};

/// A diagnostic emitted by a lint rule
///
/// Diagnostics are the primary output of the linting pipeline. Each one
/// identifies a specific issue in source code, with a severity and a
/// primary span. It can also carry origin locations, related locations,
/// and one fix.
///
/// A diagnostic is what a plugin hands back across the plugin boundary.
/// Stabby lays it out, and its strings and lists are stabby's `String` and
/// `Vec`. The message is copied in from what a caller passes, because
/// stabby's `String` cannot adopt an allocation std made.
/// A plugin allocates each diagnostic and stiles frees it, which is sound
/// because every allocation stabby makes carries its own release function.
/// The severity is one byte and sits last, so it puts no padding between
/// the pointer-sized fields before it.
///
/// # Examples
///
/// ```
/// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
///
/// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
/// let diagnostic = Diagnostic::builder()
///     .rule_id(RuleId::new("lint.example"))
///     .severity(Severity::Warn)
///     .message("a wildcard arm")
///     .span(span.clone())
///     .build();
///
/// assert_eq!(diagnostic.message(), "a wildcard arm");
/// ```
#[stabby::stabby]
#[derive(Clone, Debug)]
pub struct Diagnostic {
    rule_id: RuleId,
    message: string::String,
    span: Span,
    origins: vec::Vec<Location>,
    related: vec::Vec<Location>,
    fix: option::Option<Fix>,
    severity: Severity,
}

#[bon]
impl Diagnostic {
    /// Creates a new diagnostic with the required fields
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// assert_eq!(diagnostic.rule_id(), RuleId::new("lint.example"));
    /// ```
    #[builder]
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
            fix: option::Option::None(),
            severity,
        }
    }

    /// Adds an origin location to this diagnostic
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// let origin = Location::builder().span(span).message("the value is set here").build();
    ///
    /// let diagnostic = diagnostic.with_origin(origin);
    ///
    /// assert_eq!(diagnostic.origins().len(), 1);
    /// ```
    pub fn with_origin(mut self, origin: Location) -> Self {
        self.origins.push(origin);
        self
    }

    /// Adds a related location to this diagnostic
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// let related = Location::builder().span(span).message("the value is used here").build();
    ///
    /// let diagnostic = diagnostic.with_related(related);
    ///
    /// assert_eq!(diagnostic.related().len(), 1);
    /// ```
    pub fn with_related(mut self, related: Location) -> Self {
        self.related.push(related);
        self
    }

    /// Attaches the fix that resolves this diagnostic
    ///
    /// A diagnostic holds at most one fix, because `stiles check --fix`
    /// applies the fix without review. Two fixes would leave a choice
    /// between them. The fix must lie in the file of the diagnostic, so
    /// that a fix never changes a file the reader was not shown.
    ///
    /// # Errors
    ///
    /// Returns [`AttachFixError::OtherFile`] if the fix edits another file
    /// than the span of the diagnostic names, and
    /// [`AttachFixError::SecondFix`] if the diagnostic already holds a fix.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Edit, Fix, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    /// let fix = Fix::builder()
    ///     .message("name the variant")
    ///     .edits(vec![Edit::builder().span(span).replacement("Some(_)").build()])
    ///     .build()?;
    ///
    /// let diagnostic = diagnostic.with_fix(fix)?;
    ///
    /// assert!(diagnostic.fix().is_some());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_fix(mut self, fix: Fix) -> Result<Self, AttachFixError> {
        if fix.file() != self.span.file() {
            return Err(AttachFixError::OtherFile {
                diagnostic: self.span.file().to_path_buf(),
                fix: fix.file().to_path_buf(),
            });
        }
        if self.fix.is_some() {
            return Err(AttachFixError::SecondFix);
        }

        self.fix = option::Option::Some(fix);
        Ok(self)
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
    /// use stiles_types::{Diagnostic, RuleId, Severity, Span};
    ///
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.bool-param"))
    ///     .severity(Severity::Warn)
    ///     .message("parameter has type `bool`")
    ///     .span(Span::builder().file(PathBuf::from("src/lib.rs")).start(0).end(10).build())
    ///     .build();
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
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// assert_eq!(diagnostic.rule_id().as_str(), "lint.example");
    /// ```
    pub fn rule_id(&self) -> RuleId {
        self.rule_id
    }

    /// Returns the severity of this diagnostic
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// assert_eq!(diagnostic.severity(), Severity::Warn);
    /// ```
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the human-readable diagnostic message
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// assert_eq!(diagnostic.message(), "a wildcard arm");
    /// ```
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the primary source span
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// assert_eq!(diagnostic.span(), &span);
    /// ```
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns origin locations that explain where something was defined
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// assert!(diagnostic.origins().is_empty());
    /// ```
    pub fn origins(&self) -> &[Location] {
        &self.origins
    }

    /// Returns related locations that provide additional context
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// assert!(diagnostic.related().is_empty());
    /// ```
    pub fn related(&self) -> &[Location] {
        &self.related
    }

    /// Returns the fix that resolves this diagnostic, if the rule attached one
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Diagnostic, Location, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span.clone())
    ///     .build();
    ///
    /// assert!(diagnostic.fix().is_none());
    /// ```
    pub fn fix(&self) -> Option<&Fix> {
        self.fix.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::Edit;

    fn test_diagnostic() -> Diagnostic {
        Diagnostic::builder()
            .rule_id(RuleId::new("lint.test"))
            .severity(Severity::Warn)
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
        Location::builder()
            .span(
                Span::builder()
                    .file(PathBuf::from("other.rs"))
                    .start(5)
                    .end(15)
                    .build(),
            )
            .message("defined here")
            .build()
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
    fn new_diagnostic_has_no_fix() {
        let diag = test_diagnostic();

        assert!(diag.fix().is_none());
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

        assert_eq!(
            diag.span(),
            &Span::builder()
                .file(PathBuf::from("test.rs"))
                .start(0)
                .end(10)
                .build()
        );
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

    fn fix_in(file: &str) -> Fix {
        Fix::builder()
            .message("replace it")
            .edits(vec![
                Edit::builder()
                    .span(
                        Span::builder()
                            .file(PathBuf::from(file))
                            .start(0)
                            .end(10)
                            .build(),
                    )
                    .replacement("replacement")
                    .build(),
            ])
            .build()
            .expect("the fix should build")
    }

    #[test]
    fn with_fix_attaches_the_fix() {
        let diag = test_diagnostic()
            .with_fix(fix_in("test.rs"))
            .expect("the fix should attach");

        assert_eq!(diag.fix(), Some(&fix_in("test.rs")));
    }

    #[test]
    fn with_fix_in_another_file_returns_other_file() {
        let error = test_diagnostic()
            .with_fix(fix_in("other.rs"))
            .expect_err("the fix should not attach");

        assert_eq!(
            error,
            AttachFixError::OtherFile {
                diagnostic: PathBuf::from("test.rs"),
                fix: PathBuf::from("other.rs"),
            }
        );
    }

    #[test]
    fn with_fix_on_a_fixed_diagnostic_returns_second_fix() {
        let diag = test_diagnostic()
            .with_fix(fix_in("test.rs"))
            .expect("the first fix should attach");

        let error = diag
            .with_fix(fix_in("test.rs"))
            .expect_err("the second fix should not attach");

        assert_eq!(error, AttachFixError::SecondFix);
    }

    #[test]
    fn with_origin_adds_origin() {
        let diag = test_diagnostic().with_origin(origin());

        assert_eq!(diag.origins(), [origin()]);
    }

    #[test]
    fn with_related_adds_related() {
        let related = Location::builder()
            .span(
                Span::builder()
                    .file(PathBuf::from("other.rs"))
                    .start(5)
                    .end(15)
                    .build(),
            )
            .message("also used here")
            .build();

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

        assert_eq!(
            diag.span(),
            &Span::builder()
                .file(PathBuf::from("test.rs"))
                .start(0)
                .end(10)
                .build()
        );
    }

    #[test]
    fn with_severity_replaces_severity() {
        let diag = test_diagnostic().with_severity(Severity::Error);

        assert_eq!(diag.severity(), Severity::Error);
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
                Span::builder()
                    .file(PathBuf::from("test.rs"))
                    .start(start)
                    .end(start + delta)
                    .build()
            })
        }

        proptest! {
            #[test]
            fn new_roundtrips_fields(
                severity in arb_severity(),
                message in "\\PC{0,50}",
                span in arb_span(),
            ) {
                let diag = Diagnostic::builder()
                    .rule_id(RuleId::new("lint.prop"))
                    .severity(severity)
                    .message(message.clone())
                    .span(span.clone())
                    .build();

                prop_assert_eq!(diag.rule_id(), RuleId::new("lint.prop"));
                prop_assert_eq!(diag.severity(), severity);
                prop_assert_eq!(diag.message(), message);
                prop_assert_eq!(diag.span(), &span);
            }

            #[test]
            fn new_starts_with_empty_collections(
                severity in arb_severity(),
            ) {
                let diag = Diagnostic::builder()
                    .rule_id(RuleId::new("lint.prop"))
                    .severity(severity)
                    .message("msg")
                    .span(Span::builder().file(PathBuf::from("f.rs")).start(0).end(1).build())
                    .build();

                prop_assert!(diag.origins().is_empty());
                prop_assert!(diag.related().is_empty());
                prop_assert!(diag.fix().is_none());
            }

            #[test]
            fn with_origin_accumulates(count in 1..=10usize) {
                let mut diag = test_diagnostic();
                for i in 0..count {
                    let origin = Location::builder()
                        .span(Span::builder()
                            .file(PathBuf::from("f.rs"))
                            .start(i)
                            .end(i + 1)
                            .build())
                        .message(format!("origin {i}"))
                        .build();
                    diag = diag.with_origin(origin);
                }
                prop_assert_eq!(diag.origins().len(), count);
            }

            #[test]
            fn with_related_accumulates(count in 1..=10usize) {
                let mut diag = test_diagnostic();
                for i in 0..count {
                    let related = Location::builder()
                        .span(Span::builder()
                            .file(PathBuf::from("f.rs"))
                            .start(i)
                            .end(i + 1)
                            .build())
                        .message(format!("related {i}"))
                        .build();
                    diag = diag.with_related(related);
                }
                prop_assert_eq!(diag.related().len(), count);
            }
        }
    }
}
