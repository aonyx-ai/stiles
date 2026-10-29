use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;

/// The stiles crates a lint package can depend on
///
/// Stiles patches each one that a package names to the local source. The
/// package then builds against the stiles that loads it.
const STILES_CRATES: [&str; 5] = [
    "stiles-types",
    "stiles-plugin",
    "stiles-rust",
    "stiles-typescript",
    "stiles-testing",
];

/// The source id cargo gives crates.io
///
/// Cargo reports a dependency on crates.io under this id, whichever
/// protocol fetches the index.
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// One stiles crate that a lint package takes from a registry or a repository
///
/// A `[patch]` entry replaces a crate of one source. Cargo warns about an
/// entry that matches nothing in the graph, so stiles patches each crate
/// only under the source the package names it from.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub(super) struct StilesPatch {
    table: PatchTable,
    crate_name: &'static str,
}

impl StilesPatch {
    /// Returns the `--config` value that points the crate at `source`
    ///
    /// `source` is a checkout of stiles, with each crate under `crates`.
    pub(super) fn config(&self, source: &Path) -> String {
        let Self { table, crate_name } = self;
        let path = source.join("crates").join(crate_name);

        match table {
            PatchTable::CratesIo => {
                format!("patch.crates-io.{crate_name}.path='{}'", path.display())
            }
            PatchTable::Git(url) => {
                format!("patch.'{url}'.{crate_name}.path='{}'", path.display())
            }
        }
    }
}

/// The table of `[patch]` that replaces a crate of one source
///
/// A git repository is spelled the way the package spells it, because
/// cargo matches a patch to a dependency by that spelling.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
enum PatchTable {
    CratesIo,
    Git(String),
}

/// The output of `cargo metadata --no-deps`, as much of it as stiles reads
#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

/// One package of the workspace
#[derive(Debug, Deserialize)]
struct Package {
    dependencies: Vec<Dependency>,
}

/// One dependency a package declares
///
/// `name` is the name of the package depended on, even when the manifest
/// renames it. A path dependency has no `source`.
#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
    source: Option<String>,
}

/// Asks cargo which stiles crates the packages at `directory` depend on
///
/// The question takes the manifests alone, so it reads no lockfile and
/// reaches no network.
///
/// # Errors
///
/// Returns an error if cargo cannot be run, exits unsuccessfully, or
/// writes output that is not the metadata of a workspace.
pub(super) fn load_patches(
    cargo: &OsStr,
    directory: &Path,
) -> anyhow::Result<BTreeSet<StilesPatch>> {
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

    let stdout = String::from_utf8(output.stdout).context("cargo metadata wrote invalid UTF-8")?;

    plan_patches(&stdout).context("failed to read the output of cargo metadata")
}

/// Returns a patch for each stiles crate the packages take from a source
///
/// `metadata` is the output of `cargo metadata --no-deps`. A crate counts
/// once per source, however many packages of the workspace name it. A path
/// dependency already points where its manifest says. A crate of another
/// registry has no table that can patch it. Neither gets a patch.
///
/// # Errors
///
/// Returns an error if `metadata` is not the JSON that cargo writes.
fn plan_patches(metadata: &str) -> anyhow::Result<BTreeSet<StilesPatch>> {
    let Metadata { packages } =
        serde_json::from_str(metadata).context("failed to parse the metadata")?;

    let mut patches = BTreeSet::new();
    for Package { dependencies } in packages {
        for Dependency { name, source } in dependencies {
            let Some(crate_name) = STILES_CRATES.into_iter().find(|known| *known == name) else {
                continue;
            };
            let Some(table) = source.as_deref().and_then(patch_table) else {
                continue;
            };

            patches.insert(StilesPatch { table, crate_name });
        }
    }

    Ok(patches)
}

