use std::ffi::c_void;
use std::mem::MaybeUninit;

use stabby::str::Str;

use crate::decoration_entry::Entry;
use crate::ts_node::TsNode;
use crate::{DecoratedNode, DecorationKey, FilePath, Position};

/// Reads one number from a node
pub(crate) type CountFn = extern "C" fn(&Context, &TsNode) -> usize;

/// Reads one text from a node, borrowed from the tree
pub(crate) type TextFn = for<'a> extern "C" fn(&'a Context, &TsNode) -> Str<'a>;

/// Reads one point of a node
pub(crate) type PositionFn = extern "C" fn(&Context, &TsNode) -> Position;

/// Reads one flag of a node
pub(crate) type FlagFn = extern "C" fn(&Context, &TsNode) -> bool;

/// Writes the node one step away from a node, and returns whether one exists
pub(crate) type StepFn = extern "C" fn(&Context, &TsNode, &mut MaybeUninit<TsNode>) -> bool;

/// Writes the child at an index, and returns whether one exists
pub(crate) type ChildFn = extern "C" fn(&Context, &TsNode, u32, &mut MaybeUninit<TsNode>) -> bool;

/// Writes the child under a field name, and returns whether one exists
pub(crate) type FieldFn =
    for<'n> extern "C" fn(&Context, &TsNode, Str<'n>, &mut MaybeUninit<TsNode>) -> bool;

/// Returns the file of the tree
pub(crate) type FileFn = for<'a> extern "C" fn(&'a Context) -> &'a FilePath;

/// Returns the `n`th decoration under a key on a node, if there is one
pub(crate) type DecorationFn =
    for<'a> extern "C" fn(&'a Context, &TsNode, DecorationKey, usize) -> Option<&'a Entry>;

/// Parses a snippet, writes its root, and returns whether that worked
pub(crate) type ParseFn = for<'a, 'n> extern "C" fn(
    &'a Context,
    u8,
    Str<'n>,
    &mut MaybeUninit<DecoratedNode<'a>>,
) -> bool;

/// The host's table of the functions that read a tree
///
/// A [`DecoratedNode`] is the bytes of a tree-sitter node and a reference
/// to this table. Every method of the node calls through the table. The
/// host builds the table from functions in its own image, so the host's
/// tree-sitter reads the host's tree. A plugin never reads the node bytes
/// itself.
///
/// The table is `#[repr(C)]`, and every entry is an `extern "C"` function
/// that takes only C types: references, integers, flags, stabby's
/// strings, and stabby's laid-out values. An entry that may find no node
/// writes it through an out-parameter and returns whether it did. The
/// layout is not stabby's, because stabby cannot describe a function
/// whose arguments borrow for a lifetime of its own choosing.
///
/// `length` counts the entries that follow it. A later version appends
/// entries, and a reader reaches only the entries its own length names.
///
/// `data` points to the host's own record of the tree. Only the host's
/// functions read it.
///
/// The type is public because stabby names it in the layout it records
/// for [`DecoratedNode`]. Its module is not, so nothing outside this crate
/// can name it, and it stays out of the documentation.
///
/// [`DecoratedNode`]: crate::DecoratedNode
#[doc(hidden)]
#[repr(C)]
pub struct Context {
    pub(crate) length: usize,
    pub(crate) data: *const c_void,
    pub(crate) kind: TextFn,
    pub(crate) text: TextFn,
    pub(crate) id: CountFn,
    pub(crate) start_byte: CountFn,
    pub(crate) end_byte: CountFn,
    pub(crate) start_position: PositionFn,
    pub(crate) end_position: PositionFn,
    pub(crate) file: FileFn,
    pub(crate) is_named: FlagFn,
    pub(crate) has_error: FlagFn,
    pub(crate) child_count: CountFn,
    pub(crate) named_child_count: CountFn,
    pub(crate) child: ChildFn,
    pub(crate) named_child: ChildFn,
    pub(crate) child_by_field_name: FieldFn,
    pub(crate) parent: StepFn,
    pub(crate) next_sibling: StepFn,
    pub(crate) prev_sibling: StepFn,
    pub(crate) next_named_sibling: StepFn,
    pub(crate) prev_named_sibling: StepFn,
    pub(crate) decoration: DecorationFn,
    pub(crate) parse: ParseFn,
}

impl Context {
    /// The number of function entries this version of the table holds
    pub(crate) const LENGTH: usize = 22;
}

// SAFETY: `data` points to the host's record of a tree, which owns the
// tree, the source, the decorations, and a mutex around the snippets it
// parsed. That record is `Send` and `Sync`, as `DecoratedTree` asserts,
// and the functions of the table only read it or lock that mutex.
unsafe impl Send for Context {}
// SAFETY: as for `Send` above. Nothing writes the tree, the source, or the
// decorations while a node borrows the table.
unsafe impl Sync for Context {}

#[cfg(test)]
mod tests {
    use std::mem::{offset_of, size_of};

    use super::*;

    #[test]
    fn length_counts_every_function_entry() {
        let functions = (size_of::<Context>() - offset_of!(Context, kind)) / size_of::<usize>();

        assert_eq!(functions, Context::LENGTH);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Context>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Context>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Context>();
    }
}
