mod apply_fix_round_error;

use bon::bon;
use stiles_types::{Diagnostic, Edit, Fix, RuleId};

pub use self::apply_fix_round_error::ApplyFixRoundError;

/// The text of a file after one round of fixes, and which fixes made it
///
/// A round takes the fixes of one file in source order, by the first edit
/// of each. It applies a fix when none of its edits overlaps an edit of a
/// fix the round already took, as [`Edit::overlaps`] defines it. A fix that
/// overlaps waits, because both fixes were built against the old text and
/// only one of them can still be right. The caller checks the new text
/// again, and the rule that built the waiting fix reports it again if it
/// still applies.
///
/// Two fixes that only touch at a boundary both apply. An insertion at the
/// start of a replaced span lands before the replacement.
///
/// # Examples
///
/// ```
/// use stiles_core::FixRound;
/// use stiles_types::{Diagnostic, Edit, Fix, RuleId, Severity, Span};
///
/// let span = Span::builder().file("src/lib.rs").start(13).end(14).build();
/// let fix = Fix::builder()
///     .message("remove the period")
///     .edits(vec![Edit::builder().span(span.clone()).replacement("").build()])
///     .build()?;
/// let diagnostic = Diagnostic::builder()
///     .rule_id(RuleId::new("lint.doc-summary-period"))
///     .severity(Severity::Error)
///     .message("the summary ends with a period")
///     .span(span)
///     .build()
///     .with_fix(fix)?;
///
/// let round = FixRound::apply()
///     .text("/// Returns 1.\nfn one() {}\n")
///     .diagnostics(&[diagnostic])
///     .call()?;
///
/// assert_eq!(round.text(), "/// Returns 1\nfn one() {}\n");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct FixRound {
    text: String,
    applied: Vec<RuleId>,
    deferred: Vec<RuleId>,
}

#[bon]
impl FixRound {
    /// Applies the fixes of `diagnostics` to `text`, the text they were
    /// reported on
    ///
    /// A diagnostic without a fix takes no part. The diagnostics must all
    /// report on the file whose text this is, which holds when they come
    /// from one check of that file.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyFixRoundError::OutOfBounds`] if an edit reaches past
    /// the end of `text`, and [`ApplyFixRoundError::SplitsCharacter`] if an
    /// edit starts or ends inside a character. Either one is a mistake in
    /// the rule that built the fix. The round therefore checks every fix
    /// before it applies any, and that includes the fixes that wait.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_core::FixRound;
    /// use stiles_types::{Diagnostic, Edit, Fix, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(4).end(5).build();
    /// let fix = Fix::builder()
    ///     .message("rename the variable")
    ///     .edits(vec![Edit::builder().span(span.clone()).replacement("b").build()])
    ///     .build()?;
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Error)
    ///     .message("a vague name")
    ///     .span(span)
    ///     .build()
    ///     .with_fix(fix)?;
    ///
    /// let round = FixRound::apply().text("let a = 1;").diagnostics(&[diagnostic]).call()?;
    ///
    /// assert_eq!(round.text(), "let b = 1;");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[builder]
    pub fn apply(text: &str, diagnostics: &[Diagnostic]) -> Result<Self, ApplyFixRoundError> {
        let mut fixes: Vec<(RuleId, &Fix)> = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.fix().map(|fix| (diagnostic.rule_id(), fix)))
            .collect();

        for (rule, fix) in &fixes {
            for edit in fix.edits() {
                check_bounds(text, *rule, edit)?;
            }
        }

        fixes.sort_by_key(|(_, fix)| {
            fix.edits()
                .first()
                .map(|edit| (edit.span().start(), edit.span().end()))
        });

        let mut taken: Vec<&Edit> = Vec::new();
        let mut applied = Vec::new();
        let mut deferred = Vec::new();

        for (rule, fix) in fixes {
            let collides = fix
                .edits()
                .iter()
                .any(|edit| taken.iter().any(|other| edit.overlaps(other)));
            match collides {
                true => deferred.push(rule),
                false => {
                    taken.extend(fix.edits());
                    applied.push(rule);
                }
            }
        }

        taken.sort_by_key(|edit| (edit.span().start(), edit.span().end()));

        let mut fixed = String::with_capacity(text.len());
        let mut copied = 0;
        for edit in taken {
            fixed.push_str(&text[copied..edit.span().start()]);
            fixed.push_str(edit.replacement());
            copied = edit.span().end();
        }
        fixed.push_str(&text[copied..]);

        Ok(Self {
            text: fixed,
            applied,
            deferred,
        })
    }

    /// Returns the text after the round
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_core::FixRound;
    ///
    /// let round = FixRound::apply().text("fn one() {}\n").diagnostics(&[]).call()?;
    ///
    /// assert_eq!(round.text(), "fn one() {}\n");
    /// # Ok::<(), stiles_core::ApplyFixRoundError>(())
    /// ```
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the text after the round, without a copy
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_core::FixRound;
    ///
    /// let round = FixRound::apply().text("fn one() {}\n").diagnostics(&[]).call()?;
    ///
    /// assert_eq!(round.into_text(), "fn one() {}\n");
    /// # Ok::<(), stiles_core::ApplyFixRoundError>(())
    /// ```
    pub fn into_text(self) -> String {
        self.text
    }

    /// Returns the rule of each fix the round applied, in source order
    ///
    /// A rule appears once for every fix of it that applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_core::FixRound;
    ///
    /// let round = FixRound::apply().text("fn one() {}\n").diagnostics(&[]).call()?;
    ///
    /// assert!(round.applied().is_empty());
    /// # Ok::<(), stiles_core::ApplyFixRoundError>(())
    /// ```
    pub fn applied(&self) -> &[RuleId] {
        &self.applied
    }

    /// Returns the rule of each fix that waits for the next round
    ///
    /// A fix waits when it overlaps a fix that sorts before it.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_core::FixRound;
    ///
    /// let round = FixRound::apply().text("fn one() {}\n").diagnostics(&[]).call()?;
    ///
    /// assert!(round.deferred().is_empty());
    /// # Ok::<(), stiles_core::ApplyFixRoundError>(())
    /// ```
    pub fn deferred(&self) -> &[RuleId] {
        &self.deferred
    }
}

