use std::path::Path;

use crate::fixture_repository::stiles_crates;

/// Writes a package whose declaration claims a protocol stiles refuses
///
/// The library it builds is a real one: stiles can open it and find the
/// declaration it looks for. Only the protocol version is wrong, which is
/// the first thing the handshake compares, so the refusal comes from the
/// handshake rather than from the loader failing earlier.
///
/// A test uses this to prove that a library stiles did not compile
/// itself is held to the same standard as one it did.
///
/// # Panics
///
/// Panics if the package cannot be written.
pub fn write_mismatched_lint_package(directory: &Path, name: &str) {
    let crates = stiles_crates();
    let crates = crates.to_str().expect("the path should be UTF-8");

    std::fs::create_dir_all(directory.join("src")).expect("the package should be created");
    std::fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \
             \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n\
             stiles-types = {{ path = \"{crates}/stiles-types\" }}\n"
        ),
    )
    .expect("the manifest should be written");
    std::fs::write(
        directory.join("src").join("lib.rs"),
        r#"use stiles_types::plugin::{AbiVersion, Loaded, Plugin, PluginDeclaration};

/// Exports nothing, because the handshake refuses this library first
extern "C" fn load() -> Loaded {
    Loaded::Ok(Plugin {
        lints: std::iter::empty().collect(),
    })
}

/// A protocol no stiles at this one reads, in either era
const REFUSED: AbiVersion = AbiVersion { major: stiles_types::plugin::ABI_VERSION.major + 1, minor: 0 };

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static stiles_plugin_declaration: PluginDeclaration = PluginDeclaration {
    abi_version: REFUSED,
    types_fingerprint: stiles_types::plugin::TYPES_FINGERPRINT,
    language_fingerprint: 0,
    load,
};
"#,
    )
    .expect("the source should be written");
}
