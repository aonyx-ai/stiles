//! `stiles check --fix` against lints whose rules attach fixes

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

#[path = "support/fixing_plugin.rs"]
mod fixing_plugin;

use self::fixing_plugin::{Fixes, write_fixing_lint_package};

/// A manifest for a standalone package with no dependencies
const MANIFEST: &str = "[package]\nname = \"target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// A source whose comments end with a period and with nothing
const SOURCE: &str = "// Returns one.\npub fn one() -> u8 {\n    1\n}\n\n// Returns two\npub fn two() -> u8 {\n    2\n}\n";

/// A package to check, and the lint package that checks it
struct Project {
    target: TempDir,
    _lints: TempDir,
}

impl Project {
    /// Writes a package that holds `SOURCE`, checked by a lint with `fixes`
    fn new(name: &str, fixes: Fixes) -> Self {
        let target = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::write(target.path().join("Cargo.toml"), MANIFEST)
            .expect("manifest should be written");
        std::fs::create_dir(target.path().join("src")).expect("src should be created");
        std::fs::write(target.path().join("src").join("lib.rs"), SOURCE)
            .expect("source should be written");

        let lints = tempfile::tempdir().expect("temporary directory should be created");
        write_fixing_lint_package(lints.path(), name, fixes);
        let lint_path = toml::Value::from(lints.path().to_str().expect("the path should be UTF-8"));
        let config = target.path().join(".config");
        std::fs::create_dir_all(&config).expect("the config directory should be created");
        std::fs::write(
            config.join("stiles.toml"),
            format!("[[lints]]\npath = {lint_path}\n"),
        )
        .expect("configuration should be written");

        Self {
            target,
            _lints: lints,
        }
    }

    fn path(&self) -> &Path {
        self.target.path()
    }

    fn source(&self) -> String {
        std::fs::read_to_string(self.path().join("src").join("lib.rs"))
            .expect("the source should be readable")
    }
}

/// Returns a stiles with nothing of the caller's environment in it
fn stiles() -> Command {
    let mut command = Command::cargo_bin("stiles").expect("stiles binary should exist");
    command.env_remove("GH_TOKEN");
    command.env_remove("GITHUB_TOKEN");
    command.env_remove("STILES_GITHUB_API_URL");
    command.env("CARGO_TARGET_DIR", shared_build_directory());

    command
}

/// Returns the build directory the lint packages share
fn shared_build_directory() -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("plugin-builds");
    std::fs::create_dir_all(&directory).expect("the shared build directory should be created");

    directory
}

#[test]
fn check_fix_names_how_many_findings_it_fixed() {
    let project = Project::new("settling_lint_count", Fixes::Settle);

    stiles()
        .args(["check", "--fix"])
        .arg(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("fixed 1 finding in 1 file"));
}

#[test]
fn check_fix_with_fixes_that_settle_writes_the_fixed_text() {
    let project = Project::new("settling_lint_text", Fixes::Settle);

    stiles()
        .args(["check", "--fix"])
        .arg(project.path())
        .assert()
        .success();

    assert_eq!(
        project.source(),
        SOURCE.replace("// Returns one.", "// Returns one")
    );
}

#[test]
fn check_fix_with_rules_that_undo_each_other_fails() {
    let project = Project::new("fighting_lint_error", Fixes::Fight);

    stiles()
        .args(["check", "--fix"])
        .arg(project.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "the fixes of `custom.add-bang` and `custom.drop-bang` keep undoing each other",
        ));
}

#[test]
fn check_fix_with_rules_that_undo_each_other_offers_no_fix() {
    let project = Project::new("fighting_lint_help", Fixes::Fight);

    stiles()
        .args(["check", "--fix"])
        .arg(project.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains("`--fix` applies").not());
}

#[test]
fn check_fix_with_rules_that_undo_each_other_writes_nothing() {
    let project = Project::new("fighting_lint_text", Fixes::Fight);

    stiles()
        .args(["check", "--fix"])
        .arg(project.path())
        .assert()
        .code(1);

    assert_eq!(project.source(), SOURCE);
}

#[test]
fn check_without_fix_names_the_fixable_findings() {
    let project = Project::new("settling_lint_help", Fixes::Settle);

    stiles()
        .arg("check")
        .arg(project.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "help: 1 finding has a fix that `--fix` applies",
        ));
}

#[test]
fn check_without_fix_writes_nothing() {
    let project = Project::new("settling_lint_untouched", Fixes::Settle);

    stiles().arg("check").arg(project.path()).assert().code(1);

    assert_eq!(project.source(), SOURCE);
}
