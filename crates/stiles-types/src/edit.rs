use bon::bon;
use stabby::string;

use crate::Span;

/// A replacement of the text at one span of a file
///
/// An edit is one part of a [`Fix`]. It replaces the text at `span` with
/// `replacement`. An empty replacement deletes the span, and an empty span
/// inserts the replacement at its position.
///
/// An edit crosses the plugin boundary inside a [`Diagnostic`], so stabby
/// lays it out and the replacement is stabby's `String`.
///
/// # Examples
///
/// ```
/// use stiles_types::{Edit, Span};
///
/// let span = Span::builder().file("src/lib.rs").start(3).end(4).build();
/// let edit = Edit::builder().span(span).replacement("Some(_)").build();
///
/// assert_eq!(edit.replacement(), "Some(_)");
/// ```
///
/// [`Diagnostic`]: crate::Diagnostic
/// [`Fix`]: crate::Fix
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Edit {
    span: Span,
    replacement: string::String,
}

#[bon]
impl Edit {
    /// Creates an edit that replaces the text at `span` with `replacement`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Edit, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(4).build();
    /// let edit = Edit::builder().span(span.clone()).replacement("").build();
    ///
    /// assert_eq!(edit.span(), &span);
    /// ```
    #[builder]
    pub fn new(span: Span, #[builder(into)] replacement: String) -> Self {
        Self {
            span,
            replacement: string::String::from(replacement.as_str()),
        }
    }

    /// Returns the span of the text that the edit replaces
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Edit, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(4).build();
    /// let edit = Edit::builder().span(span).replacement("x").build();
    ///
    /// assert_eq!(edit.span().end(), 4);
    /// ```
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the text that replaces the span
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Edit, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(4).build();
    /// let edit = Edit::builder().span(span).replacement("x").build();
    ///
    /// assert_eq!(edit.replacement(), "x");
    /// ```
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// Returns whether this edit and `other` overlap
    ///
    /// Two edits overlap when their spans share a byte. An insertion
    /// overlaps a span that holds its position inside it. It also overlaps
    /// another insertion at the same position, because the order of the two
    /// texts would be a guess. Spans that only touch at a boundary do not
    /// overlap, so an insertion at the start or the end of a span is fine.
    /// Edits in two files never overlap.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Edit, Span};
    ///
    /// let edit = |start, end| {
    ///     let span = Span::builder().file("src/lib.rs").start(start).end(end).build();
    ///     Edit::builder().span(span).replacement("x").build()
    /// };
    ///
    /// assert!(edit(0, 5).overlaps(&edit(2, 2)));
    /// assert!(!edit(0, 5).overlaps(&edit(5, 5)));
    /// ```
    pub fn overlaps(&self, other: &Edit) -> bool {
        let (start, end) = (self.span.start(), self.span.end());
        let (other_start, other_end) = (other.span.start(), other.span.end());
        let same_file = self.span.file() == other.span.file();

        let share = match (start == end, other_start == other_end) {
            (true, true) => start == other_start,
            (true, false) => other_start < start && start < other_end,
            (false, true) => start < other_start && other_start < end,
            (false, false) => start.max(other_start) < end.min(other_end),
        };

        same_file && share
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use super::*;

    fn span() -> Span {
        Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(0)
            .end(5)
            .build()
    }

    fn edit(range: Range<usize>) -> Edit {
        edit_in("test.rs", range)
    }

    fn edit_in(file: &str, range: Range<usize>) -> Edit {
        let Range { start, end } = range;
        Edit::builder()
            .span(
                Span::builder()
                    .file(PathBuf::from(file))
                    .start(start)
                    .end(end)
                    .build(),
            )
            .replacement("x")
            .build()
    }

    #[test]
    fn overlaps_with_a_shared_byte_returns_true() {
        let overlaps = edit(0..3).overlaps(&edit(2..5));

        assert!(overlaps);
    }

    #[test]
    fn overlaps_with_adjacent_spans_returns_false() {
        let overlaps = edit(0..3).overlaps(&edit(3..5));

        assert!(!overlaps);
    }

    #[test]
    fn overlaps_with_an_insertion_at_a_boundary_returns_false() {
        let overlaps = edit(3..3).overlaps(&edit(3..5));

        assert!(!overlaps);
    }

    #[test]
    fn overlaps_with_an_insertion_inside_returns_true() {
        let overlaps = edit(4..4).overlaps(&edit(3..5));

        assert!(overlaps);
    }

    #[test]
    fn overlaps_with_another_file_returns_false() {
        let overlaps = edit(0..3).overlaps(&edit_in("other.rs", 0..3));

        assert!(!overlaps);
    }

    #[test]
    fn overlaps_with_two_insertions_at_one_position_returns_true() {
        let overlaps = edit(3..3).overlaps(&edit(3..3));

        assert!(overlaps);
    }

    #[test]
    fn replacement_returns_the_replacement() {
        let edit = Edit::builder().span(span()).replacement("new_text").build();

        assert_eq!(edit.replacement(), "new_text");
    }

    #[test]
    fn span_returns_the_span() {
        let edit = Edit::builder().span(span()).replacement("new_text").build();

        assert_eq!(edit.span(), &span());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Edit>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Edit>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Edit>();
    }
}
