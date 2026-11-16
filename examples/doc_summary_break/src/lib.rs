use stiles_rust::RustLintPass;
use stiles_types::{DecoratedNode, Diagnostic, Edit, Fix, RuleId, Severity, Span};

// 1: Rule struct and documentation
/// Flags a summary line that runs straight into the rest of the comment
pub struct DocSummaryBreak;

// 2: LintPass implementation
// region: check
impl RustLintPass for DocSummaryBreak {
    fn check_line_comment(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        if !is_doc_line(node) || is_blank(node) {
            return Vec::new();
        }

        let Some(next_doc_line) = get_next_doc_line(node) else {
            return Vec::new();
        };

        if !is_doc_line(&next_doc_line) || is_blank(&next_doc_line) {
            return Vec::new();
        }

        let diagnostic = Diagnostic::builder()
            .rule_id(RULE_ID)
            .severity(Severity::Warn)
            .message("the summary line needs a blank `///` line after it")
            .span(node.span())
            .build();

        let Some(fix) = add_blank_line(node, &next_doc_line) else {
            return vec![diagnostic];
        };

        vec![
            diagnostic
                .with_fix(fix)
                .expect("the fix lies in the file of the diagnostic"),
        ]
    }
}
// endregion

// 3: Rule ID
// region: declare
const RULE_ID: RuleId = RuleId::new("lint.doc-summary-break");

impl stiles_plugin::DeclaresRules for DocSummaryBreak {
    fn rules(&self) -> Vec<RuleId> {
        vec![RULE_ID]
    }

    fn reads(&self) -> stiles_plugin::Reads {
        stiles_plugin::Reads::none()
    }
}
// endregion

// 4: Lint registration
// region: declare
stiles_plugin::export_lints![rust: [DocSummaryBreak]];
// endregion

// ... and some helper functions for the lint

// region: fix
/// Returns a fix that puts a blank `///` line after the summary
///
/// The new line takes the indentation of the next line and the line ending
/// of the summary, so it matches the lines around it. Answers [`None`] when
/// code stands before the next line, because no indentation fits there.
fn add_blank_line(summary: &DecoratedNode<'_>, next: &DecoratedNode<'_>) -> Option<Fix> {
    let parent = summary.parent()?;
    let offset = parent.span().start();
    let end = summary.span().end();
    let between = parent
        .text()
        .get(end - offset..next.span().start() - offset)?;
    let indent = between.rsplit('\n').next()?;

    if !indent.trim().is_empty() {
        return None;
    }

    let line_ending = if summary.text().ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let after_summary = Span::builder()
        .file(summary.span().file())
        .start(end)
        .end(end)
        .build();
    let edit = Edit::builder()
        .span(after_summary)
        .replacement(format!("{indent}///{line_ending}"))
        .build();

    let fix = Fix::builder()
        .message("add a blank `///` line")
        .edits(vec![edit])
        .build()
        .expect("one edit never overlaps itself");

    Some(fix)
}
// endregion

/// Returns the line after this one, when this one opens a doc comment
///
/// Answers [`None`] for a line that follows another doc line, because only
/// the first line of a block is a summary. The caller decides whether what
/// comes back is a doc line at all.
fn get_next_doc_line<'a>(node: &DecoratedNode<'a>) -> Option<DecoratedNode<'a>> {
    let parent = node.parent()?;
    let lines = parent.named_children();
    let at = lines.iter().position(|line| line.id() == node.id())?;

    if at > 0 && is_doc_line(&lines[at - 1]) {
        return None;
    }

    lines.into_iter().nth(at + 1)
}

/// Returns whether the node is a `///` line
///
/// Asks the grammar for the marker of an outer doc comment instead of
/// reading the text, because `////` starts with `///` and is a plain comment.
fn is_doc_line(node: &DecoratedNode<'_>) -> bool {
    node.kind() == "line_comment" && node.child_by_field_name("outer").is_some()
}

/// Returns whether the line carries nothing but its marker
fn is_blank(node: &DecoratedNode<'_>) -> bool {
    node.text().trim_start_matches('/').trim().is_empty()
}

// <
#[cfg(test)]
mod tests {
    // region: test
    use stiles_rust::RustLintPassAdapter;
    use stiles_testing::{assert_diagnostic, assert_fix, assert_no_diagnostics, execute, parse};
    use stiles_types::{Language, LintPass};

    use super::*;

    fn passes() -> Vec<Box<dyn LintPass>> {
        vec![Box::new(RustLintPassAdapter::new(DocSummaryBreak))]
    }
    // endregion

