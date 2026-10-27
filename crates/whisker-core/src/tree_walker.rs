use whisker_types::{
    Checked, Configured, DecoratedNode, DecoratedTree, Diagnostic, Language, LintPass, RuleOptions,
};

use crate::Lint;

/// Walks a decorated syntax tree and collects diagnostics from lint passes
///
/// This is [`walk_lints`] for passes that no plugin declared, such as a
/// rule's own tests run. It accepts any rule a pass reports.
///
/// # Errors
///
/// Returns a [`PassPanic`] when a pass panics, naming the node it was
/// checking. The walk stops there, for the reason that type gives.
///
/// [`PassPanic`]: crate::PassPanic
pub fn walk(
    tree: &DecoratedTree,
    passes: &mut [Box<dyn LintPass>],
) -> anyhow::Result<Vec<Diagnostic>> {
    let mut lints: Vec<Lint<'_>> = passes
        .iter_mut()
        .map(|pass| Lint::unchecked(Box::new(BorrowedPass(pass))))
        .collect();

    walk_lints(tree, &mut lints)
}

/// Walks a decorated syntax tree and collects diagnostics from lints
///
/// Performs a depth-first traversal of all named nodes, calling each lint
/// pass for every node visited. Only the passes whose language is the
/// tree's take part; the others never see a node of it.
///
/// # Errors
///
/// Returns a [`PassPanic`] when a pass panics, naming the node it was
/// checking, and an [`UndeclaredRule`] when a pass reports a rule its
/// plugin did not declare for it. The walk stops at either one, for the
/// reasons those types give.
///
/// [`PassPanic`]: crate::PassPanic
/// [`UndeclaredRule`]: crate::UndeclaredRule
pub fn walk_lints(tree: &DecoratedTree, lints: &mut [Lint<'_>]) -> anyhow::Result<Vec<Diagnostic>> {
    let mut lints: Vec<&mut Lint<'_>> = lints
        .iter_mut()
        .filter(|lint| lint.language() == tree.language())
        .collect();
    let mut diagnostics = Vec::new();
    visit_node(&tree.root_node(), &mut lints, &mut diagnostics)?;
    Ok(diagnostics)
}

/// Lends a caller's pass to [`walk`]
///
/// A [`Lint`] owns a box, and the caller of `walk` keeps its passes, so
/// this forwards each call to the borrowed one.
struct BorrowedPass<'pass>(&'pass mut Box<dyn LintPass>);

impl LintPass for BorrowedPass<'_> {
    extern "C" fn language(&self) -> Language {
        self.0.language()
    }

    extern "C" fn configure(&mut self, options: &RuleOptions) -> Configured {
        self.0.configure(options)
    }

    extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
        self.0.check_node(node)
    }
}

