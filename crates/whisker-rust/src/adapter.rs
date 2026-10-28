use crate::RustLintPass;

whisker_types::lint_pass_adapter!(RustLintPassAdapter, RustLintPass, crate::dispatch, Rust);

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{
        DecoratedNode, DecoratedTree, Diagnostic, Language, LintPass, Panic, RuleId, RuleOptions,
        Severity,
    };

    use super::*;

    fn parse_rust(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&crate::language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(
            tree,
            Language::Rust,
            source.to_string(),
            PathBuf::from("test.rs"),
        )
    }

    #[test]
    fn trait_send() {
        struct Dummy;
        impl RustLintPass for Dummy {}

        fn assert_send<T: Send>() {}
        assert_send::<RustLintPassAdapter<Dummy>>();
    }

    #[test]
    fn trait_sync() {
        struct Dummy;
        impl RustLintPass for Dummy {}

        fn assert_sync<T: Sync>() {}
        assert_sync::<RustLintPassAdapter<Dummy>>();
    }

    #[test]
    fn trait_unpin() {
        struct Dummy;
        impl RustLintPass for Dummy {}

        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RustLintPassAdapter<Dummy>>();
    }

    #[test]
    fn adapter_delegates_to_dispatch() {
        struct FnFinder {
            found: bool,
        }
        impl RustLintPass for FnFinder {
            fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.found = true;
                vec![Diagnostic::new(
                    RuleId::new("test.fn"),
                    Severity::Warn,
                    "found".into(),
                    node.span(),
                )]
            }
        }

        let tree = parse_rust("fn main() {}");
        let fn_node = tree.root_node().named_child(0).unwrap();
        let mut adapter = RustLintPassAdapter::new(FnFinder { found: false });

        let checked: Result<_, Panic> = adapter.check_node(&fn_node).into();

        let diagnostics = checked.expect("the rule should not panic");
        let rules: Vec<RuleId> = diagnostics.iter().map(|found| found.rule_id()).collect();
        assert_eq!(rules, [RuleId::new("test.fn")]);
    }

    #[test]
    fn adapter_works_with_pipeline() {
        struct NoOp;
        impl RustLintPass for NoOp {}

        let tree = parse_rust("fn main() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(RustLintPassAdapter::new(NoOp))];

        let diagnostics = whisker_core::walk(&tree, &mut passes).expect("should walk");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn adapter_collects_diagnostics_through_pipeline() {
        struct WarnOnFn;
        impl RustLintPass for WarnOnFn {
            fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                vec![Diagnostic::new(
                    RuleId::new("test.warn"),
                    Severity::Warn,
                    "function found".into(),
                    node.span(),
                )]
            }
        }

        let tree = parse_rust("fn a() {} fn b() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(RustLintPassAdapter::new(WarnOnFn))];

        let diagnostics = whisker_core::walk(&tree, &mut passes).expect("should walk");
        assert_eq!(diagnostics.len(), 2);
    }

    /// A rule that panics hands the panic back rather than unwinding
    ///
    /// The adapter is where a plugin's edge is, so this is the one place
    /// that proves a panic becomes a value with the message intact.
    #[test]
    fn check_node_with_a_panicking_rule_returns_the_panic() {
        struct Exploding;
        impl RustLintPass for Exploding {
            fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                panic!("cannot check {}", node.kind());
            }
        }

        let tree = parse_rust("fn main() {}");
        let fn_node = tree.root_node().named_child(0).unwrap();
        let mut adapter = RustLintPassAdapter::new(Exploding);

        let checked: Result<_, Panic> = adapter.check_node(&fn_node).into();

        let panic = checked.expect_err("the rule should panic");
        assert_eq!(panic.message(), "cannot check function_item");
    }

    #[test]
    fn configure_with_a_panicking_rule_returns_the_panic() {
        struct Picky;
        impl RustLintPass for Picky {
            fn configure(&mut self, _options: &RuleOptions) {
                panic!("no options accepted");
            }
        }

        let mut adapter = RustLintPassAdapter::new(Picky);

        let configured: Result<(), Panic> = adapter.configure(&RuleOptions::default()).into();

        let panic = configured.expect_err("the rule should panic");
        assert_eq!(panic.message(), "no options accepted");
    }
}
