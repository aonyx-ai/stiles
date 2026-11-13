#![cfg(feature = "provider")]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use decoration_probes::{AnyhowBareTry, FunctionScopedImport, WildcardMatchArm};
use stiles_rust::decorations::{
    AdtFlags, AdtPath, EnumVariants, FnSignature, ImportSource, ItemExposure, PublicPath,
    Reexports, ResolvedType, ReturnMode, TypePathRef,
};
use stiles_rust::{RuleId, RustDecorationProvider, RustLintPassAdapter};
use stiles_types::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, Decoration as _, DecorationProvider,
    DecorationSet, Diagnostic, Language, LintPass, Severity, Span,
};

static PROVIDER: OnceLock<RustDecorationProvider> = OnceLock::new();

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_project")
}

/// Writes a Cargo project whose crate reaches two files that are not UTF-8
///
/// This function generates the project at run time, because a committed
/// file that is not valid UTF-8 rarely survives review or an editor
/// intact. The project is a separate package, so the bad module cannot
/// break analysis of any other crate.
///
/// The two files cover both routes on which rust-analyzer reads text.
/// It reads the module `src/bad.rs` when it builds the definition map.
/// It reads the [`include_str!`] target `src/notes.md` when it infers
/// the body that includes it.
fn not_utf8_project() -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("not_utf8_project");
    std::fs::create_dir_all(root.join("src")).expect("should create the fixture directories");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"not_utf8_project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("should write the fixture manifest");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub mod bad;\n\npub enum Color {\n    Red,\n    Green,\n}\n\npub fn match_on_enum(color: Color) {\n    let _notes = include_str!(\"notes.md\");\n    match color {\n        Color::Red => {}\n        _ => {}\n    }\n}\n",
    )
    .expect("should write the fixture crate root");
    std::fs::write(
        root.join("src/bad.rs"),
        b"pub fn f() -> u8 { b\"\xff\xfe\"[0] }\n",
    )
    .expect("should write the fixture module that is not UTF-8");
    std::fs::write(root.join("src/notes.md"), b"notes \xff\xfe\n")
        .expect("should write the fixture include_str target that is not UTF-8");

    root
}

/// Writes a Cargo project whose `use` paths rustc rejects
///
/// Rustc refuses a struct as an import prefix, and it refuses an unknown
/// crate, so neither import can sit in a fixture that compiles. This
/// function generates them instead. Rust-analyzer names the struct and
/// resolves the unknown crate to nothing, and the provider must report
/// both outcomes rather than fall silent.
fn rejected_imports_project() -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rejected_imports_project");
    std::fs::create_dir_all(root.join("src")).expect("should create the fixture directories");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"rejected_imports_project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("should write the fixture manifest");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct Loader;\n\nimpl Loader {\n    pub fn load() -> u8 {\n        0\n    }\n}\n\npub fn import_an_associated_function() -> u8 {\n    use Loader::load;\n\n    load()\n}\n\npub fn import_from_an_unknown_crate() -> u8 {\n    use nowhere::thing;\n\n    thing()\n}\n",
    )
    .expect("should write the fixture crate root");

    root
}

/// Writes a crate named `name`, for a test that changes its files
///
/// A test that changes a file needs a provider of its own, because the
/// change stays in the database. Each such test names its own directory, so
/// its Cargo run and its writes stay apart from every other test.
fn changed_files_project(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(root.join("src")).expect("should create the fixture directories");
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"
        ),
    )
    .expect("should write the fixture manifest");
    std::fs::write(root.join("src/lib.rs"), "pub fn one() -> u8 {\n    1\n}\n")
        .expect("should write the fixture crate root");

    root
}

/// Loads the fixture workspace once and lends it to every test
///
/// Loading runs `cargo metadata` and `cargo check` against the fixture. A
/// load per test made every test thread run Cargo against one directory at
/// the same time. The losers of that contention got a workspace whose
/// sysroot had not finished loading. Tests that only touch locally declared
/// types survived it; the ones resolving `std` types failed. Loading once
/// leaves nothing to contend over.
fn load_provider() -> &'static RustDecorationProvider {
    PROVIDER
        .get_or_init(|| RustDecorationProvider::load(&fixture_path()).expect("should load fixture"))
}

fn parse_source(source: String, file_path: PathBuf) -> DecoratedTree {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&stiles_rust::language()).unwrap();
    let tree = parser.parse(&source, None).unwrap();

    DecoratedTree::builder()
        .tree(tree)
        .language(Language::Rust)
        .source(source)
        .file(file_path)
        .build()
}

fn parse_fixture_file(relative: &str) -> DecoratedTree {
    let file_path = fixture_path().join(relative);
    let source = std::fs::read_to_string(&file_path).expect("should read fixture source");

    parse_source(source, file_path)
}

fn parse_and_decorate_fixture(provider: &RustDecorationProvider) -> DecoratedTree {
    decorate_fixture_file(provider, "src/lib.rs")
}

/// Parses and decorates the fixture file at `relative`, which must be covered
fn decorate_fixture_file(provider: &RustDecorationProvider, relative: &str) -> DecoratedTree {
    let mut decorated = parse_fixture_file(relative);

    let coverage = provider
        .decorate(&decorated)
        .expect("decorate should succeed");

    match coverage {
        Coverage::Covered(decorations) => decorated.merge_decorations(decorations),
        Coverage::NotCovered(gap) => panic!("fixture should be covered, got: {gap}"),
    }
    decorated
}

fn find_function_by_name<'a>(root: &DecoratedNode<'a>, name: &str) -> Option<DecoratedNode<'a>> {
    for child in root.named_children() {
        if child.kind() == "function_item"
            && let Some(name_node) = child.child_by_field_name("name")
            && name_node.text() == name
        {
            return Some(child);
        }
        if let Some(found) = find_function_by_name(&child, name) {
            return Some(found);
        }
    }
    None
}

fn find_signature_by_name<'a>(root: &DecoratedNode<'a>, name: &str) -> Option<DecoratedNode<'a>> {
    for child in root.named_children() {
        if child.kind() == "function_signature_item"
            && let Some(name_node) = child.child_by_field_name("name")
            && name_node.text() == name
        {
            return Some(child);
        }
        if let Some(found) = find_signature_by_name(&child, name) {
            return Some(found);
        }
    }
    None
}

fn find_first_node_of_kind<'a>(node: &DecoratedNode<'a>, kind: &str) -> Option<DecoratedNode<'a>> {
    if node.kind() == kind {
        return Some(*node);
    }
    for child in node.named_children() {
        if let Some(found) = find_first_node_of_kind(&child, kind) {
            return Some(found);
        }
    }
    None
}

/// Returns the [`ImportSource`] on the first `use` inside the named function
fn import_source_in(tree: &DecoratedTree, function: &str) -> ImportSource {
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, function).unwrap_or_else(|| panic!("should find {function}"));
    let import = find_first_node_of_kind(&func, "use_declaration")
        .unwrap_or_else(|| panic!("{function} should hold a `use`"));

    *import
        .decoration::<ImportSource>()
        .unwrap_or_else(|| panic!("the `use` in {function} should have an ImportSource"))
}

/// Returns the operand of the first `?` expression inside `node`
fn find_first_try_operand<'a>(node: &DecoratedNode<'a>) -> Option<DecoratedNode<'a>> {
    find_first_node_of_kind(node, "try_expression")?.named_child(0)
}

