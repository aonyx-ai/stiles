mod build_fix_error;

use std::path::Path;

use bon::bon;
use stabby::{string, vec};

pub use self::build_fix_error::BuildFixError;
use crate::{Edit, FilePath};

/// A change to a file that resolves a diagnostic
///
/// A fix is a set of edits that apply together or not at all. A rule
/// attaches a fix only when the change is correct without review, because
/// `stiles check --fix` applies every fix it finds. A change that a rule is
/// not sure of belongs in the message of the diagnostic as advice, and not
/// in a fix.
///
/// The edits of a fix all lie in one file, and no two of them overlap, as
/// [`Edit::overlaps`] defines it. The fix keeps its edits in the order of
/// their spans. An insertion at the start of a span therefore comes before
/// the replacement of that span.
///
/// Sorted edits overlap only if two neighbors do. A later edit that overlaps
/// an earlier one starts before that edit ends, and so does every edit that
/// sorts between them. The fix therefore compares neighbors only.
///
/// A fix cannot check that its spans lie inside the file and on character
/// boundaries, because it does not hold the text. The code that applies the
/// fix checks that.
///
/// A fix crosses the plugin boundary inside a [`Diagnostic`], so stabby
/// lays it out and its strings and lists are stabby's `String` and `Vec`.
///
/// # Examples
///
/// ```
/// use stiles_types::{Edit, Fix, Span};
///
/// let span = Span::builder().file("src/lib.rs").start(3).end(4).build();
/// let edit = Edit::builder().span(span).replacement("").build();
/// let fix = Fix::builder()
///     .message("remove the period")
///     .edits(vec![edit])
///     .build()?;
///
/// assert_eq!(fix.edits().len(), 1);
/// # Ok::<(), stiles_types::BuildFixError>(())
/// ```
///
/// [`Diagnostic`]: crate::Diagnostic
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Fix {
    file: FilePath,
    message: string::String,
    edits: vec::Vec<Edit>,
}

#[bon]
impl Fix {
    /// Creates a fix from its edits, in any order
    ///
    /// # Errors
    ///
    /// Returns [`BuildFixError::NoEdits`] if `edits` is empty,
    /// [`BuildFixError::SeveralFiles`] if the edits lie in more than one
    /// file, and [`BuildFixError::OverlappingEdits`] if two edits overlap.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{BuildFixError, Edit, Fix, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(4).build();
    /// let edit = Edit::builder().span(span).replacement("x").build();
    ///
    /// let error = Fix::builder()
    ///     .message("name the variable")
    ///     .edits(vec![edit.clone(), edit])
    ///     .build()
    ///     .expect_err("the edits overlap");
    ///
    /// assert_eq!(
    ///     error,
    ///     BuildFixError::OverlappingEdits {
    ///         file: "src/lib.rs".into(),
    ///         first: 3..4,
    ///         second: 3..4,
    ///     }
    /// );
    /// ```
    #[builder]
    pub fn new(#[builder(into)] message: String, edits: Vec<Edit>) -> Result<Self, BuildFixError> {
        let mut edits = edits;
        edits.sort_by_key(|edit| (edit.span().start(), edit.span().end()));

        let Some(first) = edits.first() else {
            return Err(BuildFixError::NoEdits);
        };
        let file = first.span().file_path().clone();
        if let Some(other) = edits
            .iter()
            .find(|edit| edit.span().file() != file.as_path())
        {
            return Err(BuildFixError::SeveralFiles {
                first: file.to_path_buf(),
                other: other.span().file().to_path_buf(),
            });
        }

        for pair in edits.windows(2) {
            let [earlier, later] = pair else { continue };
            if earlier.overlaps(later) {
                return Err(BuildFixError::OverlappingEdits {
                    file: file.to_path_buf(),
                    first: earlier.span().start()..earlier.span().end(),
                    second: later.span().start()..later.span().end(),
                });
            }
        }

        Ok(Self {
            file,
            message: string::String::from(message.as_str()),
            edits: edits.into_iter().collect(),
        })
    }

