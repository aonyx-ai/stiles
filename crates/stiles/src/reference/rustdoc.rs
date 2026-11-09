use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;

use super::DocumentedRule;
use super::rustdoc_json::{RuleDeclaration, RustdocCrate};

/// The flags that make rustdoc write JSON instead of HTML
const JSON_OUTPUT: &str = "-Z unstable-options --output-format json";

/// The target kinds whose documentation is a library's
const LIBRARY_KINDS: [&str; 3] = ["lib", "rlib", "cdylib"];

/// The output of `cargo metadata`, as much of it as this reads
#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    target_directory: PathBuf,
    workspace_root: PathBuf,
}

/// One package of the dependency graph
#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    targets: Vec<Target>,
}

/// One build target of a package
#[derive(Debug, Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
}

impl Target {
    /// Returns the crate name of the target, if it is a library
    ///
    /// Rustdoc names a crate with underscores, and a manifest may use
    /// hyphens.
    fn library_crate(&self) -> Option<String> {
        let Self { name, kind } = self;

        kind.iter()
            .any(|kind| LIBRARY_KINDS.contains(&kind.as_str()))
            .then(|| name.replace('-', "_"))
    }
}

impl Metadata {
    /// Returns the crate name of the library of the named package
    fn crate_of_package(&self, name: &str) -> Option<String> {
        self.packages
            .iter()
            .find(|package| package.name == name)?
            .targets
            .iter()
            .find_map(Target::library_crate)
    }

    /// Returns the name of the package whose library is the named crate
    fn package_of_crate(&self, crate_name: &str) -> Option<&str> {
        self.packages
            .iter()
            .find(|package| {
                package
                    .targets
                    .iter()
                    .filter_map(Target::library_crate)
                    .any(|library| library == crate_name)
            })
            .map(|package| package.name.as_str())
    }

    /// Reads the rustdoc JSON that cargo wrote for a crate
    fn load_crate(&self, crate_name: &str) -> anyhow::Result<RustdocCrate> {
        let path = self
            .target_directory
            .join("doc")
            .join(format!("{crate_name}.json"));
        let json = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        RustdocCrate::parse(&json).with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Reads the rule of a crate from its rustdoc and its source
    fn load_rule(&self, crate_name: &str) -> anyhow::Result<DocumentedRule> {
        let RuleDeclaration {
            docs,
            languages,
            id_span,
        } = self
            .load_crate(crate_name)
            .context("failed to read the rustdoc of the crate")?
            .rule_declaration(crate_name)
            .context("failed to find the rule of the crate")?;

        let path = self.workspace_root.join(&id_span.filename);
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let lines = id_span.lines_of(&source);
        let id = parse_rule_id(&lines).with_context(|| {
            format!(
                "the rule id in {} is not a `RuleId::new(\"...\")` call",
                path.display()
            )
        })?;

        Ok(DocumentedRule {
            id: id.to_owned(),
            languages,
            docs,
        })
    }
}

/// Reads every rule that the named package re-exports, sorted by id
///
/// The package only lists the rules. Rustdoc writes a bare `use` item for a
/// re-export of another crate, with no docs, so each rule crate gets its own
/// run.
///
/// # Errors
///
/// Returns an error if cargo or rustdoc fails, if the package re-exports no
/// crate, or if a re-exported crate does not hold exactly one rule.
pub(super) fn load_rules(directory: &Path, package: &str) -> anyhow::Result<Vec<DocumentedRule>> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let metadata = load_metadata(&cargo, directory).context("failed to read the project")?;
    let facade = metadata
        .crate_of_package(package)
        .with_context(|| format!("no library package is named `{package}`"))?;

    run_rustdoc(&cargo, directory, &[package]).context("failed to document the package")?;
    let crates = metadata
        .load_crate(&facade)
        .context("failed to read the rustdoc of the package")?
        .reexported_crates();
    anyhow::ensure!(!crates.is_empty(), "`{package}` re-exports no crate");

