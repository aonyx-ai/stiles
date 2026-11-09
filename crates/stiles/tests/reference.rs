//! Tests for `stiles reference` against a real cargo and rustdoc
//!
//! The reader looks only at the names of the lint pass traits and `RuleId`.
//! The fixture declares its own, so it builds no stiles crate.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// The stand-ins for the stiles items that a rule crate names
const STUBS: &str = "pub struct RuleId;\nimpl RuleId { pub const fn new(_: &str) -> Self { Self } }\npub trait RustLintPass {}\npub trait TypeScriptLintPass {}\n";

/// A pass with the docs that a reference entry shows
const PASS: &str = "/// Flags a `TODO` comment\n///\n/// A comment ages badly.\n///\n/// # Examples\n///\n/// ```text\n/// // DON'T\n/// // TODO: later\n///\n/// // DO\n/// // nothing\n/// ```\npub struct TodoComment;\nimpl RustLintPass for TodoComment {}\nimpl TypeScriptLintPass for TodoComment {}\n";

/// The id constant of the fixture rule
const RULE_ID: &str = "pub const RULE_ID: RuleId = RuleId::new(\"lint.todo-comment\");\n";

/// Writes `contents` to `path`, with its parent directories
fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("the path should have a parent"))
        .expect("directories should be created");
    std::fs::write(path, contents).expect("the file should be written");
}

/// Returns a manifest of a package named `name`, ending in its dependencies
fn manifest(name: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n"
    )
}

/// Returns a workspace of a facade and a rule crate with the given source
fn project(rule_source: &str) -> TempDir {
    let project = tempfile::tempdir().expect("temporary directory should be created");
    let root = project.path();

    write(
        &root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"facade\", \"todo_comment\"]\nresolver = \"3\"\n",
    );
    write(
        &root.join("facade/Cargo.toml"),
        &format!(
            "{}todo_comment = {{ path = \"../todo_comment\" }}
",
            manifest("the-facade")
        ),
    );
    write(&root.join("facade/src/lib.rs"), "pub use todo_comment;\n");
    write(
        &root.join("todo_comment/Cargo.toml"),
        &manifest("todo_comment"),
    );
    write(&root.join("todo_comment/src/lib.rs"), rule_source);

    project
}

/// Returns the command that writes the page of `package` to `output`
fn reference(project: &TempDir, output: &Path, package: &str) -> Command {
    let mut command = Command::cargo_bin("stiles").expect("the binary should exist");
    command
        .arg("reference")
        .arg("--package")
        .arg(package)
        .arg("--output")
        .arg(output)
        .arg(project.path());
    command
}

/// Generates the page of a valid project and returns it
fn generated_page() -> String {
    let project = project(&format!("{STUBS}{RULE_ID}{PASS}"));
    let output = project.path().join("site/rules.html");

    reference(&project, &output, "the-facade")
        .assert()
        .success();

    std::fs::read_to_string(output).expect("the page should be written")
}

#[test]
fn reference_shows_the_example_of_the_rule() {
    let page = generated_page();

    assert!(page.contains("<code>// TODO: later</code>"), "{page}");
}

#[test]
fn reference_shows_the_id_of_the_rule() {
    let page = generated_page();

    assert!(page.contains("<details id=\"lint.todo-comment\""), "{page}");
}

#[test]
fn reference_shows_the_languages_from_the_traits() {
    let page = generated_page();

    assert!(page.contains("Rust, TypeScript"), "{page}");
}

#[test]
fn reference_with_an_unknown_package_names_it() {
    let project = project(&format!("{STUBS}{RULE_ID}{PASS}"));
    let output = project.path().join("rules.html");

    reference(&project, &output, "elsewhere")
        .assert()
        .failure()
        .stderr(predicate::str::contains("`elsewhere`"));
}

#[test]
fn reference_with_two_passes_names_the_crate() {
    let second = "pub struct Other;\nimpl RustLintPass for Other {}\n";
    let project = project(&format!("{STUBS}{RULE_ID}{PASS}{second}"));
    let output = project.path().join("rules.html");

    reference(&project, &output, "the-facade")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`todo_comment` must hold exactly one",
        ));
}
