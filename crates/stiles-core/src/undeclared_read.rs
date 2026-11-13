use std::fmt;

use bon::Builder;
use stiles_types::{DecorationKey, RuleId, Span};

/// A lint pass read a decoration its plugin did not declare for it
///
/// A plugin declares the decorations each pass reads, and a provider
/// computes only those. A lookup of any other decoration would find
/// nothing, and the rule would report nothing, which reads like a rule
/// that found no fault. The walk therefore stops at such a lookup, the way
/// it stops at a panic, and names the rules of the lint and the key. The
/// cause is a mistake in the plugin: a decoration left out of
/// `DeclaresRules::reads`.
///
/// # Examples
///
/// ```
/// use stiles_core::UndeclaredRead;
/// use stiles_types::{DecorationKey, RuleId, Span};
///
/// let error = UndeclaredRead::builder()
///     .rules(vec![RuleId::new("lint.bar")])
///     .key(DecorationKey::new("my_crate::Signature"))
///     .span(Span::builder().file("src/lib.rs").start(3).end(9).build())
///     .build();
///
/// assert_eq!(
///     error.to_string(),
///     "the lint of `lint.bar` reads the decoration `my_crate::Signature` at byte 3 of \
///      src/lib.rs, which its plugin does not declare for it; add it to the lint's \
///      `DeclaresRules::reads`"
/// );
/// ```
#[derive(Clone, Eq, PartialEq, Debug, Builder)]
pub struct UndeclaredRead {
    rules: Vec<RuleId>,
    key: DecorationKey,
    span: Span,
}

impl UndeclaredRead {
    /// Returns the key of the decoration the pass read
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_core::UndeclaredRead;
    /// use stiles_types::{DecorationKey, RuleId, Span};
    ///
    /// let error = UndeclaredRead::builder()
    ///     .rules(vec![RuleId::new("lint.bar")])
    ///     .key(DecorationKey::new("my_crate::Signature"))
    ///     .span(Span::builder().file("a.rs").start(0).end(1).build())
    ///     .build();
    ///
    /// assert_eq!(error.key(), DecorationKey::new("my_crate::Signature"));
    /// ```
    pub fn key(&self) -> DecorationKey {
        self.key
    }

    /// Returns the rules of the lint whose pass read the decoration
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_core::UndeclaredRead;
    /// use stiles_types::{DecorationKey, RuleId, Span};
    ///
    /// let error = UndeclaredRead::builder()
    ///     .rules(vec![RuleId::new("lint.bar")])
    ///     .key(DecorationKey::new("my_crate::Signature"))
    ///     .span(Span::builder().file("a.rs").start(0).end(1).build())
    ///     .build();
    ///
    /// assert_eq!(error.rules(), [RuleId::new("lint.bar")]);
    /// ```
    pub fn rules(&self) -> &[RuleId] {
        &self.rules
    }

    /// Returns where the node the pass was checking sits in its file
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_core::UndeclaredRead;
    /// use stiles_types::{DecorationKey, RuleId, Span};
    ///
    /// let error = UndeclaredRead::builder()
    ///     .rules(vec![RuleId::new("lint.bar")])
    ///     .key(DecorationKey::new("my_crate::Signature"))
    ///     .span(Span::builder().file("a.rs").start(0).end(1).build())
    ///     .build();
    ///
    /// assert_eq!(error.span().start(), 0);
    /// ```
    pub fn span(&self) -> &Span {
        &self.span
    }
}

impl fmt::Display for UndeclaredRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.rules.as_slice() {
            [] => f.write_str("a lint that declares no rules")?,
            rules => {
                let names: Vec<String> = rules.iter().map(|rule| format!("`{rule}`")).collect();
                write!(f, "the lint of {}", names.join(", "))?;
            }
        }

        write!(
            f,
            " reads the decoration `{}` at byte {} of {}, which its plugin does not declare for \
             it; add it to the lint's `DeclaresRules::reads`",
            self.key.as_str(),
            self.span.start(),
            self.span.file().display(),
        )
    }
}

impl std::error::Error for UndeclaredRead {}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: DecorationKey = DecorationKey::new("my_crate::Signature");

    #[test]
    fn display_with_no_rules_says_so() {
        let error = UndeclaredRead::builder()
            .rules(Vec::new())
            .key(KEY)
            .span(Span::builder().file("a.rs").start(0).end(1).build())
            .build();

        let message = error.to_string();

        assert!(
            message.starts_with("a lint that declares no rules reads"),
            "{message}"
        );
    }

    #[test]
    fn display_with_two_rules_names_both() {
        let error = UndeclaredRead::builder()
            .rules(vec![RuleId::new("lint.foo"), RuleId::new("lint.baz")])
            .key(KEY)
            .span(Span::builder().file("a.rs").start(0).end(1).build())
            .build();

        let message = error.to_string();

        assert!(
            message.starts_with("the lint of `lint.foo`, `lint.baz` reads"),
            "{message}"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<UndeclaredRead>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<UndeclaredRead>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<UndeclaredRead>();
    }
}
