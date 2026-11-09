use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Context as _;
use serde::Deserialize;
use stiles_types::Language;

/// The name of the type every rule id constant has
const RULE_ID_TYPE: &str = "RuleId";

/// The crate id rustdoc gives the crate that it documents
const LOCAL_CRATE: u32 = 0;

/// The part of a rustdoc JSON file that the reference page needs
///
/// Rustdoc's JSON format is unstable and carries a version that moves with
/// the toolchain. A reader for the full format would break on most bumps.
/// This one names only the fields it reads, and serde skips the rest. A
/// field that moves surfaces as a parse error that names it.
#[derive(Debug, Deserialize)]
pub(super) struct RustdocCrate {
    root: u32,
    index: BTreeMap<String, Item>,
    paths: BTreeMap<String, ItemPath>,
}

/// One item of the documented crate
#[derive(Debug, Deserialize)]
struct Item {
    crate_id: u32,
    visibility: serde_json::Value,
    docs: Option<String>,
    span: Option<Span>,
    inner: Inner,
}

/// The kinds of item this reader looks into
///
/// Rustdoc writes the kind as the only key of an object. Each kind is an
/// optional field here, so a kind that no field names leaves them all empty.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Inner {
    module: Option<Module>,
    #[serde(rename = "struct")]
    structure: Option<Structure>,
    #[serde(rename = "impl")]
    implementation: Option<Implementation>,
    #[serde(rename = "use")]
    import: Option<Import>,
    constant: Option<Constant>,
}

/// The items of a module
#[derive(Debug, Deserialize)]
struct Module {
    items: Vec<u32>,
}

/// The `impl` blocks of a struct
#[derive(Debug, Deserialize)]
struct Structure {
    impls: Vec<u32>,
}

/// The trait that an `impl` block implements, if it names one
#[derive(Debug, Deserialize)]
struct Implementation {
    #[serde(rename = "trait")]
    implemented: Option<NamedPath>,
}

/// A `pub use` item
#[derive(Debug, Deserialize)]
struct Import {
    id: Option<u32>,
    is_glob: bool,
}

/// A constant, as much of it as names its type
#[derive(Debug, Deserialize)]
struct Constant {
    #[serde(rename = "type")]
    declared: DeclaredType,
}

/// A type that a constant declares
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DeclaredType {
    resolved_path: Option<NamedPath>,
}

/// A path as the source wrote it
#[derive(Debug, Deserialize)]
struct NamedPath {
    path: String,
}

/// Where rustdoc found an item
#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
pub(super) struct Span {
    pub(super) filename: PathBuf,
    begin: (usize, usize),
    end: (usize, usize),
}

