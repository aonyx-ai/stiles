use std::fmt;

use bon::Builder;
use whisker_types::{RuleId, Span};

/// A lint pass reported a rule its plugin did not declare for it
///
/// A plugin declares the rules each pass reports, and whisker relies on
/// that list to turn rules off, route their options, and list them. A
/// diagnostic under another id would slip past all three, so the walk
/// stops at it, the way it stops at a panic. The cause is a mistake in
/// the plugin, often a rule copied from another and renamed in one place
/// only.
///
/// # Examples
///
/// ```
/// use whisker_core::UndeclaredRule;
/// use whisker_types::{RuleId, Span};
///
/// let error = UndeclaredRule::builder()
///     .rule(RuleId::new("lint.bar"))
///     .declared(vec![RuleId::new("lint.foo")])
///     .span(Span::builder().file("src/lib.rs").start(3).end(9).build())
///     .build();
///
/// assert_eq!(
///     error.to_string(),
///     "a lint pass reported `lint.bar` at byte 3 of src/lib.rs, but its plugin declares only `lint.foo` for it"
/// );
/// ```
#[derive(Clone, Eq, PartialEq, Debug, Builder)]
pub struct UndeclaredRule {
    rule: RuleId,
    declared: Vec<RuleId>,
    span: Span,
}

impl UndeclaredRule {
    /// Returns the rule the pass reported
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_core::UndeclaredRule;
    /// use whisker_types::{RuleId, Span};
    ///
    /// let error = UndeclaredRule::builder()
    ///     .rule(RuleId::new("lint.bar"))
    ///     .declared(Vec::new())
    ///     .span(Span::builder().file("a.rs").start(0).end(1).build())
    ///     .build();
    ///
    /// assert_eq!(error.rule(), RuleId::new("lint.bar"));
    /// ```
    pub fn rule(&self) -> RuleId {
        self.rule
    }

    /// Returns where the node the pass was checking sits in its file
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_core::UndeclaredRule;
    /// use whisker_types::{RuleId, Span};
    ///
    /// let error = UndeclaredRule::builder()
    ///     .rule(RuleId::new("lint.bar"))
    ///     .declared(Vec::new())
    ///     .span(Span::builder().file("a.rs").start(0).end(1).build())
    ///     .build();
    ///
    /// assert_eq!(error.span().start(), 0);
    /// ```
    pub fn span(&self) -> &Span {
        &self.span
    }
}

impl fmt::Display for UndeclaredRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a lint pass reported `{}` at byte {} of {}, but its plugin ",
            self.rule,
            self.span.start(),
            self.span.file().display(),
        )?;

        match self.declared.as_slice() {
            [] => f.write_str("declares no rules for it"),
            declared => {
                let names: Vec<String> = declared.iter().map(|rule| format!("`{rule}`")).collect();
                write!(f, "declares only {} for it", names.join(", "))
            }
        }
    }
}

impl std::error::Error for UndeclaredRule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_with_no_declared_rules_says_so() {
        let error = UndeclaredRule::builder()
            .rule(RuleId::new("lint.bar"))
            .declared(Vec::new())
            .span(Span::builder().file("a.rs").start(0).end(1).build())
            .build();

        let message = error.to_string();

        assert!(message.ends_with("declares no rules for it"), "{message}");
    }

    #[test]
    fn display_with_two_declared_rules_names_both() {
        let error = UndeclaredRule::builder()
            .rule(RuleId::new("lint.bar"))
            .declared(vec![RuleId::new("lint.foo"), RuleId::new("lint.baz")])
            .span(Span::builder().file("a.rs").start(0).end(1).build())
            .build();

        let message = error.to_string();

        assert!(
            message.ends_with("declares only `lint.foo`, `lint.baz` for it"),
            "{message}"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<UndeclaredRule>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<UndeclaredRule>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<UndeclaredRule>();
    }
}
