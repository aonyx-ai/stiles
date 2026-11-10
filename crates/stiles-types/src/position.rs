/// A point in a source text, as a row and a column
///
/// Both coordinates count from zero. The row counts newlines before the
/// point. The column counts bytes from the start of that row, not
/// characters, so a column past a multibyte character skips its width.
///
/// A position comes from [`DecoratedNode::start_position`] and
/// [`DecoratedNode::end_position`]. A rule compares rows to tell whether
/// two nodes share a line, or whether one follows the other directly.
/// A position crosses the plugin boundary as a return value, so stabby lays
/// it out.
///
/// # Examples
///
/// ```
/// use stiles_types::{DecoratedTree, Language, Position};
///
/// let source = "fn main() {}\nfn other() {}";
/// let mut parser = tree_sitter::Parser::new();
/// parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
/// let tree = DecoratedTree::builder()
///     .tree(parser.parse(source, None).expect("the source should parse"))
///     .language(Language::Rust)
///     .source(source.to_string())
///     .file("src/main.rs")
///     .build();
/// let other = tree.root_node().named_child(1).expect("the file should hold two functions");
///
/// assert_eq!(other.start_position(), Position { row: 1, column: 0 });
/// ```
///
/// [`DecoratedNode::start_position`]: crate::DecoratedNode::start_position
/// [`DecoratedNode::end_position`]: crate::DecoratedNode::end_position
#[stabby::stabby]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct Position {
    /// The zero-based line of the point
    pub row: usize,
    /// The zero-based byte offset of the point within its line
    pub column: usize,
}

impl From<tree_sitter::Point> for Position {
    fn from(point: tree_sitter::Point) -> Self {
        let tree_sitter::Point { row, column } = point;

        Self { row, column }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_keeps_the_row_and_the_column() {
        let point = tree_sitter::Point { row: 3, column: 7 };

        let position = Position::from(point);

        assert_eq!(position, Position { row: 3, column: 7 });
    }

    #[test]
    fn ord_compares_the_row_before_the_column() {
        let earlier = Position { row: 1, column: 9 };
        let later = Position { row: 2, column: 0 };

        assert!(earlier < later);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Position>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Position>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Position>();
    }
}
