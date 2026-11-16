//! Applies the fixes of lint passes to a source, the way `--fix` does

use stiles_core::FixRound;
use stiles_types::{DecoratedTree, Language, LintPass};

use self::assert_fix_builder::{IsUnset, SetDecorate, SetTree, State};
use crate::{execute, parse, parse_at};

/// The most rounds of fixes that one source may take
///
/// `stiles check --fix` stops at the same number. A test that needs more
/// rounds shows the bug a project would hit.
const MAX_ROUNDS: usize = 10;

/// What attaches the decorations to the tree of each round
type Decorate<'d> = Box<dyn FnMut(&mut DecoratedTree) + 'd>;

/// Asserts that the fixes of the passes turn `tree` into `expected`
///
/// The assertion runs rounds of fixes, as `stiles check --fix` does. Each
/// round runs a fresh set of passes from `passes`, because a pass holds
/// state. It applies every fix that does not overlap an earlier one, and
/// parses the result again. The rounds stop when a round applies nothing.
/// The text must then equal `expected` byte for byte. The passes must then
/// report `remaining` findings on it, which is none unless the test says.
///
/// `.source(text, language)` parses the text for the first round, and
/// `.tree(tree)` takes a tree the test built, such as one from
/// [`parse_at`]. Each later round parses its text with the language and the
/// file of the first tree.
///
/// A later round parses the fixed text afresh, so it sees no decorations.
/// `decorate` attaches them again: it runs on the tree of every round, the
/// first one included. Without it, a rule that reports only with a
/// decoration reports nothing after round one, whatever its fix did.
///
/// # Panics
///
/// Panics if a fix lies outside the text or splits a character. Panics if
/// the fixes do not settle within the rounds that `--fix` allows. Panics if
/// the result differs from `expected`, or if the passes report another
/// number of findings on it than `remaining`.
///
/// # Examples
///
/// ```
/// use stiles_testing::assert_fix;
/// use stiles_types::{
///     AttachFixError, Checked, Configured, DecoratedNode, Diagnostic, Edit, Fix, Language,
///     LintPass, RuleId, RuleOptions, Severity, Sink, Span,
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
///     extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>, sink: &Sink<'_>) -> Checked {
///         let span = node.span();
///         if node.kind() != "line_comment" || !node.text().trim_end().ends_with('.') {
///             return Checked::Ok(());
///         }
///         let end = span.start() + node.text().trim_end().len();
///         let period = Span::builder().file(span.file()).start(end - 1).end(end).build();
///         let fix = Fix::builder()
///             .message("remove the period")
///             .edits(std::vec![Edit::delete(period)])
///             .build()
///             .expect("the fix should build");
///         let diagnostic = Diagnostic::builder()
///             .rule_id(RuleId::new("lint.no-period"))
///             .severity(Severity::Error)
///             .message("the comment ends with a period")
///             .span(span)
///             .build()
///             .with_fix(fix)
///             .unwrap_or_else(AttachFixError::into_diagnostic);
///         sink.report(&diagnostic);
///         Checked::Ok(())
///     }
/// }
///
/// assert_fix()
///     .source("// Returns one.\nfn one() {}\n", Language::Rust)
///     .passes(|| std::vec![Box::new(NoPeriod) as Box<dyn LintPass>])
///     .produces("// Returns one\nfn one() {}\n");
/// ```
#[bon::builder(finish_fn = produces)]
pub fn assert_fix<'e, 'd, F>(
    #[builder(finish_fn)] expected: &'e str,
    tree: DecoratedTree,
    passes: F,
    #[builder(setters(vis = "", name = decorate_internal))] decorate: Option<Decorate<'d>>,
    /// The number of findings that the passes report on the result
    #[builder(default)]
    remaining: usize,
) where
    F: FnMut() -> Vec<Box<dyn LintPass>>,
{
    let mut passes = passes;
    let mut decorate = decorate;
    let language = tree.language();
    let file = tree.file().to_path_buf();
    let mut tree = tree;

    for _round in 0..MAX_ROUNDS {
        if let Some(decorate) = decorate.as_mut() {
            decorate(&mut tree);
        }
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
                diagnostics.len() == remaining,
                "expected {} on the fixed text, got {}: {diagnostics:?}",
                findings(remaining),
                diagnostics.len()
            );
            return;
        }

        tree = parse_at(&file, &round.into_text(), language);
    }

    panic!(
        "the fixes did not settle within {MAX_ROUNDS} rounds; the last text was:\n{}",
        tree.source()
    );
}

/// Returns how a panic message names `count` findings
fn findings(count: usize) -> String {
    match count {
        0 => "no diagnostics".to_owned(),
        1 => "1 diagnostic".to_owned(),
        count => format!("{count} diagnostics"),
    }
}

