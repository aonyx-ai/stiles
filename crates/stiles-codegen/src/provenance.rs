use std::io::Write;
use std::path::Path;
use std::{env, io};

/// The cfg a boundary crate reads to learn that cargo packaged it
///
/// # Examples
///
/// A boundary crate picks its provenance from the flag:
///
/// ```ignore
/// #[cfg(stiles_published)]
/// pub const PROVENANCE: Provenance = Provenance::PUBLISHED;
/// ```
///
/// The constant names the same flag:
///
/// ```
/// assert_eq!(stiles_codegen::PUBLISHED_CFG, "stiles_published");
/// ```
pub const PUBLISHED_CFG: &str = "stiles_published";

/// The file cargo writes into every crate that it packages
const VCS_INFO: &str = ".cargo_vcs_info.json";

/// Tells a boundary crate whether cargo packaged it
///
/// A crate of the plugin boundary calls this from its build script. Cargo
/// writes `.cargo_vcs_info.json` into every crate that it packages, and
/// nowhere else. A crate from crates.io therefore has the file, and a git
/// checkout, a path dependency, or a patch does not. The function declares
/// [`PUBLISHED_CFG`] to `cargo`, and sets it when the file is present. A
/// build script passes its standard output, where cargo reads the
/// directives.
///
/// The function writes no `rerun-if-changed` directive. Whether a
/// directory holds a packaged crate does not change while cargo builds
/// from it, and a directive for a file that is absent would rerun the
/// script on every build.
///
/// # Errors
///
/// Returns an error if cargo did not set `CARGO_MANIFEST_DIR`, or if
/// `cargo` refuses a directive.
///
/// # Examples
///
/// ```no_run
/// stiles_codegen::build_provenance(&mut std::io::stdout())?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn build_provenance(cargo: &mut impl Write) -> io::Result<()> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "cargo did not set CARGO_MANIFEST_DIR",
        )
    })?;

    write_provenance(Path::new(&manifest_dir), cargo)
}

/// Writes the provenance directives for the crate at `manifest_dir`
///
/// # Errors
///
/// Returns an error if `cargo` refuses a directive.
fn write_provenance(manifest_dir: &Path, cargo: &mut impl Write) -> io::Result<()> {
    writeln!(cargo, "cargo::rustc-check-cfg=cfg({PUBLISHED_CFG})")?;

    match manifest_dir.join(VCS_INFO).is_file() {
        true => writeln!(cargo, "cargo::rustc-cfg={PUBLISHED_CFG}"),
        false => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Returns what the function writes for the crate at `directory`
    fn directives(directory: &Path) -> String {
        let mut cargo = Vec::new();
        write_provenance(directory, &mut cargo).expect("a vector should accept every write");

        String::from_utf8(cargo).expect("the directives should be UTF-8")
    }

    #[test]
    fn write_provenance_in_a_packaged_crate_sets_the_flag() {
        let directory = tempfile::tempdir().expect("should create a directory");
        fs::write(directory.path().join(VCS_INFO), "{}").expect("should write the file");

        let written = directives(directory.path());

        assert!(
            written.contains("cargo::rustc-cfg=stiles_published\n"),
            "{written}"
        );
    }

    /// Cargo warns about a cfg that no directive declared, so the
    /// declaration comes whether or not the flag is set
    #[test]
    fn write_provenance_in_a_source_tree_declares_the_flag() {
        let directory = tempfile::tempdir().expect("should create a directory");

        let written = directives(directory.path());

        assert_eq!(written, "cargo::rustc-check-cfg=cfg(stiles_published)\n");
    }

    /// A directory under that name is no packaged crate, so it does not
    /// count
    #[test]
    fn write_provenance_with_a_directory_of_that_name_leaves_the_flag_unset() {
        let directory = tempfile::tempdir().expect("should create a directory");
        fs::create_dir(directory.path().join(VCS_INFO)).expect("should create the directory");

        let written = directives(directory.path());

        assert!(!written.contains("rustc-cfg="), "{written}");
    }
}
