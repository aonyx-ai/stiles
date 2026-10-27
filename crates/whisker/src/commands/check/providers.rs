use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use bon::bon;
use whisker_rust::RustDecorationProvider;
use whisker_types::{
    Coverage, CoverageGap, DecoratedTree, DecorationProvider, Language, ProviderName,
};
use whisker_typescript::TypeScriptDecorationProvider;

use super::grammar::Grammar;

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
pub(super) struct Providers {
    rust: Option<RustDecorationProvider>,
    typescript: HashMap<PathBuf, TypeScriptDecorationProvider>,
}

#[bon]
impl Providers {
    /// Loads the providers for every language among `files`, for a check
    /// of `path`
    ///
    /// # Errors
    ///
    /// Returns an error if a needed project cannot be loaded. The context
    /// names the language, because a mixed repository may lack only one of
    /// the project files. A TypeScript file with no `tsconfig.json` is not
    /// an error here: [`Providers::for_file`] reports its gap.
    #[builder]
    pub(super) fn load(path: &Path, files: &[PathBuf]) -> anyhow::Result<Self> {
        let mut has_rust = false;
        let mut typescript = HashMap::new();

        for file in files {
            let Some(grammar) = Grammar::of(file) else {
                continue;
            };
            match grammar.language() {
                Language::Rust => has_rust = true,
                Language::TypeScript => load_typescript_project(file, &mut typescript)
                    .context("failed to load the TypeScript project for analysis")?,
            }
        }

        let rust = match has_rust {
            true => Some(
                RustDecorationProvider::load(path)
                    .context("failed to load the target project for analysis")?,
            ),
            false => None,
        };

        Ok(Self { rust, typescript })
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

    fn decorate(&self, _tree: &DecoratedTree) -> anyhow::Result<Coverage> {
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
    projects: &mut HashMap<PathBuf, TypeScriptDecorationProvider>,
) -> anyhow::Result<()> {
    let Some(root) = TypeScriptDecorationProvider::project_root(file) else {
        return Ok(());
    };
    if projects.contains_key(&root) {
        return Ok(());
    }

    let provider = TypeScriptDecorationProvider::load(&root)
        .with_context(|| format!("load the TypeScript project at {}", root.display()))?;
    projects.insert(root, provider);
    Ok(())
}

#[cfg(test)]
mod tests {
    use whisker_types::UncoveredFile;

    use super::*;

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
            .files(&[page.clone(), index.clone()])
            .call()
            .expect("both projects should load");

        let found = providers.for_file(&page, Language::TypeScript);

        assert_eq!(providers.typescript.len(), 2);
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

        let uncovered = error
            .downcast_ref::<UncoveredFile>()
            .expect("should be an UncoveredFile");
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
            .call()
            .expect("a file without a project should not stop the load");

        assert!(providers.typescript.is_empty());
    }

    #[test]
    fn load_with_no_files_loads_nothing() {
        let directory = tree(&[]);

        let providers = Providers::load()
            .path(directory.path())
            .files(&[])
            .call()
            .expect("nothing should load");

        assert!(providers.rust.is_none());
        assert!(providers.typescript.is_empty());
    }

    #[test]
    fn load_with_only_typescript_files_needs_no_cargo_project() {
        let directory = tree(&[("tsconfig.json", "{}"), ("main.ts", "export {};\n")]);
        let file = directory.path().join("main.ts");

        let providers = Providers::load()
            .path(directory.path())
            .files(std::slice::from_ref(&file))
            .call()
            .expect("the TypeScript project should load");

        assert!(providers.rust.is_none());
        assert_eq!(providers.for_file(&file, Language::TypeScript).len(), 1);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<NoTypeScriptProject>();
        assert_send::<Providers>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<NoTypeScriptProject>();
        assert_sync::<Providers>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<NoTypeScriptProject>();
        assert_unpin::<Providers>();
    }
}
