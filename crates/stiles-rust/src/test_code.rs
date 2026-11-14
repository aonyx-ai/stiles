//! Answers whether a node sits in code that only a test build compiles
//!
//! A rule that spares tests calls [`is_test_code`] instead of walking the
//! attributes itself, so every rule draws the line in the same place. The
//! answer comes from two decorations of the Rust provider, which a rule
//! declares through [`ReadsTestCode`].

use stiles_types::DecoratedNode;
use stiles_types::plugin::Reads;

use crate::decorations::{Target, TargetKind, TargetKinds, TestCode};

/// Returns whether `node` sits in code that only a test build compiles
///
/// The walk climbs from `node` to the file. It answers true at the first
/// node that carries [`TestCode`], so a statement in a test function
/// counts. At the file it reads the kinds of [`Target`]. A file that only
/// integration tests and benches compile counts, such as `tests/api.rs`.
///
/// A file whose target the provider does not know counts as shipped code,
/// because an exemption needs proof. So does a node without decorations,
/// as in a rule test that inserts none.
///
/// A rule that calls this function must declare both decorations in its
/// `reads`. [`ReadsTestCode::and_test_code`] adds both at once. Without
/// them, the host ends the run at the first lookup, naming the rule.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use stiles_rust::decorations::TestCode;
/// use stiles_types::{DecoratedTree, DecorationMap, Language};
///
/// let source = "#[test]\nfn works() {\n    let answer = 42;\n}\n";
/// let mut parser = tree_sitter::Parser::new();
/// parser.set_language(&stiles_rust::language())?;
/// let parsed = parser.parse(source, None).expect("should parse");
/// let mut tree = DecoratedTree::builder()
///     .tree(parsed)
///     .language(Language::Rust)
///     .source(source.to_string())
///     .file(PathBuf::from("src/lib.rs"))
///     .build();
/// let function_id = tree.root_node().named_child(1).expect("should hold the function").id();
/// let mut decorations = DecorationMap::new();
/// decorations.insert(function_id, TestCode::Marked);
/// tree.merge_decorations(decorations);
///
/// let function = tree.root_node().named_child(1).expect("should hold the function");
/// let body = function.child_by_field_name("body").expect("should hold a body");
/// let binding = body.named_child(0).expect("should hold the binding");
///
/// assert!(stiles_rust::is_test_code(&binding));
/// # Ok::<(), tree_sitter::LanguageError>(())
/// ```
pub fn is_test_code(node: &DecoratedNode<'_>) -> bool {
    let mut level = *node;
    loop {
        if level.decoration::<TestCode>().is_some() {
            return true;
        }
        match level.parent() {
            Some(parent) => level = parent,
            None => break,
        }
    }

    level
        .decoration::<Target>()
        .is_some_and(|target| only_tests_compile(target.kinds()))
}

/// Returns whether every kind of `kinds` is an integration test or a bench
///
/// An empty set holds no proof, so it yields false.
fn only_tests_compile(kinds: TargetKinds) -> bool {
    if kinds.is_empty() {
        return false;
    }

    kinds.iter().all(|kind| match kind {
        TargetKind::IntegrationTest | TargetKind::Bench => true,
        TargetKind::Library
        | TargetKind::Binary
        | TargetKind::Example
        | TargetKind::BuildScript => false,
    })
}

/// Adds what [`is_test_code`] reads to the reads of a lint
///
/// [`is_test_code`] looks up [`Target`] and [`TestCode`], and a lint that
/// declares only one of them fails the run. The method adds both, so a
/// lint cannot declare half. It chains with [`Reads::and`] like any other
/// read.
///
/// # Examples
///
/// ```
/// use stiles_rust::ReadsTestCode as _;
/// use stiles_rust::decorations::{FnSignature, Target, TestCode};
/// use stiles_types::plugin::Reads;
///
/// let reads = Reads::of::<FnSignature>().and_test_code();
///
/// assert_eq!(reads, Reads::of::<FnSignature>().and::<Target>().and::<TestCode>());
/// ```
///
/// [`Reads::and`]: stiles_types::plugin::Reads::and
pub trait ReadsTestCode {
    /// Returns these reads with [`Target`] and [`TestCode`] added
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::ReadsTestCode as _;
    /// use stiles_types::plugin::Reads;
    ///
    /// let reads = Reads::none().and_test_code();
    ///
    /// assert_eq!(reads.declared().len(), 2);
    /// ```
    #[must_use]
    fn and_test_code(self) -> Reads;
}

