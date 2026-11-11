use std::ffi::c_void;
use std::mem::transmute;

/// The bytes of a tree-sitter node, which only the host reads
///
/// A [`DecoratedNode`] carries one of these where it used to carry a
/// `tree_sitter::Node`. The layout is that of tree-sitter's `TSNode`, a C
/// struct of four `u32`s and two pointers, which `tree_sitter::Node` wraps
/// with `#[repr(transparent)]`. Stabby cannot look inside a foreign type,
/// so this struct names the layout for it.
///
/// A plugin copies these bytes and hands them back to the host's table in
/// [`Context`], and never reads them. Only the host converts them back to
/// a node, so only the host's copy of the tree-sitter library walks the
/// host's tree.
///
/// The type is public because stabby names it in the layout it records
/// for [`DecoratedNode`]. Its module is not, so nothing outside this crate
/// can name it, and it stays out of the documentation.
///
/// [`Context`]: crate::context::Context
/// [`DecoratedNode`]: crate::DecoratedNode
#[doc(hidden)]
#[stabby::stabby]
#[derive(Copy, Clone)]
pub struct TsNode {
    context: [u32; 4],
    id: *const c_void,
    tree: *const c_void,
}

impl TsNode {
    /// Returns the tree-sitter node these bytes hold
    ///
    /// # Safety
    ///
    /// The bytes must come from [`TsNode::from`] on a node of a tree that
    /// is alive and unchanged for all of `'tree`.
    pub(crate) unsafe fn into_node<'tree>(self) -> tree_sitter::Node<'tree> {
        // SAFETY: `tree_sitter::Node` is `#[repr(transparent)]` over `TSNode`,
        // whose layout this struct copies field for field, and `transmute`
        // refuses to compile if the sizes differ. The caller vouches that
        // the bytes name a node of a tree that outlives `'tree`.
        unsafe { transmute::<Self, tree_sitter::Node<'tree>>(self) }
    }
}

impl From<tree_sitter::Node<'_>> for TsNode {
    fn from(node: tree_sitter::Node<'_>) -> Self {
        // SAFETY: `tree_sitter::Node` is `#[repr(transparent)]` over `TSNode`,
        // whose layout this struct copies field for field, and `transmute`
        // refuses to compile if the sizes differ. Every bit pattern of the
        // node is a valid `TsNode`, which holds only integers and pointers.
        unsafe { transmute::<tree_sitter::Node<'_>, Self>(node) }
    }
}

// SAFETY: the bytes point into a tree-sitter tree, which the host never
// changes while a node of it lives. Tree-sitter's own `Node` is `Send` for
// the same reason, and a `TsNode` is only ever read back as that node.
unsafe impl Send for TsNode {}
// SAFETY: as for `Send` above. Tree-sitter reads an unchanged tree from any
// number of threads at once, and its own `Node` is `Sync`.
unsafe impl Sync for TsNode {}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, size_of};

    use super::*;

    fn parse_tree(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn alignment_matches_the_tree_sitter_node() {
        assert_eq!(align_of::<TsNode>(), align_of::<tree_sitter::Node<'_>>());
    }

    #[test]
    fn into_node_returns_the_node_it_came_from() {
        let tree = parse_tree("fn main() {}");
        let node = tree.root_node();

        // SAFETY: the bytes come from a node of `tree`, which outlives the
        // node read back.
        let back = unsafe { TsNode::from(node).into_node() };

        assert_eq!(back, node);
    }

    #[test]
    fn size_matches_the_tree_sitter_node() {
        assert_eq!(size_of::<TsNode>(), size_of::<tree_sitter::Node<'_>>());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TsNode>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TsNode>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TsNode>();
    }
}
