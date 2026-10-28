//! The TypeScript provider against real projects on disk
//!
//! Each test writes a small project with a `tsconfig.json`, loads it
//! through tsrs, and asks the provider about one file. The projects live
//! in temporary directories so each test controls exactly what the
//! program holds.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use whisker_types::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationProvider, Diagnostic, Language,
    LintPass, RuleId, Severity,
};
use whisker_typescript::decorations::{
    FnSignature, ImportSource, Primitive, ResolvedType, StringUnion, TypeText,
};
use whisker_typescript::{
    LoadTypeScriptProjectError, TypeScriptDecorationProvider, TypeScriptLintPass,
    TypeScriptLintPassAdapter,
};

/// A temporary project: its directory and the files written into it
struct Project {
    directory: TempDir,
}

impl Project {
    /// Writes `tsconfig` and every `(path, text)` file into a new project
    fn new(tsconfig: &str, files: &[(&str, &str)]) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::write(directory.path().join("tsconfig.json"), tsconfig)
            .expect("tsconfig should be written");
        for (path, text) in files {
            let path = directory.path().join(path);
            std::fs::create_dir_all(path.parent().expect("a file has a parent"))
                .expect("directories should be created");
            std::fs::write(path, text).expect("file should be written");
        }
        Self { directory }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.directory.path().join(relative)
    }

    fn provider(&self) -> TypeScriptDecorationProvider {
        TypeScriptDecorationProvider::load(self.directory.path()).expect("the project should load")
    }
}

const STRICT: &str = r#"{ "compilerOptions": { "strict": true, "target": "es2022" }, "include": ["src"], "exclude": ["src/generated"] }"#;

/// Parses `source` as the file at `path`, picking the grammar by extension
fn parse(path: &Path, source: &str) -> DecoratedTree {
    let grammar = match path.extension().and_then(|extension| extension.to_str()) {
        Some("tsx") => whisker_typescript::tsx_language(),
        Some(_) | None => whisker_typescript::language(),
    };
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&grammar)
        .expect("the grammar should load");
    let tree = parser.parse(source, None).expect("the source should parse");
    DecoratedTree::builder()
        .tree(tree)
        .language(Language::TypeScript)
        .source(source.to_owned())
        .file(path)
        .build()
}

/// Decorates the file at `relative` with its text on disk
///
/// Returns the tree with the provider's decorations merged in, or the gap
/// the provider declined the file with.
fn decorate(project: &Project, relative: &str) -> Result<DecoratedTree, CoverageGap> {
    let path = project.path(relative);
    let source = std::fs::read_to_string(&path).expect("the file should exist");
    let mut tree = parse(&path, &source);
    let coverage = project
        .provider()
        .decorate(&tree)
        .expect("decoration should not fail");
    match coverage {
        Coverage::Covered(decorations) => {
            tree.merge_decorations(decorations);
            Ok(tree)
        }
        Coverage::NotCovered(gap) => Err(gap),
    }
}

/// Returns the first node of `kind` whose text is `text`
fn find<'a>(tree: &'a DecoratedTree, kind: &str, text: &str) -> DecoratedNode<'a> {
    fn walk<'a>(node: &DecoratedNode<'a>, kind: &str, text: &str) -> Option<DecoratedNode<'a>> {
        if node.kind() == kind && node.text() == text {
            return Some(*node);
        }
        node.named_children()
            .iter()
            .find_map(|child| walk(child, kind, text))
    }

    walk(&tree.root_node(), kind, text).unwrap_or_else(|| panic!("no {kind} node reads {text}"))
}

/// Returns the first function declaration in `tree`
fn first_function(tree: &DecoratedTree) -> DecoratedNode<'_> {
    tree.root_node()
        .named_children()
        .into_iter()
        .find_map(|node| match node.kind() {
            "function_declaration" => Some(node),
            "export_statement" => node
                .named_children()
                .into_iter()
                .find(|child| child.kind() == "function_declaration"),
            _ => None,
        })
        .expect("the file should declare a function")
}

const ALIASES: &str = "type Greeting = string;\ntype Name = string;\n";

/// A rule in miniature: two parameters a caller can swap
///
/// It reads the [`FnSignature`] the provider attaches. Two parameters
/// written as different aliases of one primitive are interchangeable to
/// the checker, so the rule reports the pair.
struct SwappableParameters;

