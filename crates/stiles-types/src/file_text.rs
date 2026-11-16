//! The text of a whole file, as a node of that file reads it

mod line_ending;

use std::ops::Range;

pub use self::line_ending::LineEnding;
use crate::DecoratedNode;

/// The text of the file that a node lies in, with file offsets
///
/// A fix edits spans, and a span counts bytes from the start of the file.
/// Every offset that this type takes or returns counts the same way, so a
/// rule passes span offsets in and gets span offsets out.
///
/// The text is the text of the root node. Tree-sitter starts the root after
/// the whitespace that opens the file, so the text can start after offset
/// 0. [`FileText::start`] says where. The whitespace before it is not part
/// of the text, so a line that starts before it starts at
/// [`FileText::start`] here.
///
/// A node reads it through [`DecoratedNode::file_text`]. The node holds no
/// copy of the file, so this borrows the text that the host holds.
///
/// A method that takes an offset returns [`None`] when the offset lies
/// outside the text or inside a character.
///
/// # Examples
///
/// ```
/// # use stiles_types::{DecoratedTree, Language};
/// # let source = "fn one() {}\nfn two() {}\n";
/// # let mut parser = tree_sitter::Parser::new();
/// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
/// # let tree = DecoratedTree::builder()
/// #     .tree(parser.parse(source, None).expect("the source should parse"))
/// #     .language(Language::Rust)
/// #     .source(source.to_string())
/// #     .file("src/lib.rs")
/// #     .build();
/// let two = tree.root_node().named_child(1).expect("the file should hold two functions");
/// let text = two.file_text();
///
/// assert_eq!(text.line_start(two.span().start() + 3), Some(12));
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct FileText<'a> {
    text: &'a str,
    start: usize,
}

impl<'a> FileText<'a> {
    /// Reads the text of `root`, the root node of a file
    pub(crate) fn of(root: &DecoratedNode<'a>) -> Self {
        Self {
            text: root.text(),
            start: root.span().start(),
        }
    }