/// Returns the [`FnSignature`] on the named function of the fixture
fn signature_of(function: &str) -> FnSignature {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, function).unwrap_or_else(|| panic!("should find {function}"));

    func.decoration::<FnSignature>()
        .expect("should have FnSignature decoration")
        .clone()
}

/// Returns the text and the [`ResolvedType`] of the first `match`
/// scrutinee inside the named function of the fixture
fn scrutinee_of(function: &str) -> (String, ResolvedType) {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, function).unwrap_or_else(|| panic!("should find {function}"));
    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");
    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("scrutinee should have ResolvedType");
    (scrutinee.text().to_owned(), ty.clone())
}

/// Returns the text and the [`ResolvedType`] of the first `?` operand
/// inside the named function of the fixture
fn try_operand_of(function: &str) -> (String, ResolvedType) {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, function).unwrap_or_else(|| panic!("should find {function}"));
    let operand = find_first_try_operand(&func).expect("should find a `?` operand");

    let ty = operand
        .decoration::<ResolvedType>()
        .expect("`?` operand should have ResolvedType");
    (operand.text().to_owned(), ty.clone())
}

/// Returns the source text covered by each diagnostic raised inside `func`
///
/// A diagnostic belongs to `func` when `func` contains the whole diagnostic
/// span. An expectation therefore does not depend on positions in the
/// diagnostics list. The helper returns source text, not offsets, so the
/// fixture can grow without renumbering every expectation.
fn flagged_within<'a>(
    tree: &'a DecoratedTree,
    diagnostics: &[Diagnostic],
    func: &DecoratedNode<'_>,
) -> Vec<&'a str> {
    let span = func.span();
    let start = span.start();
    let end = span.end();

    diagnostics
        .iter()
        .map(Diagnostic::span)
        .filter(|span| start <= span.start() && span.end() <= end)
        .map(|span| &tree.source()[span.start()..span.end()])
        .collect()
}

/// Returns the source text covered by every diagnostic in the file
///
/// A per-function expectation cannot notice a rule that fires somewhere
/// else. A whole-file expectation fails on any change in the rule's reach.
fn flagged_in_file<'a>(tree: &'a DecoratedTree, diagnostics: &[Diagnostic]) -> Vec<&'a str> {
    diagnostics
        .iter()
        .map(Diagnostic::span)
        .map(|span| &tree.source()[span.start()..span.end()])
        .collect()
}

/// Returns the first `match` scrutinee inside `function`
fn first_scrutinee<'a>(tree: &'a DecoratedTree, function: &str) -> DecoratedNode<'a> {
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, function).unwrap_or_else(|| panic!("should find {function}"));
    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    match_expr
        .child_by_field_name("value")
        .expect("match should have value field")
}

/// Returns the rendered [`AdtPath`] on `node`, if it has one
fn rendered_adt_path(node: &DecoratedNode<'_>) -> Option<String> {
    node.decoration::<AdtPath>()
        .map(|adt| adt.path().to_string())
}

#[test]
fn adt_path_of_a_local_enum_scrutinee_is_its_definition() {
    let tree = parse_and_decorate_fixture(load_provider());

    let path = rendered_adt_path(&first_scrutinee(&tree, "match_on_enum"));

    assert_eq!(path.as_deref(), Some("sample_project::Color"));
}

/// An enum that a dependency re-exports gets the path of its definition
///
/// `facade` re-exports `tint_core::tint::Tint` as `facade::Tint`, and the
/// fixture names it only through the facade. The crate segment also shows
/// that the dash of `tint-core` becomes an underscore.
#[test]
fn adt_path_of_a_reexported_enum_is_its_definition() {
    let tree = parse_and_decorate_fixture(load_provider());

    let value = branch_value(
        &tree,
        "if_let_on_a_reexported_enum",
        BranchSite::LetCondition,
    );

    assert_eq!(
        rendered_adt_path(&value).as_deref(),
        Some("tint_core::tint::Tint")
    );
}

/// `match self` in a `&self` method names the enum behind the reference
#[test]
fn adt_path_of_a_reference_scrutinee_names_the_enum() {
    let tree = parse_and_decorate_fixture(load_provider());

    let path = rendered_adt_path(&first_scrutinee(&tree, "is_red"));

    assert_eq!(path.as_deref(), Some("sample_project::Color"));
}

#[test]
fn adt_path_of_a_reference_value_names_the_enum() {
    let tree = parse_and_decorate_fixture(load_provider());

    let value = branch_value(&tree, "predicate_on_a_reference", BranchSite::Call);

    assert_eq!(
        rendered_adt_path(&value).as_deref(),
        Some("sample_project::Color")
    );
}

#[test]
fn adt_path_of_a_struct_scrutinee_is_its_definition() {
    let tree = parse_and_decorate_fixture(load_provider());

    let path = rendered_adt_path(&first_scrutinee(&tree, "match_on_struct"));

    assert_eq!(path.as_deref(), Some("sample_project::Palette"));
}

#[test]
fn adt_path_of_an_integer_scrutinee_is_absent() {
    let tree = parse_and_decorate_fixture(load_provider());

    let path = rendered_adt_path(&first_scrutinee(&tree, "match_on_integer"));

    assert_eq!(path, None);
}

/// The bare-try probe fires on decorations the provider made
///
/// The probe's own tests attach decorations by hand, so only an end-to-end
/// run shows that real decorations satisfy a rule that reads them. The
/// expectation lists everything the probe flags in the function, so the
/// `.context(..)` call next to the bare `?` must stay unflagged.
#[test]
fn anyhow_bare_try_flags_a_bare_try_in_an_anyhow_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(\"anyhow_bare.txt\")?"]
    );
}

/// An `async fn` that returns `anyhow::Result` is still an anyhow function
///
/// The signature output is the opaque future, so a provider that reads the
/// signature verbatim reports no error type and misses every `async fn`.
#[test]
fn anyhow_bare_try_flags_a_bare_try_in_an_async_anyhow_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_anyhow_result_async")
        .expect("should find returns_anyhow_result_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(\"async_anyhow_bare.txt\")?"]
    );
}

/// An `async fn` in an impl block is reached the same way a free one is
///
/// The trait declares the method with an `impl Future` return type, a
/// different shape from the `async fn` that implements it. The awaited
/// type must come from the implementation, not the declaration.
#[test]
fn anyhow_bare_try_flags_a_bare_try_in_an_async_trait_method() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "load_async").expect("should find load_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(&self.path)?"]
    );
}

#[test]
fn anyhow_bare_try_flags_a_bare_try_on_a_method_call() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, "try_on_method_call").expect("should find try_on_method_call");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["loader.load()?"]
    );
}

/// Only the anyhow function is flagged, out of four that all render `Error`
///
/// `syn::Result`, `std::fmt::Result`, `std::io::Result`, and
/// `anyhow::Result` all render their `E` as `Error`, so a rule that reads
/// the rendering flags all four. The `syn` case also covers an `Error`
/// whose definition path differs from its public re-export path.
#[test]
fn anyhow_bare_try_flags_only_the_anyhow_function_among_lookalike_errors() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    let flagged = [
        "returns_anyhow_result",
        "returns_syn_result",
        "fmt",
        "returns_io_result",
    ]
    .map(|name| {
        let func =
            find_function_by_name(&root, name).unwrap_or_else(|| panic!("should find {name}"));
        flagged_within(&tree, &diagnostics, &func)
    });
    assert_eq!(
        flagged,
        [
            vec!["std::fs::read_to_string(\"anyhow_bare.txt\")?"],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ]
    );
}

