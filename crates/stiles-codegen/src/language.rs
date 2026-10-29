use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use crate::{Fingerprint, LanguageName, generate_visitor, merge_node_types};

/// The file a language crate includes for its generated lint pass trait
///
/// # Examples
///
/// A language crate includes the file from the output directory of its
/// build script:
///
/// ```ignore
/// include!(concat!(env!("OUT_DIR"), "/lint_pass.rs"));
/// ```
///
/// The constant names the same file:
///
/// ```
/// assert_eq!(stiles_codegen::LINT_PASS_FILE, "lint_pass.rs");
/// ```
pub const LINT_PASS_FILE: &str = "lint_pass.rs";

/// The file a language crate includes for the fingerprint of that trait
///
/// # Examples
///
/// A language crate includes the file from the output directory of its
/// build script:
///
/// ```ignore
/// include!(concat!(env!("OUT_DIR"), "/visitor_fingerprint.rs"));
/// ```
///
/// The constant names the same file:
///
/// ```
/// assert_eq!(stiles_codegen::FINGERPRINT_FILE, "visitor_fingerprint.rs");
/// ```
pub const FINGERPRINT_FILE: &str = "visitor_fingerprint.rs";

/// Generates a language crate's lint pass trait and its fingerprint
///
/// A language crate calls this from its build script. `node_types` names
/// the `node-types.json` of every grammar the language parses with,
/// relative to the crate's manifest directory. The function merges them,
/// generates `<language>LintPass` and its `dispatch` function into
/// [`LINT_PASS_FILE`], and writes `VISITOR_FINGERPRINT` into
/// [`FINGERPRINT_FILE`], both in `OUT_DIR`. It writes the directives that
/// tell cargo to rerun the build script when a grammar's file changes to
/// `cargo`. A build script passes its standard output, where cargo reads
/// them.
///
/// The fingerprint covers the generated code alone. It joins the plugin
/// handshake through the language crate's `LANGUAGE_FINGERPRINT`. A plugin
/// whose dispatch came from a different grammar would not crash. Its checks
/// would just never fire, and the handshake turns that into a refusal.
///
/// # Errors
///
/// Returns an error if cargo did not set `CARGO_MANIFEST_DIR` or `OUT_DIR`,
/// or if a `node-types.json` cannot be read or parsed. Returns an error too
/// if a generated file cannot be written, or if `cargo` refuses a directive.
///
/// # Examples
///
/// ```no_run
/// stiles_codegen::build_language("Rust", &["src/node-types.json"], &mut std::io::stdout())?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn build_language(
    language: &str,
    node_types: &[&str],
    cargo: &mut impl Write,
) -> io::Result<()> {
    let manifest_dir = cargo_directory("CARGO_MANIFEST_DIR")?;
    let out_dir = cargo_directory("OUT_DIR")?;

    let mut grammars = Vec::with_capacity(node_types.len());
    for relative in node_types {
        writeln!(cargo, "cargo::rerun-if-changed={relative}")?;
        let path = manifest_dir.join(relative);
        let text = fs::read_to_string(&path).map_err(|error| {
            io::Error::new(error.kind(), format!("read {}: {error}", path.display()))
        })?;
        grammars.push(text);
    }

    write_language(language, &grammars, &out_dir)
}

/// Writes the trait and the fingerprint for `language` into `out_dir`
///
/// This is [`build_language`] without the environment, so a test can run it
/// against a directory of its own.
fn write_language(language: &str, grammars: &[String], out_dir: &Path) -> io::Result<()> {
    let grammars: Vec<&str> = grammars.iter().map(String::as_str).collect();
    let node_types = merge_node_types(&grammars).map_err(invalid_data)?;
    let code = generate_visitor(&node_types, LanguageName::new(language)).map_err(invalid_data)?;

    fs::write(out_dir.join(LINT_PASS_FILE), &code)?;

    let mut fingerprint = Fingerprint::new();
    fingerprint.add_bytes(code.as_bytes());
    fs::write(
        out_dir.join(FINGERPRINT_FILE),
        format!("pub const VISITOR_FINGERPRINT: u64 = 0x{fingerprint};\n"),
    )
}

/// Returns the directory cargo names in the environment variable `name`
fn cargo_directory(name: &str) -> io::Result<PathBuf> {
    env::var_os(name).map(PathBuf::from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{name} is not set; call this from a build script"),
        )
    })
}

/// Wraps a `node-types.json` parse failure as an I/O error
fn invalid_data(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAMMAR: &str = r#"[{"type": "comment", "named": true}]"#;

    fn written(language: &str, grammars: &[&str]) -> (String, String) {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let grammars: Vec<String> = grammars.iter().map(|text| (*text).to_owned()).collect();

        write_language(language, &grammars, directory.path()).expect("should write");

        let code = fs::read_to_string(directory.path().join(LINT_PASS_FILE))
            .expect("the trait should be written");
        let fingerprint = fs::read_to_string(directory.path().join(FINGERPRINT_FILE))
            .expect("the fingerprint should be written");
        (code, fingerprint)
    }

    #[test]
    fn write_language_names_the_trait_after_the_language() {
        let (code, _) = written("Go", &[GRAMMAR]);

        assert!(code.contains("pub trait GoLintPass: Send + Sync {"));
    }

    #[test]
    fn write_language_with_invalid_json_returns_invalid_data() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        let error = write_language("Go", &["not json".to_owned()], directory.path())
            .expect_err("should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn write_language_with_two_grammars_generates_from_their_union() {
        let jsx = r#"[{"type": "jsx_element", "named": true}]"#;

        let (code, _) = written("Script", &[GRAMMAR, jsx]);

        let missing: Vec<&str> = ["fn check_comment(", "fn check_jsx_element("]
            .into_iter()
            .filter(|method| !code.contains(method))
            .collect();
        assert_eq!(missing, Vec::<&str>::new());
    }

    #[test]
    fn write_language_writes_a_fingerprint_constant() {
        let (_, fingerprint) = written("Go", &[GRAMMAR]);

        assert!(fingerprint.starts_with("pub const VISITOR_FINGERPRINT: u64 = 0x"));
    }

    #[test]
    fn write_language_writes_a_fingerprint_of_the_code() {
        let (_, one) = written("Go", &[GRAMMAR]);
        let (_, other) = written("Zig", &[GRAMMAR]);

        assert_ne!(one, other);
    }

    #[test]
    fn write_language_writes_a_method_per_node() {
        let (code, _) = written("Go", &[GRAMMAR]);

        assert!(code.contains("fn check_comment("));
    }
}
