/// Exports lint passes as a custom lint plugin
///
/// Invoke this once, at the crate root of a `cdylib` lint crate. The lints
/// are grouped by the language their pass is written for, and each group
/// lists one expression per lint:
///
/// ```ignore
/// whisker_plugin::export_lints![
///     rust: [NoTodo, PreferExpect],
///     typescript: [NoTodo],
/// ];
/// ```
///
/// A `rust` group takes types implementing [`RustLintPass`], and a
/// `typescript` group types implementing [`TypeScriptLintPass`]. The group
/// decides the adapter that bridges a lint to [`LintPass`], and so the
/// language whisker runs it on. One type may appear in more than one
/// group: a rule that reads only comments can implement several language
/// traits and run on each.
///
/// Each lint carries the language of its group and the rule names its
/// [`DeclaresRules`] implementation returns. Whisker refuses two lints that
/// declare one rule for one language, and it ends a run in which a pass
/// reports a rule it did not declare.
///
/// The macro writes the `whisker_plugin_declaration` static that whisker's
/// loader looks up, filling in the handshake constants of the whisker
/// crates this plugin was compiled against. Each lint becomes a factory,
/// because passes are stateful and the check command constructs a fresh
/// set for every file. The factories are plain function pointers, so each
/// expression must construct its lint from nothing; an expression that
/// captures its surroundings does not compile.
///
/// Every function the macro exports is `extern "C"` and runs the plugin's
/// own code under [`Panic::catch`]. A panic can happen in a constructor, in
/// the listing of the factories, or in a rule's declaration. It comes back
/// to whisker as a value rather than unwinding across the boundary.
///
/// [`DeclaresRules`]: crate::DeclaresRules
/// [`LintPass`]: whisker_types::LintPass
/// [`Panic::catch`]: crate::Panic::catch
/// [`RustLintPass`]: whisker_rust::RustLintPass
/// [`TypeScriptLintPass`]: whisker_typescript::TypeScriptLintPass
#[macro_export]
macro_rules! export_lints {
    ($($language:ident: [$($lint:expr),+ $(,)?]),+ $(,)?) => {
        #[unsafe(no_mangle)]
        #[allow(non_upper_case_globals)]
        pub static whisker_plugin_declaration: $crate::PluginDeclaration =
            $crate::PluginDeclaration {
                abi_version: $crate::ABI_VERSION,
                types_fingerprint: $crate::TYPES_FINGERPRINT,
                language_fingerprint: $crate::LANGUAGE_FINGERPRINT,
                load: __whisker_load,
            };

        #[doc(hidden)]
        extern "C" fn __whisker_load() -> $crate::Loaded {
            $crate::Panic::catch(|| {
                let mut lints = ::std::vec::Vec::new();
                $($(
                    lints.push($crate::ExportedLint {
                        factory: $crate::factory(|| {
                            $crate::Panic::catch(|| $crate::__adapter!($language)($lint)).into()
                        }),
                        language: $crate::__language!($language),
                        rules: $crate::DeclaresRules::rules(&$lint).into_iter().collect(),
                    });
                )+)+
                $crate::Plugin {
                    lints: lints.into_iter().collect(),
                }
            })
            .into()
        }
    };
}

/// Names the language of one group
#[doc(hidden)]
#[macro_export]
macro_rules! __language {
    (rust) => {
        $crate::Language::Rust
    };
    (typescript) => {
        $crate::Language::TypeScript
    };
}