/// An `async fn` that returns `std::io::Result` is not an anyhow function
///
/// The async projection must identify the awaited error type, not just
/// make every `async fn` visible to the rule.
#[test]
fn anyhow_bare_try_ignores_a_bare_try_in_an_async_io_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_io_result_async")
        .expect("should find returns_io_result_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A function that returns `std::io::Result` is outside this rule
///
/// `std::io::Result<()>` renders as `Result<(), Error>`, so a rule that
/// reads the rendering cannot tell this `Error` from anyhow's.
#[test]
fn anyhow_bare_try_ignores_a_bare_try_in_an_io_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, "returns_io_result").expect("should find returns_io_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// `Box<dyn Error>` occupies the `E` slot as an ADT, and it is not anyhow's
///
/// The `E` resolves to `Box`, defined in `alloc`, so the rule sees a named
/// error type with the wrong path.
#[test]
fn anyhow_bare_try_ignores_a_boxed_error() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_boxed_error")
        .expect("should find returns_boxed_error");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// An error type the signature leaves open cannot be anyhow's
///
/// A caller may instantiate `E` as `anyhow::Error`, but the body must
/// compile for every `E` the bounds admit. `.context(..)` is not available
/// there, so the rule reports nothing.
#[test]
fn anyhow_bare_try_ignores_a_generic_error() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_generic_error")
        .expect("should find returns_generic_error");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A crate's own `Error` type must not match anyhow's
///
/// This shape produced most of the false positives when the rule first ran
/// over stiles's own source.
#[test]
fn anyhow_bare_try_ignores_a_local_error_type() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_local_error_result")
        .expect("should find returns_local_error_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A `?` in a closure belongs to the closure, not to the function around it
///
/// The closure returns `std::io::Result`, so `.context(..)` on its `?`
/// would not compile. The function's own `read()?` is a genuine hit, so a
/// removed barrier fails the expectation with an extra entry.
#[test]
fn anyhow_bare_try_ignores_a_try_inside_a_closure() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "closure_returning_io_result")
        .expect("should find closure_returning_io_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(flagged_within(&tree, &diagnostics, &func), vec!["read()?"]);
}

/// Every place the rule fires in the fixture, listed once
///
/// Per-function expectations once stayed green while the rule fired on a
/// `std::io::Result` function nothing asserted about. The whole-file list
/// pins the rule's reach.
#[test]
fn anyhow_bare_try_over_the_whole_file_reports_only_anyhow_bodies() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(
        flagged_in_file(&tree, &diagnostics),
        vec![
            "std::fs::read_to_string(\"anyhow_bare.txt\")?",
            "loader.load()?",
            "std::fs::read_to_string(\"async_anyhow_bare.txt\")?",
            "std::fs::read_to_string(&self.path)?",
            "read()?",
        ]
    );
}

#[test]
fn branch_value_of_equality_on_integers_names_no_variant() {
    let tree = parse_and_decorate_fixture(load_provider());

    assert!(branch_node(&tree, "equality_on_integers", BranchSite::Comparison).is_empty());
}

#[test]
fn branch_value_of_foreign_non_exhaustive_enum_is_marked() {
    let tree = parse_and_decorate_fixture(load_provider());
    let root = tree.root_node();
    let func = find_function_by_name(&root, "if_let_on_a_foreign_non_exhaustive_enum")
        .expect("should find function");
    let condition = find_first_node_of_kind(&func, "let_condition").expect("should find let");
    let value = condition.child_by_field_name("value").expect("value field");

    let flags = value.decoration::<AdtFlags>().expect("should have flags");

    assert!(flags.non_exhaustive_external());
}

#[test]
fn branch_value_of_if_let_names_the_variants() {
    let tree = parse_and_decorate_fixture(load_provider());

    assert_eq!(
        branch_node(&tree, "if_let_on_enum", BranchSite::LetCondition),
        COLORS
    );
}

#[test]
fn branch_value_of_let_else_names_the_variants() {
    let tree = parse_and_decorate_fixture(load_provider());

    assert_eq!(
        branch_node(&tree, "let_else_on_enum", BranchSite::LetDeclaration),
        COLORS
    );
}

#[test]
fn branch_value_of_matches_on_a_call_names_the_variants() {
    let tree = parse_and_decorate_fixture(load_provider());

    assert_eq!(
        branch_node(&tree, "matches_on_a_call", BranchSite::Matches),
        COLORS
    );
}

#[test]
fn branch_value_of_matches_on_a_name_names_the_variants() {
    let tree = parse_and_decorate_fixture(load_provider());

    assert_eq!(
        branch_node(&tree, "matches_on_a_name", BranchSite::Matches),
        COLORS
    );
}

#[test]
fn branch_value_of_option_is_marked_as_option() {
    let tree = parse_and_decorate_fixture(load_provider());
    let root = tree.root_node();
    let func = find_function_by_name(&root, "if_let_on_option").expect("should find function");
    let condition = find_first_node_of_kind(&func, "let_condition").expect("should find let");
    let value = condition.child_by_field_name("value").expect("value field");

    let resolved = value.decoration::<ResolvedType>().expect("should be typed");

    assert!(resolved.is_option());
}

#[test]
fn branch_value_of_plain_let_has_no_decoration() {
    let tree = parse_and_decorate_fixture(load_provider());

    assert!(branch_node(&tree, "plain_let_on_enum", BranchSite::LetDeclaration).is_empty());
}

#[test]
fn branch_value_of_predicate_looks_through_a_reference() {
    let tree = parse_and_decorate_fixture(load_provider());

    assert_eq!(
        branch_node(&tree, "predicate_on_a_reference", BranchSite::Call),
        COLORS
    );
}

#[test]
fn branch_value_of_predicate_on_a_string_names_no_variant() {
    let tree = parse_and_decorate_fixture(load_provider());

    assert!(branch_node(&tree, "predicate_on_a_string", BranchSite::Call).is_empty());
}

/// Checks that a module that is not UTF-8 leaves its siblings covered
///
/// Before the load repaired such files, rust-analyzer panicked over the
/// missing file text and aborted the whole test binary. The fixture
/// holds a bad module and a bad [`include_str!`] target, so a repair
/// narrowed to Rust source also fails this test.
#[test]
fn change_files_covers_the_changed_text() {
    let root = changed_files_project("changed_text_project");
    let mut provider = RustDecorationProvider::load(&root).expect("should load the project");
    let file_path = root.join("src/lib.rs");
    let source = "pub fn one() -> u8 {\n    2\n}\n".to_owned();
    let tree = parse_source(source.clone(), file_path.clone());

    provider
        .change_files([(file_path, source)])
        .expect("the change should apply");
    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => {}
        Coverage::NotCovered(gap) => panic!("the changed text should be covered, got: {gap}"),
    }
}

