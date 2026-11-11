//! The host's functions behind every [`DecoratedNode`]
//!
//! Each function here is an entry of the [`Context`] that a tree builds.
//! The code that runs is therefore the code of the image that built the
//! tree. A function takes the table and follows its `data` to the tree's
//! [`Record`]. It then reads the node bytes back as a tree-sitter node.
//! Both steps rest on one invariant. Only the host writes a table or a
//! node, and a node always travels with the table of its own tree.
//!
//! A function here must not panic. It is `extern "C"`, so a panic aborts
//! the process instead of reaching the pass that called it.
//!
//! [`DecoratedNode`]: crate::DecoratedNode

use std::ffi::c_void;
use std::mem::MaybeUninit;

use stabby::str::Str;

use super::Record;
use crate::context::Context;
use crate::decoration_entry::Entry;
use crate::ts_node::TsNode;
use crate::{DecoratedNode, DecorationKey, FilePath, Language, Position};

/// Returns the table whose functions read the record at `data`
pub(super) fn context(data: *const c_void) -> Context {
    Context {
        length: Context::LENGTH,
        data,
        kind,
        text,
        id,
        start_byte,
        end_byte,
        start_position,
        end_position,
        file,
        is_named,
        has_error,
        child_count,
        named_child_count,
        child,
        named_child,
        child_by_field_name,
        parent,
        next_sibling,
        prev_sibling,
        next_named_sibling,
        prev_named_sibling,
        decoration,
        parse,
    }
}

/// Returns the record that `context` belongs to
fn record(context: &Context) -> &Record {
    // SAFETY: only `DecoratedTree::new` writes `data`, as the address of the
    // record that holds `context`. The record therefore lives at least as
    // long as the borrow of `context`, and nothing changes it meanwhile.
    unsafe { &*context.data.cast::<Record>() }
}

/// Returns the tree-sitter node that `node` holds, from the tree of the table
///
/// The table only lends its lifetime, because the bytes already point into
/// the tree.
fn node_of<'a>(_context: &'a Context, node: &TsNode) -> tree_sitter::Node<'a> {
    // SAFETY: only the host writes node bytes, from a node of the tree of
    // the table it hands out with them, and `DecoratedNode` keeps the two
    // together. That tree lives in the record of `context`, so it outlives
    // `'a`.
    unsafe { node.into_node() }
}

/// Writes `found` to `out`, and returns whether there was a node to write
fn write(found: Option<tree_sitter::Node<'_>>, out: &mut MaybeUninit<TsNode>) -> bool {
    let Some(found) = found else {
        return false;
    };

    out.write(TsNode::from(found));
    true
}

extern "C" fn kind<'a>(context: &'a Context, node: &TsNode) -> Str<'a> {
    Str::new(node_of(context, node).kind())
}

/// Returns the source text under `node`
///
/// Tree-sitter splits a UTF-8 source only between characters, so the
/// range always lands on character boundaries. The empty fallback exists
/// because a slice that panicked here would abort the process.
extern "C" fn text<'a>(context: &'a Context, node: &TsNode) -> Str<'a> {
    let range = node_of(context, node).byte_range();

    Str::new(record(context).source.get(range).unwrap_or_default())
}

extern "C" fn id(context: &Context, node: &TsNode) -> usize {
    node_of(context, node).id()
}

extern "C" fn start_byte(context: &Context, node: &TsNode) -> usize {
    node_of(context, node).start_byte()
}

extern "C" fn end_byte(context: &Context, node: &TsNode) -> usize {
    node_of(context, node).end_byte()
}

extern "C" fn start_position(context: &Context, node: &TsNode) -> Position {
    Position::from(node_of(context, node).start_position())
}

extern "C" fn end_position(context: &Context, node: &TsNode) -> Position {
    Position::from(node_of(context, node).end_position())
}

extern "C" fn file(context: &Context) -> &FilePath {
    &record(context).file
}

extern "C" fn is_named(context: &Context, node: &TsNode) -> bool {
    node_of(context, node).is_named()
}

extern "C" fn has_error(context: &Context, node: &TsNode) -> bool {
    node_of(context, node).has_error()
}

