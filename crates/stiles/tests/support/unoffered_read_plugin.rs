//! A lint package whose rule reads a decoration this stiles does not offer
//!
//! The rule declares a decoration the way a plugin built against another
//! stiles would. It names a key no provider computes, or a key a provider
//! offers at another layout. Stiles has to refuse it at load, by
//! the name of the decoration, rather than load it and leave the rule to
//! find nothing.

use std::path::Path;

/// The key of the decoration that no provider of this stiles offers
pub(crate) const UNOFFERED: &str = "stiles_rust::decorations::ExprType#0000000000000000";

/// What the lint declares that it reads
#[derive(Copy, Clone, Debug)]
pub(crate) enum Declared {
    /// A decoration under [`UNOFFERED`]
    Unoffered,
    /// A decoration under the key of `FnSignature`, at another layout
    AtAnotherLayout,
}

impl Declared {
    /// Returns the key the declared decoration claims, as Rust source
    fn key(self) -> String {
        match self {
            Self::Unoffered => format!("DecorationKey::new(\"{UNOFFERED}\")"),
            Self::AtAnotherLayout => {
                "<stiles_rust::decorations::FnSignature as Decoration>::KEY".to_owned()
            }
        }
    }
}

/// Writes a cdylib lint package whose one Rust lint reads what `declared` names
pub(crate) fn write_unoffered_read_lint_package(directory: &Path, name: &str, declared: Declared) {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let crates = std::fs::canonicalize(crates).expect("the crates directory should exist");
    let crates = crates.to_str().expect("the path should be UTF-8");

    std::fs::create_dir_all(directory.join("src")).expect("the package should be created");
    std::fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \
             \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n\
             stabby = \"72.1\"\nstiles-plugin = {{ path = '{crates}/stiles-plugin' }}\n\
             stiles-rust = {{ path = '{crates}/stiles-rust', default-features = false }}\n\
             stiles-types = {{ path = '{crates}/stiles-types' }}\n"
        ),
    )
    .expect("the manifest should be written");
    std::fs::write(
        directory.join("src").join("lib.rs"),
        format!(
            r#"use stiles_rust::RustLintPass;
use stiles_types::plugin::Reads;
use stiles_types::{{DecoratedNode, Decoration, DecorationKey, RuleId}};

/// A decoration that this stiles does not offer as declared
#[stabby::stabby]
pub struct Elsewhere(u8);

impl Decoration for Elsewhere {{
    const KEY: DecorationKey = {key};

    type Ref<'a> = Option<&'a Self>;

    fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {{
        node.decoration::<Self>()
    }}
}}

/// Reports nothing, and reads a decoration this stiles does not offer
pub struct ReadsElsewhere;

impl RustLintPass for ReadsElsewhere {{}}

impl stiles_plugin::DeclaresRules for ReadsElsewhere {{
    fn rules(&self) -> Vec<RuleId> {{
        vec![RuleId::new("custom.reads-elsewhere")]
    }}

    fn reads(&self) -> Reads {{
        Reads::of::<Elsewhere>()
    }}
}}

stiles_plugin::export_lints![rust: [ReadsElsewhere]];
"#,
            key = declared.key(),
        ),
    )
    .expect("the source should be written");
}