#[test]
fn change_files_signs_a_function_the_change_added() {
    let root = changed_files_project("added_function_project");
    let mut provider = RustDecorationProvider::load(&root).expect("should load the project");
    let file_path = root.join("src/lib.rs");
    let source = "pub fn one() -> u8 {\n    1\n}\n\npub fn two() -> u16 {\n    2\n}\n".to_owned();
    let mut tree = parse_source(source.clone(), file_path.clone());

    provider
        .change_files([(file_path, source)])
        .expect("the change should apply");
    let Coverage::Covered(decorations) = provider.decorate(&tree).expect("should decorate") else {
        panic!("the changed text should be covered");
    };
    tree.merge_decorations(decorations);

    let root_node = tree.root_node();
    let two = find_function_by_name(&root_node, "two").expect("should find two");
    let display = two
        .decoration::<FnSignature>()
        .and_then(FnSignature::return_type)
        .map(ResolvedType::display);
    assert_eq!(display, Some("u16"));
}

#[test]
fn decorate_with_a_module_that_is_not_utf8_covers_its_siblings() {
    let root = not_utf8_project();
    let provider = RustDecorationProvider::load(&root).expect("should load the fixture");
    let file_path = root.join("src/lib.rs");
    let source = std::fs::read_to_string(&file_path).expect("should read the fixture crate root");
    let mut tree = parse_source(source, file_path);

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(decorations) => tree.merge_decorations(decorations),
        Coverage::NotCovered(gap) => panic!("the crate root should be covered, got: {gap}"),
    }
    let root_node = tree.root_node();
    let func =
        find_function_by_name(&root_node, "match_on_enum").expect("should find the function");
    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");
    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");
    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("scrutinee should have ResolvedType");
    assert!(ty.is_enum(), "Color should be an enum");
}

/// Checks that code under `#[cfg(test)]` resolves like any other code
///
/// Cargo leaves `test` out of the cfg options by default. Without it,
/// every test block resolves to nothing and stiles reports the file
/// clean.
#[test]
fn decorate_with_code_under_cfg_test_resolves_types() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_enum_under_cfg_test")
        .expect("should find match_on_enum_under_cfg_test");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");
    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");
    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("a scrutinee under cfg(test) should have ResolvedType");
    assert!(ty.is_enum(), "Color should be an enum");
}

/// Returns the [`Reexports`] on the imported path `leaf` in `src/reexports.rs`
///
/// The `use` lines of that file each name their items once, so the text of
/// a path that a `use` imports picks out its node. Only a node whose parent
/// is a `use`, a list, or a rename is an imported path: the `Shape` inside
/// `facade_core::Shape::Circle` is a qualifier.
fn decorated_reexports_of(tree: &DecoratedTree, leaf: &str) -> Reexports {
    fn find<'a>(node: &DecoratedNode<'a>, leaf: &str) -> Option<DecoratedNode<'a>> {
        let parent_kind = node.parent().map(|parent| parent.kind());
        let imported = match parent_kind {
            Some("use_declaration" | "use_list" | "use_as_clause") => true,
            Some(_) | None => false,
        };
        if imported && node.text() == leaf {
            return Some(*node);
        }
        node.named_children()
            .iter()
            .find_map(|child| find(child, leaf))
    }

    let node = find(&tree.root_node(), leaf)
        .unwrap_or_else(|| panic!("src/reexports.rs should import {leaf}"));

    node.decoration::<Reexports>()
        .cloned()
        .unwrap_or_else(|| panic!("{leaf} should carry Reexports"))
}

fn rendered(reexports: &Reexports) -> Vec<String> {
    reexports.paths().iter().map(ToString::to_string).collect()
}

#[test]
fn decorate_with_file_outside_workspace_reports_outside_workspace() {
    let provider = load_provider();
    let tree = parse_source(
        "fn main() {}\n".to_string(),
        PathBuf::from("/tmp/not_a_real_file.rs"),
    );

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => panic!("a file outside the loaded root must not be covered"),
        Coverage::NotCovered(CoverageGap::OutsideRoot { .. }) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

/// Checks that a generated file under the root reports
/// [`CoverageGap::Unreachable`]
///
/// Build scripts create files under `target` that rust-analyzer never
/// interned. The [`CoverageGap::OutsideRoot`] verdict would print a
/// path inside the root next to a message that claims it is outside.
#[test]
fn decorate_with_generated_file_under_root_reports_unreachable() {
    let provider = load_provider();
    let tree = parse_source(
        "fn main() {}\n".to_string(),
        fixture_path().join("target/debug/build/generated_out/out/generated.rs"),
    );

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => panic!("a file no crate reaches must not be covered"),
        Coverage::NotCovered(CoverageGap::Unreachable { .. }) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

/// A qualifier reports `Other` when it names another item, and
/// `Unresolved` when it names nothing
#[test]
fn decorate_with_imports_rustc_rejects_reports_other_and_unresolved() {
    let root = rejected_imports_project();
    let provider = RustDecorationProvider::load(&root).expect("should load the fixture");
    let file_path = root.join("src/lib.rs");
    let source = std::fs::read_to_string(&file_path).expect("should read the fixture crate root");
    let mut tree = parse_source(source, file_path);

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(decorations) => tree.merge_decorations(decorations),
        Coverage::NotCovered(gap) => panic!("the crate root should be covered, got: {gap}"),
    }
    let sources = [
        "import_an_associated_function",
        "import_from_an_unknown_crate",
    ]
    .map(|function| import_source_in(&tree, function));
    assert_eq!(sources, [ImportSource::Other, ImportSource::Unresolved]);
}

#[test]
fn decorate_with_modified_source_reports_stale_source() {
    let provider = load_provider();
    let file_path = fixture_path().join("src/lib.rs");
    let source = std::fs::read_to_string(&file_path).expect("should read fixture source");
    let tree = parse_source(
        format!("{source}\npub fn added_after_load() {{}}\n"),
        file_path,
    );

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => panic!("text the toolchain never saw must not be covered"),
        Coverage::NotCovered(CoverageGap::StaleSource) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

#[test]
fn decorate_with_orphan_file_reports_unreachable() {
    let provider = load_provider();
    let tree = parse_fixture_file("orphan.rs");

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => panic!("a file no crate reaches must not be covered"),
        Coverage::NotCovered(CoverageGap::Unreachable { .. }) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

#[test]
fn every_function_item_has_fn_signature() {
    let (fn_count, sig_count) = fixture_signature_counts();

    assert_eq!(
        fn_count, sig_count,
        "every function should have a FnSignature decoration"
    );
}

#[test]
fn fixture_has_function_items() {
    let (fn_count, _) = fixture_signature_counts();

    assert!(fn_count > 0, "fixture should have functions");
}

/// A diagnostic that starts inside `func` but ends past it does not count
///
/// A lint reports one node, and nodes nest, so no fixture produces this
/// shape. The test builds the span by hand.
#[test]
fn flagged_within_with_a_span_past_the_function_end_returns_nothing() {
    let tree = parse_fixture_file("src/lib.rs");
    let root = tree.root_node();
    let func = find_function_by_name(&root, "match_on_enum").expect("should find match_on_enum");
    let span = func.span();
    let start = span.start();
    let end = span.end();
    let diagnostics = vec![
        Diagnostic::builder()
            .rule_id(RuleId::new("lint.test"))
            .severity(Severity::Warn)
            .message("a span that leaves the function".to_string())
            .span(
                Span::builder()
                    .file(tree.file().to_path_buf())
                    .start(start)
                    .end(end + 1)
                    .build(),
            )
            .build(),
    ];

    let flagged = flagged_within(&tree, &diagnostics, &func);

    assert!(flagged.is_empty(), "unexpected matches: {flagged:?}");
}

#[test]
fn fn_signature_on_anyhow_result_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");

    let sig = func
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    let ret = sig.return_type().expect("should have return type");
    assert!(
        ret.is_result(),
        "return type should be Result, got display: {}",
        ret.display()
    );
}

#[test]
fn fn_signature_on_async_function_reports_the_awaited_error() {
    let sig = signature_of("returns_anyhow_result_async");

    let error = sig.error_type().expect("should have an error type");

    assert!(
        error.is(TypePathRef::builder()
            .krate("anyhow")
            .modules(&[])
            .name("Error")
            .build()),
        "the awaited error should be anyhow's, got: {error:?}"
    );
}

#[test]
fn fn_signature_on_async_function_reports_the_awaited_result() {
    let sig = signature_of("returns_anyhow_result_async");

    let ret = sig.return_type().expect("should have return type");

    assert!(
        ret.is_result(),
        "the awaited type should be Result, got display: {}",
        ret.display()
    );
}

/// An `async fn`'s signature must describe the awaited type
///
/// `Function::ret_type` reads the opaque future from the signature, and a
/// regression to it fails the three tests of this async function.
#[test]
fn fn_signature_on_async_function_reports_the_awaited_type() {
    let sig = signature_of("returns_anyhow_result_async");

    assert_eq!(sig.return_mode(), ReturnMode::Awaited);
}

/// The error of `std::io::Result` resolves to `core::io::error::Error`
///
/// `std::io::Error` is a re-export, so the definition path names `core`.
#[test]
fn fn_signature_on_io_result_function() {
    let sig = signature_of("returns_io_result");

    let error = sig.error_type().expect("should have an error type");

    assert!(
        error.is(TypePathRef::builder()
            .krate("core")
            .modules(&["io", "error"])
            .name("Error")
            .build()),
        "the error should be io's, got: {error:?}"
    );
}

#[test]
fn fn_signature_on_io_result_function_returns_a_result() {
    let sig = signature_of("returns_io_result");

    let ret = sig.return_type().expect("should have return type");

    assert!(
        ret.is_result(),
        "return type should be Result, got display: {}",
        ret.display()
    );
}

#[test]
fn fn_signature_on_non_result_function() {
    let sig = signature_of("bool_param");

    let ret = sig.return_type().expect("should have return type");

    assert!(!ret.is_result(), "return type should not be Result");
}

#[test]
fn fn_signature_on_non_result_function_has_no_error_type() {
    let sig = signature_of("bool_param");

    assert!(sig.error_type().is_none());
}

/// The kind of node that holds a value deciding a branch
#[derive(Copy, Clone)]
enum BranchSite {
    /// The `let` of an `if let`
    LetCondition,
    /// A `let` statement
    LetDeclaration,
    /// A method call, whose receiver is the value
    Call,
    /// An `==` or `!=`, whose left operand is the value
    Comparison,
    /// A `matches!` call, which holds the decoration itself
    Matches,
}

impl BranchSite {
    /// Returns the name of the node kind in the grammar
    fn kind(self) -> &'static str {
        match self {
            Self::LetCondition => "let_condition",
            Self::LetDeclaration => "let_declaration",
            Self::Call => "call_expression",
            Self::Comparison => "binary_expression",
            Self::Matches => "macro_invocation",
        }
    }
}

/// Returns the value that decides the first `site` inside `function`
fn branch_value<'a>(
    tree: &'a DecoratedTree,
    function: &str,
    site: BranchSite,
) -> DecoratedNode<'a> {
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, function).unwrap_or_else(|| panic!("should find {function}"));
    let node = find_first_node_of_kind(&func, site.kind()).expect("should find the site");
    match site {
        BranchSite::Call => node
            .child_by_field_name("function")
            .and_then(|f| f.child_by_field_name("value"))
            .expect("call should have a receiver"),
        BranchSite::Comparison => node.child_by_field_name("left").expect("left operand"),
        BranchSite::Matches => node,
        BranchSite::LetCondition | BranchSite::LetDeclaration => {
            node.child_by_field_name("value").expect("value field")
        }
    }
}