/// Checks that `edit` lies inside `text` and on character boundaries
///
/// # Errors
///
/// Returns an error that names `rule` if the edit does not.
fn check_bounds(text: &str, rule: RuleId, edit: &Edit) -> Result<(), ApplyFixRoundError> {
    let span = edit.span();
    if span.end() > text.len() {
        return Err(ApplyFixRoundError::OutOfBounds {
            rule,
            file: span.file().to_path_buf(),
            range: span.start()..span.end(),
            length: text.len(),
        });
    }

    let inside = [span.start(), span.end()]
        .into_iter()
        .find(|offset| !text.is_char_boundary(*offset));
    if let Some(offset) = inside {
        return Err(ApplyFixRoundError::SplitsCharacter {
            rule,
            file: span.file().to_path_buf(),
            offset,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use stiles_types::{Severity, Span};

    use super::*;

    fn edit(range: Range<usize>, replacement: &str) -> Edit {
        let Range { start, end } = range;
        Edit::builder()
            .span(
                Span::builder()
                    .file(PathBuf::from("test.rs"))
                    .start(start)
                    .end(end)
                    .build(),
            )
            .replacement(replacement)
            .build()
    }

    fn fixed(rule: &'static str, edits: Vec<Edit>) -> Diagnostic {
        let fix = Fix::builder()
            .message("fix it")
            .edits(edits)
            .build()
            .expect("the fix should build");
        unfixed(rule).with_fix(fix).expect("the fix should attach")
    }

    fn round(text: &str, diagnostics: &[Diagnostic]) -> FixRound {
        FixRound::apply()
            .text(text)
            .diagnostics(diagnostics)
            .call()
            .expect("the round should apply")
    }

    fn unfixed(rule: &'static str) -> Diagnostic {
        Diagnostic::builder()
            .rule_id(RuleId::new(rule))
            .severity(Severity::Error)
            .message("a finding")
            .span(
                Span::builder()
                    .file(PathBuf::from("test.rs"))
                    .start(0)
                    .end(1)
                    .build(),
            )
            .build()
    }

    #[test]
    fn apply_defers_a_fix_that_overlaps_an_earlier_one() {
        let diagnostics = [
            fixed("lint.later", vec![edit(2..4, "B")]),
            fixed("lint.earlier", vec![edit(0..3, "A")]),
        ];

        let round = round("abcdef", &diagnostics);

        assert_eq!(round.deferred(), [RuleId::new("lint.later")]);
    }

    #[test]
    fn apply_ignores_a_diagnostic_without_a_fix() {
        let diagnostics = [unfixed("lint.a")];

        let round = round("abcdef", &diagnostics);

        assert_eq!(round.text(), "abcdef");
    }

    #[test]
    fn apply_inserts_before_a_replacement_that_starts_at_the_same_position() {
        let diagnostics = [
            fixed("lint.replace", vec![edit(2..4, "XY")]),
            fixed("lint.insert", vec![edit(2..2, "+")]),
        ];

        let round = round("abcdef", &diagnostics);

        assert_eq!(round.text(), "ab+XYef");
    }

    #[test]
    fn apply_keeps_the_earlier_of_two_overlapping_fixes() {
        let diagnostics = [
            fixed("lint.later", vec![edit(2..4, "B")]),
            fixed("lint.earlier", vec![edit(0..3, "A")]),
        ];

        let round = round("abcdef", &diagnostics);

        assert_eq!(round.text(), "Adef");
    }

    #[test]
    fn apply_lists_the_applied_rules_in_source_order() {
        let diagnostics = [
            fixed("lint.second", vec![edit(4..5, "")]),
            fixed("lint.first", vec![edit(0..1, "")]),
        ];

        let round = round("abcdef", &diagnostics);

        assert_eq!(
            round.applied(),
            [RuleId::new("lint.first"), RuleId::new("lint.second")]
        );
    }

    #[test]
    fn apply_with_a_fix_of_several_edits_applies_every_edit() {
        let diagnostics = [fixed("lint.move", vec![edit(4..6, ""), edit(0..0, "ef")])];

        let round = round("abcdef", &diagnostics);

        assert_eq!(round.text(), "efabcd");
    }

    #[test]
    fn apply_with_a_multibyte_text_keeps_the_other_characters() {
        let diagnostics = [fixed("lint.a", vec![edit(2..3, "!")])];

        let round = round("é.ü", &diagnostics);

        assert_eq!(round.text(), "é!ü");
    }

    #[test]
    fn apply_with_a_waiting_fix_out_of_bounds_returns_out_of_bounds() {
        let diagnostics = [
            fixed("lint.first", vec![edit(0..3, "")]),
            fixed("lint.waiting", vec![edit(1..9, "")]),
        ];

        let error = FixRound::apply()
            .text("abc")
            .diagnostics(&diagnostics)
            .call()
            .expect_err("the waiting edit reaches past the end");

        assert_eq!(
            error,
            ApplyFixRoundError::OutOfBounds {
                rule: RuleId::new("lint.waiting"),
                file: PathBuf::from("test.rs"),
                range: 1..9,
                length: 3,
            }
        );
    }

    #[test]
    fn apply_with_adjacent_fixes_applies_both() {
        let diagnostics = [
            fixed("lint.a", vec![edit(0..2, "X")]),
            fixed("lint.b", vec![edit(2..4, "Y")]),
        ];

        let round = round("abcdef", &diagnostics);

        assert_eq!(round.text(), "XYef");
    }

    #[test]
    fn apply_with_an_edit_inside_a_character_returns_splits_character() {
        let diagnostics = [fixed("lint.a", vec![edit(1..2, "")])];

        let error = FixRound::apply()
            .text("é")
            .diagnostics(&diagnostics)
            .call()
            .expect_err("the edit splits a character");

        assert_eq!(
            error,
            ApplyFixRoundError::SplitsCharacter {
                rule: RuleId::new("lint.a"),
                file: PathBuf::from("test.rs"),
                offset: 1,
            }
        );
    }

    #[test]
    fn apply_with_an_edit_past_the_end_returns_out_of_bounds() {
        let diagnostics = [fixed("lint.a", vec![edit(2..9, "")])];

        let error = FixRound::apply()
            .text("abc")
            .diagnostics(&diagnostics)
            .call()
            .expect_err("the edit reaches past the end");

        assert_eq!(
            error,
            ApplyFixRoundError::OutOfBounds {
                rule: RuleId::new("lint.a"),
                file: PathBuf::from("test.rs"),
                range: 2..9,
                length: 3,
            }
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FixRound>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FixRound>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FixRound>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn apply_without_overlaps_matches_applying_one_edit_at_a_time_from_the_end(
                cuts in proptest::collection::btree_set(0..30usize, 0..8),
                text in "[a-z]{30}",
            ) {
                let cuts: Vec<usize> = cuts.into_iter().collect();
                let edits: Vec<(usize, usize)> = cuts
                    .chunks(2)
                    .filter_map(|pair| match pair {
                        [start, end] => Some((*start, *end)),
                        [_] | [] | [_, _, _, ..] => None,
                    })
                    .collect();
                let diagnostics: Vec<Diagnostic> = edits
                    .iter()
                    .map(|&(start, end)| fixed("lint.prop", vec![edit(start..end, "_")]))
                    .collect();
                let mut expected = text.clone();
                for &(start, end) in edits.iter().rev() {
                    expected.replace_range(start..end, "_");
                }

                let round = round(&text, &diagnostics);

                prop_assert_eq!(round.text(), expected);
            }
        }
    }
}
