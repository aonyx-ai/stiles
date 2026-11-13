//! A lint package whose rule reads a decoration it does not declare
//!
//! The rule declares that it reads nothing, and looks up the signature of
//! every function all the same. Stiles offers the decoration, so the
//! plugin loads. The lookup has to end the run by the name of the rule and
//! the decoration, rather than find nothing and leave the rule silent.

use std::path::Path;

/// The rule the lint reports, and the one the error names
pub(crate) const RULE: &str = "custom.reads-undeclared";

/// Writes a cdylib lint package whose one Rust lint reads an undeclared decoration
pub(crate) fn write_undeclared_read_lint_package(directory: &Path, name: &str) {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let crates = std::fs::canonicalize(crates).expect("the crates directory should exist");
    let crates = crates.to_str().expect("the path should be UTF-8");

    std::fs::create_dir_all(directory.join("src")).expect("the package should be created");
    std::fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \
             \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n\
             stiles-plugin = {{ path = '{crates}/stiles-plugin' }}\n\
             stiles-rust = {{ path = '{crates}/stiles-rust', default-features = false }}\n\
             stiles-types = {{ path = '{crates}/stiles-types' }}\n"
        ),
    )
    .expect("the manifest should be written");
    std::fs::write(
        directory.join("src").join("lib.rs"),
        format!(
            r#"use stiles_rust::RustLintPass;
use stiles_rust::decorations::FnSignature;
use stiles_types::plugin::Reads;
use stiles_types::{{DecoratedNode, Diagnostic, RuleId}};

/// Reports nothing, and reads the signature it did not declare
pub struct ReadsUndeclared;

impl RustLintPass for ReadsUndeclared {{
    fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{
        let _ = node.get::<FnSignature>();
        Vec::new()
    }}
}}

impl stiles_plugin::DeclaresRules for ReadsUndeclared {{
    fn rules(&self) -> Vec<RuleId> {{
        vec![RuleId::new("{RULE}")]
    }}

    fn reads(&self) -> Reads {{
        Reads::none()
    }}
}}

stiles_plugin::export_lints![rust: [ReadsUndeclared]];
"#,
        ),
    )
    .expect("the source should be written");
}