/// Returns the variants decorated on the first `site` inside `function`
fn branch_node(tree: &DecoratedTree, function: &str, site: BranchSite) -> Vec<String> {
    branch_value(tree, function, site)
        .decoration::<EnumVariants>()
        .map(|variants| variants.names().map(str::to_owned).collect())
        .unwrap_or_default()
}

const COLORS: [&str; 3] = ["Red", "Green", "Blue"];

#[test]
fn fn_signature_on_sync_function_reports_direct_mode() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");

    let sig = func
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    assert_eq!(sig.return_mode(), ReturnMode::Direct);
}

/// A crate's own `Result` is not the standard one, however it renders
///
/// `myres::Result<()>` renders as `Result<()>`, which any prefix test on the
/// rendering accepts. Only a comparison against the enum `FamousDefs`
/// resolves can tell the two apart.
/// A trait method without a body gets a signature too
///
/// Tree-sitter reads the declaration as a `function_signature_item`, not as
/// a `function_item`. Rust-analyzer reads both as a function, so the
/// provider resolves the declaration the same way.
#[test]
fn fn_signature_on_trait_method_without_body() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let declaration = find_signature_by_name(&root, "fetch").expect("should find fetch");

    let sig = declaration
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    let error = sig.error_type().expect("should have an error type");
    assert!(
        error.is(TypePathRef::builder()
            .krate("anyhow")
            .modules(&[])
            .name("Error")
            .build()),
        "the error should be anyhow's, got: {error:?}"
    );
}

#[test]
fn fn_signature_on_user_defined_result_has_no_error_type() {
    let sig = signature_of("user_defined_result");

    assert!(sig.error_type().is_none());
}

#[test]
fn fn_signature_on_user_defined_result_is_not_a_result() {
    let sig = signature_of("user_defined_result");

    let ret = sig.return_type().expect("should have return type");

    assert!(
        !ret.is_result(),
        "a user-defined Result is not core's, got display: {}",
        ret.display()
    );
}

/// Every place the function-scoped-import probe fires in the fixture
///
/// Among imports inside function bodies, the rule spares only those whose
/// qualifier resolves to an enum.
#[test]
fn function_scoped_import_over_the_whole_file_spares_only_variant_imports() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let mut passes: Vec<Box<dyn LintPass>> =
        vec![Box::new(RustLintPassAdapter::new(FunctionScopedImport))];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(
        flagged_in_file(&tree, &diagnostics),
        vec!["use std::collections::HashMap;", "use Shapes::draw;"]
    );
}

#[test]
fn if_else_clause_gets_type_decoration() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "if_let_with_non_diverging_else")
        .expect("should find if_let_with_non_diverging_else");

    let if_expr =
        find_first_node_of_kind(&func, "if_expression").expect("should find if_expression");

    let else_clause = if_expr
        .child_by_field_name("alternative")
        .expect("should have else branch");

    let ty = else_clause
        .decoration::<ResolvedType>()
        .expect("else clause should have ResolvedType");

    assert!(
        !ty.is_never(),
        "non-diverging else should not be never type"
    );
}

#[test]
fn if_let_with_non_diverging_else_is_not_never() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "if_let_with_non_diverging_else")
        .expect("should find if_let_with_non_diverging_else");

    let if_expr =
        find_first_node_of_kind(&func, "if_expression").expect("should find if_expression");

    let else_clause = if_expr
        .child_by_field_name("alternative")
        .expect("should have else branch");

    let ty = else_clause
        .decoration::<ResolvedType>()
        .expect("else clause should have ResolvedType");

    assert!(
        !ty.is_never(),
        "non-diverging else should not have never type"
    );
}

