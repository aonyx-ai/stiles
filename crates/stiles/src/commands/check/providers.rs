use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use bon::bon;
use stiles_rust::RustDecorationProvider;
use stiles_types::plugin::RegisteredLint;
use stiles_types::{
    Coverage, CoverageGap, DecorateTreeError, DecoratedTree, DecorationIdentity,
    DecorationProvider, DecorationSet, Language, ProviderName,
};
use stiles_typescript::TypeScriptDecorationProvider;

use super::grammar::Grammar;
use crate::custom_lints::offered;

/// The decoration providers a run loaded, for the languages it found
///
/// Loading a project is the expensive part of a check. rust-analyzer runs
/// build scripts, and tsrs reads every file a `tsconfig.json` names. A run
/// therefore loads a language's provider only when discovery found a file
/// in that language. A repository of one language needs no project file of
/// the other.
///
/// Rust loads one project, the workspace around the checked path. A
/// TypeScript file belongs to the project of the `tsconfig.json` nearest
/// it, as it does for `tsc` and an editor. One repository often holds
/// several, such as a documentation site beside its packages. The run loads
/// each such project once, and hands a file only the project it belongs to.
///
/// A TypeScript file with no `tsconfig.json` at or above it belongs to no
/// project. It gets a [`CoverageGap::NoProject`] like any file a provider
/// cannot analyze, so one stray script does not stop the check of every
/// other file.
///
/// A provider computes only the decorations that the loaded passes of its
/// language read. For a language that no pass inspects, it computes none.
/// It still decides coverage, because a file that no toolchain covers is an
/// error whether or not a rule reads it.
pub(super) struct Providers {
    rust: Option<RustDecorationProvider>,
    typescript: HashMap<PathBuf, TypeScriptDecorationProvider>,
}

#[bon]
impl Providers {
    /// Loads the providers for every language among `files`, for a check
    /// of `path`
    ///
    /// `lints` holds every loaded lint. A provider computes the decorations
    /// that the lints of its language read, as [`wanted`] decides. A
    /// provider of a language that no lint inspects answers for coverage
    /// alone.
    ///
    /// # Errors
    ///
    /// Returns an error if a needed project cannot be loaded. The context
    /// names the language, because a mixed repository may lack only one of
    /// the project files. A TypeScript file with no `tsconfig.json` is not
    /// an error here: [`Providers::for_file`] reports its gap.
    #[builder]
    pub(super) fn load(
        path: &Path,
        files: &[PathBuf],
        lints: &[&RegisteredLint],
    ) -> anyhow::Result<Self> {
        let mut has_rust = false;
        let mut typescript = HashMap::new();

        for file in files {
            let Some(grammar) = Grammar::of(file) else {
                continue;
            };
            match grammar.language() {
                Language::Rust => has_rust = true,
                Language::TypeScript => {
                    let wanted = wanted(lints, Language::TypeScript);
                    load_typescript_project(file, &wanted, &mut typescript)
                        .context("failed to load the TypeScript project for analysis")?;
                }
            }
        }

        let rust = match has_rust {
            true => Some(
                RustDecorationProvider::load(path)
                    .context("failed to load the target project for analysis")?
                    .wanting(wanted(lints, Language::Rust)),
            ),
            false => None,
        };

        Ok(Self { rust, typescript })
    }

    /// Gives each provider the changed text of the files it may decorate
    ///
    /// A round of `stiles check --fix` changes files in memory. The next
    /// round checks the changed text, and a provider declines a file whose
    /// text differs from the one it analyzed. Each change therefore goes to
    /// the provider that [`Providers::for_file`] picks for its file. A file
    /// that no loaded provider covers needs no change.
    ///
    /// # Errors
    ///
    /// Returns an error if a provider cannot take the changed text.
    pub(super) fn change_files(&mut self, changes: &[(PathBuf, String)]) -> anyhow::Result<()> {
        let mut rust = Vec::new();
        let mut typescript: HashMap<PathBuf, Vec<(PathBuf, String)>> = HashMap::new();

        for (file, text) in changes {
            let Some(grammar) = Grammar::of(file) else {
                continue;
            };
            let change = (file.clone(), text.clone());
            match grammar.language() {
                Language::Rust => rust.push(change),
                Language::TypeScript => {
                    let Some(root) = TypeScriptDecorationProvider::project_root(file) else {
                        continue;
                    };
                    typescript.entry(root).or_default().push(change);
                }
            }
        }

        if let Some(provider) = &mut self.rust
            && !rust.is_empty()
        {
            provider
                .change_files(rust)
                .context("failed to give the Rust provider the fixed text")?;
        }

        for (root, changes) in typescript {
            let Some(provider) = self.typescript.get_mut(&root) else {
                continue;
            };
            provider.change_files(changes).with_context(|| {
                format!(
                    "give the TypeScript project at {} the fixed text",
                    root.display()
                )
            })?;
        }

        Ok(())
    }