impl<'e, 'd, F, S> AssertFixBuilder<'e, 'd, F, S>
where
    F: FnMut() -> Vec<Box<dyn LintPass>>,
    S: State,
{
    /// Parses `text` as `language` for the first round
    ///
    /// It is short for `.tree(parse(text, language))`.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_testing::assert_fix;
    /// use stiles_types::{Language, LintPass};
    ///
    /// assert_fix()
    ///     .source("fn one() {}\n", Language::Rust)
    ///     .passes(Vec::<Box<dyn LintPass>>::new)
    ///     .produces("fn one() {}\n");
    /// ```
    pub fn source(self, text: &str, language: Language) -> AssertFixBuilder<'e, 'd, F, SetTree<S>>
    where
        S::Tree: IsUnset,
    {
        self.tree(parse(text, language))
    }

    /// Attaches decorations to the tree of every round
    ///
    /// `decorate` receives each tree before the passes run on it, the tree
    /// of the first round included. It attaches what a decorated rule
    /// reads, usually through [`decorate`].
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_testing::{assert_fix, decorate};
    /// use stiles_types::{DecorationMap, Language, LintPass};
    ///
    /// assert_fix()
    ///     .source("fn one() {}\n", Language::Rust)
    ///     .passes(Vec::<Box<dyn LintPass>>::new)
    ///     .decorate(|tree| decorate(tree, DecorationMap::new()))
    ///     .produces("fn one() {}\n");
    /// ```
    ///
    /// [`decorate`]: crate::decorate
    pub fn decorate(
        self,
        decorate: impl FnMut(&mut DecoratedTree) + 'd,
    ) -> AssertFixBuilder<'e, 'd, F, SetDecorate<S>>
    where
        S::Decorate: IsUnset,
    {
        self.decorate_internal(Box::new(decorate))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use stiles_types::{
        Checked, Configured, DecoratedNode, Decoration, DecorationKey, DecorationMap, Diagnostic,
        Edit, Fix, Language, RuleId, RuleOptions, Severity, Sink, Span,
    };

    use super::*;
    use crate::decorate;

    /// Marks a file in which the gated test pass reports
    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Mark;

    impl Decoration for Mark {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Mark"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    /// Where a test pass reports
    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    enum Gate {
        /// In every file
        Always,
        /// Only in a file whose root carries [`Mark`]
        Marked,
        /// Only in `src/lib.rs`
        Library,
    }

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

    /// Answers a line comment that ends with a period as its [`Answer`]
    /// says, where its [`Gate`] lets it
    struct Period(Answer, Gate);

    impl LintPass for Period {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>, sink: &Sink<'_>) -> Checked {
            let span = node.span();
            let text = node.text().trim_end();
            if node.kind() != "line_comment" || !text.ends_with('.') {
                return Checked::Ok(());
            }
            let open = match self.1 {
                Gate::Always => true,
                Gate::Marked => node.file_decoration::<Mark>().is_some(),
                Gate::Library => span.file() == Path::new("src/lib.rs"),
            };
            if !open {
                return Checked::Ok(());
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
                sink.report(&diagnostic);
                return Checked::Ok(());
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
            sink.report(&diagnostic);
            Checked::Ok(())
        }
    }

    fn passes(answer: Answer) -> impl FnMut() -> Vec<Box<dyn LintPass>> {
        gated(answer, Gate::Always)
    }

    fn gated(answer: Answer, gate: Gate) -> impl FnMut() -> Vec<Box<dyn LintPass>> {
        move || std::vec![Box::new(Period(answer, gate)) as Box<dyn LintPass>]
    }

    /// Attaches [`Mark`] to the root of `tree`
    fn mark(tree: &mut DecoratedTree) {
        let mut map = DecorationMap::new();
        map.insert(tree.root_node().id(), Mark);
        decorate(tree, map);
    }

    #[test]
    fn assert_fix_with_a_finding_left_and_remaining_passes() {
        assert_fix()
            .source("// One.\nfn f() {}\n", Language::Rust)
            .passes(passes(Answer::Report))
            .remaining(1)
            .produces("// One.\nfn f() {}\n");
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
    fn assert_fix_with_a_source_parses_it() {
        assert_fix()
            .source("// One.\nfn f() {}\n", Language::Rust)
            .passes(passes(Answer::Remove))
            .produces("// One\nfn f() {}\n");
    }

    #[test]
    fn assert_fix_with_a_tree_at_a_path_keeps_the_path_every_round() {
        let tree = parse_at("src/lib.rs", "// One..\nfn f() {}\n", Language::Rust);

        assert_fix()
            .tree(tree)
            .passes(gated(Answer::Remove, Gate::Library))
            .produces("// One\nfn f() {}\n");
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
    fn assert_fix_with_decorate_decorates_every_round() {
        assert_fix()
            .source("// One..\nfn f() {}\n", Language::Rust)
            .passes(gated(Answer::Remove, Gate::Marked))
            .decorate(mark)
            .produces("// One\nfn f() {}\n");
    }

    #[test]
    fn assert_fix_with_every_comment_fixed_passes() {
        let tree = parse("// One.\n// Two.\nfn f() {}\n", Language::Rust);

        assert_fix()
            .tree(tree)
            .passes(passes(Answer::Remove))
            .produces("// One\n// Two\nfn f() {}\n");
    }

    #[test]
    #[should_panic(expected = "expected 1 diagnostic on the fixed text, got 0")]
    fn assert_fix_with_fewer_findings_than_remaining_panics() {
        assert_fix()
            .source("// One.\nfn f() {}\n", Language::Rust)
            .passes(passes(Answer::Remove))
            .remaining(1)
            .produces("// One\nfn f() {}\n");
    }

    #[test]
    fn findings_of_one_names_one_diagnostic() {
        assert_eq!(findings(1), "1 diagnostic");
    }

    #[test]
    fn findings_of_two_names_two_diagnostics() {
        assert_eq!(findings(2), "2 diagnostics");
    }

    #[test]
    fn findings_of_zero_names_no_diagnostics() {
        assert_eq!(findings(0), "no diagnostics");
    }
}