impl TypeScriptLintPass for SwappableParameters {
    fn check_function_declaration(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(signature) = node.decoration::<FnSignature>() else {
            return Vec::new();
        };
        let parameters = signature.parameters();

        parameters
            .iter()
            .enumerate()
            .flat_map(|(index, first)| {
                parameters[index + 1..]
                    .iter()
                    .map(move |second| (first, second))
            })
            .filter(|(first, second)| {
                let (one, other) = (first.ty(), second.ty());
                one.primitive() != Primitive::NotPrimitive
                    && one.primitive() == other.primitive()
                    && one.alias().is_some()
                    && one.alias() != other.alias()
            })
            .map(|(first, second)| {
                Diagnostic::builder()
                    .rule_id(RuleId::new("test.swappable-parameters"))
                    .severity(Severity::Warn)
                    .message(format!(
                        "`{}` and `{}` can be swapped",
                        first.name(),
                        second.name()
                    ))
                    .span(node.span())
                    .build()
            })
            .collect()
    }
}

/// Returns the type text of every node whose text is `name`, in order
fn types_of(tree: &DecoratedTree, name: &str) -> Vec<Option<String>> {
    fn walk(node: &DecoratedNode<'_>, name: &str, found: &mut Vec<Option<String>>) {
        if node.text() == name && node.named_child_count() == 0 {
            found.push(
                node.decoration::<TypeText>()
                    .map(|ty| ty.as_str().to_owned()),
            );
        }
        for child in node.named_children() {
            walk(&child, name, found);
        }
    }

    let mut found = Vec::new();
    walk(&tree.root_node(), name, &mut found);
    found
}

#[test]
fn decorate_a_changed_file_reports_stale_source() {
    let project = Project::new(STRICT, &[("src/main.ts", "export const a = 1;\n")]);
    let path = project.path("src/main.ts");
    let tree = parse(&path, "export const a = 2;\n");

    let coverage = project.provider().decorate(&tree).expect("should decorate");

    let Coverage::NotCovered(CoverageGap::StaleSource) = &coverage else {
        panic!("expected a stale-source gap, got {coverage:?}");
    };
}

#[test]
fn decorate_a_file_no_include_names_reports_unreachable() {
    let project = Project::new(
        STRICT,
        &[
            ("src/main.ts", "export const a = 1;\n"),
            ("scripts/build.ts", "export const b = 2;\n"),
        ],
    );

    let gap = decorate(&project, "scripts/build.ts").expect_err("should decline");

    let CoverageGap::Unreachable { .. } = gap else {
        panic!("expected an unreachable gap, got {gap:?}");
    };
}

#[test]
fn decorate_a_file_outside_the_project_reports_outside_root() {
    let project = Project::new(STRICT, &[("src/main.ts", "export const a = 1;\n")]);
    let elsewhere = tempfile::tempdir().expect("temporary directory should be created");
    let path = elsewhere.path().join("other.ts");
    let tree = parse(&path, "export const a = 1;\n");

    let coverage = project.provider().decorate(&tree).expect("should decorate");

    let Coverage::NotCovered(CoverageGap::OutsideRoot { root }) = &coverage else {
        panic!("expected an outside-root gap, got {coverage:?}");
    };
    assert_eq!(
        std::fs::canonicalize(&**root).unwrap(),
        std::fs::canonicalize(project.directory.path()).unwrap()
    );
}

#[test]
fn decorate_a_file_the_exclude_list_names_reports_excluded() {
    let project = Project::new(
        STRICT,
        &[
            ("src/main.ts", "export const a = 1;\n"),
            ("src/generated/types.ts", "export const b = 2;\n"),
        ],
    );

    let gap = decorate(&project, "src/generated/types.ts").expect_err("should decline");

    let CoverageGap::ExcludedByToolchain { .. } = gap else {
        panic!("expected an excluded gap, got {gap:?}");
    };
}

#[test]
fn decorate_feeds_a_rule_that_flags_swappable_parameters() {
    let source = format!(
        "{ALIASES}export function sign(greeting: Greeting, name: Name): string {{ return greeting + name; }}\n"
    );
    let project = Project::new(STRICT, &[("src/main.ts", &source)]);
    let tree = decorate(&project, "src/main.ts").expect("the file should be covered");
    let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(TypeScriptLintPassAdapter::new(
        SwappableParameters,
    ))];

    let diagnostics = whisker_core::walk(&tree, &mut passes).expect("the walk should succeed");

    let messages: Vec<&str> = diagnostics.iter().map(|found| found.message()).collect();
    assert_eq!(messages, ["`greeting` and `name` can be swapped"]);
}