    let packages = crates
        .iter()
        .map(|crate_name| {
            metadata
                .package_of_crate(crate_name)
                .with_context(|| format!("no package holds the crate `{crate_name}`"))
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .context("failed to find the rule crates")?;
    run_rustdoc(&cargo, directory, &packages).context("failed to document the rule crates")?;

    let mut rules = crates
        .iter()
        .map(|crate_name| {
            metadata
                .load_rule(crate_name)
                .with_context(|| format!("failed to document the crate `{crate_name}`"))
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .context("failed to read the rules")?;
    rules.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(rules)
}

/// Asks cargo for the packages of the project at `directory`
fn load_metadata(cargo: &OsStr, directory: &Path) -> anyhow::Result<Metadata> {
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1"])
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

/// Runs rustdoc with JSON output over the named packages
///
/// The JSON output is unstable, so it needs a nightly toolchain unless
/// `RUSTC_BOOTSTRAP` is set. Setting it lets a project on a stable toolchain
/// generate the page too. The flags of the caller are dropped, because one
/// such as `-D warnings` would fail the run over docs that are fine to
/// publish.
fn run_rustdoc(cargo: &OsStr, directory: &Path, packages: &[&str]) -> anyhow::Result<()> {
    let mut command = Command::new(cargo);
    command.args(["doc", "--no-deps"]);
    for package in packages {
        command.args(["--package", package]);
    }

    let output = command
        .env("RUSTC_BOOTSTRAP", "1")
        .env("RUSTDOCFLAGS", JSON_OUTPUT)
        .env_remove("CARGO_ENCODED_RUSTDOCFLAGS")
        .current_dir(directory)
        .output()
        .context("failed to run cargo doc")?;

    anyhow::ensure!(
        output.status.success(),
        "cargo doc failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );

    Ok(())
}

/// Returns the id that a `RuleId::new("...")` call spells
///
/// Rustfmt moves a long argument onto its own line, so whitespace may sit
/// around it. A string with an escape is not read, because its source text
/// is not its value.
fn parse_rule_id(source: &str) -> Option<&str> {
    let (_, rest) = source.split_once("RuleId::new(")?;
    let rest = rest.trim_start().strip_prefix('"')?;
    let (id, _) = rest.split_once('"')?;

    match id.contains('\\') {
        true => None,
        false => Some(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_auto_traits<T: Send + Sync + Unpin>() {}

    fn build_script() -> Target {
        Target {
            name: "build-script-build".to_owned(),
            kind: vec!["custom-build".to_owned()],
        }
    }

    fn library() -> Target {
        Target {
            name: "stiles-aonyx-rules".to_owned(),
            kind: vec!["lib".to_owned()],
        }
    }

    fn metadata() -> Metadata {
        Metadata {
            packages: vec![Package {
                name: "stiles-aonyx-rules".to_owned(),
                targets: vec![build_script(), library()],
            }],
            target_directory: PathBuf::from("/target"),
            workspace_root: PathBuf::from("/"),
        }
    }

    #[test]
    fn crate_of_package_uses_underscores() {
        let crate_name = metadata().crate_of_package("stiles-aonyx-rules");

        assert_eq!(crate_name.as_deref(), Some("stiles_aonyx_rules"));
    }

    #[test]
    fn crate_of_package_with_an_unknown_name_returns_none() {
        let crate_name = metadata().crate_of_package("elsewhere");

        assert!(crate_name.is_none());
    }

    #[test]
    fn library_crate_of_a_build_script_returns_none() {
        let library = build_script().library_crate();

        assert!(library.is_none());
    }

    #[test]
    fn package_of_crate_finds_the_hyphenated_package() {
        let metadata = metadata();

        let package = metadata.package_of_crate("stiles_aonyx_rules");

        assert_eq!(package, Some("stiles-aonyx-rules"));
    }

    #[test]
    fn parse_rule_id_on_its_own_line_returns_the_id() {
        let id = parse_rule_id("pub const A: RuleId = RuleId::new(\n    \"lint.a\",\n);");

        assert_eq!(id, Some("lint.a"));
    }

    #[test]
    fn parse_rule_id_with_a_constant_argument_returns_none() {
        let id = parse_rule_id("pub const A: RuleId = RuleId::new(NAME);");

        assert!(id.is_none());
    }

    #[test]
    fn parse_rule_id_with_a_literal_returns_the_id() {
        let id = parse_rule_id(r#"pub const A: RuleId = RuleId::new("lint.a");"#);

        assert_eq!(id, Some("lint.a"));
    }

    #[test]
    fn parse_rule_id_with_an_escape_returns_none() {
        let id = parse_rule_id(r#"pub const A: RuleId = RuleId::new("a\"b");"#);

        assert!(id.is_none());
    }

    #[test]
    fn trait_auto_metadata() {
        assert_auto_traits::<Metadata>();
    }

    #[test]
    fn trait_auto_package() {
        assert_auto_traits::<Package>();
    }

    #[test]
    fn trait_auto_target() {
        assert_auto_traits::<Target>();
    }
}
