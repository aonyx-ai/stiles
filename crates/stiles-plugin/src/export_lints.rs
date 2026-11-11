/// Exports lint passes as a custom lint plugin
///
/// Invoke this once, at the crate root of a `cdylib` lint crate. The lints
/// are grouped by the language their pass is written for, and each group
/// lists one expression per lint:
///
/// ```ignore
/// stiles_plugin::export_lints![
///     rust: [NoTodo, PreferExpect],
///     typescript: [NoTodo],
/// ];
/// ```
///
/// A `rust` group takes types implementing [`RustLintPass`], and a
/// `typescript` group types implementing [`TypeScriptLintPass`]. The group
/// decides the adapter that bridges a lint to [`LintPass`], and so the
/// language stiles runs it on. One type may appear in more than one
/// group: a rule that reads only comments can implement several language
/// traits and run on each.
///
/// Each lint carries the language of its group and the rule names its
/// [`DeclaresRules`] implementation returns. Stiles refuses two lints that
/// declare one rule for one language. It also ends a run in which a pass
/// reports a rule it did not declare.
///
/// The macro writes the `stiles_plugin_declaration` static that stiles's
/// loader looks up, filling in the handshake constants of the stiles
/// crates this plugin was compiled against. It also writes the `register`
/// function the declaration names. That function registers each lint
/// through the host's [`Registrar`], with its language, its rules, and a
/// factory. Passes are stateful, and the check command constructs a fresh
/// set for every file. Each factory is a plain `extern "C"` function, so
/// each expression must construct its lint from nothing; an expression
/// that captures its surroundings does not compile.
///
/// A factory hands stiles a [`PassInstance`]: the rule inside its adapter,
/// boxed in the plugin, and the table of `extern "C"` entries that run it.
/// The host frees the pass through that table, so the plugin's allocator
/// frees what it allocated.
///
/// Every function the macro exports is `extern "C"` and runs the plugin's
/// own code under [`Panic::catch`]. A panic can happen in a constructor, in
/// the registration, or in a rule's declaration. It comes back to stiles
/// as a value rather than unwinding across the boundary.
///
/// [`DeclaresRules`]: crate::DeclaresRules
/// [`LintPass`]: stiles_types::LintPass
/// [`Panic::catch`]: crate::Panic::catch
/// [`PassInstance`]: crate::PassInstance
/// [`Registrar`]: crate::Registrar
/// [`RustLintPass`]: stiles_rust::RustLintPass
/// [`TypeScriptLintPass`]: stiles_typescript::TypeScriptLintPass
#[macro_export]
macro_rules! export_lints {
    ($($language:ident: [$($lint:expr),+ $(,)?]),+ $(,)?) => {
        #[unsafe(no_mangle)]
        #[allow(non_upper_case_globals)]
        pub static stiles_plugin_declaration: $crate::PluginDeclaration =
            $crate::PluginDeclaration {
                abi_version: $crate::ABI_VERSION,
                types_fingerprint: $crate::TYPES_FINGERPRINT,
                language_fingerprint: $crate::LANGUAGE_FINGERPRINT,
                register: __stiles_register,
                provenance: $crate::PROVENANCE,
            };

        #[doc(hidden)]
        extern "C" fn __stiles_register(registrar: &$crate::Registrar) -> $crate::Registered {
            $crate::Panic::catch(|| {
                $($({
                    extern "C" fn build(
                        out: &mut ::core::mem::MaybeUninit<$crate::PassInstance>,
                    ) -> $crate::Built {
                        $crate::Panic::catch(|| {
                            out.write($crate::__adapter!($language)($lint));
                        })
                        .into()
                    }

                    let rules = $crate::DeclaresRules::rules(&$lint);
                    // SAFETY: `build` writes the instance before it returns
                    // `Ok`, and returns the panic instead when construction
                    // panics.
                    unsafe { registrar.lint($crate::__language!($language), build, &rules) };
                })+)+
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

/// Names the function that wraps a lint of one language for the boundary
#[doc(hidden)]
#[macro_export]
macro_rules! __adapter {
    (rust) => {
        $crate::__private::stiles_rust::RustLintPassAdapter::instance
    };
    (typescript) => {
        $crate::__private::stiles_typescript::TypeScriptLintPassAdapter::instance
    };
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stiles_rust::{RuleId, RustLintPass};
    use stiles_types::plugin::{LoadedPass, RegisteredLint, Registrations};
    use stiles_types::{
        Collector, DecoratedNode, DecoratedTree, Diagnostic, Language, LintPass, Panic, Severity,
    };
    use stiles_typescript::TypeScriptLintPass;

    use crate::{ABI_VERSION, DeclaresRules, LANGUAGE_FINGERPRINT, TYPES_FINGERPRINT};

    struct FlagEveryFunction;

    impl DeclaresRules for FlagEveryFunction {
        fn rules(&self) -> Vec<RuleId> {
            vec![RuleId::new("test.flag-every-function")]
        }
    }

    impl RustLintPass for FlagEveryFunction {
        fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            vec![
                Diagnostic::builder()
                    .rule_id(RuleId::new("test.flag-every-function"))
                    .severity(Severity::Warn)
                    .message("found a function")
                    .span(node.span())
                    .build(),
            ]
        }
    }

    struct FlagEveryComment;

    impl FlagEveryComment {
        fn flag(node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            vec![
                Diagnostic::builder()
                    .rule_id(RuleId::new("test.flag-every-comment"))
                    .severity(Severity::Warn)
                    .message("found a comment")
                    .span(node.span())
                    .build(),
            ]
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
        DecoratedTree::builder()
            .tree(tree)
            .language(language)
            .source(source.into())
            .file(PathBuf::from("test"))
            .build()
    }

    fn registered() -> Vec<RegisteredLint> {
        let registrations = Registrations::new();
        let registered: Result<(), Panic> = registrations
            .lend(|registrar| (stiles_plugin_declaration.register)(registrar))
            .into();
        registered.expect("registering the plugin should not panic");

        registrations
            .finish()
            .expect("the registration should be whole")
    }

    fn built(index: usize) -> LoadedPass {
        registered()[index]
            .build()
            .expect("construction should not panic")
    }

    #[test]
    fn declaration_carries_the_abi_version() {
        let version = stiles_plugin_declaration.abi_version;

        assert_eq!(version, ABI_VERSION);
    }

    #[test]
    fn declaration_carries_the_language_fingerprint() {
        let language = stiles_plugin_declaration.language_fingerprint;

        assert_eq!(language, LANGUAGE_FINGERPRINT);
    }

    #[test]
    fn declaration_carries_the_types_fingerprint() {
        let types = stiles_plugin_declaration.types_fingerprint;

        assert_eq!(types, TYPES_FINGERPRINT);
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
        let tree = parse(stiles_rust::language(), Language::Rust, "fn main() {}");
        let function = tree.root_node().named_child(0).expect("should parse a fn");
        let mut pass = built(0);

        let collector = Collector::new();

        let checked: Result<(), Panic> = collector
            .lend(|sink| LintPass::check_node(&mut pass, &function, sink))
            .into();

        checked.expect("the pass should not panic");
        let diagnostics = collector.finish().expect("the report should be whole");
        let rules: Vec<RuleId> = diagnostics.iter().map(|found| found.rule_id()).collect();
        assert_eq!(rules, [RuleId::new("test.flag-every-function")]);
    }

    #[test]
    fn factories_build_working_typescript_passes() {
        let tree = parse(
            stiles_typescript::language(),
            Language::TypeScript,
            "// note\nlet a = 1;",
        );
        let comment = tree
            .root_node()
            .named_child(0)
            .expect("should parse a comment");
        let mut pass = built(2);

        let collector = Collector::new();

        let checked: Result<(), Panic> = collector
            .lend(|sink| LintPass::check_node(&mut pass, &comment, sink))
            .into();

        checked.expect("the pass should not panic");
        let diagnostics = collector.finish().expect("the report should be whole");
        let rules: Vec<RuleId> = diagnostics.iter().map(|found| found.rule_id()).collect();
        assert_eq!(rules, [RuleId::new("test.flag-every-comment")]);
    }

    #[test]
    fn register_carries_the_rules_of_each_lint() {
        let lints = registered();

        let rules: Vec<Vec<RuleId>> = lints.iter().map(|lint| lint.rules().to_vec()).collect();

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
    fn register_tags_each_lint_with_its_group_language() {
        let lints = registered();

        let languages: Vec<Language> = lints.iter().map(RegisteredLint::language).collect();

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
    fn register_yields_one_lint_per_listed_entry() {
        let lints = registered();

        assert_eq!(lints.len(), 4);
    }
}