#[test]
fn decorate_lists_the_members_of_a_string_union() {
    let project = Project::new(
        STRICT,
        &[(
            "src/main.ts",
            "type Color = \"red\" | \"green\";\nexport function paint(color: Color): Color { return color; }\n",
        )],
    );

    let tree = decorate(&project, "src/main.ts").expect("the file should be covered");

    let color = find(&tree, "identifier", "color");
    let mut members: Vec<&str> = color
        .decoration::<StringUnion>()
        .expect("the parameter should carry its union")
        .members()
        .collect();
    members.sort_unstable();
    assert_eq!(members, ["green", "red"]);
}

/// Returns the type of the `greeting` parameter, declared with the alias
/// `Greeting` of `string`
fn greeting_type() -> ResolvedType {
    let source = format!("{ALIASES}export function greet(greeting: Greeting): void {{}}\n");
    let project = Project::new(STRICT, &[("src/main.ts", &source)]);

    let tree = decorate(&project, "src/main.ts").expect("the file should be covered");

    let greeting = find(&tree, "identifier", "greeting");
    greeting
        .decoration::<ResolvedType>()
        .expect("the parameter should be resolved")
        .clone()
}

#[test]
fn decorate_resolves_a_primitive_alias_on_a_parameter() {
    let ty = greeting_type();

    assert_eq!(ty.alias(), Some("Greeting"));
}

#[test]
fn decorate_resolves_the_primitive_behind_an_alias() {
    let ty = greeting_type();

    assert_eq!(ty.primitive(), Primitive::String);
}

#[test]
fn decorate_resolves_the_text_behind_an_alias() {
    let ty = greeting_type();

    assert_eq!(ty.display(), "string");
}

/// Returns the signature of a function that takes an aliased and a plain
/// parameter and returns an alias
fn sign_signature() -> FnSignature {
    let source = format!(
        "{ALIASES}export function sign(greeting: Greeting, count: number): Name {{ return greeting; }}\n"
    );
    let project = Project::new(STRICT, &[("src/main.ts", &source)]);

    let tree = decorate(&project, "src/main.ts").expect("the file should be covered");

    let function = first_function(&tree);
    function
        .decoration::<FnSignature>()
        .expect("the function should carry its signature")
        .clone()
}

#[test]
fn decorate_signs_a_function_declaration() {
    let signature = sign_signature();

    let parameters: Vec<(&str, Option<&str>, Primitive)> = signature
        .parameters()
        .iter()
        .map(|parameter| {
            (
                parameter.name(),
                parameter.ty().alias(),
                parameter.ty().primitive(),
            )
        })
        .collect();

    assert_eq!(
        parameters,
        [
            ("greeting", Some("Greeting"), Primitive::String),
            ("count", None, Primitive::Number),
        ]
    );
}

#[test]
fn decorate_signs_the_return_type_of_a_function_declaration() {
    let signature = sign_signature();

    assert_eq!(signature.return_type().alias(), Some("Name"));
}

#[test]
fn decorate_sorts_imports_by_where_they_resolve() {
    const BUNDLER: &str = r#"{ "compilerOptions": { "strict": true, "target": "es2022", "module": "esnext", "moduleResolution": "bundler" }, "include": ["src"] }"#;
    let project = Project::new(
        BUNDLER,
        &[
            (
                "src/main.ts",
                "import { a } from \"./a\";\nimport { dep } from \"dep\";\nimport { lib } from \"@ws/lib\";\nimport { gone } from \"gone\";\nexport const all = [a, dep, lib, gone];\n",
            ),
            ("src/a.ts", "export const a = 1;\n"),
            (
                "node_modules/dep/package.json",
                r#"{ "name": "dep", "types": "index.d.ts" }"#,
            ),
            (
                "node_modules/dep/index.d.ts",
                "export declare const dep: number;\n",
            ),
            (
                "packages/lib/package.json",
                r#"{ "name": "@ws/lib", "types": "index.ts" }"#,
            ),
            ("packages/lib/index.ts", "export const lib = 1;\n"),
        ],
    );
    std::fs::create_dir_all(project.path("node_modules/@ws")).expect("the scope should be created");
    std::os::unix::fs::symlink(
        project.path("packages/lib"),
        project.path("node_modules/@ws/lib"),
    )
    .expect("the workspace link should be created");

    let tree = decorate(&project, "src/main.ts").expect("the file should be covered");

    let sources: Vec<Option<ImportSource>> = ["\"./a\"", "\"dep\"", "\"@ws/lib\"", "\"gone\""]
        .iter()
        .map(|specifier| {
            find(&tree, "string", specifier)
                .decoration::<ImportSource>()
                .copied()
        })
        .collect();
    assert_eq!(
        sources,
        [
            Some(ImportSource::Relative),
            Some(ImportSource::Dependency),
            Some(ImportSource::Workspace),
            Some(ImportSource::Unresolved),
        ]
    );
}

