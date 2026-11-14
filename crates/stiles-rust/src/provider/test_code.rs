//! Finds the items of a file that only a test build compiles
//!
//! The answer is syntactic, so it needs no type inference. The walk reads
//! the attributes of each item in the tree-sitter tree. rust-analyzer only
//! says whether the module of the file was declared as test code.

use ra_ap_hir::{Module, ModuleSource, Semantics};
use ra_ap_ide_db::{FileId, RootDatabase};
use ra_ap_syntax::AstNode as _;
use ra_ap_syntax::ast;
use ra_ap_syntax::ast::HasAttrs as _;
use stiles_types::DecoratedNode;

use crate::decorations::TestCode;

mod cfg_predicate;

/// Says whether the code around a node is test code
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(super) enum Scope {
    /// Code that every build compiles, as far as the walk knows
    Ordinary,
    /// Code that only a test build compiles
    Test,
}

/// Says whether the attributes in front of an item mark it as test code
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
enum OuterMark {
    /// No attribute in front of the item marks it
    Absent,
    /// An attribute in front of the item marks it
    Test,
}

/// Returns each item under `root` that is test code, with why it is
///
/// `scope` says whether the whole file is test code already, as when its
/// module is declared under `#[cfg(test)]`. The walk visits each node once,
/// and it reads the attributes in front of an item as it passes them.
pub(super) fn collect_test_code(root: &DecoratedNode<'_>, scope: Scope) -> Vec<(usize, TestCode)> {
    let mut found = Vec::new();
    visit(root, scope, &mut found);

    found
}

/// Records the test code among the descendants of `node`
///
/// Tree-sitter makes each outer attribute a sibling in front of the item
/// it decorates, with comments between them. The loop therefore remembers
/// whether an attribute it passed marks test code. It forgets at the next
/// node that is neither an attribute nor a comment.
///
/// Only a file, a module body, or a block can open with inner attributes,
/// so only those three are searched for them.
fn visit(node: &DecoratedNode<'_>, scope: Scope, found: &mut Vec<(usize, TestCode)>) {
    let opens_scope = match node.kind() {
        "source_file" | "declaration_list" | "block" => opens_with_test_mark(node),
        _ => false,
    };
    let scope = match opens_scope {
        true => Scope::Test,
        false => scope,
    };
    let mut outer = OuterMark::Absent;

    for child in node.named_children() {
        match child.kind() {
            "attribute_item" => {
                if attribute_item_marks_test(&child) {
                    outer = OuterMark::Test;
                }
                continue;
            }
            "line_comment" | "block_comment" => continue,
            _ => {}
        }

        let marked_outside = std::mem::replace(&mut outer, OuterMark::Absent);
        let child_scope = match is_item(&child, node) {
            true => mark_item(&child, scope, marked_outside, found),
            false => scope,
        };
        visit(&child, child_scope, found);
    }
}

/// Records whether `item` is test code, and returns the scope of its body
///
/// An item that carries a mark itself is [`TestCode::Marked`], whatever
/// encloses it. Its mark may sit in front of it, in `marked_outside`, or
/// open its body as an inner attribute.
fn mark_item(
    item: &DecoratedNode<'_>,
    scope: Scope,
    marked_outside: OuterMark,
    found: &mut Vec<(usize, TestCode)>,
) -> Scope {
    let marked_inside = item
        .child_by_field_name("body")
        .is_some_and(|body| opens_with_test_mark(&body));
    let marked = match (marked_outside, marked_inside) {
        (OuterMark::Test, true | false) | (OuterMark::Absent, true) => true,
        (OuterMark::Absent, false) => false,
    };
    let test_code = match (marked, scope) {
        (true, Scope::Test | Scope::Ordinary) => Some(TestCode::Marked),
        (false, Scope::Test) => Some(TestCode::Enclosed),
        (false, Scope::Ordinary) => None,
    };

    let Some(test_code) = test_code else {
        return Scope::Ordinary;
    };
    found.push((item.id(), test_code));
    Scope::Test
}

/// Returns whether `node` is an item that [`TestCode`] describes
///
/// A macro call is an item only where it stands in place of one, directly
/// in a file or a module. Elsewhere it is an expression or a statement.
fn is_item(node: &DecoratedNode<'_>, parent: &DecoratedNode<'_>) -> bool {
    match node.kind() {
        "associated_type"
        | "const_item"
        | "enum_item"
        | "extern_crate_declaration"
        | "foreign_mod_item"
        | "function_item"
        | "function_signature_item"
        | "impl_item"
        | "macro_definition"
        | "mod_item"
        | "static_item"
        | "struct_item"
        | "trait_item"
        | "type_item"
        | "union_item"
        | "use_declaration" => true,
        "macro_invocation" => match parent.kind() {
            "source_file" | "declaration_list" => true,
            _ => false,
        },
        _ => false,
    }
}

