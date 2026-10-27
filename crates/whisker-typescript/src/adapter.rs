use crate::TypeScriptLintPass;

whisker_types::lint_pass_adapter!(
    TypeScriptLintPassAdapter,
    TypeScriptLintPass,
    crate::dispatch,
    TypeScript
);

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{
        DecoratedNode, DecoratedTree, Diagnostic, Language, LintPass, Panic, RuleId, RuleOptions,
        Severity,
    };

    use super::*;

    fn parse_typescript(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&crate::language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(
            tree,
            Language::TypeScript,
            source.to_string(),
            PathBuf::from("test.ts"),
        )
    }

    struct WarnOnFunction;

    impl TypeScriptLintPass for WarnOnFunction {
        fn check_function_declaration(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            vec![Diagnostic::new(
                RuleId::new("test.function"),
                Severity::Warn,
                "found a function".into(),
                node.span(),
            )]
        }
    }

    #[test]
    fn adapter_collects_diagnostics_through_the_walker() {
        let tree = parse_typescript("function a() {}\nfunction b() {}");
        let mut passes: Vec<Box<dyn LintPass>> =
            vec![Box::new(TypeScriptLintPassAdapter::new(WarnOnFunction))];

        let diagnostics = whisker_core::walk(&tree, &mut passes).expect("should walk");

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn adapter_delegates_to_dispatch() {
        let tree = parse_typescript("function main() {}");
        let function = tree.root_node().named_child(0).unwrap();
        let mut adapter = TypeScriptLintPassAdapter::new(WarnOnFunction);

        let checked: Result<_, Panic> = adapter.check_node(&function).into();

        let diagnostics = checked.expect("the rule should not panic");
        let rules: Vec<RuleId> = diagnostics.iter().map(|found| found.rule_id()).collect();
        assert_eq!(rules, [RuleId::new("test.function")]);
    }

    #[test]
    fn adapter_names_typescript_as_its_language() {
        let adapter = TypeScriptLintPassAdapter::new(WarnOnFunction);

        let language = adapter.language();

        assert_eq!(language, Language::TypeScript);
    }

    #[test]
    fn check_node_with_a_panicking_rule_returns_the_panic() {
        struct Exploding;
        impl TypeScriptLintPass for Exploding {
            fn check_function_declaration(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                panic!("cannot check {}", node.kind());
            }
        }

        let tree = parse_typescript("function main() {}");
        let function = tree.root_node().named_child(0).unwrap();
        let mut adapter = TypeScriptLintPassAdapter::new(Exploding);

        let checked: Result<_, Panic> = adapter.check_node(&function).into();

        let panic = checked.expect_err("the rule should panic");
        assert_eq!(panic.message(), "cannot check function_declaration");
    }

    #[test]
    fn configure_with_a_panicking_rule_returns_the_panic() {
        struct Picky;
        impl TypeScriptLintPass for Picky {
            fn configure(&mut self, _options: &RuleOptions) {
                panic!("no options accepted");
            }
        }

        let mut adapter = TypeScriptLintPassAdapter::new(Picky);

        let configured: Result<(), Panic> = adapter.configure(&RuleOptions::default()).into();

        let panic = configured.expect_err("the rule should panic");
        assert_eq!(panic.message(), "no options accepted");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TypeScriptLintPassAdapter<WarnOnFunction>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TypeScriptLintPassAdapter<WarnOnFunction>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TypeScriptLintPassAdapter<WarnOnFunction>>();
    }
}
