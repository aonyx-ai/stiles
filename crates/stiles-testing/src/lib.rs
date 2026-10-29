use std::path::{Path, PathBuf};

use stiles_types::{DecoratedTree, DecorationMap, Diagnostic, Language, LintPass, Severity, Span};

/// Parses source text into a decorated tree for the given language
///
/// The returned tree has no decorations attached. Use [`decorate`] to
/// add manually constructed decorations for testing. TypeScript source
/// parses with the TypeScript grammar, so it may not hold JSX.
///
/// # Panics
///
/// Panics if tree-sitter fails to parse.
///
/// # Examples
///
/// ```
/// use stiles_testing::parse;
/// use stiles_types::Language;
///
/// let tree = parse("fn main() {}", Language::Rust);
///
/// assert_eq!(tree.root_node().kind(), "source_file");
/// ```
pub fn parse(source: &str, language: Language) -> DecoratedTree {
    let ts_language = match language {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    };

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&ts_language)
        .expect("failed to set tree-sitter language");

    let tree = parser
        .parse(source, None)
        .expect("tree-sitter parse returned None");

    DecoratedTree::builder()
        .tree(tree)
        .language(language)
        .source(source.to_string())
        .file(PathBuf::from("<test>"))
        .build()
}

/// Attaches manual decorations to a parsed tree
///
/// Replaces the tree's decoration map with the provided one. This
/// allows testing rules that depend on semantic information without
/// running a real language toolchain.
///
/// # Examples
///
/// ```
/// use stiles_testing::{decorate, parse};
/// use stiles_types::{DecorationMap, Language};
///
/// let mut tree = parse("fn main() {}", Language::Rust);
///
/// decorate(&mut tree, DecorationMap::new());
///
/// assert_eq!(tree.root_node().kind(), "source_file");
/// ```
pub fn decorate(tree: &mut DecoratedTree, decorations: DecorationMap) {
    *tree.decorations_mut() = decorations;
}

/// Executes lint passes against a decorated tree and returns diagnostics
///
/// # Panics
///
/// Panics if a pass panics. The pass catches its own panic at the plugin
/// boundary and hands it back as a value, which this turns into a test
/// failure. The message is the one stiles reports, and it names the file,
/// the node, and the pass's own message.
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_testing::{execute, parse};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleOptions,
/// };
///
/// /// Reports nothing
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let tree = parse("fn main() {}", Language::Rust);
/// let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(Quiet)];
///
/// let diagnostics = execute(&tree, &mut passes);
///
/// assert!(diagnostics.is_empty());
/// ```
pub fn execute(tree: &DecoratedTree, passes: &mut [Box<dyn LintPass>]) -> Vec<Diagnostic> {
    stiles_core::walk(tree, passes).unwrap_or_else(|error| panic!("{error:#}"))
}

/// Starts a chain of assertions on the properties of a diagnostic
///
/// # Examples
///
/// ```ignore
/// let diag = &diagnostics[0];
/// assert_diagnostic(diag)
///     .has_rule_id("lint.test")
///     .has_severity(Severity::Warn)
///     .message_contains("wildcard");
/// ```
pub fn assert_diagnostic(diagnostic: &Diagnostic) -> DiagnosticAssertion<'_> {
    DiagnosticAssertion { diagnostic }
}

/// Asserts that no diagnostics were emitted
///
/// # Panics
///
/// Panics if `diagnostics` is not empty, printing all diagnostics.
///
/// # Examples
///
/// ```
/// stiles_testing::assert_no_diagnostics(&[]);
/// ```
pub fn assert_no_diagnostics(diagnostics: &[Diagnostic]) {
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {}: {:?}",
        diagnostics.len(),
        diagnostics
    );
}

