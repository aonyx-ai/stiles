use std::path::Path;

use bon::bon;
use whisker_types::{
    Coverage, DecoratedTree, DecorationProvider, Diagnostic, Language, UncoveredFile,
};

use crate::{Lint, tree_walker};

mod build_pipeline_error;
mod detect_language_error;
mod run_pipeline_error;

pub use self::build_pipeline_error::BuildPipelineError;
pub use self::detect_language_error::DetectLanguageError;
pub use self::run_pipeline_error::RunPipelineError;

/// Orchestrates the parse-decorate-execute pipeline
///
/// The pipeline reads a source file, parses it with the appropriate
/// tree-sitter grammar, runs decoration providers, then walks the
/// decorated tree through all enabled lint passes.
pub struct Pipeline {
    language: Language,
    parser: tree_sitter::Parser,
}

#[bon]
impl Pipeline {
    /// Creates a pipeline that parses `language` with `grammar`
    ///
    /// Every tree the pipeline produces carries `language`, which decides
    /// the lint passes that see it, so `grammar` must be that language's.
    ///
    /// # Errors
    ///
    /// Returns [`BuildPipelineError::IncompatibleGrammar`] if tree-sitter
    /// refuses the grammar.
    #[builder]
    pub fn new(
        language: Language,
        grammar: &tree_sitter::Language,
    ) -> Result<Self, BuildPipelineError> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(grammar)
            .map_err(|source| BuildPipelineError::IncompatibleGrammar { source })?;
        Ok(Self { language, parser })
    }

    /// Runs the full pipeline on a source file
    ///
    /// # Errors
    ///
    /// Returns [`RunPipelineError::UnreadableSource`] if the file cannot be
    /// read. Any other error comes from [`Pipeline::run_on_source`].
    pub fn run(
        &mut self,
        path: &Path,
        providers: &[&dyn DecorationProvider],
        lints: &mut [Lint<'_>],
    ) -> Result<Vec<Diagnostic>, RunPipelineError> {
        let source =
            std::fs::read_to_string(path).map_err(|source| RunPipelineError::UnreadableSource {
                path: path.to_path_buf(),
                source,
            })?;

        self.run_on_source(&source, path, providers, lints)
    }

    /// Runs the pipeline on source text directly
    ///
    /// The pipeline offers the file to every provider, and one claim is
    /// enough. If no provider claims the file, the pipeline returns an
    /// [`UncoveredFile`] error instead of a false clean result.
    ///
    /// Decorations merge in provider order. The first decoration of a
    /// type on a node is the one lint rules see.
    ///
    /// # Errors
    ///
    /// Returns [`RunPipelineError::NoSyntaxTree`] if parsing fails, and
    /// [`RunPipelineError::ProviderFailed`] if a provider's toolchain
    /// malfunctions. Returns [`RunPipelineError::Uncovered`] if no provider
    /// claims the file, and an empty provider list counts as no claims.
    /// Returns [`RunPipelineError::LintsFailed`] if a pass panics or reports
    /// a rule it did not declare.
    pub fn run_on_source(
        &mut self,
        source: &str,
        path: &Path,
        providers: &[&dyn DecorationProvider],
        lints: &mut [Lint<'_>],
    ) -> Result<Vec<Diagnostic>, RunPipelineError> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or(RunPipelineError::NoSyntaxTree)?;

        let mut decorated =
            DecoratedTree::new(tree, self.language, source.to_string(), path.to_path_buf());

        let mut covered = Vec::new();
        let mut gaps = Vec::new();

        for provider in providers {
            let coverage = provider.decorate(&decorated).map_err(|source| {
                RunPipelineError::ProviderFailed {
                    provider: provider.name(),
                    source,
                }
            })?;

            match coverage {
                Coverage::Covered(decorations) => covered.push(decorations),
                Coverage::NotCovered(gap) => gaps.push((provider.name(), gap)),
            }
        }

        if covered.is_empty() {
            return Err(RunPipelineError::Uncovered {
                source: UncoveredFile::builder()
                    .file(path.to_path_buf())
                    .gaps(gaps)
                    .build(),
            });
        }

        for decorations in covered {
            decorated.merge_decorations(decorations);
        }

        tree_walker::walk_lints(&decorated, lints)
            .map_err(|source| RunPipelineError::LintsFailed { source })
    }
}

