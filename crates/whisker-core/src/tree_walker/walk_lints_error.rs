use thiserror::Error;

use crate::{PassPanic, UndeclaredRule};

/// The reason a walk of the lint passes stopped
///
/// The walk stops at the first pass that misbehaves. Each variant holds
/// the error that names the pass's fault and the node it was checking.
///
/// # Examples
///
/// ```
/// use whisker_core::{UndeclaredRule, WalkLintsError};
/// use whisker_types::{RuleId, Span};
///
/// let error = WalkLintsError::Undeclared {
///     source: UndeclaredRule::builder()
///         .rule(RuleId::new("lint.bar"))
///         .declared(Vec::new())
///         .span(Span::builder().file("src/lib.rs").start(3).end(9).build())
///         .build(),
/// };
///
/// assert!(error.to_string().starts_with("a lint pass reported `lint.bar`"));
/// ```
#[derive(Clone, Eq, PartialEq, Debug, Error)]
pub enum WalkLintsError {
    /// A pass panicked while it checked a node
    #[error(transparent)]
    Panicked {
        /// The panic, with the node the pass was checking
        source: PassPanic,
    },

    /// A pass reported a rule that its plugin did not declare for it
    #[error(transparent)]
    Undeclared {
        /// The rule the pass reported, with the rules it declared
        source: UndeclaredRule,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WalkLintsError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<WalkLintsError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<WalkLintsError>();
    }
}