    /// Returns what the fix changes, in words a reader can act on
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Edit, Fix, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(4).build();
    /// let edit = Edit::builder().span(span).replacement("").build();
    /// let fix = Fix::builder().message("remove the period").edits(vec![edit]).build()?;
    ///
    /// assert_eq!(fix.message(), "remove the period");
    /// # Ok::<(), stiles_types::BuildFixError>(())
    /// ```
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the edits of the fix, in the order of their spans
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Edit, Fix, Span};
    ///
    /// let later = Span::builder().file("src/lib.rs").start(8).end(9).build();
    /// let earlier = Span::builder().file("src/lib.rs").start(3).end(4).build();
    /// let fix = Fix::builder()
    ///     .message("swap the names")
    ///     .edits(vec![
    ///         Edit::builder().span(later).replacement("a").build(),
    ///         Edit::builder().span(earlier).replacement("b").build(),
    ///     ])
    ///     .build()?;
    ///
    /// assert_eq!(fix.edits()[0].replacement(), "b");
    /// # Ok::<(), stiles_types::BuildFixError>(())
    /// ```
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    /// Returns the file that every edit of the fix lies in
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// use stiles_types::{Edit, Fix, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(4).build();
    /// let edit = Edit::builder().span(span).replacement("").build();
    /// let fix = Fix::builder().message("remove the period").edits(vec![edit]).build()?;
    ///
    /// assert_eq!(fix.file(), Path::new("src/lib.rs"));
    /// # Ok::<(), stiles_types::BuildFixError>(())
    /// ```
    pub fn file(&self) -> &Path {
        self.file.as_path()
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use super::*;
    use crate::Span;

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

    fn fix(edits: Vec<Edit>) -> Result<Fix, BuildFixError> {
        Fix::builder().message("fix it").edits(edits).build()
    }

    #[test]
    fn edits_are_sorted_by_span() {
        let fix = fix(vec![edit(8..9), edit(3..3), edit(3..4)]).expect("should build");

        let starts: Vec<_> = fix
            .edits()
            .iter()
            .map(|edit| (edit.span().start(), edit.span().end()))
            .collect();

        assert_eq!(starts, [(3, 3), (3, 4), (8, 9)]);
    }

    #[test]
    fn file_returns_the_file_of_the_edits() {
        let fix = fix(vec![edit_in("src/lib.rs", 0..1)]).expect("should build");

        let file = fix.file();

        assert_eq!(file, Path::new("src/lib.rs"));
    }

    #[test]
    fn message_returns_the_message() {
        let fix = fix(vec![edit(0..1)]).expect("should build");

        let message = fix.message();

        assert_eq!(message, "fix it");
    }

    #[test]
    fn new_with_adjacent_edits_builds() {
        let result = fix(vec![edit(0..3), edit(3..5)]);

        assert!(result.is_ok());
    }

    #[test]
    fn new_with_an_insertion_at_the_end_of_a_span_builds() {
        let result = fix(vec![edit(0..3), edit(3..3)]);

        assert!(result.is_ok());
    }

    #[test]
    fn new_with_an_insertion_at_the_start_of_a_span_builds() {
        let result = fix(vec![edit(3..5), edit(3..3)]);

        assert!(result.is_ok());
    }

    #[test]
    fn new_with_an_insertion_inside_a_span_returns_overlapping_edits() {
        let error = fix(vec![edit(0..5), edit(2..2)]).expect_err("should fail");

        assert_eq!(
            error,
            BuildFixError::OverlappingEdits {
                file: PathBuf::from("test.rs"),
                first: 0..5,
                second: 2..2,
            }
        );
    }

    #[test]
    fn new_with_edits_in_two_files_returns_several_files() {
        let error =
            fix(vec![edit_in("a.rs", 0..1), edit_in("b.rs", 2..3)]).expect_err("should fail");

        assert_eq!(
            error,
            BuildFixError::SeveralFiles {
                first: PathBuf::from("a.rs"),
                other: PathBuf::from("b.rs"),
            }
        );
    }

    #[test]
    fn new_with_no_edits_returns_no_edits() {
        let error = fix(Vec::new()).expect_err("should fail");

        assert_eq!(error, BuildFixError::NoEdits);
    }

    #[test]
    fn new_with_overlapping_edits_names_both_ranges() {
        let error = fix(vec![edit(4..9), edit(0..5)]).expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "the fix edits bytes 0..5 and 4..9 of test.rs, which overlap"
        );
    }

    #[test]
    fn new_with_two_insertions_at_one_position_returns_overlapping_edits() {
        let error = fix(vec![edit(3..3), edit(3..3)]).expect_err("should fail");

        assert_eq!(
            error,
            BuildFixError::OverlappingEdits {
                file: PathBuf::from("test.rs"),
                first: 3..3,
                second: 3..3,
            }
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Fix>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Fix>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Fix>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        /// Returns whether any two of the edits overlap, comparing every pair
        fn any_pair_overlaps(edits: &[Edit]) -> bool {
            edits
                .iter()
                .enumerate()
                .any(|(index, edit)| edits[index + 1..].iter().any(|other| edit.overlaps(other)))
        }

        proptest! {
            #[test]
            fn new_refuses_exactly_the_overlapping_edits(
                spans in proptest::collection::vec((0..20usize, 0..4usize), 1..6),
            ) {
                let spans: Vec<(usize, usize)> = spans
                    .into_iter()
                    .map(|(start, length)| (start, start + length))
                    .collect();
                let edits: Vec<Edit> = spans.iter().map(|&(start, end)| edit(start..end)).collect();
                let overlapping = any_pair_overlaps(&edits);

                let result = fix(edits);

                prop_assert_eq!(result.is_err(), overlapping);
            }
        }
    }
}