    /// Returns the providers that may decorate `file`, a file in `language`
    ///
    /// A provider of another language, or of another TypeScript project,
    /// never sees the file. A file is therefore never reported as a gap of
    /// a project it was never part of.
    pub(super) fn for_file(&self, file: &Path, language: Language) -> Vec<&dyn DecorationProvider> {
        let provider: Option<&dyn DecorationProvider> = match language {
            Language::Rust => self
                .rust
                .as_ref()
                .map(|provider| provider as &dyn DecorationProvider),
            Language::TypeScript => match TypeScriptDecorationProvider::project_root(file) {
                Some(root) => self
                    .typescript
                    .get(&root)
                    .map(|provider| provider as &dyn DecorationProvider),
                None => Some(&NoTypeScriptProject),
            },
        };
        provider.into_iter().collect()
    }
}

/// Stands in for the TypeScript provider of a file that no project holds
///
/// The pipeline turns a file that no provider covers into an error, and the
/// gap it carries is what tells the user the remedy. This provider declines
/// every file with [`CoverageGap::NoProject`], under the TypeScript
/// provider's name, so such a file reads like any other uncovered file.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct NoTypeScriptProject;

impl DecorationProvider for NoTypeScriptProject {
    fn name(&self) -> ProviderName {
        TypeScriptDecorationProvider::NAME
    }

    fn decorate(&self, _tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError> {
        Ok(Coverage::NotCovered(CoverageGap::NoProject))
    }
}

/// Loads the TypeScript project that holds `file`, unless it is loaded
///
/// A file with no `tsconfig.json` at or above it loads nothing.
///
/// # Errors
///
/// Returns an error if the project that holds `file` cannot be loaded.
fn load_typescript_project(
    file: &Path,
    wanted: &DecorationSet,
    projects: &mut HashMap<PathBuf, TypeScriptDecorationProvider>,
) -> anyhow::Result<()> {
    let Some(root) = TypeScriptDecorationProvider::project_root(file) else {
        return Ok(());
    };
    if projects.contains_key(&root) {
        return Ok(());
    }

    let provider = TypeScriptDecorationProvider::load(&root)
        .with_context(|| format!("load the TypeScript project at {}", root.display()))?
        .wanting(wanted.clone());
    projects.insert(root, provider);
    Ok(())
}

/// Returns the decoration types that a provider of `language` computes
///
/// The set is the union of the keys that the loaded lints of `language`
/// declare. A language that no lint inspects gets none.
///
/// A lint that declares [`Reads::none`] adds nothing. A lint whose reads
/// are [`Reads::undeclared`] gets every type of its provider. One such
/// lint therefore makes the provider compute all of them for its language. That
/// fallback is staged, and [stiles#651] removes it.
///
/// [`Reads::none`]: stiles_types::plugin::Reads::none
/// [`Reads::undeclared`]: stiles_types::plugin::Reads::undeclared
///
/// [stiles#651]: https://github.com/aonyx-ai/stiles/issues/651
fn wanted(lints: &[&RegisteredLint], language: Language) -> DecorationSet {
    let mut keys = Vec::new();

    for lint in lints.iter().filter(|lint| lint.language() == language) {
        let Some(identities) = lint.reads().declared() else {
            return offered(language)
                .iter()
                .map(DecorationIdentity::key)
                .collect();
        };
        keys.extend(identities.iter().map(DecorationIdentity::key));
    }

    keys.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use stiles_core::RunPipelineError;
    use stiles_rust::RuleId;
    use stiles_rust::decorations::{FnSignature, ResolvedType};
    use stiles_types::plugin::{Built, PassInstance, Reads, Registrations};
    use stiles_types::{Decoration, Panic};
    use stiles_typescript::decorations::TypeText;

    use super::*;

    /// Builds nothing, because no test here calls it
    extern "C" fn never(_out: &mut MaybeUninit<PassInstance>) -> Built {
        Panic::catch(|| unreachable!("never built")).into()
    }

    /// Returns a lint of `language` that declares it reads `reads`
    fn registered(language: Language, reads: Reads) -> RegisteredLint {
        let registrations = Registrations::new();
        registrations.lend(|registrar| {
            registrar.require_host_tables();
            // SAFETY: `never` never returns `Ok`.
            let lint = unsafe { registrar.lint(language, never, &[RuleId::new("test.rule")]) };
            registrar.reads(lint, &reads);
        });

        let mut lints = registrations
            .finish()
            .expect("the registration should be whole");

        lints.remove(0)
    }

    /// Writes `(path, text)` files into a new temporary directory
    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        for (path, text) in files {
            let path = directory.path().join(path);
            std::fs::create_dir_all(path.parent().expect("a file has a parent"))
                .expect("directories should be created");
            std::fs::write(path, text).expect("the file should be written");
        }
        directory
    }