#[test]
fn decorate_types_identifiers_after_multibyte_text() {
    let project = Project::new(
        STRICT,
        &[(
            "src/main.ts",
            "// é and 日本 come first\nconst café = \"x\";\nexport const tasse = café;\n",
        )],
    );

    let tree = decorate(&project, "src/main.ts").expect("should cover the file");

    assert_eq!(
        types_of(&tree, "café"),
        vec![Some("\"x\"".to_owned()), Some("\"x\"".to_owned())]
    );
}

#[test]
fn decorate_types_the_identifiers_of_a_file_in_the_program() {
    let project = Project::new(
        STRICT,
        &[(
            "src/main.ts",
            "function add(a: number, b: number) { return a + b; }\n\
             export const total = add(1, 2);\n",
        )],
    );

    let tree = decorate(&project, "src/main.ts").expect("should cover the file");

    let types = ["total", "add"].map(|name| types_of(&tree, name));
    assert_eq!(
        types,
        [
            vec![Some("number".to_owned())],
            vec![
                Some("(a: number, b: number) => number".to_owned()),
                Some("(a: number, b: number) => number".to_owned())
            ],
        ]
    );
}

#[test]
fn decorate_types_the_identifiers_of_a_tsx_file() {
    let project = Project::new(
        r#"{ "compilerOptions": { "strict": true, "jsx": "preserve" }, "include": ["src"] }"#,
        &[(
            "src/view.tsx",
            "declare namespace JSX { interface IntrinsicElements { div: {} } }\n\
             const label = \"hi\";\n\
             export const view = <div>{label}</div>;\n",
        )],
    );

    let tree = decorate(&project, "src/view.tsx").expect("should cover the file");

    assert_eq!(
        types_of(&tree, "label"),
        vec![Some("\"hi\"".to_owned()), Some("\"hi\"".to_owned())]
    );
}

#[test]
fn load_finds_the_configuration_above_the_path() {
    let project = Project::new(STRICT, &[("src/main.ts", "export const a = 1;\n")]);

    let provider = TypeScriptDecorationProvider::load(&project.path("src"));

    assert!(provider.is_ok());
}

#[test]
fn load_without_a_configuration_returns_error() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    let error = TypeScriptDecorationProvider::load(directory.path())
        .err()
        .expect("loading should fail");

    let LoadTypeScriptProjectError::MissingConfig { path } = error else {
        panic!("should be a missing configuration, got {error:?}");
    };
    assert_eq!(path, directory.path());
}

#[test]
fn name_returns_typescript() {
    let project = Project::new(STRICT, &[("src/main.ts", "export const a = 1;\n")]);

    let name = project.provider().name();

    assert_eq!(name.as_str(), "typescript");
}

#[test]
fn project_root_finds_the_nearest_configuration() {
    let project = Project::new(
        STRICT,
        &[
            ("src/main.ts", "export const a = 1;\n"),
            ("packages/b/tsconfig.json", "{}"),
            ("packages/b/index.ts", "export const b = 2;\n"),
        ],
    );

    let outer = TypeScriptDecorationProvider::project_root(&project.path("src/main.ts"));
    let inner = TypeScriptDecorationProvider::project_root(&project.path("packages/b/index.ts"));

    assert_eq!(
        (outer, inner),
        (
            Some(project.directory.path().to_path_buf()),
            Some(project.path("packages/b"))
        )
    );
}

#[test]
fn project_root_without_a_configuration_returns_none() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    let root = TypeScriptDecorationProvider::project_root(&directory.path().join("main.ts"));

    assert_eq!(root, None);
}