/// Returns the table of `[patch]` that replaces a crate of `source`
///
/// A git source id carries the revision after the URL, as in
/// `git+https://host/repo.git?rev=abc`. The table names the URL alone.
fn patch_table(source: &str) -> Option<PatchTable> {
    if source == CRATES_IO {
        return Some(PatchTable::CratesIo);
    }

    let url = source.strip_prefix("git+")?;
    let url = url.split(['?', '#']).next().unwrap_or(url);

    Some(PatchTable::Git(url.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STILES_GIT: &str = "https://github.com/aonyx-ai/stiles.git";

    /// Returns the metadata of one package with the given dependencies
    fn metadata(dependencies: &[(&str, Option<&str>)]) -> String {
        let dependencies: Vec<serde_json::Value> = dependencies
            .iter()
            .map(|(name, source)| serde_json::json!({ "name": name, "source": source }))
            .collect();

        serde_json::json!({ "packages": [{ "dependencies": dependencies }] }).to_string()
    }

    #[test]
    fn config_of_a_crates_io_patch_names_the_crates_io_table() {
        let patch = StilesPatch {
            table: PatchTable::CratesIo,
            crate_name: "stiles-types",
        };

        let config = patch.config(Path::new("/stiles"));

        assert_eq!(
            config,
            "patch.crates-io.stiles-types.path='/stiles/crates/stiles-types'"
        );
    }

    #[test]
    fn config_of_a_git_patch_quotes_the_url() {
        let patch = StilesPatch {
            table: PatchTable::Git(STILES_GIT.to_owned()),
            crate_name: "stiles-rust",
        };

        let config = patch.config(Path::new("/stiles"));

        assert_eq!(
            config,
            format!("patch.'{STILES_GIT}'.stiles-rust.path='/stiles/crates/stiles-rust'")
        );
    }

    #[test]
    fn plan_patches_counts_a_crate_once_per_source() {
        let metadata = serde_json::json!({
            "packages": [
                { "dependencies": [{ "name": "stiles-types", "source": CRATES_IO }] },
                { "dependencies": [{ "name": "stiles-types", "source": CRATES_IO }] },
            ]
        })
        .to_string();

        let patches = plan_patches(&metadata).expect("the metadata should parse");

        assert_eq!(patches.len(), 1);
    }

    #[test]
    fn plan_patches_of_a_crates_io_dependency_patches_crates_io() {
        let metadata = metadata(&[("stiles-plugin", Some(CRATES_IO))]);

        let patches = plan_patches(&metadata).expect("the metadata should parse");

        assert_eq!(
            patches,
            BTreeSet::from([StilesPatch {
                table: PatchTable::CratesIo,
                crate_name: "stiles-plugin",
            }])
        );
    }

    #[test]
    fn plan_patches_of_a_git_dependency_patches_its_url() {
        let source = format!("git+{STILES_GIT}?rev=4d401f8ca0950ad2112948eba5e9889e240bcd0e");
        let metadata = metadata(&[("stiles-rust", Some(&source))]);

        let patches = plan_patches(&metadata).expect("the metadata should parse");

        assert_eq!(
            patches,
            BTreeSet::from([StilesPatch {
                table: PatchTable::Git(STILES_GIT.to_owned()),
                crate_name: "stiles-rust",
            }])
        );
    }

    #[test]
    fn plan_patches_skips_a_crate_that_stiles_does_not_ship() {
        let metadata = metadata(&[("serde", Some(CRATES_IO))]);

        let patches = plan_patches(&metadata).expect("the metadata should parse");

        assert!(patches.is_empty());
    }

    #[test]
    fn plan_patches_skips_a_path_dependency() {
        let metadata = metadata(&[("stiles-types", None)]);

        let patches = plan_patches(&metadata).expect("the metadata should parse");

        assert!(patches.is_empty());
    }

    #[test]
    fn plan_patches_skips_another_registry() {
        let metadata = metadata(&[(
            "stiles-types",
            Some("registry+https://registry.example/index"),
        )]);

        let patches = plan_patches(&metadata).expect("the metadata should parse");

        assert!(patches.is_empty());
    }

    #[test]
    fn plan_patches_with_malformed_metadata_returns_error() {
        let metadata = "this is not json";

        let error = plan_patches(metadata).expect_err("should fail");

        assert!(
            format!("{error:#}").contains("failed to parse the metadata"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn trait_send_dependency() {
        fn assert_send<T: Send>() {}
        assert_send::<Dependency>();
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
    fn trait_send_patch_table() {
        fn assert_send<T: Send>() {}
        assert_send::<PatchTable>();
    }

    #[test]
    fn trait_send_stiles_patch() {
        fn assert_send<T: Send>() {}
        assert_send::<StilesPatch>();
    }

    #[test]
    fn trait_sync_dependency() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Dependency>();
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
    fn trait_sync_patch_table() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PatchTable>();
    }

    #[test]
    fn trait_sync_stiles_patch() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<StilesPatch>();
    }

    #[test]
    fn trait_unpin_dependency() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Dependency>();
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
    fn trait_unpin_patch_table() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PatchTable>();
    }

    #[test]
    fn trait_unpin_stiles_patch() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<StilesPatch>();
    }
}