    #[test]
    fn for_file_routes_a_file_to_its_own_project() {
        let directory = tree(&[
            ("site/tsconfig.json", "{}"),
            ("site/page.ts", "export {};\n"),
            ("packages/a/tsconfig.json", "{}"),
            ("packages/a/index.ts", "export {};\n"),
        ]);
        let page = directory.path().join("site/page.ts");
        let index = directory.path().join("packages/a/index.ts");
        let providers = Providers::load()
            .path(directory.path())
            .files(&[page.clone(), index])
            .lints(&[])
            .call()
            .expect("both projects should load");

        let found = providers.for_file(&page, Language::TypeScript);

        assert_eq!(found.len(), 1);
    }

    #[test]
    fn for_file_without_a_loaded_provider_returns_none() {
        let providers = Providers {
            rust: None,
            typescript: HashMap::new(),
        };

        let found = providers.for_file(Path::new("main.rs"), Language::Rust);

        assert!(found.is_empty());
    }

    #[test]
    fn for_file_without_a_tsconfig_reports_no_project() {
        let directory = tree(&[("main.ts", "export {};\n")]);
        let file = directory.path().join("main.ts");
        let providers = Providers::load()
            .path(directory.path())
            .files(std::slice::from_ref(&file))
            .lints(&[])
            .call()
            .expect("a file without a project should not stop the load");
        let mut pipeline = Grammar::of(&file)
            .expect("a TypeScript grammar")
            .pipeline()
            .expect("the pipeline should start");

        let found = providers.for_file(&file, Language::TypeScript);
        let error = pipeline
            .run_on_source("export {};\n", &file, &found, &mut [])
            .expect_err("a file without a project should not be covered");

        let RunPipelineError::Uncovered { source: uncovered } = error else {
            panic!("should be an UncoveredFile, got {error:?}");
        };
        assert_eq!(
            uncovered.gaps(),
            [(TypeScriptDecorationProvider::NAME, CoverageGap::NoProject)]
        );
    }

    #[test]
    fn load_loads_a_shared_project_once() {
        let directory = tree(&[
            ("tsconfig.json", "{}"),
            ("a.ts", "export {};\n"),
            ("b.ts", "export {};\n"),
        ]);
        let files = [directory.path().join("a.ts"), directory.path().join("b.ts")];

        let providers = Providers::load()
            .path(directory.path())
            .files(&files)
            .lints(&[])
            .call()
            .expect("the project should load");

        assert_eq!(providers.typescript.len(), 1);
    }

    #[test]
    fn load_with_a_typescript_file_and_no_tsconfig_loads_no_project() {
        let directory = tree(&[("main.ts", "export {};\n")]);
        let file = directory.path().join("main.ts");

        let providers = Providers::load()
            .path(directory.path())
            .files(std::slice::from_ref(&file))
            .lints(&[])
            .call()
            .expect("a file without a project should not stop the load");

        assert!(providers.typescript.is_empty());
    }

    #[test]
    fn load_with_no_files_loads_no_rust_project() {
        let directory = tree(&[]);

        let providers = Providers::load()
            .path(directory.path())
            .files(&[])
            .lints(&[])
            .call()
            .expect("nothing should load");

        assert!(providers.rust.is_none());
    }

    #[test]
    fn load_with_no_files_loads_no_typescript_project() {
        let directory = tree(&[]);

        let providers = Providers::load()
            .path(directory.path())
            .files(&[])
            .lints(&[])
            .call()
            .expect("nothing should load");

        assert!(providers.typescript.is_empty());
    }

    #[test]
    fn load_with_only_typescript_files_covers_them() {
        let directory = tree(&[("tsconfig.json", "{}"), ("main.ts", "export {};\n")]);
        let file = directory.path().join("main.ts");

        let providers = Providers::load()
            .path(directory.path())
            .files(std::slice::from_ref(&file))
            .lints(&[])
            .call()
            .expect("the TypeScript project should load");

        assert_eq!(providers.for_file(&file, Language::TypeScript).len(), 1);
    }

