use bon::bon;
use stabby::string;

use crate::Span;

/// A suggested source code replacement
///
/// Represents a machine-applicable fix: replace the text at `span` with
/// `replacement`. An empty replacement means "delete this span".
///
/// A suggestion crosses the plugin boundary inside a [`Diagnostic`], so
/// stabby lays it out and its strings are stabby's `String`.
///
/// [`Diagnostic`]: crate::Diagnostic
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Suggestion {
    span: Span,
    replacement: string::String,
    message: string::String,
}

#[bon]
impl Suggestion {
    /// Creates a suggestion to replace the text at `span` with `replacement`
    #[builder]
    pub fn new(
        span: Span,
        #[builder(into)] replacement: String,
        #[builder(into)] message: String,
    ) -> Self {
        Self {
            span,
            replacement: string::String::from(replacement.as_str()),
            message: string::String::from(message.as_str()),
        }
    }

    /// Returns the span of the text to be replaced
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the replacement text
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// Returns the human-readable description of this suggestion
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn message_returns_the_message() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(0)
            .end(5)
            .build();
        let suggestion = Suggestion::builder()
            .span(span)
            .replacement("new_text")
            .message("try this")
            .build();

        assert_eq!(suggestion.message(), "try this");
    }

    #[test]
    fn replacement_returns_the_replacement() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(0)
            .end(5)
            .build();
        let suggestion = Suggestion::builder()
            .span(span)
            .replacement("new_text")
            .message("try this")
            .build();

        assert_eq!(suggestion.replacement(), "new_text");
    }

    #[test]
    fn span_returns_the_span() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(0)
            .end(5)
            .build();
        let suggestion = Suggestion::builder()
            .span(span.clone())
            .replacement("new_text")
            .message("try this")
            .build();

        assert_eq!(suggestion.span(), &span);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Suggestion>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Suggestion>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Suggestion>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn new_roundtrips_all_fields(
                start in 0..=1000usize,
                delta in 0..=1000usize,
                replacement in "\\PC{0,50}",
                message in "\\PC{0,50}",
            ) {
                let span = Span::builder()
                    .file(PathBuf::from("f.rs"))
                    .start(start)
                    .end(start + delta)
                    .build();
                let s = Suggestion::builder()
                    .span(span.clone())
                    .replacement(replacement.clone())
                    .message(message.clone())
                    .build();

                prop_assert_eq!(s.span(), &span);
                prop_assert_eq!(s.replacement(), replacement);
                prop_assert_eq!(s.message(), message);
            }
        }
    }
}