#[test]
fn import_source_on_a_module_qualifier_is_module() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);

    assert_eq!(
        import_source_in(&tree, "import_from_module"),
        ImportSource::Module
    );
}

/// A list import and a glob import out of an enum both report the enum
///
/// Both shapes hold the qualifier as the use tree's own path, so one
/// lookup serves them.
#[test]
fn import_source_on_an_enum_qualifier_is_enum() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);

    let sources = ["import_enum_variants", "import_enum_variants_by_glob"]
        .map(|function| import_source_in(&tree, function));

    assert_eq!(sources, [ImportSource::Enum, ImportSource::Enum]);
}

/// A module whose name starts with a capital is still a module
///
/// The name looks like a type, and only the resolution says otherwise.
#[test]
fn import_source_on_an_uppercase_module_is_module() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);

    assert_eq!(
        import_source_in(&tree, "import_from_uppercase_module"),
        ImportSource::Module
    );
}

/// Returns the node of the item called `name`, anywhere under `root`
fn find_item_by_name<'a>(root: &DecoratedNode<'a>, name: &str) -> Option<DecoratedNode<'a>> {
    for child in root.named_children() {
        if child.kind().ends_with("_item")
            && let Some(name_node) = child.child_by_field_name("name")
            && name_node.text() == name
        {
            return Some(child);
        }
        if let Some(found) = find_item_by_name(&child, name) {
            return Some(found);
        }
    }
    None
}

/// Returns the [`ItemExposure`] on the named item of `tree`
fn exposure_of(tree: &DecoratedTree, name: &str) -> Option<ItemExposure> {
    let root = tree.root_node();
    let item = find_item_by_name(&root, name).unwrap_or_else(|| panic!("should find {name}"));

    item.decoration::<ItemExposure>().copied()
}

/// Returns the [`ItemExposure`] on the named item of `src/lib.rs`
fn lib_exposure(name: &str) -> Option<ItemExposure> {
    exposure_of(&parse_and_decorate_fixture(load_provider()), name)
}

/// Returns the [`ItemExposure`] on the named item of `src/seal.rs`
fn seal_exposure(name: &str) -> Option<ItemExposure> {
    exposure_of(&decorate_fixture_file(load_provider(), "src/seal.rs"), name)
}

/// Returns the [`ItemExposure`] on each named item of `src/lib.rs`
fn exposures_of(names: [&str; 8]) -> Vec<Option<ItemExposure>> {
    names.into_iter().map(lib_exposure).collect()
}

#[test]
fn item_exposure_is_absent_on_a_function_in_a_body() {
    let exposure = lib_exposure("exposed_inside_a_body");

    assert_eq!(exposure, None);
}

#[test]
fn item_exposure_is_absent_on_a_function_in_a_module_in_a_body() {
    let exposure = lib_exposure("exposed_inside_a_module_in_a_body");

    assert_eq!(exposure, None);
}

#[test]
fn item_exposure_is_absent_on_a_method() {
    let exposure = lib_exposure("exposed_method");

    assert_eq!(exposure, None);
}

#[test]
fn item_exposure_is_absent_on_an_item_a_cfg_disables() {
    let exposure = seal_exposure("exposed_under_a_disabled_cfg");

    assert_eq!(exposure, None);
}

#[test]
fn item_exposure_is_internal_for_a_crate_visible_item() {
    let exposure = lib_exposure("exposed_to_the_crate");

    assert_eq!(exposure, Some(ItemExposure::Internal));
}

#[test]
fn item_exposure_is_internal_for_a_struct_in_a_private_file_module() {
    let exposure = seal_exposure("Entries");

    assert_eq!(exposure, Some(ItemExposure::Internal));
}

#[test]
fn item_exposure_is_internal_for_a_trait_in_a_private_file_module() {
    let exposure = seal_exposure("Sealed");

    assert_eq!(exposure, Some(ItemExposure::Internal));
}

#[test]
fn item_exposure_is_internal_for_an_item_in_a_private_module() {
    let exposure = lib_exposure("exposed_private_function");

    assert_eq!(exposure, Some(ItemExposure::Internal));
}

#[test]
fn item_exposure_is_internal_for_every_kind_in_a_private_module() {
    let exposures = exposures_of([
        "exposed_private_function",
        "ExposedPrivateStruct",
        "ExposedPrivateEnum",
        "ExposedPrivateUnion",
        "ExposedPrivateTrait",
        "ExposedPrivateAlias",
        "EXPOSED_PRIVATE_CONST",
        "EXPOSED_PRIVATE_STATIC",
    ]);

    assert_eq!(exposures, vec![Some(ItemExposure::Internal); 8]);
}

#[test]
fn item_exposure_is_public_for_a_doc_hidden_item() {
    let exposure = lib_exposure("exposed_but_doc_hidden");

    assert_eq!(exposure, Some(ItemExposure::Public));
}

#[test]
fn item_exposure_is_public_for_a_reexported_item() {
    let exposure = lib_exposure("exposed_through_a_reexport");

    assert_eq!(exposure, Some(ItemExposure::Public));
}

#[test]
fn item_exposure_is_public_for_an_item_in_a_public_module() {
    let exposure = lib_exposure("exposed_public_function");

    assert_eq!(exposure, Some(ItemExposure::Public));
}

#[test]
fn item_exposure_is_public_for_every_kind_in_a_public_module() {
    let exposures = exposures_of([
        "exposed_public_function",
        "ExposedPublicStruct",
        "ExposedPublicEnum",
        "ExposedPublicUnion",
        "ExposedPublicTrait",
        "ExposedPublicAlias",
        "EXPOSED_PUBLIC_CONST",
        "EXPOSED_PUBLIC_STATIC",
    ]);

    assert_eq!(exposures, vec![Some(ItemExposure::Public); 8]);
}

#[test]
fn local_enum_has_non_exhaustive_external_false() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_enum").expect("should find match_on_enum");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let flags = scrutinee
        .decoration::<AdtFlags>()
        .expect("a local enum scrutinee should have AdtFlags");

    assert!(
        !flags.non_exhaustive_external(),
        "local enum should not be non_exhaustive_external"
    );
}

/// A scrutinee returned by a call must be typed as the call's result
///
/// A lookup at the start offset of `pick_color()` finds the callee path,
/// whose type is the function itself, not the `Color` it returns.
#[test]
fn match_scrutinee_on_call_expression_has_is_enum_true() {
    let (_, ty) = scrutinee_of("match_on_call_expression");

    assert!(
        ty.is_enum(),
        "the returned Color type should be an enum, got: {}",
        ty.display()
    );
}

#[test]
fn match_scrutinee_on_call_expression_is_the_call_expression() {
    let (text, _) = scrutinee_of("match_on_call_expression");

    assert_eq!(text, "pick_color()");
}

#[test]
fn match_scrutinee_on_enum_has_is_enum_true() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_enum").expect("should find match_on_enum");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("scrutinee should have ResolvedType");

    assert!(ty.is_enum(), "Color should be an enum");
}