impl ReadsTestCode for Reads {
    fn and_test_code(self) -> Reads {
        self.and::<Target>().and::<TestCode>()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stiles_types::{DecoratedTree, DecorationMap, Language};

    use super::*;
    use crate::decorations::{Edition, FnSignature};

    /// The source every test asks about, with a test beside shipped code
    const SOURCE: &str = "#[test]\nfn works() {\n    let answer = 42;\n}\n\nfn shipped() {\n    let answer = 42;\n}\n";

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
            .file(PathBuf::from("src/lib.rs"))
            .build()
    }

    fn target(kinds: impl IntoIterator<Item = TargetKind>) -> Target {
        Target::builder()
            .krate("sample")
            .kinds(kinds.into_iter().collect())
            .edition(Edition::Edition2024)
            .build()
    }

    /// Returns the tree of [`SOURCE`], with `test_code` on `works` and `file` on the root
    fn decorated(test_code: Option<TestCode>, file: Option<Target>) -> DecoratedTree {
        let mut tree = parse(SOURCE);
        let root = tree.root_node();
        let works = function(&root, "works").id();
        let root_id = root.id();
        let mut decorations = DecorationMap::new();
        if let Some(test_code) = test_code {
            decorations.insert(works, test_code);
        }
        if let Some(file) = file {
            decorations.insert(root_id, file);
        }
        tree.merge_decorations(decorations);

        tree
    }

    fn function<'a>(root: &DecoratedNode<'a>, name: &str) -> DecoratedNode<'a> {
        root.named_children()
            .into_iter()
            .find(|child| {
                child
                    .child_by_field_name("name")
                    .is_some_and(|found| found.text() == name)
            })
            .expect("the source should hold the function")
    }

    /// Returns the binding in the body of the function `name`
    fn binding<'a>(root: &DecoratedNode<'a>, name: &str) -> DecoratedNode<'a> {
        function(root, name)
            .child_by_field_name("body")
            .and_then(|body| body.named_child(0))
            .expect("the function should hold a binding")
    }

    #[test]
    fn and_test_code_adds_both_decorations() {
        let reads = Reads::none();

        let reads = reads.and_test_code();

        assert_eq!(reads, Reads::of::<Target>().and::<TestCode>());
    }

    #[test]
    fn and_test_code_keeps_earlier_reads() {
        let reads = Reads::of::<FnSignature>();

        let reads = reads.and_test_code();

        assert_eq!(
            reads,
            Reads::of::<FnSignature>().and::<Target>().and::<TestCode>()
        );
    }

    #[test]
    fn is_test_code_with_a_bench_file_returns_true() {
        let tree = decorated(None, Some(target([TargetKind::Bench])));

        let test_code = is_test_code(&binding(&tree.root_node(), "shipped"));

        assert!(test_code);
    }

    #[test]
    fn is_test_code_with_a_file_of_no_known_target_returns_false() {
        let tree = decorated(None, Some(target([])));

        let test_code = is_test_code(&binding(&tree.root_node(), "shipped"));

        assert!(!test_code);
    }

    #[test]
    fn is_test_code_with_a_file_that_a_library_shares_returns_false() {
        let tree = decorated(
            None,
            Some(target([TargetKind::IntegrationTest, TargetKind::Library])),
        );

        let test_code = is_test_code(&binding(&tree.root_node(), "shipped"));

        assert!(!test_code);
    }

    #[test]
    fn is_test_code_with_an_integration_test_file_returns_true() {
        let tree = decorated(None, Some(target([TargetKind::IntegrationTest])));

        let test_code = is_test_code(&binding(&tree.root_node(), "shipped"));

        assert!(test_code);
    }

    #[test]
    fn is_test_code_with_no_decorations_returns_false() {
        let tree = decorated(None, None);

        let test_code = is_test_code(&binding(&tree.root_node(), "works"));

        assert!(!test_code);
    }

    #[test]
    fn is_test_code_with_the_marked_item_itself_returns_true() {
        let tree = decorated(Some(TestCode::Marked), None);

        let test_code = is_test_code(&function(&tree.root_node(), "works"));

        assert!(test_code);
    }

    #[test]
    fn is_test_code_within_a_library_file_beside_a_test_returns_false() {
        let tree = decorated(Some(TestCode::Marked), Some(target([TargetKind::Library])));

        let test_code = is_test_code(&binding(&tree.root_node(), "shipped"));

        assert!(!test_code);
    }

    #[test]
    fn is_test_code_within_a_marked_item_returns_true() {
        let tree = decorated(Some(TestCode::Marked), Some(target([TargetKind::Library])));

        let test_code = is_test_code(&binding(&tree.root_node(), "works"));

        assert!(test_code);
    }

    #[test]
    fn is_test_code_within_an_enclosed_item_returns_true() {
        let tree = decorated(Some(TestCode::Enclosed), None);

        let test_code = is_test_code(&binding(&tree.root_node(), "works"));

        assert!(test_code);
    }
}