/// Returns whether the inner attributes that open `node` mark test code
///
/// `#![cfg(test)]` sits inside the file, the module, or the block it
/// gates. The search stops at the first child that is neither an inner
/// attribute nor a comment.
fn opens_with_test_mark(node: &DecoratedNode<'_>) -> bool {
    for child in node.named_children() {
        match child.kind() {
            "inner_attribute_item" => {
                if attribute_item_marks_test(&child) {
                    return true;
                }
            }
            "line_comment" | "block_comment" => {}
            _ => return false,
        }
    }

    false
}

/// Returns whether an outer or an inner attribute item marks test code
///
/// Both kinds of item hold the attribute as their first named child, and
/// the attribute holds its path first.
fn attribute_item_marks_test(item: &DecoratedNode<'_>) -> bool {
    let Some(attribute) = item.named_child(0) else {
        return false;
    };
    let Some(path) = attribute.named_child(0) else {
        return false;
    };
    let arguments = attribute.child_by_field_name("arguments");

    AttributeText {
        path: path.text(),
        arguments: arguments.as_ref().map(DecoratedNode::text),
    }
    .marks_test()
}

/// The text of one attribute, as either parser holds it
struct AttributeText<'a> {
    /// The path, such as `cfg` or `tokio::test`
    path: &'a str,
    /// The argument list, parentheses included, if the attribute has one
    arguments: Option<&'a str>,
}

impl AttributeText<'_> {
    /// Returns whether the attribute marks test code
    ///
    /// A path whose last segment is `test` or `bench` marks a test. A
    /// `cfg` marks one when its predicate implies `test`. `cfg_attr` never
    /// does, because it adds an attribute rather than removing the item.
    fn marks_test(&self) -> bool {
        let compact: String = self.path.chars().filter(|c| !c.is_whitespace()).collect();
        let name = compact.rsplit("::").next().unwrap_or_default();

        match name {
            "test" | "bench" => true,
            "cfg" if compact == "cfg" => self
                .arguments
                .is_some_and(self::cfg_predicate::implies_test),
            _ => false,
        }
    }
}

/// Returns whether the module of `file_id` is declared as test code
///
/// A file such as `tests.rs` holds no mark of its own when its parent
/// declares it with `#[cfg(test)] mod tests;`. The search climbs from each
/// module of the file to the crate root. A file that two crates compile is
/// test code only when both declare it so.
pub(super) fn file_scope(sema: &Semantics<'_, RootDatabase>, file_id: FileId) -> Scope {
    let mut modules = sema.file_to_module_defs(file_id).peekable();
    if modules.peek().is_none() {
        return Scope::Ordinary;
    }

    match modules.all(|module| within_test_module(sema.db, module)) {
        true => Scope::Test,
        false => Scope::Ordinary,
    }
}

/// Returns whether `module` or a module around it carries a test mark
fn within_test_module(db: &RootDatabase, module: Module) -> bool {
    let mut current = Some(module);
    while let Some(module) = current {
        if module_marks_test(db, module) {
            return true;
        }
        current = module.parent(db);
    }

    false
}

/// Returns whether the declaration or the file of `module` marks test code
///
/// The mark can sit in front of `mod name`, open the body of an inline
/// module, or open the file that defines the module.
fn module_marks_test(db: &RootDatabase, module: Module) -> bool {
    let declared = module.declaration_source(db).is_some_and(|declaration| {
        let declaration = declaration.value;
        let opens_body = declaration
            .item_list()
            .is_some_and(|items| items.attrs().any(|attr| attr_marks_test(&attr)));

        opens_body || declaration.attrs().any(|attr| attr_marks_test(&attr))
    });
    let defined = match module.definition_source(db).value {
        ModuleSource::SourceFile(file) => file.attrs().any(|attr| attr_marks_test(&attr)),
        ModuleSource::Module(_) | ModuleSource::BlockExpr(_) => false,
    };

    declared || defined
}

/// Returns whether an attribute of rust-analyzer's tree marks test code
///
/// rust-analyzer parses a `cfg` into a predicate of its own, without a path
/// or a token tree. Its text after the `cfg` keyword is the argument list
/// that [`AttributeText`] reads. A `cfg_attr` and an `unsafe(..)` wrapper
/// never mark test code.
fn attr_marks_test(attr: &ast::Attr) -> bool {
    let Some(meta) = attr.meta() else {
        return false;
    };
    let (path, arguments) = match meta {
        ast::Meta::CfgMeta(cfg) => {
            let text = cfg.syntax().text().to_string();
            let Some(arguments) = text.trim_start().strip_prefix("cfg") else {
                return false;
            };
            ("cfg".to_owned(), Some(arguments.to_owned()))
        }
        ast::Meta::PathMeta(meta) => (text_of(meta.path()), None),
        ast::Meta::KeyValueMeta(meta) => (text_of(meta.path()), None),
        ast::Meta::TokenTreeMeta(meta) => (
            text_of(meta.path()),
            meta.token_tree()
                .map(|tree| tree.syntax().text().to_string()),
        ),
        ast::Meta::CfgAttrMeta(_) | ast::Meta::UnsafeMeta(_) => return false,
    };

    AttributeText {
        path: &path,
        arguments: arguments.as_deref(),
    }
    .marks_test()
}