/// A scrutinee reached through a field must be typed as the field
///
/// A lookup at the start offset of `palette.primary` finds the path
/// `palette` and reports the struct, not the enum.
#[test]
fn match_scrutinee_on_field_expression_has_is_enum_true() {
    let (_, ty) = scrutinee_of("match_on_field_expression");

    assert!(
        ty.is_enum(),
        "the field's Color type should be an enum, got: {}",
        ty.display()
    );
}

#[test]
fn match_scrutinee_on_field_expression_is_the_field_expression() {
    let (text, _) = scrutinee_of("match_on_field_expression");

    assert_eq!(text, "palette.primary");
}

#[test]
fn match_scrutinee_on_integer_has_is_enum_false() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func =
        find_function_by_name(&root, "match_on_integer").expect("should find match_on_integer");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("scrutinee should have ResolvedType");

    assert!(!ty.is_enum(), "i32 should not be an enum");
}

#[test]
fn provider_decorates_without_panic() {
    let provider = load_provider();
    let _tree = parse_and_decorate_fixture(provider);
}

#[test]
fn provider_loads_fixture_project() {
    let _provider = load_provider();
}

/// An item of `core` is exposed by `std`, which every crate depends on
#[test]
fn reexports_on_a_core_item_lists_its_std_path() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    let reexports = decorated_reexports_of(&tree, "core::fmt::Debug");

    let std_path = reexports.through("std").expect("std exposes core's Debug");
    assert_eq!(std_path.to_string(), "std::fmt::Debug");
}

#[test]
fn reexports_on_a_core_item_lists_no_core_path() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    let reexports = decorated_reexports_of(&tree, "core::fmt::Debug");

    assert!(reexports.through("core").is_none());
}

/// A crate that depends on another proves nothing about re-exports
///
/// `lookalike_json` depends on `lookalike` as `serde_json` depends on
/// `serde`, and exposes none of it. `anyhow` stands in for an unrelated
/// crate from the registry.
#[test]
fn reexports_on_a_crate_that_reexports_nothing_is_empty() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    for leaf in [
        "lookalike::Serialize",
        "lookalike_json::Value",
        "anyhow::Context",
    ] {
        let reexports = decorated_reexports_of(&tree, leaf);

        assert!(
            reexports.paths().is_empty(),
            "{leaf} should have no other path, got {:?}",
            rendered(&reexports)
        );
    }
}

/// A re-export marked `#[doc(hidden)]` is not offered as a path
#[test]
fn reexports_on_a_doc_hidden_reexport_is_empty() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    let reexports = decorated_reexports_of(&tree, "facade_core::hidden::Hidden");

    assert!(reexports.paths().is_empty());
}

/// Each name in a grouped import gets its own decoration
///
/// The names inside the braces resolve with the prefix in front of them,
/// so `Shape` and `thing::make` name `facade_core`'s items.
#[test]
fn reexports_on_a_grouped_import_decorates_each_name() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    let paths = ["Shape", "thing::make"].map(|leaf| rendered(&decorated_reexports_of(&tree, leaf)));

    assert_eq!(paths, [vec!["facade::Shape"], vec!["facade::thing::make"]]);
}

#[test]
fn reexports_on_a_local_item_is_empty() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    let reexports = decorated_reexports_of(&tree, "crate::Palette");

    assert!(reexports.paths().is_empty());
}

/// A path that already goes through the facade lists the facade itself
///
/// The defining crate is the one left out, not the crate the `use` names,
/// so a rule sees that `facade::thing::Thing` already takes the facade.
/// The decoration sits on the path of a rename, not on the rename.
#[test]
fn reexports_on_a_path_through_the_facade_lists_the_facade() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    let reexports = decorated_reexports_of(&tree, "facade::thing::Thing");

    assert_eq!(rendered(&reexports), vec!["facade::thing::Thing"]);
}

/// The item `facade` re-exports through a module gets the facade's path
///
/// The facade re-exports the whole `thing` module, so the path runs
/// through a module that `facade_core` defines.
#[test]
fn reexports_on_a_reexported_item_lists_the_facade_path() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    let reexports = decorated_reexports_of(&tree, "facade_core::thing::Thing");

    assert_eq!(
        reexports.paths(),
        &[PublicPath::builder()
            .krate("facade")
            .segments(["thing", "Thing"])
            .build()]
    );
}

/// A variant takes the facade's path to its enum
#[test]
fn reexports_on_an_enum_variant_goes_through_its_enum() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    let reexports = decorated_reexports_of(&tree, "facade_core::Shape::Circle");

    assert_eq!(rendered(&reexports), vec!["facade::Shape::Circle"]);
}

/// Items of `facade_core` that `facade` does not offer get no path
///
/// `facade` re-exports the module that holds `Internal`, but marks the
/// re-export `#[doc(hidden)]`, so no path through it counts.
#[test]
fn reexports_on_an_item_the_facade_leaves_out_is_empty() {
    let provider = load_provider();
    let tree = decorate_fixture_file(provider, "src/reexports.rs");

    for leaf in ["facade_core::unexported::Internal", "facade_core::Draw"] {
        let reexports = decorated_reexports_of(&tree, leaf);

        assert!(
            reexports.paths().is_empty(),
            "{leaf} should have no other path, got {:?}",
            rendered(&reexports)
        );
    }
}

#[test]
fn try_operand_on_call_expression_is_the_whole_call() {
    let (text, _) = try_operand_of("returns_anyhow_result");

    assert_eq!(text, "std::fs::read_to_string(\"anyhow_bare.txt\")");
}

/// The operand of `?` must be typed as the call, not as the callee
///
/// A lookup at the start offset of the operand finds the callee path,
/// whose type is a function, not a `Result`. Lints that require a `Result`
/// operand then never fire.
#[test]
fn try_operand_on_call_expression_is_typed_as_the_call_result() {
    let (_, ty) = try_operand_of("returns_anyhow_result");

    assert!(
        ty.is_result(),
        "read_to_string returns a Result, got: {}",
        ty.display()
    );
}

#[test]
fn try_operand_on_method_call_is_the_whole_method_call() {
    let (text, _) = try_operand_of("try_on_method_call");

    assert_eq!(text, "loader.load()");
}

/// The operand of `?` must be typed as the method call, not as the receiver
///
/// A lookup at the start offset of `loader.load()?` finds the token
/// `loader`, whose type is a reference to the struct. This test pins the
/// method-call shape of the same fault.
#[test]
fn try_operand_on_method_call_is_typed_as_the_call_result() {
    let (_, ty) = try_operand_of("try_on_method_call");

    assert!(
        ty.is_result(),
        "Loader::load returns a Result, got: {}",
        ty.display()
    );
}

/// The wildcard probe fires on a scrutinee reached through a field
///
/// The old start-offset lookup only resolved scrutinees that are a bare
/// name. This test pins a shape that lookup used to miss.
/// Loads the fixture with a provider that computes only `wanted`
///
/// Each test loads its own provider, because the shared one computes every
/// decoration type.
fn load_provider_wanting(wanted: DecorationSet) -> RustDecorationProvider {
    RustDecorationProvider::load(&fixture_path())
        .expect("should load fixture")
        .wanting(wanted)
}

/// Returns the scrutinee of the first match in `match_on_enum`
fn match_on_enum_scrutinee<'a>(root: &DecoratedNode<'a>) -> DecoratedNode<'a> {
    let func = find_function_by_name(root, "match_on_enum").expect("should find the function");
    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");
    match_expr
        .child_by_field_name("value")
        .expect("match should have value field")
}

