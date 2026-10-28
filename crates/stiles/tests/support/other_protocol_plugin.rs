use std::path::Path;

/// Writes a lint package whose declaration names a protocol stiles refuses
///
/// The package spells the head of the declaration out itself, as a
/// `#[repr(C)]` struct holding the version and nothing after it. That is
/// the least a plugin of any protocol exports. The package depends on no
/// stiles crate, so the library carries nothing past the version for
/// stiles to read.
///
/// A test uses this to prove two things at once. Stiles refuses such a
/// plugin with an error that names both protocols. Stiles also reaches
/// that refusal from the version alone, because no fingerprint follows
/// the version here.
///
/// # Panics
///
/// Panics if the package cannot be written.
pub fn write_other_protocol_lint_package(directory: &Path, name: &str) {
    std::fs::create_dir_all(directory.join("src")).expect("the package should be created");
    std::fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \
             \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n"
        ),
    )
    .expect("the manifest should be written");
    std::fs::write(
        directory.join("src").join("lib.rs"),
        r#"/// The head of a declaration, which every protocol starts with
#[repr(C)]
pub struct DeclarationHead {
    pub major: u32,
    pub minor: u32,
}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static stiles_plugin_declaration: DeclarationHead = DeclarationHead { major: 0, minor: 0 };
"#,
    )
    .expect("the source should be written");
}