/// Loads test fixtures from a directory for the given language
///
/// Each source file matching the language's extension becomes a test
/// case. Returns pairs of `(filename, source_text)` sorted by filename.
///
/// # Panics
///
/// Panics if the directory cannot be read.
///
/// # Examples
///
/// ```
/// use stiles_testing::fixtures;
/// use stiles_types::Language;
///
/// let cases = fixtures(concat!(env!("CARGO_MANIFEST_DIR"), "/src"), Language::Rust);
///
/// assert_eq!(cases[0].0, "lib.rs");
/// ```
pub fn fixtures(dir: &str, language: Language) -> Vec<(String, String)> {
    let path = Path::new(dir);
    let mut cases = Vec::new();

    let entries = std::fs::read_dir(path).unwrap_or_else(|e| panic!("read fixture dir {dir}: {e}"));

    for entry in entries.flatten() {
        let file_path = entry.path();
        let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if Language::from_extension(ext) != Some(language) {
            continue;
        }
        let name = file_path.file_name().unwrap().to_string_lossy().to_string();
        let source = std::fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", file_path.display()));
        cases.push((name, source));
    }

    cases.sort_by(|a, b| a.0.cmp(&b.0));
    cases
}

/// Fluent assertion helper for [`Diagnostic`] values
///
/// [`assert_diagnostic`] starts one, and each method checks one property
/// and hands the helper back for the next.
///
/// # Examples
///
/// ```
/// use stiles_testing::assert_diagnostic;
/// use stiles_types::{Diagnostic, RuleId, Severity, Span};
///
/// let diagnostic = Diagnostic::builder()
///     .rule_id(RuleId::new("lint.example"))
///     .severity(Severity::Warn)
///     .message("a wildcard arm")
///     .span(Span::builder().file("<test>").start(3).end(7).build())
///     .build();
///
/// assert_diagnostic(&diagnostic)
///     .has_rule_id("lint.example")
///     .has_severity(Severity::Warn)
///     .message_contains("wildcard");
/// ```
pub struct DiagnosticAssertion<'a> {
    diagnostic: &'a Diagnostic,
}

