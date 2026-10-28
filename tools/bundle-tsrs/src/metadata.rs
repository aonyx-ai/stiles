//! The tsrs workspace as `cargo metadata` describes it
//!
//! Cargo resolves the inheritance of each manifest. So every dependency here
//! carries its own version requirement and features, and the tool never
//! reads a `Cargo.toml` of tsrs by hand.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use serde::Deserialize;
use serde::Deserializer;

/// The packages of the tsrs workspace
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Metadata {
    packages: Vec<Package>,
}

/// One crate of tsrs
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Package {
    name: String,
    manifest_path: PathBuf,
    edition: String,
    license: Option<String>,
    rust_version: Option<String>,
    dependencies: Vec<Dependency>,
    targets: Vec<Target>,
}

/// One dependency of a tsrs crate, as its manifest declares it
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Dependency {
    name: String,
    req: String,
    kind: Option<String>,
    optional: Optionality,
    uses_default_features: DefaultFeatures,
    features: Vec<String>,
    target: Option<String>,
    rename: Option<String>,
}

/// Whether a feature of the crate switches the dependency on
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum Optionality {
    Required,
    Optional,
}

/// Whether the dependency builds with its default features
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum DefaultFeatures {
    On,
    Off,
}

/// One build target of a tsrs crate, such as its library or its build script
#[derive(Clone, Debug, Deserialize)]
struct Target {
    kind: Vec<String>,
    src_path: PathBuf,
}

impl<'de> Deserialize<'de> for Optionality {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(if bool::deserialize(deserializer)? {
            Self::Optional
        } else {
            Self::Required
        })
    }
}

impl<'de> Deserialize<'de> for DefaultFeatures {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(if bool::deserialize(deserializer)? {
            Self::On
        } else {
            Self::Off
        })
    }
}

impl Metadata {
    /// Reads the workspace whose root manifest is in `source`
    pub(crate) fn read(source: &Path) -> Result<Self> {
        let output = Command::new("cargo")
            .args([
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--manifest-path",
            ])
            .arg(source.join("Cargo.toml"))
            .output()
            .context("failed to run cargo metadata")?;
        ensure!(
            output.status.success(),
            "failed to read the tsrs workspace: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).context("failed to parse cargo metadata")
    }

    /// Returns `roots` and every tsrs crate they depend on, ordered by name
    ///
    /// Dev-dependencies stay out, because the bundle builds no tests.
    pub(crate) fn closure(&self, roots: &[String]) -> Result<Vec<Package>> {
        let by_name: BTreeMap<&str, &Package> = self
            .packages
            .iter()
            .map(|package| (package.name.as_str(), package))
            .collect();

        let mut seen = BTreeSet::new();
        let mut pending: Vec<&str> = roots.iter().map(String::as_str).collect();
        while let Some(name) = pending.pop() {
            let package = by_name
                .get(name)
                .with_context(|| format!("tsrs has no crate named {name}"))?;
            if !seen.insert(name) {
                continue;
            }
            for dependency in package.dependencies() {
                if by_name.contains_key(dependency.name()) {
                    pending.push(dependency.name());
                }
            }
        }

        Ok(seen.into_iter().map(|name| by_name[name].clone()).collect())
    }
}

impl Package {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn edition(&self) -> &str {
        &self.edition
    }

    pub(crate) fn license(&self) -> Option<&str> {
        self.license.as_deref()
    }

    pub(crate) fn rust_version(&self) -> Option<&str> {
        self.rust_version.as_deref()
    }

    /// Returns the directory that holds the manifest of the crate
    pub(crate) fn dir(&self) -> PathBuf {
        self.manifest_path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf)
    }

    /// Returns the directory that holds the sources of the library
    pub(crate) fn src_dir(&self) -> PathBuf {
        self.dir().join("src")
    }

    /// Checks that the library of the crate starts at `src/lib.rs`
    ///
    /// The bundle loads each crate as a module from that file, and the
    /// modules of the crate resolve against its directory.
    pub(crate) fn check_layout(&self) -> Result<()> {
        let expected = self.src_dir().join("lib.rs");
        let library = self
            .targets
            .iter()
            .find(|target| target.kind.iter().any(|kind| kind == "lib"))
            .with_context(|| format!("{} has no library", self.name))?;
        ensure!(
            library.src_path == expected,
            "the library of {} starts at {}, not at {}",
            self.name,
            library.src_path.display(),
            expected.display()
        );
        Ok(())
    }

    /// Returns the build script of the crate, if it has one
    pub(crate) fn build_script(&self) -> Option<&Path> {
        self.targets
            .iter()
            .find(|target| target.kind.iter().any(|kind| kind == "custom-build"))
            .map(|target| target.src_path.as_path())
    }

    /// Returns the dependencies that a build of the library needs
    ///
    /// The dev-dependencies are left out.
    pub(crate) fn dependencies(&self) -> impl Iterator<Item = &Dependency> {
        self.dependencies
            .iter()
            .filter(|dependency| dependency.kind() != Some("dev"))
    }
}

impl Dependency {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns the version requirement, as Cargo normalizes it, such as `^2`
    pub(crate) fn req(&self) -> &str {
        &self.req
    }

    /// Returns the kind of the dependency, or `None` for a normal one
    ///
    /// The kind of any other dependency is `build` or `dev`.
    pub(crate) fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }

    pub(crate) fn optional(&self) -> Optionality {
        self.optional
    }

    pub(crate) fn default_features(&self) -> DefaultFeatures {
        self.uses_default_features
    }

    pub(crate) fn features(&self) -> &[String] {
        &self.features
    }

    /// Returns the platform the dependency applies to, such as `cfg(unix)`
    pub(crate) fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    /// Returns the name the crate uses for the dependency
    ///
    /// It is `None` when the crate uses the package name.
    pub(crate) fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_reads_the_booleans_of_cargo_as_enums() {
        let json = r#"{"name":"libc","req":"^0.2","kind":null,"optional":false,
            "uses_default_features":false,"features":[],"target":"cfg(unix)","rename":null}"#;

        let dependency: Dependency = serde_json::from_str(json).expect("the JSON should parse");

        assert_eq!(
            (dependency.optional(), dependency.default_features()),
            (Optionality::Required, DefaultFeatures::Off)
        );
    }

    #[test]
    fn trait_auto_default_features() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<DefaultFeatures>();
    }

    #[test]
    fn trait_auto_dependency() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Dependency>();
    }

    #[test]
    fn trait_auto_metadata() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Metadata>();
    }

    #[test]
    fn trait_auto_optionality() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Optionality>();
    }

    #[test]
    fn trait_auto_package() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Package>();
    }

    #[test]
    fn trait_auto_target() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Target>();
    }
}