/// Detects the language of a source file from its extension
///
/// # Errors
///
/// Returns [`DetectLanguageError::MissingExtension`] if the file has no
/// extension, and [`DetectLanguageError::UnsupportedExtension`] if no
/// language uses it.
pub fn detect_language(path: &Path) -> Result<Language, DetectLanguageError> {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return Err(DetectLanguageError::MissingExtension {
            path: path.to_path_buf(),
        });
    };

    Language::from_extension(extension).ok_or_else(|| DetectLanguageError::UnsupportedExtension {
        extension: extension.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stabby::str::Str;
    use stabby::vec;
    use whisker_types::{
        Checked, Configured, CoverageGap, DecorateTreeError, DecoratedNode, Decoration,
        DecorationKey, DecorationMap, Diagnostic, Language, LintPass, Panic, ProviderName, RuleId,
        RuleOptions, Severity,
    };

    use super::*;

    /// Pairs `pass` with the rules it may report
    fn lint(pass: impl LintPass + 'static, rules: &[&'static str]) -> Lint<'static> {
        Lint::builder()
            .pass(Box::new(pass))
            .rules(rules.iter().map(|rule| RuleId::new(rule)).collect())
            .build()
    }

    /// Claims every file and decorates nothing
    struct Covering;

    impl DecorationProvider for Covering {
        fn name(&self) -> ProviderName {
            ProviderName("covering")
        }

        fn decorate(&self, _tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError> {
            Ok(Coverage::Covered(DecorationMap::new()))
        }
    }

    /// Claims nothing, so the pipeline must treat the file as unanalyzable
    struct Declining;

    impl DecorationProvider for Declining {
        fn name(&self) -> ProviderName {
            ProviderName("declining")
        }

        fn decorate(&self, _tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError> {
            Ok(Coverage::NotCovered(CoverageGap::StaleSource))
        }
    }

    /// Claims every file and puts a known marker on the root node
    struct Decorating(&'static str);

    impl DecorationProvider for Decorating {
        fn name(&self) -> ProviderName {
            ProviderName("decorating")
        }

        fn decorate(&self, tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError> {
            let mut decorations = DecorationMap::new();
            decorations.insert(tree.root_node().id(), Marker(Str::new(self.0)));
            Ok(Coverage::Covered(decorations))
        }
    }

    #[stabby::stabby]
    #[derive(Debug)]
    struct Marker(Str<'static>);

    impl Decoration for Marker {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Marker"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    /// Reports whichever [`Marker`] a provider left on the root node
    ///
    /// The assertion runs through a lint pass, so the test observes what
    /// a real rule would see, not what the pipeline happened to store.
    struct ReportMarker;

    impl LintPass for ReportMarker {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
            if node.kind() != "source_file" {
                return Checked::Ok(vec::Vec::new());
            }
            let Some(marker) = node.decoration::<Marker>() else {
                return Checked::Ok(vec::Vec::new());
            };
            Checked::Ok(
                [Diagnostic::new(
                    RuleId::new("test.marker"),
                    Severity::Warn,
                    marker.0.as_str().into(),
                    node.span(),
                )]
                .into_iter()
                .collect(),
            )
        }
    }

    #[test]
    fn detect_language_with_no_extension_returns_error() {
        let result = detect_language(Path::new("Makefile"));
        assert!(result.is_err());
    }

    #[test]
    fn detect_language_with_rs_returns_rust() {
        let lang = detect_language(Path::new("test.rs")).expect("should detect");
        assert_eq!(lang, Language::Rust);
    }

    #[test]
    fn detect_language_with_unknown_extension_returns_error() {
        let result = detect_language(Path::new("test.py"));
        assert!(result.is_err());
    }

    /// A pass that panics ends the file with an error that says where
    ///
    /// The panic comes back across the boundary as a value. The run can
    /// therefore name the node it was checking.
    #[test]
    fn run_on_source_with_a_panicking_pass_returns_an_error_naming_the_node() {
        struct Panicking;
        impl whisker_types::LintPass for Panicking {
            extern "C" fn language(&self) -> whisker_types::Language {
                whisker_types::Language::Rust
            }

            extern "C" fn configure(
                &mut self,
                _options: &whisker_types::RuleOptions,
            ) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                let kind = node.kind();
                Panic::catch(move || -> Vec<Diagnostic> {
                    match kind == "function_item" {
                        true => panic!("no functions allowed"),
                        false => Vec::new(),
                    }
                })
                .map(|found| found.into_iter().collect())
                .into()
            }
        }

        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::builder()
            .language(Language::Rust)
            .grammar(&ts_lang)
            .build()
            .unwrap();
        let mut passes = vec![lint(Panicking, &[])];

        let error = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[&Covering as &dyn DecorationProvider],
                &mut passes,
            )
            .expect_err("a panicking pass should fail the file");

        let RunPipelineError::LintsFailed { source } = error else {
            panic!("should be a failed walk, got {error:?}");
        };
        assert_eq!(
            source.to_string(),
            "a lint pass panicked while checking a function_item at byte 0 of test.rs: no functions \
             allowed"
        );
    }

    #[test]
    fn run_on_source_with_covering_and_declining_providers_returns_diagnostics() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::builder()
            .language(Language::Rust)
            .grammar(&ts_lang)
            .build()
            .unwrap();
        let mut passes = vec![lint(ReportMarker, &["test.marker"])];

        let diagnostics = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[
                    &Declining as &dyn DecorationProvider,
                    &Decorating("covered"),
                ],
                &mut passes,
            )
            .expect("one covering provider should be enough");

        let messages: Vec<&str> = diagnostics.iter().map(Diagnostic::message).collect();
        assert_eq!(messages, ["covered"]);
    }

    #[test]
    fn run_on_source_with_declining_provider_names_the_file() {
        let uncovered = uncovered(&[&Declining]);

        assert_eq!(uncovered.file(), Path::new("test.rs"));
    }

    #[test]
    fn run_on_source_with_declining_provider_returns_its_gap() {
        let uncovered = uncovered(&[&Declining]);

        assert_eq!(
            uncovered.gaps(),
            [(ProviderName("declining"), CoverageGap::StaleSource)]
        );
    }

    /// Runs a pipeline over `fn main() {}` with the given providers and
    /// returns the file they left uncovered
    fn uncovered(providers: &[&dyn DecorationProvider]) -> UncoveredFile {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::builder()
            .language(Language::Rust)
            .grammar(&ts_lang)
            .build()
            .unwrap();

        let error = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                providers,
                &mut Vec::new(),
            )
            .expect_err("the providers should not cover the file");

        let RunPipelineError::Uncovered { source: uncovered } = error else {
            panic!("should be an UncoveredFile, got {error:?}");
        };
        uncovered
    }

    #[test]
    fn run_on_source_with_empty_passes_produces_no_diagnostics() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::builder()
            .language(Language::Rust)
            .grammar(&ts_lang)
            .build()
            .unwrap();

        let diagnostics = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[&Covering as &dyn DecorationProvider],
                &mut Vec::new(),
            )
            .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn run_on_source_with_lint_pass_collects_diagnostics() {
        struct AlwaysWarn;
        impl whisker_types::LintPass for AlwaysWarn {
            extern "C" fn language(&self) -> whisker_types::Language {
                whisker_types::Language::Rust
            }

            extern "C" fn configure(
                &mut self,
                _options: &whisker_types::RuleOptions,
            ) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                let found = match node.kind() == "function_item" {
                    true => vec![Diagnostic::new(
                        RuleId::new("test.always"),
                        Severity::Warn,
                        "found function".into(),
                        node.span(),
                    )],
                    false => Vec::new(),
                };
                Checked::Ok(found.into_iter().collect())
            }
        }

        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::builder()
            .language(Language::Rust)
            .grammar(&ts_lang)
            .build()
            .unwrap();
        let mut passes = vec![lint(AlwaysWarn, &["test.always"])];

        let diagnostics = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[&Covering as &dyn DecorationProvider],
                &mut passes,
            )
            .unwrap();

        let messages: Vec<&str> = diagnostics.iter().map(Diagnostic::message).collect();
        assert_eq!(messages, ["found function"]);
    }

    #[test]
    fn run_on_source_with_no_providers_reports_no_providers_configured() {
        let uncovered = uncovered(&[]);

        assert_eq!(
            uncovered.to_string(),
            "no decoration providers were configured, so no file can be analyzed"
        );
    }

    #[test]
    fn run_on_source_with_no_providers_returns_no_gaps() {
        let uncovered = uncovered(&[]);

        assert!(uncovered.gaps().is_empty());
    }

    #[test]
    fn run_on_source_with_two_decorating_providers_keeps_first_decoration() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::builder()
            .language(Language::Rust)
            .grammar(&ts_lang)
            .build()
            .unwrap();
        let mut passes = vec![lint(ReportMarker, &["test.marker"])];

        let diagnostics = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[
                    &Decorating("first") as &dyn DecorationProvider,
                    &Decorating("second"),
                ],
                &mut passes,
            )
            .expect("both providers cover the file");

        let messages: Vec<&str> = diagnostics.iter().map(Diagnostic::message).collect();
        assert_eq!(messages, ["first"]);
    }

    #[test]
    fn run_on_source_with_uncovered_file_does_not_run_passes() {
        struct Exploding;
        impl whisker_types::LintPass for Exploding {
            extern "C" fn language(&self) -> whisker_types::Language {
                whisker_types::Language::Rust
            }

            extern "C" fn configure(
                &mut self,
                _options: &whisker_types::RuleOptions,
            ) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                Panic::catch(|| -> Vec<Diagnostic> {
                    panic!("lint passes must not run on an uncovered file")
                })
                .map(|found| found.into_iter().collect())
                .into()
            }
        }

        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::builder()
            .language(Language::Rust)
            .grammar(&ts_lang)
            .build()
            .unwrap();
        let mut passes = vec![lint(Exploding, &[])];

        let error = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[&Declining as &dyn DecorationProvider],
                &mut passes,
            )
            .expect_err("a declining provider should not cover the file");

        let RunPipelineError::Uncovered { .. } = error else {
            panic!("should be an UncoveredFile, got {error:?}");
        };
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Pipeline>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Pipeline>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Pipeline>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn detect_language_rs_files_always_succeed(name in "[a-z]{1,10}") {
                let path = PathBuf::from(format!("{name}.rs"));
                let result = detect_language(&path);
                prop_assert!(result.is_ok());
                prop_assert_eq!(result.unwrap(), Language::Rust);
            }

            #[test]
            fn detect_language_unknown_extensions_always_fail(ext in "[a-z]{1,5}") {
                prop_assume!(!["rs", "ts", "mts", "cts", "tsx"].contains(&ext.as_str()));
                let path = PathBuf::from(format!("file.{ext}"));
                prop_assert!(detect_language(&path).is_err());
            }

            #[test]
            fn run_on_source_never_panics_on_valid_rust(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
                let mut pipeline = Pipeline::builder()
                    .language(Language::Rust)
                    .grammar(&ts_lang)
                    .build()
                    .unwrap();

                let result = pipeline.run_on_source(
                    &source,
                    Path::new("test.rs"),
                    &[&Covering as &dyn DecorationProvider],
                    &mut Vec::new(),
                );

                prop_assert!(result.is_ok());
                prop_assert!(result.unwrap().is_empty());
            }

            #[test]
            fn run_on_source_with_arbitrary_input_does_not_panic(
                source in "\\PC{0,200}",
            ) {
                let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
                let mut pipeline = Pipeline::builder()
                    .language(Language::Rust)
                    .grammar(&ts_lang)
                    .build()
                    .unwrap();

                let result = pipeline.run_on_source(
                    &source,
                    Path::new("test.rs"),
                    &[&Covering as &dyn DecorationProvider],
                    &mut Vec::new(),
                );

                prop_assert!(result.is_ok());
            }

            #[test]
            fn run_on_source_without_providers_always_reports_uncovered(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
                let mut pipeline = Pipeline::builder()
                    .language(Language::Rust)
                    .grammar(&ts_lang)
                    .build()
                    .unwrap();

                let result = pipeline.run_on_source(
                    &source,
                    Path::new("test.rs"),
                    &[],
                    &mut Vec::new(),
                );

                let error = result.expect_err("no providers can never cover a file");
                let is_uncovered = match error {
                    RunPipelineError::Uncovered { .. } => true,
                    RunPipelineError::UnreadableSource { .. }
                    | RunPipelineError::NoSyntaxTree
                    | RunPipelineError::ProviderFailed { .. }
                    | RunPipelineError::LintsFailed { .. } => false,
                };
                prop_assert!(is_uncovered);
            }
        }
    }
}