    #[test]
    fn load_with_only_typescript_files_needs_no_cargo_project() {
        let directory = tree(&[("tsconfig.json", "{}"), ("main.ts", "export {};\n")]);
        let file = directory.path().join("main.ts");

        let providers = Providers::load()
            .path(directory.path())
            .files(std::slice::from_ref(&file))
            .lints(&[])
            .call()
            .expect("the TypeScript project should load");

        assert!(providers.rust.is_none());
    }

    #[test]
    fn load_with_two_typescript_projects_loads_both() {
        let directory = tree(&[
            ("site/tsconfig.json", "{}"),
            ("site/page.ts", "export {};\n"),
            ("packages/a/tsconfig.json", "{}"),
            ("packages/a/index.ts", "export {};\n"),
        ]);
        let page = directory.path().join("site/page.ts");
        let index = directory.path().join("packages/a/index.ts");

        let providers = Providers::load()
            .path(directory.path())
            .files(&[page, index])
            .lints(&[])
            .call()
            .expect("both projects should load");

        assert_eq!(providers.typescript.len(), 2);
    }

    #[test]
    fn trait_send_no_typescript_project() {
        fn assert_send<T: Send>() {}
        assert_send::<NoTypeScriptProject>();
    }

    #[test]
    fn trait_send_providers() {
        fn assert_send<T: Send>() {}
        assert_send::<Providers>();
    }

    #[test]
    fn trait_sync_no_typescript_project() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<NoTypeScriptProject>();
    }

    #[test]
    fn trait_sync_providers() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Providers>();
    }

    #[test]
    fn trait_unpin_no_typescript_project() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<NoTypeScriptProject>();
    }

    #[test]
    fn trait_unpin_providers() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Providers>();
    }

    /// Returns every key the Rust provider offers
    fn every_rust_key() -> DecorationSet {
        RustDecorationProvider::DECORATIONS
            .iter()
            .map(DecorationIdentity::key)
            .collect()
    }

    #[test]
    fn wanted_for_a_language_no_lint_inspects_is_empty() {
        let lint = registered(Language::Rust, Reads::undeclared());

        let wanted = wanted(&[&lint], Language::TypeScript);

        assert!(wanted.is_empty());
    }

    #[test]
    fn wanted_ignores_the_reads_of_another_language() {
        let rust = registered(Language::Rust, Reads::of::<FnSignature>());
        let typescript = registered(Language::TypeScript, Reads::of::<TypeText>());

        let wanted = wanted(&[&rust, &typescript], Language::Rust);

        assert_eq!(wanted, DecorationSet::from_iter([FnSignature::KEY]));
    }

    #[test]
    fn wanted_with_a_lint_that_reads_none_adds_nothing() {
        let signatures = registered(Language::Rust, Reads::of::<FnSignature>());
        let silent = registered(Language::Rust, Reads::none());

        let wanted = wanted(&[&signatures, &silent], Language::Rust);

        assert_eq!(wanted, DecorationSet::from_iter([FnSignature::KEY]));
    }

    #[test]
    fn wanted_with_an_undeclared_lint_wants_every_type() {
        let lint = registered(Language::Rust, Reads::undeclared());

        let wanted = wanted(&[&lint], Language::Rust);

        assert_eq!(wanted, every_rust_key());
    }

    #[test]
    fn wanted_with_declared_reads_wants_their_union() {
        let signatures = registered(Language::Rust, Reads::of::<FnSignature>());
        let types = registered(
            Language::Rust,
            Reads::of::<ResolvedType>().and::<FnSignature>(),
        );

        let wanted = wanted(&[&signatures, &types], Language::Rust);

        assert_eq!(
            wanted,
            DecorationSet::from_iter([FnSignature::KEY, ResolvedType::KEY])
        );
    }

    #[test]
    fn wanted_with_one_undeclared_lint_wants_every_type() {
        let signatures = registered(Language::Rust, Reads::of::<FnSignature>());
        let undeclared = registered(Language::Rust, Reads::undeclared());

        let wanted = wanted(&[&signatures, &undeclared], Language::Rust);

        assert_eq!(wanted, every_rust_key());
    }

    #[test]
    fn wanted_with_only_lints_that_read_none_is_empty() {
        let lint = registered(Language::Rust, Reads::none());

        let wanted = wanted(&[&lint], Language::Rust);

        assert!(wanted.is_empty());
    }
}
