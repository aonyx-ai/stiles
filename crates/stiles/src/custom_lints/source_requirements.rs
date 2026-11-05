use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;
use toml_edit::{DocumentMut, Item, TableLike};

use super::stiles_patch::STILES_CRATES;

/// The tables of a manifest that declare dependencies
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// The files of a checkout as they were before stiles rewrote them
///
/// A `[patch]` entry replaces a crate only with a version that meets the
/// requirement. Lints that require `=0.3.0` therefore ignore a source at
/// 0.4.0, and cargo builds them against crates.io instead. That happens
/// whenever the source moves ahead of the release the lints pin, as on a
/// release branch. Stiles sets each requirement of a stiles crate to the
/// version of the source, so the patch applies.
///
/// The checkout belongs to the cache, and a later run without a source
/// must find it as it was fetched. So stiles keeps every file it rewrote,
/// with the lockfile, and puts them back after the build.
#[must_use = "the checkout keeps the rewritten requirements until `restore` runs"]
pub(super) struct RewrittenRequirements {
    originals: Vec<(PathBuf, String)>,
}

impl RewrittenRequirements {
    /// Puts back every file as it was before the rewrite
    ///
    /// # Errors
    ///
    /// Returns an error if a file cannot be written. Stiles tries every
    /// file before it reports the first failure.
    pub(super) fn restore(self) -> anyhow::Result<()> {
        let mut first_failure = None;
        for (path, contents) in self.originals {
            if let Err(error) = std::fs::write(&path, contents) {
                first_failure.get_or_insert_with(|| {
                    anyhow::Error::new(error)
                        .context(format!("failed to restore {}", path.display()))
                });
            }
        }
        first_failure.map_or(Ok(()), Err)
    }
}

/// The output of `cargo metadata --no-deps`, as much of it as this reads
#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_root: PathBuf,
}

/// One package of the workspace
#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
    manifest_path: PathBuf,
}

/// Points every requirement of a stiles crate at the version of `source`
///
/// `directory` is a checkout that the cache holds, never a tree of the
/// project, because stiles writes its manifests.
///
/// # Errors
///
/// Returns an error if cargo cannot read the metadata of either
/// workspace, or if a manifest cannot be read, parsed, or written.
pub(super) fn rewrite(
    cargo: &OsStr,
    directory: &Path,
    source: &Path,
) -> anyhow::Result<RewrittenRequirements> {
    let versions: BTreeMap<String, String> = load_metadata(cargo, source)
        .context("failed to read the stiles source")?
        .packages
        .into_iter()
        .filter(|package| STILES_CRATES.contains(&package.name.as_str()))
        .map(|package| (package.name, package.version))
        .collect();

    let lints = load_metadata(cargo, directory).context("failed to read the lints")?;
    let mut manifests: BTreeSet<PathBuf> = lints
        .packages
        .into_iter()
        .map(|package| package.manifest_path)
        .collect();
    manifests.insert(lints.workspace_root.join("Cargo.toml"));

    let mut originals = Vec::new();
    let lockfile = lints.workspace_root.join("Cargo.lock");
    if lockfile.is_file() {
        let contents = std::fs::read_to_string(&lockfile)
            .with_context(|| format!("failed to read {}", lockfile.display()))?;
        originals.push((lockfile, contents));
    }

    for manifest in manifests {
        let contents = std::fs::read_to_string(&manifest)
            .with_context(|| format!("failed to read {}", manifest.display()))?;
        let Some(rewritten) = pin_requirements(&contents, &versions)
            .with_context(|| format!("failed to parse {}", manifest.display()))?
        else {
            continue;
        };
        std::fs::write(&manifest, rewritten)
            .with_context(|| format!("failed to write {}", manifest.display()))?;
        originals.push((manifest, contents));
    }

    Ok(RewrittenRequirements { originals })
}

/// Asks cargo for the packages of the workspace at `directory`
///
/// The question takes the manifests alone, so it reads no lockfile and
/// reaches no network.
fn load_metadata(cargo: &OsStr, directory: &Path) -> anyhow::Result<Metadata> {
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(directory)
        .output()
        .context("failed to run cargo metadata")?;

    anyhow::ensure!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );

    serde_json::from_slice(&output.stdout).context("failed to parse the output of cargo metadata")
}

/// Returns the manifest with each stiles requirement set to its version
///
/// `versions` maps a stiles crate to the version that the source holds.
/// Each dependency table counts: those of the package, those under a
/// `target`, and `[workspace.dependencies]`. An entry that inherits from
/// the workspace names no version and stays as it is. Returns nothing
/// when no entry changed.
///
/// # Errors
///
/// Returns an error if `manifest` is not TOML.
fn pin_requirements(
    manifest: &str,
    versions: &BTreeMap<String, String>,
) -> anyhow::Result<Option<String>> {
    let mut document: DocumentMut = manifest.parse().context("the manifest is not TOML")?;
    let root = document.as_table_mut();
    let mut changed = pin_tables(root, versions);

    if let Some(targets) = root.get_mut("target").and_then(Item::as_table_like_mut) {
        for (_, target) in targets.iter_mut() {
            if let Some(target) = target.as_table_like_mut() {
                changed |= pin_tables(target, versions);
            }
        }
    }

    if let Some(dependencies) = root
        .get_mut("workspace")
        .and_then(Item::as_table_like_mut)
        .and_then(|workspace| workspace.get_mut("dependencies"))
        .and_then(Item::as_table_like_mut)
    {
        changed |= pin_table(dependencies, versions);
    }

    Ok(changed.then(|| document.to_string()))
}