impl DiagnosticAssertion<'_> {
    /// Asserts the diagnostic has the given rule ID
    ///
    /// # Panics
    ///
    /// Panics if the rule ID does not match `expected`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_testing::assert_diagnostic;
    /// # use stiles_types::{Diagnostic, Location, RuleId, Severity, Span, Suggestion};
    /// # let span = Span::builder().file("<test>").start(3).end(7).build();
    /// # let diagnostic = Diagnostic::builder()
    /// #     .rule_id(RuleId::new("lint.example"))
    /// #     .severity(Severity::Warn)
    /// #     .message("a wildcard arm")
    /// #     .span(span.clone())
    /// #     .build();
    /// #
    /// assert_diagnostic(&diagnostic).has_rule_id("lint.example");
    /// ```
    pub fn has_rule_id(self, expected: &str) -> Self {
        assert_eq!(
            self.diagnostic.rule_id().as_str(),
            expected,
            "expected rule_id `{expected}`, got `{}`",
            self.diagnostic.rule_id()
        );
        self
    }

    /// Asserts the diagnostic has the given severity
    ///
    /// # Panics
    ///
    /// Panics if the severity does not match `expected`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_testing::assert_diagnostic;
    /// # use stiles_types::{Diagnostic, Location, RuleId, Severity, Span, Suggestion};
    /// # let span = Span::builder().file("<test>").start(3).end(7).build();
    /// # let diagnostic = Diagnostic::builder()
    /// #     .rule_id(RuleId::new("lint.example"))
    /// #     .severity(Severity::Warn)
    /// #     .message("a wildcard arm")
    /// #     .span(span.clone())
    /// #     .build();
    /// #
    /// assert_diagnostic(&diagnostic).has_severity(Severity::Warn);
    /// ```
    pub fn has_severity(self, expected: Severity) -> Self {
        assert_eq!(
            self.diagnostic.severity(),
            expected,
            "expected severity {expected:?}, got {:?}",
            self.diagnostic.severity()
        );
        self
    }

    /// Asserts the diagnostic message contains the given substring
    ///
    /// # Panics
    ///
    /// Panics if the message does not contain `substring`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_testing::assert_diagnostic;
    /// # use stiles_types::{Diagnostic, Location, RuleId, Severity, Span, Suggestion};
    /// # let span = Span::builder().file("<test>").start(3).end(7).build();
    /// # let diagnostic = Diagnostic::builder()
    /// #     .rule_id(RuleId::new("lint.example"))
    /// #     .severity(Severity::Warn)
    /// #     .message("a wildcard arm")
    /// #     .span(span.clone())
    /// #     .build();
    /// #
    /// assert_diagnostic(&diagnostic).message_contains("wildcard");
    /// ```
    pub fn message_contains(self, substring: &str) -> Self {
        assert!(
            self.diagnostic.message().contains(substring),
            "expected message containing `{substring}`, got `{}`",
            self.diagnostic.message()
        );
        self
    }

    /// Asserts the diagnostic span covers the given byte range
    ///
    /// # Panics
    ///
    /// Panics if the span does not match the expected file, start, and end.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_testing::assert_diagnostic;
    /// # use stiles_types::{Diagnostic, Location, RuleId, Severity, Span, Suggestion};
    /// # let span = Span::builder().file("<test>").start(3).end(7).build();
    /// # let diagnostic = Diagnostic::builder()
    /// #     .rule_id(RuleId::new("lint.example"))
    /// #     .severity(Severity::Warn)
    /// #     .message("a wildcard arm")
    /// #     .span(span.clone())
    /// #     .build();
    /// #
    /// assert_diagnostic(&diagnostic).has_span("<test>", 3, 7);
    /// ```
    pub fn has_span(self, file: &str, start: usize, end: usize) -> Self {
        let expected = Span::builder()
            .file(PathBuf::from(file))
            .start(start)
            .end(end)
            .build();
        assert_eq!(
            self.diagnostic.span(),
            &expected,
            "expected span {start}..{end} in {file}, got {}..{} in {}",
            self.diagnostic.span().start(),
            self.diagnostic.span().end(),
            self.diagnostic.span().file().display()
        );
        self
    }

    /// Asserts the diagnostic has the given number of origin locations
    ///
    /// # Panics
    ///
    /// Panics if the origin count does not match `expected`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_testing::assert_diagnostic;
    /// # use stiles_types::{Diagnostic, Location, RuleId, Severity, Span, Suggestion};
    /// # let span = Span::builder().file("<test>").start(3).end(7).build();
    /// # let diagnostic = Diagnostic::builder()
    /// #     .rule_id(RuleId::new("lint.example"))
    /// #     .severity(Severity::Warn)
    /// #     .message("a wildcard arm")
    /// #     .span(span.clone())
    /// #     .build();
    /// #
    /// let origin = Location::builder().span(span).message("set here").build();
    ///
    /// assert_diagnostic(&diagnostic.with_origin(origin)).has_origin_count(1);
    /// ```
    pub fn has_origin_count(self, expected: usize) -> Self {
        assert_eq!(
            self.diagnostic.origins().len(),
            expected,
            "expected {expected} origins, got {}",
            self.diagnostic.origins().len()
        );
        self
    }

    /// Asserts the diagnostic has the given number of related locations
    ///
    /// # Panics
    ///
    /// Panics if the related location count does not match `expected`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_testing::assert_diagnostic;
    /// # use stiles_types::{Diagnostic, Location, RuleId, Severity, Span, Suggestion};
    /// # let span = Span::builder().file("<test>").start(3).end(7).build();
    /// # let diagnostic = Diagnostic::builder()
    /// #     .rule_id(RuleId::new("lint.example"))
    /// #     .severity(Severity::Warn)
    /// #     .message("a wildcard arm")
    /// #     .span(span.clone())
    /// #     .build();
    /// #
    /// let related = Location::builder().span(span).message("used here").build();
    ///
    /// assert_diagnostic(&diagnostic.with_related(related)).has_related_count(1);
    /// ```
    pub fn has_related_count(self, expected: usize) -> Self {
        assert_eq!(
            self.diagnostic.related().len(),
            expected,
            "expected {expected} related locations, got {}",
            self.diagnostic.related().len()
        );
        self
    }

    /// Asserts the diagnostic has the given number of suggestions
    ///
    /// # Panics
    ///
    /// Panics if the suggestion count does not match `expected`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_testing::assert_diagnostic;
    /// # use stiles_types::{Diagnostic, Location, RuleId, Severity, Span, Suggestion};
    /// # let span = Span::builder().file("<test>").start(3).end(7).build();
    /// # let diagnostic = Diagnostic::builder()
    /// #     .rule_id(RuleId::new("lint.example"))
    /// #     .severity(Severity::Warn)
    /// #     .message("a wildcard arm")
    /// #     .span(span.clone())
    /// #     .build();
    /// #
    /// let suggestion = Suggestion::builder()
    ///     .span(span)
    ///     .replacement("Some(_)")
    ///     .message("name the variant")
    ///     .build();
    ///
    /// assert_diagnostic(&diagnostic.with_suggestion(suggestion)).has_suggestion_count(1);
    /// ```
    pub fn has_suggestion_count(self, expected: usize) -> Self {
        assert_eq!(
            self.diagnostic.suggestions().len(),
            expected,
            "expected {expected} suggestions, got {}",
            self.diagnostic.suggestions().len()
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use stiles_types::{
        Checked, Configured, DecoratedNode, Decoration, DecorationKey, Diagnostic, RuleId,
        RuleOptions, Severity,
    };

    use super::*;

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Tag(u32);

    impl Decoration for Tag {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Tag"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[test]
    fn assert_no_diagnostics_with_empty_slice_succeeds() {
        assert_no_diagnostics(&[]);
    }

    #[test]
    #[should_panic(expected = "expected no diagnostics")]
    fn assert_no_diagnostics_with_non_empty_slice_panics() {
        let diag = Diagnostic::builder()
            .rule_id(RuleId::new("test"))
            .severity(Severity::Warn)
            .message("msg")
            .span(
                stiles_types::Span::builder()
                    .file(std::path::PathBuf::from("f.rs"))
                    .start(0)
                    .end(1)
                    .build(),
            )
            .build();
        assert_no_diagnostics(&[diag]);
    }

    #[test]
    fn decorate_replaces_decoration_map() {
        let mut tree = parse("fn main() {}", Language::Rust);
        let mut map = DecorationMap::new();
        map.insert(0, Tag(42));
        decorate(&mut tree, map);

        assert_eq!(tree.decorations_mut().get::<Tag>(0), Some(&Tag(42)));
    }

    #[test]
    fn execute_collects_diagnostics_from_pass() {
        struct FnFinder;
        impl LintPass for FnFinder {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                let found = match node.kind() == "function_item" {
                    true => vec![
                        Diagnostic::builder()
                            .rule_id(RuleId::new("test.fn"))
                            .severity(Severity::Warn)
                            .message("found fn")
                            .span(node.span())
                            .build(),
                    ],
                    false => Vec::new(),
                };
                Checked::Ok(found.into_iter().collect())
            }
        }

        let tree = parse("fn main() {}", Language::Rust);
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(FnFinder)];
        let diagnostics = execute(&tree, &mut passes);
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("expected one diagnostic, got {diagnostics:?}");
        };

        assert_diagnostic(diagnostic)
            .has_rule_id("test.fn")
            .has_severity(Severity::Warn)
            .message_contains("found fn");
    }

    #[test]
    fn execute_with_no_passes_returns_empty() {
        let tree = parse("fn main() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut Vec::new());

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_returns_tree_with_source_file_root() {
        let tree = parse("fn main() {}", Language::Rust);
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DiagnosticAssertion<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DiagnosticAssertion<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DiagnosticAssertion<'_>>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn parse_never_panics(source in "\\PC{0,200}") {
                let _tree = parse(&source, Language::Rust);
            }

            #[test]
            fn parse_root_is_source_file_for_valid_rust(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let tree = parse(&source, Language::Rust);
                let root = tree.root_node();
                prop_assert_eq!(root.kind(), "source_file");
            }

            #[test]
            fn parse_preserves_source(source in "\\PC{0,200}") {
                let tree = parse(&source, Language::Rust);
                prop_assert_eq!(tree.source(), source.as_str());
            }

            #[test]
            fn execute_with_no_passes_always_empty(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let tree = parse(&source, Language::Rust);
                let diagnostics = execute(&tree, &mut Vec::new());
                prop_assert!(diagnostics.is_empty());
            }
        }
    }
}