/// Names the function that boxes a lint of one language for the boundary
#[doc(hidden)]
#[macro_export]
macro_rules! __adapter {
    (rust) => {
        $crate::__private::whisker_rust::RustLintPassAdapter::boxed
    };
    (typescript) => {
        $crate::__private::whisker_typescript::TypeScriptLintPassAdapter::boxed
    };
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stabby::closure::Call0Dyn;
    use whisker_rust::RustLintPass;
    use whisker_types::{
        BoxedLintPass, DecoratedNode, DecoratedTree, Diagnostic, Language, LintPass, Panic, RuleId,
        Severity,
    };
    use whisker_typescript::TypeScriptLintPass;

    use crate::{ABI_VERSION, DeclaresRules, LANGUAGE_FINGERPRINT, Plugin, TYPES_FINGERPRINT};

    struct FlagEveryFunction;

    impl DeclaresRules for FlagEveryFunction {
        fn rules(&self) -> Vec<RuleId> {
            vec![RuleId::new("test.flag-every-function")]
        }
    }

    impl RustLintPass for FlagEveryFunction {
        fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            vec![Diagnostic::new(
                RuleId::new("test.flag-every-function"),
                Severity::Warn,
                "found a function".into(),
                node.span(),
            )]
        }
    }

    struct FlagEveryComment;

    impl FlagEveryComment {
        fn flag(node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            vec![Diagnostic::new(
                RuleId::new("test.flag-every-comment"),
                Severity::Warn,
                "found a comment".into(),
                node.span(),
            )]
        }
    }

    impl DeclaresRules for FlagEveryComment {
        fn rules(&self) -> Vec<RuleId> {
            vec![RuleId::new("test.flag-every-comment")]
        }
    }

    impl RustLintPass for FlagEveryComment {
        fn check_line_comment(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            Self::flag(node)
        }
    }

    impl TypeScriptLintPass for FlagEveryComment {
        fn check_comment(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            Self::flag(node)
        }
    }

    struct QuietLint;

    impl DeclaresRules for QuietLint {
        fn rules(&self) -> Vec<RuleId> {
            vec![RuleId::new("test.quiet")]
        }
    }

    impl TypeScriptLintPass for QuietLint {}

    export_lints![
        rust: [FlagEveryFunction, FlagEveryComment],
        typescript: [FlagEveryComment, QuietLint],
    ];

    fn parse(grammar: tree_sitter::Language, language: Language, source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&grammar).unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(tree, language, source.into(), PathBuf::from("test"))
    }

    fn loaded() -> Plugin {
        let loaded: Result<_, Panic> = (whisker_plugin_declaration.load)().into();

        loaded.expect("loading the plugin should not panic")
    }

    fn built(index: usize) -> BoxedLintPass {
        let constructed: Result<_, Panic> = loaded().lints[index].factory.call().into();

        constructed.expect("construction should not panic")
    }

    #[test]
    fn declaration_carries_the_handshake_constants() {
        assert_eq!(whisker_plugin_declaration.abi_version, ABI_VERSION);

        let types = whisker_plugin_declaration.types_fingerprint;
        assert_eq!(types, TYPES_FINGERPRINT);

        let language = whisker_plugin_declaration.language_fingerprint;
        assert_eq!(language, LANGUAGE_FINGERPRINT);
    }

    #[test]
    fn factories_build_passes_of_their_group_language() {
        let rust = built(1);
        let typescript = built(2);

        let languages = (rust.language(), typescript.language());

        assert_eq!(languages, (Language::Rust, Language::TypeScript));
    }

    #[test]
    fn factories_build_working_rust_passes() {
        let tree = parse(whisker_rust::language(), Language::Rust, "fn main() {}");
        let function = tree.root_node().named_child(0).expect("should parse a fn");
        let mut pass = built(0);

        let checked: Result<_, Panic> = LintPass::check_node(&mut pass, &function).into();

        let diagnostics = checked.expect("the pass should not panic");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].rule_id(),
            RuleId::new("test.flag-every-function")
        );
    }

    #[test]
    fn factories_build_working_typescript_passes() {
        let tree = parse(
            whisker_typescript::language(),
            Language::TypeScript,
            "// note\nlet a = 1;",
        );
        let comment = tree
            .root_node()
            .named_child(0)
            .expect("should parse a comment");
        let mut pass = built(2);

        let checked: Result<_, Panic> = LintPass::check_node(&mut pass, &comment).into();

        let diagnostics = checked.expect("the pass should not panic");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].rule_id(),
            RuleId::new("test.flag-every-comment")
        );
    }

    #[test]
    fn load_carries_the_rules_of_each_lint() {
        let lints = loaded().lints;

        let rules: Vec<Vec<RuleId>> = lints
            .iter()
            .map(|lint| lint.rules.iter().copied().collect())
            .collect();

        assert_eq!(
            rules,
            [
                vec![RuleId::new("test.flag-every-function")],
                vec![RuleId::new("test.flag-every-comment")],
                vec![RuleId::new("test.flag-every-comment")],
                vec![RuleId::new("test.quiet")],
            ]
        );
    }

    #[test]
    fn load_tags_each_lint_with_its_group_language() {
        let lints = loaded().lints;

        let languages: Vec<Language> = lints.iter().map(|lint| lint.language).collect();

        assert_eq!(
            languages,
            [
                Language::Rust,
                Language::Rust,
                Language::TypeScript,
                Language::TypeScript
            ]
        );
    }

    #[test]
    fn load_yields_one_lint_per_listed_entry() {
        let lints = loaded().lints;

        assert_eq!(lints.len(), 4);
    }
}