/// Returns the value that the first `if let` in `if_let_on_enum` tests
fn if_let_on_enum_value<'a>(root: &DecoratedNode<'a>) -> DecoratedNode<'a> {
    let func = find_function_by_name(root, "if_let_on_enum").expect("should find the function");
    let condition = find_first_node_of_kind(&func, "let_condition").expect("should find let");
    condition
        .child_by_field_name("value")
        .expect("let should have value field")
}

#[test]
fn wanting_item_exposure_decorates_an_item() {
    let provider = load_provider_wanting(DecorationSet::from_iter([ItemExposure::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let item = find_item_by_name(&root, "exposed_to_the_crate").expect("should find the item");

    assert!(item.decoration::<ItemExposure>().is_some());
}

#[test]
fn wanting_no_decorations_attaches_no_signature() {
    let provider = load_provider_wanting(DecorationSet::default());
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_enum").expect("should find the function");

    assert!(func.decoration::<FnSignature>().is_none());
}

#[test]
fn wanting_only_adt_flags_flags_a_scrutinee() {
    let provider = load_provider_wanting(DecorationSet::from_iter([AdtFlags::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let scrutinee = match_on_enum_scrutinee(&root);

    assert!(scrutinee.decoration::<AdtFlags>().is_some());
}

/// A scrutinee yields two types, and a provider builds only the wanted one
#[test]
fn wanting_only_adt_flags_leaves_a_scrutinee_untyped() {
    let provider = load_provider_wanting(DecorationSet::from_iter([AdtFlags::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let scrutinee = match_on_enum_scrutinee(&root);

    assert!(scrutinee.decoration::<ResolvedType>().is_none());
}

/// A scrutinee yields three types, and the flags alone build no path
#[test]
fn wanting_only_adt_flags_leaves_a_scrutinee_without_a_path() {
    let provider = load_provider_wanting(DecorationSet::from_iter([AdtFlags::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let scrutinee = match_on_enum_scrutinee(&root);

    assert!(scrutinee.decoration::<AdtPath>().is_none());
}

#[test]
fn wanting_only_adt_path_leaves_a_scrutinee_without_flags() {
    let provider = load_provider_wanting(DecorationSet::from_iter([AdtPath::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let scrutinee = match_on_enum_scrutinee(&root);

    assert!(scrutinee.decoration::<AdtFlags>().is_none());
}

/// A pass that reads only paths still gets the values that decide a branch
#[test]
fn wanting_only_adt_path_names_a_branch_value() {
    let provider = load_provider_wanting(DecorationSet::from_iter([AdtPath::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let value = if_let_on_enum_value(&root);

    assert_eq!(
        rendered_adt_path(&value).as_deref(),
        Some("sample_project::Color")
    );
}

#[test]
fn wanting_only_adt_path_names_a_scrutinee() {
    let provider = load_provider_wanting(DecorationSet::from_iter([AdtPath::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let scrutinee = match_on_enum_scrutinee(&root);

    assert_eq!(
        rendered_adt_path(&scrutinee).as_deref(),
        Some("sample_project::Color")
    );
}

#[test]
fn wanting_only_enum_variants_leaves_a_branch_value_untyped() {
    let provider = load_provider_wanting(DecorationSet::from_iter([EnumVariants::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let value = if_let_on_enum_value(&root);

    assert!(value.decoration::<ResolvedType>().is_none());
}

#[test]
fn wanting_only_enum_variants_names_the_variants_of_a_branch_value() {
    let provider = load_provider_wanting(DecorationSet::from_iter([EnumVariants::KEY]));
    let tree = parse_and_decorate_fixture(&provider);

    let variants = branch_node(&tree, "if_let_on_enum", BranchSite::LetCondition);

    assert_eq!(variants, COLORS);
}

/// A branch value is typed for a pass that reads types and no variants
#[test]
fn wanting_only_resolved_types_types_a_branch_value() {
    let provider = load_provider_wanting(DecorationSet::from_iter([ResolvedType::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let value = if_let_on_enum_value(&root);

    assert!(value.decoration::<ResolvedType>().is_some());
}

#[test]
fn wanting_only_signatures_leaves_a_scrutinee_untyped() {
    let provider = load_provider_wanting(DecorationSet::from_iter([FnSignature::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let scrutinee = match_on_enum_scrutinee(&root);

    assert!(scrutinee.decoration::<ResolvedType>().is_none());
}

#[test]
fn wanting_only_signatures_leaves_an_item_without_exposure() {
    let provider = load_provider_wanting(DecorationSet::from_iter([FnSignature::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let item = find_item_by_name(&root, "exposed_to_the_crate").expect("should find the item");

    assert!(item.decoration::<ItemExposure>().is_none());
}

#[test]
fn wanting_only_signatures_signs_a_function() {
    let provider = load_provider_wanting(DecorationSet::from_iter([FnSignature::KEY]));
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_enum").expect("should find the function");

    assert!(func.decoration::<FnSignature>().is_some());
}

#[test]
fn wildcard_match_arm_flags_a_scrutinee_reached_through_a_field() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "match_on_field_expression")
        .expect("should find match_on_field_expression");
    let mut passes: Vec<Box<dyn LintPass>> =
        vec![Box::new(RustLintPassAdapter::new(WildcardMatchArm))];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(flagged_within(&tree, &diagnostics, &func), vec!["_"]);
}

/// The `function_item` nodes a walk saw, and how many carry a signature
#[derive(Copy, Clone, Debug, Default)]
struct SignatureCounts {
    functions: usize,
    signatures: usize,
}

/// Counts the fixture's `function_item` nodes and how many carry a signature
///
/// The walk recurses because a method inside an `impl` block is a
/// `function_item` too. A top-level loop would never count the `async`
/// trait method.
fn count_fn_signatures(node: &DecoratedNode<'_>, counts: &mut SignatureCounts) {
    if node.kind() == "function_item" {
        counts.functions += 1;
        if node.decoration::<FnSignature>().is_some() {
            counts.signatures += 1;
        }
    }
    for child in node.named_children() {
        count_fn_signatures(&child, counts);
    }
}

/// Returns how many `function_item` nodes the fixture holds, and how many
/// of them carry a signature
fn fixture_signature_counts() -> (usize, usize) {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);

    let mut counts = SignatureCounts::default();
    count_fn_signatures(&tree.root_node(), &mut counts);
    let SignatureCounts {
        functions,
        signatures,
    } = counts;
    (functions, signatures)
}

#[test]
fn wildcard_match_arm_flags_a_scrutinee_returned_by_a_call() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "match_on_call_expression")
        .expect("should find match_on_call_expression");
    let mut passes: Vec<Box<dyn LintPass>> =
        vec![Box::new(RustLintPassAdapter::new(WildcardMatchArm))];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert_eq!(flagged_within(&tree, &diagnostics, &func), vec!["_"]);
}

/// The wildcard probe still ignores a non-enum scrutinee
///
/// A lookup that resolves too broadly would make `is_enum` true for
/// scrutinees that are not enums, and this test would catch that.
#[test]
fn wildcard_match_arm_ignores_a_scrutinee_that_is_not_an_enum() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, "match_on_integer").expect("should find match_on_integer");
    let mut passes: Vec<Box<dyn LintPass>> =
        vec![Box::new(RustLintPassAdapter::new(WildcardMatchArm))];

    let diagnostics = stiles_core::walk(&tree, &mut passes).expect("the walk should not panic");

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}