    /// Returns the text, which starts at [`FileText::start`]
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "\n\nfn one() {}\n";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/lib.rs")
    /// #     .build();
    /// let text = tree.root_node().file_text();
    ///
    /// assert_eq!(text.as_str(), "fn one() {}\n");
    /// ```
    pub fn as_str(&self) -> &'a str {
        self.text
    }

    /// Returns the file offset where the text starts
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "\n\nfn one() {}\n";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/lib.rs")
    /// #     .build();
    /// let text = tree.root_node().file_text();
    ///
    /// assert_eq!(text.start(), 2);
    /// ```
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the file offset where the text ends
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "\n\nfn one() {}\n";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/lib.rs")
    /// #     .build();
    /// let text = tree.root_node().file_text();
    ///
    /// assert_eq!(text.end(), source.len());
    /// ```
    pub fn end(&self) -> usize {
        self.start + self.text.len()
    }

    /// Returns the text between two file offsets
    ///
    /// The result is [`None`] when the range lies outside the text, ends
    /// before it starts, or splits a character.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "\n\nfn one() {}\n";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/lib.rs")
    /// #     .build();
    /// let text = tree.root_node().file_text();
    ///
    /// assert_eq!(text.get(5..8), Some("one"));
    /// ```
    pub fn get(&self, range: Range<usize>) -> Option<&'a str> {
        let Range { start, end } = range;
        let start = start.checked_sub(self.start)?;
        let end = end.checked_sub(self.start)?;

        self.text.get(start..end)
    }

    /// Returns the line ending of the file
    ///
    /// The first line break of the text decides. A file with mixed endings
    /// therefore reads as its first. A file without a line break reads as
    /// [`LineEnding::Lf`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// use stiles_types::LineEnding;
    ///
    /// # let source = "fn one() {}\r\nfn two() {}\r\n";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/lib.rs")
    /// #     .build();
    /// let text = tree.root_node().file_text();
    ///
    /// assert_eq!(text.line_ending(), LineEnding::CrLf);
    /// ```
    pub fn line_ending(&self) -> LineEnding {
        let Some(newline) = self.text.find('\n') else {
            return LineEnding::Lf;
        };

        match self.text[..newline].ends_with('\r') {
            true => LineEnding::CrLf,
            false => LineEnding::Lf,
        }
    }

    /// Returns the file offset where the line that holds `offset` starts
    ///
    /// The first line of the text starts at [`FileText::start`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn one() {}\nfn two() {}\n";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/lib.rs")
    /// #     .build();
    /// let text = tree.root_node().file_text();
    ///
    /// assert_eq!(text.line_start(15), Some(12));
    /// ```
    pub fn line_start(&self, offset: usize) -> Option<usize> {
        let before = self.get(self.start..offset)?;

        Some(
            before
                .rfind('\n')
                .map_or(self.start, |newline| self.start + newline + 1),
        )
    }

    /// Returns the file offset of the `\n` that ends the line holding
    /// `offset`, or the end of the text when no `\n` follows
    ///
    /// A line that ends in `\r\n` keeps its `\r` before the result. One past
    /// the result is therefore the start of the next line in either ending.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn one() {}\nfn two() {}";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/lib.rs")
    /// #     .build();
    /// let text = tree.root_node().file_text();
    ///
    /// assert_eq!(text.line_end(3), Some(11));
    /// ```
    pub fn line_end(&self, offset: usize) -> Option<usize> {
        let after = self.get(offset..self.end())?;

        Some(
            after
                .find('\n')
                .map_or(self.end(), |newline| offset + newline),
        )
    }

    /// Returns the whitespace between the start of its line and `offset`
    ///
    /// The result is [`None`] when anything else stands before `offset` on
    /// its line. An empty result means that `offset` starts its line.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn one() {\n    1\n}\n";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/lib.rs")
    /// #     .build();
    /// let text = tree.root_node().file_text();
    ///
    /// assert_eq!(text.indent(15), Some("    "));
    /// ```
    pub fn indent(&self, offset: usize) -> Option<&'a str> {
        let before = self.get(self.line_start(offset)?..offset)?;

        match before.trim().is_empty() {
            true => Some(before),
            false => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecoratedTree, Language};

    fn tree_of(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("the grammar should load");

        DecoratedTree::builder()
            .tree(parser.parse(source, None).expect("the source should parse"))
            .language(Language::Rust)
            .source(source.to_owned())
            .file("test.rs")
            .build()
    }

    #[test]
    fn end_with_trailing_whitespace_is_the_end_of_the_file() {
        let tree = tree_of("fn f() {}\n\n  \n");

        let end = tree.root_node().file_text().end();

        assert_eq!(end, 14);
    }

    #[test]
    fn get_before_the_start_returns_none() {
        let tree = tree_of("\n\nfn f() {}\n");

        let text = tree.root_node().file_text().get(1..4);

        assert_eq!(text, None);
    }

    #[test]
    fn get_inside_a_character_returns_none() {
        let tree = tree_of("// é\nfn f() {}\n");

        let text = tree.root_node().file_text().get(0..4);

        assert_eq!(text, None);
    }

    #[test]
    fn get_past_the_end_returns_none() {
        let tree = tree_of("fn f() {}\n");

        let text = tree.root_node().file_text().get(0..11);

        assert_eq!(text, None);
    }

    #[test]
    fn get_with_leading_whitespace_counts_from_the_file_start() {
        let tree = tree_of("\n\nfn f() {}\n");

        let text = tree.root_node().file_text().get(5..6);

        assert_eq!(text, Some("f"));
    }

    #[test]
    fn indent_after_code_returns_none() {
        let tree = tree_of("fn f() { 1 }\n");

        let indent = tree.root_node().file_text().indent(9);

        assert_eq!(indent, None);
    }

    #[test]
    fn indent_at_a_line_start_returns_an_empty_text() {
        let tree = tree_of("fn f() {}\nfn g() {}\n");

        let indent = tree.root_node().file_text().indent(10);

        assert_eq!(indent, Some(""));
    }

    #[test]
    fn indent_with_a_tab_returns_the_tab() {
        let tree = tree_of("fn f() {\n\t1\n}\n");

        let indent = tree.root_node().file_text().indent(10);

        assert_eq!(indent, Some("\t"));
    }

    #[test]
    fn line_end_at_the_last_line_without_a_break_returns_the_end() {
        let tree = tree_of("fn f() {}");

        let end = tree.root_node().file_text().line_end(3);

        assert_eq!(end, Some(9));
    }

    #[test]
    fn line_end_with_crlf_returns_the_line_feed() {
        let tree = tree_of("fn f() {}\r\nfn g() {}\r\n");

        let end = tree.root_node().file_text().line_end(3);

        assert_eq!(end, Some(10));
    }

    #[test]
    fn line_ending_of_a_crlf_file_returns_crlf() {
        let tree = tree_of("fn f() {}\r\nfn g() {}\r\n");

        let ending = tree.root_node().file_text().line_ending();

        assert_eq!(ending, LineEnding::CrLf);
    }

    #[test]
    fn line_ending_of_a_file_without_a_break_returns_lf() {
        let tree = tree_of("fn f() {}");

        let ending = tree.root_node().file_text().line_ending();

        assert_eq!(ending, LineEnding::Lf);
    }

    #[test]
    fn line_ending_of_an_lf_file_returns_lf() {
        let tree = tree_of("fn f() {}\nfn g() {}\n");

        let ending = tree.root_node().file_text().line_ending();

        assert_eq!(ending, LineEnding::Lf);
    }

    #[test]
    fn line_start_on_a_later_line_returns_the_offset_after_the_break() {
        let tree = tree_of("\n\nfn f() {}\nfn g() {}\n");

        let start = tree.root_node().file_text().line_start(16);

        assert_eq!(start, Some(12));
    }

    #[test]
    fn line_start_on_the_first_line_after_leading_whitespace_returns_the_start() {
        let tree = tree_of("\n  fn f() {}\n");

        let start = tree.root_node().file_text().line_start(6);

        assert_eq!(start, Some(3));
    }

    #[test]
    fn line_start_past_the_end_returns_none() {
        let tree = tree_of("fn f() {}\n");

        let start = tree.root_node().file_text().line_start(11);

        assert_eq!(start, None);
    }

    #[test]
    fn start_after_leading_whitespace_is_the_first_token() {
        let tree = tree_of("\n\n  fn f() {}\n");

        let start = tree.root_node().file_text().start();

        assert_eq!(start, 4);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FileText<'static>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FileText<'static>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FileText<'static>>();
    }
}
