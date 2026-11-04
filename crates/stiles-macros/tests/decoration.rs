//! Exercises the derived `Decoration` implementations against a real tree
//!
//! The unit tests in the crate itself check the generated tokens. These
//! tests check that the generated code compiles, and that a node reads
//! decorations back in the shape that the type declares.

use std::path::PathBuf;

use stabby::str::Str;
use stiles_macros::Decoration;
use stiles_types::{DecoratedTree, Decoration, Language};

#[stabby::stabby]
#[derive(Decoration)]
#[decoration(cardinality = "one")]
struct ResolvedType(Str<'static>);

#[stabby::stabby]
#[derive(Decoration)]
#[decoration(cardinality = "many")]
struct TraitImpl(Str<'static>);

/// Defines `Counter` without documentation, the baseline the modules
/// below compare their keys against
mod undocumented {
    use stiles_macros::Decoration;

    #[stabby::stabby]
    #[derive(Decoration)]
    #[decoration(cardinality = "one")]
    pub struct Counter {
        pub count: u32,
    }
}

/// Defines `Counter` with the fields of the baseline and documentation on
/// the type and on its field
mod documented {
    use stiles_macros::Decoration;

    /// Counts what a provider saw on a node
    #[stabby::stabby]
    #[derive(Decoration)]
    #[decoration(cardinality = "one")]
    #[doc(alias = "Tally")]
    pub struct Counter {
        /// How many times the provider saw it
        pub count: u32,
    }
}

/// Defines `Counter` with the fields of the baseline and documentation
/// behind a `cfg_attr`
mod documented_behind_cfg_attr {
    use stiles_macros::Decoration;

    #[stabby::stabby]
    #[derive(Decoration)]
    #[decoration(cardinality = "one")]
    #[cfg_attr(all(), doc = "Counts what a provider saw on a node")]
    pub struct Counter {
        #[cfg_attr(all(), doc = "How many times the provider saw it")]
        pub count: u32,
    }
}

/// Defines `Counter` with the field name of the baseline and a wider type
mod widened {
    use stiles_macros::Decoration;

    #[stabby::stabby]
    #[derive(Decoration)]
    #[decoration(cardinality = "one")]
    pub struct Counter {
        pub count: u64,
    }
}

/// Returns the part of a key that the derive hashes from the definition
fn definition_digits<T: Decoration>() -> &'static str {
    T::KEY
        .as_str()
        .rsplit_once('#')
        .expect("a derived key should end in its definition hash")
        .1
}

fn parse(source: &str) -> DecoratedTree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("language should be valid");
    let tree = parser.parse(source, None).expect("should parse");

    DecoratedTree::builder()
        .tree(tree)
        .language(Language::Rust)
        .source(source.to_string())
        .file(PathBuf::from("test.rs"))
        .build()
}

#[test]
fn decorations_are_keyed_per_node() {
    let tree = decorated_root();

    let child = tree
        .root_node()
        .named_child(0)
        .expect("should have a function item");

    assert!(child.get::<ResolvedType>().is_none());
}

#[test]
fn decorations_are_read_back_on_their_node() {
    let tree = decorated_root();

    let found = tree
        .root_node()
        .get::<ResolvedType>()
        .expect("root should be decorated");

    assert_eq!(found.0.as_str(), "root");
}

#[test]
fn key_follows_a_changed_field_type() {
    let baseline = definition_digits::<undocumented::Counter>();

    let widened = definition_digits::<widened::Counter>();

    assert_ne!(widened, baseline);
}

#[test]
fn key_ignores_documentation() {
    let baseline = definition_digits::<undocumented::Counter>();

    let documented = definition_digits::<documented::Counter>();

    assert_eq!(documented, baseline);
}

#[test]
fn key_ignores_documentation_behind_cfg_attr() {
    let baseline = definition_digits::<undocumented::Counter>();

    let documented = definition_digits::<documented_behind_cfg_attr::Counter>();

    assert_eq!(documented, baseline);
}

#[test]
fn lookup_can_be_called_through_the_trait() {
    let mut tree = parse("fn main() {}");
    let id = tree.root_node().id();
    tree.decorations_mut()
        .insert(id, ResolvedType(Str::new("u32")));

    let found = ResolvedType::lookup(&tree.root_node());

    assert_eq!(found.expect("should be present").0.as_str(), "u32");
}

#[test]
fn many_cardinality_reads_back_every_instance_in_order() {
    let mut tree = parse("fn main() {}");
    let id = tree.root_node().id();
    tree.decorations_mut()
        .insert(id, TraitImpl(Str::new("Debug")));
    tree.decorations_mut()
        .insert(id, TraitImpl(Str::new("Clone")));

    let found = tree.root_node().get::<TraitImpl>();

    let names: Vec<&str> = found.iter().map(|found| found.0.as_str()).collect();
    assert_eq!(names, ["Debug", "Clone"]);
}

/// Returns a tree whose root, and none of its children, holds a
/// `ResolvedType`
fn decorated_root() -> DecoratedTree {
    let mut tree = parse("fn main() {}");
    let root_id = tree.root_node().id();
    tree.decorations_mut()
        .insert(root_id, ResolvedType(Str::new("root")));
    tree
}

#[test]
fn many_cardinality_when_absent_reads_back_empty() {
    let tree = parse("fn main() {}");

    let found = tree.root_node().get::<TraitImpl>();

    assert!(found.is_empty());
}

#[test]
fn one_cardinality_reads_back_a_single_value() {
    let mut tree = parse("fn main() {}");
    let id = tree.root_node().id();
    tree.decorations_mut()
        .insert(id, ResolvedType(Str::new("u32")));

    let found = tree.root_node().get::<ResolvedType>();

    assert_eq!(found.expect("should be present").0.as_str(), "u32");
}

#[test]
fn one_cardinality_when_absent_reads_back_none() {
    let tree = parse("fn main() {}");

    let found = tree.root_node().get::<ResolvedType>();

    assert!(found.is_none());
}

/// Decorates the root with a type named `Local` that only this function
/// sees, and returns the value read back
fn decorate_with_a_local_type(tree: &mut DecoratedTree) -> u64 {
    #[stabby::stabby]
    #[derive(Decoration)]
    #[decoration(cardinality = "one")]
    struct Local(u64);

    let id = tree.root_node().id();
    tree.decorations_mut().insert(id, Local(7));

    tree.root_node().get::<Local>().expect("should read back").0
}

/// Two types of one name in two function bodies must stay apart
///
/// `module_path!` cannot see a function body. A key built from the module
/// path and the name alone would therefore be the same for both. The map
/// would then answer a lookup of one with the memory of the other.
#[test]
fn same_named_types_in_two_functions_do_not_collide() {
    #[stabby::stabby]
    #[derive(Decoration)]
    #[decoration(cardinality = "one")]
    struct Local;

    let mut tree = parse("fn main() {}");

    decorate_with_a_local_type(&mut tree);

    assert!(tree.root_node().get::<Local>().is_none());
}

#[test]
fn same_named_types_in_two_functions_read_back_their_own_value() {
    let mut tree = parse("fn main() {}");

    let decorated = decorate_with_a_local_type(&mut tree);

    assert_eq!(decorated, 7);
}