impl Span {
    /// Returns the lines of `source` that the span covers
    pub(super) fn lines_of(&self, source: &str) -> String {
        let Self { begin, end, .. } = self;
        let skipped = begin.0.saturating_sub(1);
        let taken = (end.0 + 1).saturating_sub(begin.0);

        source
            .lines()
            .skip(skipped)
            .take(taken)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// An entry of the table of every path that the crate refers to
#[derive(Debug, Deserialize)]
struct ItemPath {
    crate_id: u32,
    path: Vec<String>,
    kind: String,
}

/// What the rustdoc of one rule crate says about its rule
///
/// The id itself is absent. Rustdoc writes a constant that is not a literal
/// as `_`, so the span says where the source spells it.
#[derive(Clone, Eq, PartialEq, Debug)]
pub(super) struct RuleDeclaration {
    pub(super) docs: String,
    pub(super) languages: Vec<Language>,
    pub(super) id_span: Span,
}

impl RustdocCrate {
    /// Parses the content of a rustdoc JSON file
    ///
    /// # Errors
    ///
    /// Returns an error if `json` lacks a field this reader names.
    pub(super) fn parse(json: &str) -> anyhow::Result<Self> {
        serde_json::from_str(json).context("failed to read the rustdoc JSON format")
    }

    /// Returns the names of the crates that the crate root re-exports whole
    pub(super) fn reexported_crates(&self) -> Vec<String> {
        let Self { root, paths, .. } = self;
        let Some(Inner {
            module: Some(Module { items }),
            ..
        }) = self.item(*root).map(|item| &item.inner)
        else {
            return Vec::new();
        };

        items
            .iter()
            .filter_map(|id| self.item(*id)?.inner.import.as_ref())
            .filter(|import| !import.is_glob)
            .filter_map(|import| paths.get(&import.id?.to_string()))
            .filter_map(external_crate_name)
            .collect()
    }

    /// Returns what the crate declares about its one rule
    ///
    /// # Errors
    ///
    /// Returns an error unless the crate has exactly one public lint pass
    /// and exactly one public [`RULE_ID_TYPE`] constant.
    pub(super) fn rule_declaration(&self, crate_name: &str) -> anyhow::Result<RuleDeclaration> {
        let passes: Vec<(&Item, Vec<Language>)> = self
            .public_items()
            .filter_map(|item| Some((item, self.languages_of(item)?)))
            .collect();
        let ids: Vec<&Item> = self
            .public_items()
            .filter(|item| is_rule_id(item))
            .collect();

        let [(pass, languages)] = passes.as_slice() else {
            anyhow::bail!(
                "crate `{crate_name}` must hold exactly one public lint pass, and it holds {}",
                passes.len()
            );
        };
        let [id] = ids.as_slice() else {
            anyhow::bail!(
                "crate `{crate_name}` must hold exactly one public `{RULE_ID_TYPE}` constant, and \
                 it holds {}",
                ids.len()
            );
        };
        let id_span = id
            .span
            .clone()
            .with_context(|| format!("rustdoc gave no span for the rule id of `{crate_name}`"))?;

        Ok(RuleDeclaration {
            docs: pass.docs.clone().unwrap_or_default(),
            languages: languages.clone(),
            id_span,
        })
    }

    /// Returns the item with the given id
    fn item(&self, id: u32) -> Option<&Item> {
        self.index.get(&id.to_string())
    }

    /// Returns the public items that the crate itself defines
    fn public_items(&self) -> impl Iterator<Item = &Item> {
        self.index
            .values()
            .filter(|item| item.crate_id == LOCAL_CRATE && item.visibility == "public")
    }

    /// Returns the languages of a lint pass, or nothing for any other item
    fn languages_of(&self, item: &Item) -> Option<Vec<Language>> {
        let Structure { impls } = item.inner.structure.as_ref()?;
        let traits: Vec<&str> = impls
            .iter()
            .filter_map(|id| self.item(*id)?.inner.implementation.as_ref())
            .filter_map(|implementation| implementation.implemented.as_ref())
            .map(|implemented| last_segment(&implemented.path))
            .collect();

        let languages: Vec<Language> = [
            ("RustLintPass", Language::Rust),
            ("TypeScriptLintPass", Language::TypeScript),
        ]
        .into_iter()
        .filter(|(name, _)| traits.contains(name))
        .map(|(_, language)| language)
        .collect();

        match languages.as_slice() {
            [] => None,
            [_, ..] => Some(languages),
        }
    }
}

/// Returns the name of the crate that a path entry is the root of
///
/// A re-export of a whole crate points at a module of another crate with a
/// path of one segment.
fn external_crate_name(entry: &ItemPath) -> Option<String> {
    let ItemPath {
        crate_id,
        path,
        kind,
    } = entry;

    match (path.as_slice(), kind.as_str()) {
        ([name], "module") if *crate_id != LOCAL_CRATE => Some(name.clone()),
        _ => None,
    }
}

/// Returns whether an item is a constant of the rule id type
fn is_rule_id(item: &Item) -> bool {
    item.inner
        .constant
        .as_ref()
        .and_then(|constant| constant.declared.resolved_path.as_ref())
        .is_some_and(|resolved| last_segment(&resolved.path) == RULE_ID_TYPE)
}

/// Returns the last segment of a path
fn last_segment(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn assert_auto_traits<T: Send + Sync + Unpin>() {}

    fn item(inner: serde_json::Value) -> serde_json::Value {
        json!({
            "crate_id": 0,
            "visibility": "public",
            "docs": "Flags a thing",
            "span": { "filename": "src/lib.rs", "begin": [3, 1], "end": [3, 40] },
            "inner": inner,
            "unrelated": 1,
        })
    }

    fn pass(impls: &[u32]) -> serde_json::Value {
        item(json!({ "struct": { "kind": "unit", "impls": impls } }))
    }

    fn implementation(name: &str) -> serde_json::Value {
        item(json!({ "impl": { "trait": { "path": name } } }))
    }

    fn rule_id() -> serde_json::Value {
        item(json!({ "constant": {
            "type": { "resolved_path": { "path": "stiles_rust::RuleId" } },
            "const": { "expr": "_" },
        } }))
    }

    fn krate(index: serde_json::Value) -> RustdocCrate {
        serde_json::from_value(json!({ "root": 0, "index": index, "paths": {} }))
            .expect("the crate should parse")
    }

    fn one_rule() -> RustdocCrate {
        krate(json!({
            "1": pass(&[2]),
            "2": implementation("RustLintPass"),
            "3": rule_id(),
        }))
    }

    fn facade(paths: serde_json::Value) -> RustdocCrate {
        serde_json::from_value(json!({
            "root": 0,
            "index": {
                "0": item(json!({ "module": { "items": [1] } })),
                "1": item(json!({ "use": { "id": 7, "is_glob": false } })),
            },
            "paths": paths,
        }))
        .expect("the crate should parse")
    }

    #[test]
    fn lines_of_returns_the_lines_of_the_span() {
        let span = Span {
            filename: PathBuf::from("src/lib.rs"),
            begin: (2, 1),
            end: (3, 2),
        };

        let lines = span.lines_of("one\ntwo\nthree\nfour");

        assert_eq!(lines, "two\nthree");
    }

    #[test]
    fn parse_without_a_field_it_names_fails() {
        let error = RustdocCrate::parse(r#"{"root": 0}"#).expect_err("should fail");

        assert!(format!("{error:#}").contains("missing field"));
    }

    #[test]
    fn reexported_crates_names_a_whole_crate() {
        let facade = facade(json!({
            "7": { "crate_id": 3, "path": ["bool_param"], "kind": "module" },
        }));

        let crates = facade.reexported_crates();

        assert_eq!(crates, ["bool_param"]);
    }

    #[test]
    fn reexported_crates_skips_an_item_of_another_crate() {
        let facade = facade(json!({
            "7": { "crate_id": 3, "path": ["bool_param", "BoolParam"], "kind": "struct" },
        }));

        let crates = facade.reexported_crates();

        assert!(crates.is_empty());
    }

    #[test]
    fn rule_declaration_of_two_passes_names_the_crate() {
        let krate = krate(json!({
            "1": pass(&[3]),
            "2": pass(&[3]),
            "3": implementation("RustLintPass"),
            "4": rule_id(),
        }));

        let error = krate.rule_declaration("twice").expect_err("should fail");

        assert!(error.to_string().contains("`twice`"));
    }

    #[test]
    fn rule_declaration_reads_both_languages() {
        let krate = krate(json!({
            "1": pass(&[2, 3]),
            "2": implementation("stiles_typescript::TypeScriptLintPass"),
            "3": implementation("RustLintPass"),
            "4": rule_id(),
        }));

        let declaration = krate
            .rule_declaration("both")
            .expect("the crate should hold a rule");

        assert_eq!(
            declaration.languages,
            [Language::Rust, Language::TypeScript]
        );
    }

    #[test]
    fn rule_declaration_reads_the_docs_of_the_pass() {
        let declaration = one_rule()
            .rule_declaration("one")
            .expect("the crate should hold a rule");

        assert_eq!(declaration.docs, "Flags a thing");
    }

    #[test]
    fn rule_declaration_skips_a_struct_with_another_trait() {
        let krate = krate(json!({
            "1": pass(&[2]),
            "2": implementation("Default"),
            "3": rule_id(),
        }));

        let error = krate.rule_declaration("none").expect_err("should fail");

        assert!(error.to_string().contains("it holds 0"));
    }

    #[test]
    fn rule_declaration_without_an_id_names_the_type() {
        let krate = krate(json!({
            "1": pass(&[2]),
            "2": implementation("RustLintPass"),
        }));

        let error = krate.rule_declaration("none").expect_err("should fail");

        assert!(error.to_string().contains("`RuleId`"));
    }

    #[test]
    fn trait_auto_constant() {
        assert_auto_traits::<Constant>();
    }

    #[test]
    fn trait_auto_declared_type() {
        assert_auto_traits::<DeclaredType>();
    }

    #[test]
    fn trait_auto_implementation() {
        assert_auto_traits::<Implementation>();
    }

    #[test]
    fn trait_auto_import() {
        assert_auto_traits::<Import>();
    }

    #[test]
    fn trait_auto_inner() {
        assert_auto_traits::<Inner>();
    }

    #[test]
    fn trait_auto_item() {
        assert_auto_traits::<Item>();
    }

    #[test]
    fn trait_auto_item_path() {
        assert_auto_traits::<ItemPath>();
    }

    #[test]
    fn trait_auto_module() {
        assert_auto_traits::<Module>();
    }

    #[test]
    fn trait_auto_named_path() {
        assert_auto_traits::<NamedPath>();
    }

    #[test]
    fn trait_auto_rule_declaration() {
        assert_auto_traits::<RuleDeclaration>();
    }

    #[test]
    fn trait_auto_rustdoc_crate() {
        assert_auto_traits::<RustdocCrate>();
    }

    #[test]
    fn trait_auto_span() {
        assert_auto_traits::<Span>();
    }

    #[test]
    fn trait_auto_structure() {
        assert_auto_traits::<Structure>();
    }
}