/// Pins the stiles requirements in each dependency table of one table
fn pin_tables(table: &mut dyn TableLike, versions: &BTreeMap<String, String>) -> bool {
    let mut changed = false;
    for name in DEPENDENCY_TABLES {
        if let Some(dependencies) = table.get_mut(name).and_then(Item::as_table_like_mut) {
            changed |= pin_table(dependencies, versions);
        }
    }
    changed
}

/// Pins the stiles requirements of one dependency table
///
/// An entry names its crate by its key, or by `package` when it renames
/// the crate.
fn pin_table(dependencies: &mut dyn TableLike, versions: &BTreeMap<String, String>) -> bool {
    let mut changed = false;
    for (key, entry) in dependencies.iter_mut() {
        let package = entry
            .get("package")
            .and_then(Item::as_str)
            .unwrap_or(key.get())
            .to_owned();
        let Some(version) = versions.get(&package) else {
            continue;
        };
        let requirement = toml_edit::value(format!("={version}"));

        if entry.is_str() {
            *entry = requirement;
            changed = true;
        } else if let Some(table) = entry.as_table_like_mut()
            && table.contains_key("version")
        {
            table.insert("version", requirement);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the versions of a source at 0.4.0
    fn source_at_0_4() -> BTreeMap<String, String> {
        STILES_CRATES
            .iter()
            .map(|name| ((*name).to_owned(), "0.4.0".to_owned()))
            .collect()
    }

    /// Returns the manifest after the rewrite, or the input when nothing changed
    fn pinned(manifest: &str) -> String {
        pin_requirements(manifest, &source_at_0_4())
            .expect("the manifest should parse")
            .unwrap_or_else(|| manifest.to_owned())
    }

    #[test]
    fn pin_requirements_keeps_the_other_keys_of_an_entry() {
        let manifest =
            "[dependencies]\nstiles-rust = { version = \"=0.3.0\", default-features = false }\n";

        let rewritten = pinned(manifest);

        assert_eq!(
            rewritten,
            "[dependencies]\nstiles-rust = { version = \"=0.4.0\", default-features = false }\n"
        );
    }

    #[test]
    fn pin_requirements_leaves_an_inherited_entry_alone() {
        let manifest = "[dependencies]\nstiles-plugin = { workspace = true }\n";

        let rewritten =
            pin_requirements(manifest, &source_at_0_4()).expect("the manifest should parse");

        assert!(rewritten.is_none());
    }

    #[test]
    fn pin_requirements_leaves_another_crate_alone() {
        let manifest = "[dependencies]\ntree-sitter = \"0.27\"\n";

        let rewritten =
            pin_requirements(manifest, &source_at_0_4()).expect("the manifest should parse");

        assert!(rewritten.is_none());
    }

    #[test]
    fn pin_requirements_pins_a_bare_string() {
        let manifest = "[dev-dependencies]\nstiles-testing = \"=0.3.0\"\n";

        let rewritten = pinned(manifest);

        assert_eq!(
            rewritten,
            "[dev-dependencies]\nstiles-testing = \"=0.4.0\"\n"
        );
    }

    #[test]
    fn pin_requirements_pins_a_renamed_crate_by_its_package() {
        let manifest =
            "[dependencies]\nplugin = { package = \"stiles-plugin\", version = \"=0.3.0\" }\n";

        let rewritten = pinned(manifest);

        assert!(rewritten.contains("version = \"=0.4.0\""), "{rewritten}");
    }

    #[test]
    fn pin_requirements_pins_a_target_table() {
        let manifest = "[target.'cfg(unix)'.dev-dependencies]\nstiles-testing = \"=0.3.0\"\n";

        let rewritten = pinned(manifest);

        assert!(
            rewritten.contains("stiles-testing = \"=0.4.0\""),
            "{rewritten}"
        );
    }

    #[test]
    fn pin_requirements_pins_the_workspace_dependencies() {
        let manifest = "[workspace.dependencies]\nstiles-types = \"=0.3.0\"\n";

        let rewritten = pinned(manifest);

        assert_eq!(
            rewritten,
            "[workspace.dependencies]\nstiles-types = \"=0.4.0\"\n"
        );
    }

    #[test]
    fn restore_writes_back_every_original() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let manifest = directory.path().join("Cargo.toml");
        std::fs::write(&manifest, "rewritten").expect("the manifest should be written");
        let rewritten = RewrittenRequirements {
            originals: vec![(manifest.clone(), "original".to_owned())],
        };

        rewritten.restore().expect("the restore should succeed");

        assert_eq!(
            std::fs::read_to_string(&manifest).expect("the manifest should be readable"),
            "original"
        );
    }

    #[test]
    fn trait_send_metadata() {
        fn assert_send<T: Send>() {}
        assert_send::<Metadata>();
    }

    #[test]
    fn trait_send_package() {
        fn assert_send<T: Send>() {}
        assert_send::<Package>();
    }

    #[test]
    fn trait_send_rewritten_requirements() {
        fn assert_send<T: Send>() {}
        assert_send::<RewrittenRequirements>();
    }

    #[test]
    fn trait_sync_metadata() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Metadata>();
    }

    #[test]
    fn trait_sync_package() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Package>();
    }

    #[test]
    fn trait_sync_rewritten_requirements() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RewrittenRequirements>();
    }

    #[test]
    fn trait_unpin_metadata() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Metadata>();
    }

    #[test]
    fn trait_unpin_package() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Package>();
    }

    #[test]
    fn trait_unpin_rewritten_requirements() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RewrittenRequirements>();
    }
}