/// Returns the text of a path, or nothing when the parser found none
fn text_of(path: Option<ast::Path>) -> String {
    path.map(|path| path.syntax().text().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stiles_types::{DecoratedTree, Language};

    use super::*;

    fn parse(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&crate::language())
            .expect("the Rust grammar should load");
        let tree = parser.parse(source, None).expect("should parse");

        DecoratedTree::builder()
            .tree(tree)
            .language(Language::Rust)
            .source(source.to_string())
            .file(PathBuf::from("test_code.rs"))
            .build()
    }

    /// Returns the text of each item of test code in `source`, by its first line
    fn test_code_in(source: &str, scope: Scope) -> Vec<(String, TestCode)> {
        let tree = parse(source);
        let found = collect_test_code(&tree.root_node(), scope);

        let mut texts = Vec::new();
        collect_texts(&tree.root_node(), &found, &mut texts);
        texts
    }

    fn collect_texts(
        node: &DecoratedNode<'_>,
        found: &[(usize, TestCode)],
        texts: &mut Vec<(String, TestCode)>,
    ) {
        if let Some((_, test_code)) = found.iter().find(|(id, _)| *id == node.id()) {
            let first_line = node.text().lines().next().unwrap_or_default();
            texts.push((first_line.to_owned(), *test_code));
        }
        for child in node.named_children() {
            collect_texts(&child, found, texts);
        }
    }

    fn marked(text: &str) -> (String, TestCode) {
        (text.to_owned(), TestCode::Marked)
    }

    fn enclosed(text: &str) -> (String, TestCode) {
        (text.to_owned(), TestCode::Enclosed)
    }

    #[test]
    fn attribute_text_marks_test_with_a_bare_cfg_returns_false() {
        let text = AttributeText {
            path: "cfg",
            arguments: None,
        };

        let marks = text.marks_test();

        assert!(!marks);
    }

    #[test]
    fn attribute_text_marks_test_with_a_bench_returns_true() {
        let text = AttributeText {
            path: "bench",
            arguments: None,
        };

        let marks = text.marks_test();

        assert!(marks);
    }

    #[test]
    fn attribute_text_marks_test_with_a_cfg_attr_returns_false() {
        let text = AttributeText {
            path: "cfg_attr",
            arguments: Some("(test, derive(Debug))"),
        };

        let marks = text.marks_test();

        assert!(!marks);
    }

    #[test]
    fn attribute_text_marks_test_with_a_qualified_cfg_returns_false() {
        let text = AttributeText {
            path: "my::cfg",
            arguments: Some("(test)"),
        };

        let marks = text.marks_test();

        assert!(!marks);
    }

    #[test]
    fn attribute_text_marks_test_with_a_qualified_test_returns_true() {
        let text = AttributeText {
            path: "tokio :: test",
            arguments: None,
        };

        let marks = text.marks_test();

        assert!(marks);
    }

    #[test]
    fn attribute_text_marks_test_with_another_attribute_returns_false() {
        let text = AttributeText {
            path: "derive",
            arguments: Some("(Debug)"),
        };

        let marks = text.marks_test();

        assert!(!marks);
    }

    #[test]
    fn collect_test_code_with_a_bench_marks_it() {
        let found = test_code_in("#[bench]\nfn speed() {}\n", Scope::Ordinary);

        assert_eq!(found, [marked("fn speed() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_cfg_all_holding_test_marks_it() {
        let found = test_code_in(
            "#[cfg(all(test, feature = \"x\"))]\nfn helper() {}\n",
            Scope::Ordinary,
        );

        assert_eq!(found, [marked("fn helper() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_cfg_not_test_finds_nothing() {
        let found = test_code_in("#[cfg(not(test))]\nfn shipped() {}\n", Scope::Ordinary);

        assert!(found.is_empty());
    }

    #[test]
    fn collect_test_code_with_a_cfg_test_module_marks_it_and_encloses_its_items() {
        let found = test_code_in(
            "#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn works() {}\n\n    fn helper() {}\n}\n",
            Scope::Ordinary,
        );

        assert_eq!(
            found,
            [
                marked("mod tests {"),
                enclosed("use super::*;"),
                marked("fn works() {}"),
                enclosed("fn helper() {}"),
            ]
        );
    }

    #[test]
    fn collect_test_code_with_a_comment_between_mark_and_item_marks_it() {
        let found = test_code_in(
            "#[test]\n// explains the test\nfn works() {}\n",
            Scope::Ordinary,
        );

        assert_eq!(found, [marked("fn works() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_declared_test_file_encloses_every_item() {
        let found = test_code_in("fn helper() {}\n", Scope::Test);

        assert_eq!(found, [enclosed("fn helper() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_file_that_opens_with_cfg_test_encloses_its_items() {
        let found = test_code_in("#![cfg(test)]\n\nfn helper() {}\n", Scope::Ordinary);

        assert_eq!(found, [enclosed("fn helper() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_function_body_that_opens_with_cfg_test_marks_it() {
        let found = test_code_in("fn helper() {\n    #![cfg(test)]\n}\n", Scope::Ordinary);

        assert_eq!(found, [marked("fn helper() {")]);
    }

    #[test]
    fn collect_test_code_with_a_macro_call_in_a_test_body_skips_it() {
        let found = test_code_in(
            "#[test]\nfn works() {\n    println!(\"x\");\n}\n",
            Scope::Ordinary,
        );

        assert_eq!(found, [marked("fn works() {")]);
    }

    #[test]
    fn collect_test_code_with_a_macro_call_in_a_test_module_encloses_it() {
        let found = test_code_in(
            "#[cfg(test)]\nmod tests {\n    fixtures!();\n}\n",
            Scope::Ordinary,
        );

        assert_eq!(found, [marked("mod tests {"), enclosed("fixtures!()")]);
    }

    #[test]
    fn collect_test_code_with_a_mark_on_a_statement_finds_nothing() {
        let found = test_code_in(
            "fn shipped() {\n    #[cfg(test)]\n    let x = 1;\n}\n",
            Scope::Ordinary,
        );

        assert!(found.is_empty());
    }

    #[test]
    fn collect_test_code_with_a_mark_on_one_item_leaves_the_next_alone() {
        let found = test_code_in(
            "#[test]\nfn works() {}\n\nfn shipped() {}\n",
            Scope::Ordinary,
        );

        assert_eq!(found, [marked("fn works() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_module_that_opens_with_cfg_test_marks_it() {
        let found = test_code_in(
            "mod tests {\n    #![cfg(test)]\n\n    fn helper() {}\n}\n",
            Scope::Ordinary,
        );

        assert_eq!(found, [marked("mod tests {"), enclosed("fn helper() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_nested_function_in_a_test_encloses_it() {
        let found = test_code_in(
            "#[test]\nfn works() {\n    fn local() {}\n}\n",
            Scope::Ordinary,
        );

        assert_eq!(found, [marked("fn works() {"), enclosed("fn local() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_test_function_marks_it() {
        let found = test_code_in("#[test]\nfn works() {}\n", Scope::Ordinary);

        assert_eq!(found, [marked("fn works() {}")]);
    }

    #[test]
    fn collect_test_code_with_a_test_impl_encloses_its_methods() {
        let found = test_code_in(
            "#[cfg(test)]\nimpl Fixture {\n    fn build() -> Self { Fixture }\n}\n",
            Scope::Ordinary,
        );

        assert_eq!(
            found,
            [
                marked("impl Fixture {"),
                enclosed("fn build() -> Self { Fixture }"),
            ]
        );
    }

    #[test]
    fn collect_test_code_with_a_tokio_test_marks_it() {
        let found = test_code_in("#[tokio::test]\nasync fn works() {}\n", Scope::Ordinary);

        assert_eq!(found, [marked("async fn works() {}")]);
    }

    #[test]
    fn collect_test_code_with_ordinary_code_finds_nothing() {
        let found = test_code_in(
            "#[derive(Debug)]\nstruct Shipped;\n\nfn run() {}\n",
            Scope::Ordinary,
        );

        assert!(found.is_empty());
    }

    #[test]
    fn trait_send_attribute_text() {
        fn assert_send<T: Send>() {}
        assert_send::<AttributeText<'static>>();
    }

    #[test]
    fn trait_send_outer_mark() {
        fn assert_send<T: Send>() {}
        assert_send::<OuterMark>();
    }

    #[test]
    fn trait_send_scope() {
        fn assert_send<T: Send>() {}
        assert_send::<Scope>();
    }

    #[test]
    fn trait_sync_attribute_text() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AttributeText<'static>>();
    }

    #[test]
    fn trait_sync_outer_mark() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<OuterMark>();
    }

    #[test]
    fn trait_sync_scope() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Scope>();
    }

    #[test]
    fn trait_unpin_attribute_text() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AttributeText<'static>>();
    }

    #[test]
    fn trait_unpin_outer_mark() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<OuterMark>();
    }

    #[test]
    fn trait_unpin_scope() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Scope>();
    }
}