fn visit_node(
    node: &DecoratedNode<'_>,
    lints: &mut [&mut Lint<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) -> anyhow::Result<()> {
    if node.is_named() {
        for lint in lints.iter_mut() {
            diagnostics.extend(lint.check_node(node)?);
        }
    }

    for child in node.named_children() {
        visit_node(&child, lints, diagnostics)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use stabby::vec;
    use whisker_types::{Panic, RuleId};

    use super::*;
    use crate::{PassPanic, UndeclaredRule};

    /// Pairs `pass` with the rules it may report
    fn lint(pass: impl LintPass + 'static, rules: &[&'static str]) -> Lint<'static> {
        Lint::new(
            Box::new(pass),
            rules.iter().map(|rule| RuleId::new(rule)).collect(),
        )
    }

    fn parse_rust(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(
            tree,
            Language::Rust,
            source.to_string(),
            PathBuf::from("test.rs"),
        )
    }

    fn parse_typescript(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(
            tree,
            Language::TypeScript,
            source.to_string(),
            PathBuf::from("test.ts"),
        )
    }

    /// Counts the nodes a pass of one language sees
    struct NodeCounter {
        language: Language,
        visited: Arc<AtomicUsize>,
    }

    impl NodeCounter {
        fn boxed(language: Language) -> (Lint<'static>, Arc<AtomicUsize>) {
            let visited = Arc::new(AtomicUsize::new(0));
            let pass = Self {
                language,
                visited: Arc::clone(&visited),
            };
            (lint(pass, &[]), visited)
        }
    }

    impl LintPass for NodeCounter {
        extern "C" fn language(&self) -> Language {
            self.language
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
            self.visited.fetch_add(1, Ordering::Relaxed);
            Checked::Ok(vec::Vec::new())
        }
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<BorrowedPass<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<BorrowedPass<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<BorrowedPass<'_>>();
    }

    #[test]
    fn walk_with_empty_passes_returns_empty() {
        let tree = parse_rust("fn main() {}");
        let diagnostics = walk(&tree, &mut Vec::new()).expect("should walk");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn walk_visits_all_named_nodes() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Counter;
        impl LintPass for Counter {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                COUNT.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn main() {}");
        let mut passes = vec![lint(Counter, &[])];
        walk_lints(&tree, &mut passes).expect("should walk");

        assert!(COUNT.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn walk_on_empty_source_returns_empty() {
        let tree = parse_rust("");
        let mut passes = vec![lint(CounterPass(0), &[])];
        let diagnostics = walk_lints(&tree, &mut passes).expect("should walk");
        assert!(diagnostics.is_empty());
    }

    struct CounterPass(usize);
    impl LintPass for CounterPass {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
            self.0 += 1;
            Checked::Ok(vec::Vec::new())
        }
    }

    #[test]
    fn walk_visits_nested_nodes() {
        static NESTED_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct KindCounter;
        impl LintPass for KindCounter {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                NESTED_COUNT.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        NESTED_COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn main() { let x = 1; }");
        let mut passes = vec![lint(KindCounter, &[])];
        walk_lints(&tree, &mut passes).expect("should walk");

        let count = NESTED_COUNT.load(Ordering::Relaxed);
        assert!(count > 1, "should visit multiple nested nodes, got {count}");
    }

    #[test]
    fn walk_visits_deeply_nested_code() {
        static DEEP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct DepthCounter;
        impl LintPass for DepthCounter {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                DEEP_COUNT.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        DEEP_COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn f() { if true { if true { if true { let x = 1; } } } }");
        let mut passes = vec![lint(DepthCounter, &[])];
        walk_lints(&tree, &mut passes).expect("should walk");

        let count = DEEP_COUNT.load(Ordering::Relaxed);
        assert!(
            count >= 4,
            "deeply nested source should visit at least 4 nodes, got {count}"
        );
    }

    #[test]
    fn walk_multiple_passes_each_see_same_nodes() {
        static PASS_A: AtomicUsize = AtomicUsize::new(0);
        static PASS_B: AtomicUsize = AtomicUsize::new(0);

        struct CounterA;
        impl LintPass for CounterA {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                PASS_A.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        struct CounterB;
        impl LintPass for CounterB {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                PASS_B.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        PASS_A.store(0, Ordering::Relaxed);
        PASS_B.store(0, Ordering::Relaxed);

        let tree = parse_rust("fn a() {} fn b() {}");
        let mut passes = vec![lint(CounterA, &[]), lint(CounterB, &[])];
        walk_lints(&tree, &mut passes).expect("should walk");

        let a = PASS_A.load(Ordering::Relaxed);
        let b = PASS_B.load(Ordering::Relaxed);
        assert!(a > 0, "pass A should have been called");
        assert_eq!(a, b, "both passes should see the same number of nodes");
    }

    #[test]
    fn walk_on_a_rust_tree_skips_the_typescript_passes() {
        let (rust, rust_visited) = NodeCounter::boxed(Language::Rust);
        let (typescript, typescript_visited) = NodeCounter::boxed(Language::TypeScript);
        let tree = parse_rust("fn a() {}");
        let mut passes = vec![rust, typescript];

        walk_lints(&tree, &mut passes).expect("should walk");

        assert!(rust_visited.load(Ordering::Relaxed) > 0);
        assert_eq!(typescript_visited.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn walk_on_a_typescript_tree_skips_the_rust_passes() {
        let (rust, rust_visited) = NodeCounter::boxed(Language::Rust);
        let (typescript, typescript_visited) = NodeCounter::boxed(Language::TypeScript);
        let tree = parse_typescript("function a(): void {}");
        let mut passes = vec![rust, typescript];

        walk_lints(&tree, &mut passes).expect("should walk");

        assert_eq!(rust_visited.load(Ordering::Relaxed), 0);
        assert!(typescript_visited.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn walk_collects_diagnostics_from_multiple_passes() {
        use whisker_types::{RuleId, Severity};

        struct WarnOnFn(&'static str);
        impl LintPass for WarnOnFn {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                let found = match node.kind() == "function_item" {
                    true => vec![Diagnostic::new(
                        RuleId::new(self.0),
                        Severity::Warn,
                        format!("{} found fn", self.0),
                        node.span(),
                    )],
                    false => Vec::new(),
                };
                Checked::Ok(found.into_iter().collect())
            }
        }

        let tree = parse_rust("fn main() {}");
        let mut passes = vec![
            lint(WarnOnFn("pass.a"), &["pass.a"]),
            lint(WarnOnFn("pass.b"), &["pass.b"]),
        ];
        let diagnostics = walk_lints(&tree, &mut passes).expect("should walk");

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].rule_id(), RuleId::new("pass.a"));
        assert_eq!(diagnostics[1].rule_id(), RuleId::new("pass.b"));
    }

    #[test]
    fn walk_diagnostic_spans_are_valid() {
        use whisker_types::{RuleId, Severity};

        struct SpanChecker;
        impl LintPass for SpanChecker {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                Checked::Ok(
                    [Diagnostic::new(
                        RuleId::new("test"),
                        Severity::Info,
                        "span check".into(),
                        node.span(),
                    )]
                    .into_iter()
                    .collect(),
                )
            }
        }

        let source = "fn main() { let x = 42; }";
        let tree = parse_rust(source);
        let mut passes = vec![lint(SpanChecker, &["test"])];
        let diagnostics = walk_lints(&tree, &mut passes).expect("should walk");

        for diag in &diagnostics {
            assert!(diag.span().start() <= diag.span().end());
            assert!(diag.span().end() <= source.len());
        }
    }

    /// A panic stops the walk where it happened and says where that was
    ///
    /// Nodes after the panic go unchecked on purpose: the pass is in a
    /// state its author never meant.
    #[test]
    fn walk_with_a_panicking_pass_stops_and_names_the_node() {
        static AFTER_PANIC: AtomicUsize = AtomicUsize::new(0);

        struct PanicOnFn;
        impl LintPass for PanicOnFn {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                let kind = node.kind();
                Panic::catch(move || -> Vec<Diagnostic> {
                    match kind == "function_item" {
                        true => panic!("no functions"),
                        false => Vec::new(),
                    }
                })
                .map(|found| found.into_iter().collect())
                .into()
            }
        }

        struct CountAfter;
        impl LintPass for CountAfter {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                AFTER_PANIC.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        AFTER_PANIC.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn main() { let x = 1; }");
        let mut passes = vec![lint(PanicOnFn, &[]), lint(CountAfter, &[])];

        let error = walk_lints(&tree, &mut passes).expect_err("the panic should end the walk");

        let error = error
            .downcast_ref::<PassPanic>()
            .expect("should be a PassPanic");
        assert_eq!(error.kind(), "function_item");
        assert_eq!(error.span().start(), 0);
        assert_eq!(error.panic().message(), "no functions");
        assert_eq!(AFTER_PANIC.load(Ordering::Relaxed), 1);
    }

    /// A rule outside the pass's declaration stops the walk and names itself
    ///
    /// The diagnostic would otherwise slip past every switch and option a
    /// project sets by rule id.
    #[test]
    fn walk_with_an_undeclared_rule_stops_and_names_it() {
        use whisker_types::Severity;

        struct Mislabeled;
        impl LintPass for Mislabeled {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                let found = match node.kind() == "function_item" {
                    true => vec![Diagnostic::new(
                        RuleId::new("pass.copied"),
                        Severity::Warn,
                        "found fn".into(),
                        node.span(),
                    )],
                    false => Vec::new(),
                };
                Checked::Ok(found.into_iter().collect())
            }
        }

        let tree = parse_rust("fn main() {}");
        let mut passes = vec![lint(Mislabeled, &["pass.declared"])];

        let error = walk_lints(&tree, &mut passes).expect_err("the stray rule should end the walk");

        let error = error
            .downcast_ref::<UndeclaredRule>()
            .expect("should be an UndeclaredRule");
        assert_eq!(error.rule(), RuleId::new("pass.copied"));
        assert_eq!(error.span().start(), 0);
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn walk_with_no_passes_always_empty(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let tree = parse_rust(&source);
                let diagnostics = walk(&tree, &mut Vec::new()).expect("should walk");
                prop_assert!(diagnostics.is_empty());
            }

            #[test]
            fn walk_diagnostic_count_equals_pass_count_times_nodes(
                source in "fn [a-z]+\\(\\) \\{\\}",
                num_passes in 1..=5usize,
            ) {
                struct CountAll;
                impl LintPass for CountAll {
                    extern "C" fn language(&self) -> Language {
                        Language::Rust
                    }

                    extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                        Configured::Ok(())
                    }

                    extern "C" fn check_node(
                        &mut self,
                        node: &DecoratedNode<'_>,
                    ) -> Checked {
                        Checked::Ok(
                            [Diagnostic::new(
                                whisker_types::RuleId::new("test"),
                                whisker_types::Severity::Warn,
                                "hit".into(),
                                node.span(),
                            )]
                            .into_iter()
                            .collect(),
                        )
                    }
                }

                let tree = parse_rust(&source);

                let mut single_pass = vec![lint(CountAll, &["test"])];
                let single_count = walk_lints(&tree, &mut single_pass).expect("should walk").len();

                let mut multi_passes: Vec<Lint<'static>> = (0..num_passes)
                    .map(|_| lint(CountAll, &["test"]))
                    .collect();
                let multi_count = walk_lints(&tree, &mut multi_passes).expect("should walk").len();

                prop_assert_eq!(multi_count, single_count * num_passes);
            }
        }
    }
}