extern "C" fn child_count(context: &Context, node: &TsNode) -> usize {
    usize::try_from(node_of(context, node).child_count()).unwrap_or(usize::MAX)
}

extern "C" fn named_child_count(context: &Context, node: &TsNode) -> usize {
    node_of(context, node).named_child_count()
}

extern "C" fn child(
    context: &Context,
    node: &TsNode,
    index: u32,
    out: &mut MaybeUninit<TsNode>,
) -> bool {
    write(node_of(context, node).child(index), out)
}

extern "C" fn named_child(
    context: &Context,
    node: &TsNode,
    index: u32,
    out: &mut MaybeUninit<TsNode>,
) -> bool {
    write(node_of(context, node).named_child(index), out)
}

extern "C" fn child_by_field_name(
    context: &Context,
    node: &TsNode,
    name: Str<'_>,
    out: &mut MaybeUninit<TsNode>,
) -> bool {
    write(
        node_of(context, node).child_by_field_name(name.as_str()),
        out,
    )
}

extern "C" fn parent(context: &Context, node: &TsNode, out: &mut MaybeUninit<TsNode>) -> bool {
    write(node_of(context, node).parent(), out)
}

extern "C" fn next_sibling(
    context: &Context,
    node: &TsNode,
    out: &mut MaybeUninit<TsNode>,
) -> bool {
    write(node_of(context, node).next_sibling(), out)
}

extern "C" fn prev_sibling(
    context: &Context,
    node: &TsNode,
    out: &mut MaybeUninit<TsNode>,
) -> bool {
    write(node_of(context, node).prev_sibling(), out)
}

extern "C" fn next_named_sibling(
    context: &Context,
    node: &TsNode,
    out: &mut MaybeUninit<TsNode>,
) -> bool {
    write(node_of(context, node).next_named_sibling(), out)
}

extern "C" fn prev_named_sibling(
    context: &Context,
    node: &TsNode,
    out: &mut MaybeUninit<TsNode>,
) -> bool {
    write(node_of(context, node).prev_named_sibling(), out)
}

/// Returns the `n`th entry under `key` on `node`, counting from zero
extern "C" fn decoration<'a>(
    context: &'a Context,
    node: &TsNode,
    key: DecorationKey,
    n: usize,
) -> Option<&'a Entry> {
    let id = node_of(context, node).id();

    record(context).decorations.under(id, key).nth(n)
}

/// Parses `text` as the language whose discriminant is `language`
///
/// The language crosses as a byte. A plugin that names a language this
/// host lacks therefore gets `false`, not a value the host cannot name.
extern "C" fn parse<'a>(
    context: &'a Context,
    language: u8,
    text: Str<'_>,
    out: &mut MaybeUninit<DecoratedNode<'a>>,
) -> bool {
    let Some(language) = Language::from_byte(language) else {
        return false;
    };
    let Some(snippet) = record(context).parse_snippet(language, text.as_str()) else {
        return false;
    };

    out.write(snippet.root_node());
    true
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::*;
    use crate::DecoratedTree;

    fn tree_of(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();

        DecoratedTree::builder()
            .tree(parser.parse(source, None).unwrap())
            .language(Language::Rust)
            .source(source.to_owned())
            .file("test.rs")
            .build()
    }

    #[test]
    fn context_counts_its_function_entries() {
        let context = context(ptr::null());

        assert_eq!(context.length, Context::LENGTH);
    }

    #[test]
    fn parse_with_an_unknown_language_keeps_no_snippet() {
        let tree = tree_of("fn main() {}");
        let record = tree.record();
        let mut out = MaybeUninit::uninit();

        parse(&record.context, u8::MAX, Str::new("fn main() {}"), &mut out);

        assert!(record.snippets.lock().unwrap().is_empty());
    }

    #[test]
    fn parse_with_an_unknown_language_returns_false() {
        let tree = tree_of("fn main() {}");
        let record = tree.record();
        let mut out = MaybeUninit::uninit();

        let parsed = parse(&record.context, u8::MAX, Str::new("fn main() {}"), &mut out);

        assert!(!parsed);
    }
}