    #[test]
    fn a_blank_first_line_is_not_flagged() {
        let tree = parse("///\n/// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn a_blank_line_after_the_summary_is_not_flagged() {
        let tree = parse("/// Adds\n///\n/// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn a_lone_summary_is_not_flagged() {
        let tree = parse("/// Adds\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn a_plain_comment_is_not_flagged() {
        let tree = parse("// Adds\n// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    // region: test
    #[test]
    fn a_second_line_against_the_summary_is_flagged() {
        let tree = parse("/// Adds\n/// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0]).has_rule_id("lint.doc-summary-break");
    }
    // endregion

    /// `////` is a comment and not a doc comment, so it ends the block rather
    /// than continuing it, and the summary above it stands alone
    #[test]
    fn a_summary_above_a_slash_rule_is_not_flagged() {
        let tree = parse("/// Adds\n////\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    /// `////` is a plain comment, not a doc comment, so the summary below it
    /// starts a block and wants a blank line after it
    #[test]
    fn a_summary_under_a_slash_rule_is_flagged() {
        let tree = parse("////\n/// Adds\n/// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics[0].span().start(), 5);
    }

    #[test]
    fn only_the_first_line_of_a_block_is_flagged() {
        let tree = parse(
            "/// Adds\n/// The rest.\n/// And more.\nfn f() {}",
            Language::Rust,
        );

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
    }

    // region: fix-test
    #[test]
    fn the_fix_adds_a_blank_line_after_the_summary() {
        assert_fix()
            .tree(parse(
                "/// Adds\n/// The rest.\nfn f() {}\n",
                Language::Rust,
            ))
            .passes(passes)
            .produces("/// Adds\n///\n/// The rest.\nfn f() {}\n");
    }
    // endregion

    #[test]
    fn the_fix_after_a_slash_rule_adds_a_blank_line() {
        assert_fix()
            .tree(parse(
                "////\n/// Adds\n/// The rest.\nfn f() {}\n",
                Language::Rust,
            ))
            .passes(passes)
            .produces("////\n/// Adds\n///\n/// The rest.\nfn f() {}\n");
    }

    #[test]
    fn the_fix_at_the_end_of_the_file_adds_a_blank_line() {
        assert_fix()
            .tree(parse("/// Adds\n/// The rest.", Language::Rust))
            .passes(passes)
            .produces("/// Adds\n///\n/// The rest.");
    }

    #[test]
    fn the_fix_in_a_crlf_file_ends_the_line_with_crlf() {
        assert_fix()
            .tree(parse(
                "/// Adds\r\n/// The rest.\r\nfn f() {}\r\n",
                Language::Rust,
            ))
            .passes(passes)
            .produces("/// Adds\r\n///\r\n/// The rest.\r\nfn f() {}\r\n");
    }

    #[test]
    fn the_fix_in_a_long_block_adds_one_blank_line() {
        assert_fix()
            .tree(parse(
                "/// Adds\n/// The rest.\n/// And more.\nfn f() {}\n",
                Language::Rust,
            ))
            .passes(passes)
            .produces("/// Adds\n///\n/// The rest.\n/// And more.\nfn f() {}\n");
    }

    #[test]
    fn the_fix_keeps_the_indentation_of_the_block() {
        assert_fix()
            .tree(parse(
                "mod m {\n\t/// Adds\n\t/// The rest.\n\tfn f() {}\n}\n",
                Language::Rust,
            ))
            .passes(passes)
            .produces("mod m {\n\t/// Adds\n\t///\n\t/// The rest.\n\tfn f() {}\n}\n");
    }

    #[test]
    fn the_fix_over_an_empty_line_adds_a_blank_line_after_the_summary() {
        assert_fix()
            .tree(parse(
                "/// Adds\n\n/// The rest.\nfn f() {}\n",
                Language::Rust,
            ))
            .passes(passes)
            .produces("/// Adds\n///\n\n/// The rest.\nfn f() {}\n");
    }

    #[test]
    fn the_fix_with_a_blank_line_after_the_summary_changes_nothing() {
        assert_fix()
            .tree(parse(
                "/// Adds\n///\n/// The rest.\nfn f() {}\n",
                Language::Rust,
            ))
            .passes(passes)
            .produces("/// Adds\n///\n/// The rest.\nfn f() {}\n");
    }

    #[test]
    fn the_fix_with_code_before_the_next_line_is_not_offered() {
        let tree = parse(
            "fn f() {\n    g(1 /// Adds\n, /// The rest.\n2);\n}\n",
            Language::Rust,
        );

        let diagnostics = execute(&tree, &mut passes());

        assert!(diagnostics[0].fix().is_none());
    }

    #[test]
    fn the_fix_with_inner_docs_changes_nothing() {
        assert_fix()
            .tree(parse("//! Adds\n//! The rest.\n", Language::Rust))
            .passes(passes)
            .produces("//! Adds\n//! The rest.\n");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DocSummaryBreak>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DocSummaryBreak>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DocSummaryBreak>();
    }
}
// >
