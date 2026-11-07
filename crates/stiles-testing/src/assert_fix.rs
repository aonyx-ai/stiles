//! Applies the fixes of lint passes to a source, the way `--fix` does

use stiles_core::FixRound;
use stiles_types::{DecoratedTree, LintPass};

use crate::{execute, parse};

/// The most rounds of fixes that one source may take
///
/// `stiles check --fix` stops at the same number. A test that needs more
/// rounds shows the bug a project would hit.
const MAX_ROUNDS: usize = 10;

/// Asserts that the fixes of the passes turn `tree` into `expected`
///
/// The assertion runs rounds of fixes, as `stiles check --fix` does. Each
/// round runs a fresh set of passes from `passes`, because a pass holds
/// state. It applies every fix that does not overlap an earlier one, and
/// parses the result again. The rounds stop when a round applies nothing.
/// The text must then equal `expected` byte for byte, and the passes must
/// report nothing on it.
///
/// The first round sees the decorations of `tree`. Later rounds parse the
/// fixed text afresh, so they see no decorations. A rule that reports only
/// with a decoration therefore reports nothing after its fix, whatever the
/// fix did.
///
/// # Panics
///
/// Panics if a fix lies outside the text or splits a character. Panics if
/// the fixes do not settle within the rounds that `--fix` allows. Panics if
/// the result differs from `expected`, or if the passes report on it.
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_testing::{assert_fix, parse};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Diagnostic, Edit, Fix, Language, LintPass,
///     RuleId, RuleOptions, Severity, Span,
/// };
///
/// /// Removes the period at the end of a line comment
/// struct NoPeriod;
///
/// impl LintPass for NoPeriod {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
///         let span = node.span();
///         if node.kind() != "line_comment" || !node.text().trim_end().ends_with('.') {
///             return Checked::Ok(vec::Vec::new());
///         }
///         let end = span.start() + node.text().trim_end().len();
///         let period = Span::builder().file(span.file()).start(end - 1).end(end).build();
///         let fix = Fix::builder()
///             .message("remove the period")
///             .edits(std::vec![Edit::builder().span(period).replacement("").build()])
///             .build()
///             .expect("the fix should build");
///         let diagnostic = Diagnostic::builder()
///             .rule_id(RuleId::new("lint.no-period"))
///             .severity(Severity::Error)
///             .message("the comment ends with a period")
///             .span(span)
///             .build()
///             .with_fix(fix)
///             .expect("the fix should attach");
///         Checked::Ok([diagnostic].into_iter().collect())
///     }
/// }
///
/// assert_fix()
///     .tree(parse("// Returns one.\nfn one() {}\n", Language::Rust))
///     .passes(|| std::vec![Box::new(NoPeriod) as Box<dyn LintPass>])
///     .produces("// Returns one\nfn one() {}\n");
/// ```
#[bon::builder(finish_fn = produces)]
pub fn assert_fix<F>(#[builder(finish_fn)] expected: &str, tree: DecoratedTree, passes: F)
where
    F: FnMut() -> Vec<Box<dyn LintPass>>,
{
    let mut passes = passes;
    let language = tree.language();
    let mut tree = tree;

    for _round in 0..MAX_ROUNDS {
        let diagnostics = execute(&tree, &mut passes());
        let round = FixRound::apply()
            .text(tree.source())
            .diagnostics(&diagnostics)
            .call()
            .unwrap_or_else(|error| panic!("{error}"));

        if round.applied().is_empty() {
            assert_eq!(
                tree.source(),
                expected,
                "the fixes should produce the expected text"
            );
            assert!(
                diagnostics.is_empty(),
                "expected no diagnostics on the fixed text, got {}: {diagnostics:?}",
                diagnostics.len()
            );
            return;
        }

        tree = parse(&round.into_text(), language);
    }

    panic!(
        "the fixes did not settle within {MAX_ROUNDS} rounds; the last text was:\n{}",
        tree.source()
    );
}

#[cfg(test)]
mod tests {
    use stabby::vec;
    use stiles_types::{
        Checked, Configured, DecoratedNode, Diagnostic, Edit, Fix, Language, RuleId, RuleOptions,
        Severity, Span,
    };

    use super::*;

    /// How a test pass answers a line comment that ends with a period
    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    enum Answer {
        /// Removes the period
        Remove,
        /// Adds another period, so the comment never settles
        Repeat,
        /// Reports the comment and attaches no fix
        Report,
    }

    /// Answers a line comment that ends with a period as its [`Answer`] says
    struct Period(Answer);

    impl LintPass for Period {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
            let span = node.span();
            let text = node.text().trim_end();
            if node.kind() != "line_comment" || !text.ends_with('.') {
                return Checked::Ok(vec::Vec::new());
            }
            let end = span.start() + text.len();
            let diagnostic = Diagnostic::builder()
                .rule_id(RuleId::new("lint.period"))
                .severity(Severity::Error)
                .message("the comment ends with a period")
                .span(span.clone())
                .build();
            let edit = match self.0 {
                Answer::Remove => Some((end - 1, "")),
                Answer::Repeat => Some((end, ".")),
                Answer::Report => None,
            };
            let Some((start, replacement)) = edit else {
                return Checked::Ok([diagnostic].into_iter().collect());
            };
            let edit = Edit::builder()
                .span(
                    Span::builder()
                        .file(span.file())
                        .start(start)
                        .end(end)
                        .build(),
                )
                .replacement(replacement)
                .build();
            let fix = Fix::builder()
                .message("change the period")
                .edits(std::vec![edit])
                .build()
                .expect("the fix should build");
            let diagnostic = diagnostic.with_fix(fix).expect("the fix should attach");
            Checked::Ok([diagnostic].into_iter().collect())
        }
    }

    fn passes(answer: Answer) -> impl FnMut() -> Vec<Box<dyn LintPass>> {
        move || std::vec![Box::new(Period(answer)) as Box<dyn LintPass>]
    }

    #[test]
    #[should_panic(expected = "expected no diagnostics on the fixed text")]
    fn assert_fix_with_a_finding_left_panics() {
        let tree = parse("// One.\nfn f() {}\n", Language::Rust);

        assert_fix()
            .tree(tree)
            .passes(passes(Answer::Report))
            .produces("// One.\nfn f() {}\n");
    }

    #[test]
    #[should_panic(expected = "the fixes did not settle within 10 rounds")]
    fn assert_fix_with_a_fix_that_never_settles_panics() {
        let tree = parse("// One.\nfn f() {}\n", Language::Rust);

        assert_fix()
            .tree(tree)
            .passes(passes(Answer::Repeat))
            .produces("// One.\nfn f() {}\n");
    }

    #[test]
    #[should_panic(expected = "the fixes should produce the expected text")]
    fn assert_fix_with_another_result_panics() {
        let tree = parse("// One.\nfn f() {}\n", Language::Rust);

        assert_fix()
            .tree(tree)
            .passes(passes(Answer::Remove))
            .produces("// One.\nfn f() {}\n");
    }

    #[test]
    fn assert_fix_with_every_comment_fixed_passes() {
        let tree = parse("// One.\n// Two.\nfn f() {}\n", Language::Rust);

        assert_fix()
            .tree(tree)
            .passes(passes(Answer::Remove))
            .produces("// One\n// Two\nfn f() {}\n");
    }
}
