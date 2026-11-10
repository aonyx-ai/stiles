use std::path::Path;
use std::ptr::{self, NonNull};
use std::sync::{Mutex, PoisonError};

use bon::bon;

use crate::context::Context;
use crate::ts_node::TsNode;
use crate::{DecoratedNode, DecorationMap, FilePath, Language};

mod table;

/// A parsed syntax tree with an overlay of semantic decorations
///
/// Wraps a tree-sitter [`Tree`] together with its source text, file path,
/// and a [`DecorationMap`] that providers populate with per-node semantic
/// information. The tree and decorations together form the input to lint
/// rules.
///
/// The tree never crosses the plugin boundary. Its nodes do, and each one
/// reads the tree through a table of the host's functions that the tree
/// holds. The tree keeps everything a node reads in one allocation that
/// never moves, so moving the tree leaves the table's pointer to it valid.
/// The same allocation holds every snippet that a node parsed, so a
/// snippet lives exactly as long as the tree.
///
/// # Examples
///
/// ```
/// use stiles_types::{DecoratedTree, Language};
///
/// let source = "fn main() {}";
/// let mut parser = tree_sitter::Parser::new();
/// parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
/// let tree = DecoratedTree::builder()
///     .tree(parser.parse(source, None).expect("the source should parse"))
///     .language(Language::Rust)
///     .source(source.to_string())
///     .file("src/main.rs")
///     .build();
///
/// assert_eq!(tree.root_node().kind(), "source_file");
/// ```
///
/// [`Tree`]: tree_sitter::Tree
pub struct DecoratedTree {
    record: NonNull<Record>,
}

/// Everything the host's table reads for one tree
///
/// The record sits in an allocation of its own, which the tree owns as a
/// [`Box`] would. Its table's `data` points back at it. A plugin holds a
/// reference to the table, and only the host's functions follow `data`.
struct Record {
    context: Context,
    tree: tree_sitter::Tree,
    language: Language,
    source: String,
    file: FilePath,
    decorations: DecorationMap,
    snippets: Mutex<Vec<DecoratedTree>>,
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Record>();
};

#[bon]
impl DecoratedTree {
    /// Creates a decorated tree from a tree-sitter tree parsed as `language`
    ///
    /// The language decides which lint passes see the tree, so it must be
    /// the language of the grammar that produced it.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedTree, Language};
    ///
    /// let source = "fn main() {}";
    /// let mut parser = tree_sitter::Parser::new();
    /// parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    ///
    /// assert_eq!(tree.language(), Language::Rust);
    /// ```
    #[builder]
    pub fn new(
        tree: tree_sitter::Tree,
        language: Language,
        source: String,
        file: impl Into<FilePath>,
    ) -> Self {
        let record = Box::into_raw(Box::new(Record {
            context: self::table::context(ptr::null()),
            tree,
            language,
            source,
            file: file.into(),
            decorations: DecorationMap::new(),
            snippets: Mutex::new(Vec::new()),
        }));

        // SAFETY: `Box::into_raw` returned a valid pointer that nothing else
        // holds yet. The table keeps the same pointer, so the record and its
        // table share one provenance.
        unsafe {
            (*record).context.data = record.cast_const().cast();
        }

        // SAFETY: `Box::into_raw` never returns null.
        let record = unsafe { NonNull::new_unchecked(record) };

        Self { record }
    }

    /// Returns a mutable reference to the decoration map for providers to
    /// populate
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, DecoratedTree, Decoration, DecorationKey, DecorationMap, Language};
    ///
    /// /// Marks the root of a file
    /// #[stabby::stabby]
    /// struct Root;
    ///
    /// impl Decoration for Root {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Root"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// # let source = "fn main() {}";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let mut tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    /// let root = tree.root_node().id();
    ///
    /// tree.decorations_mut().insert(root, Root);
    ///
    /// assert!(tree.root_node().get::<Root>().is_some());
    /// ```
    pub fn decorations_mut(&mut self) -> &mut DecorationMap {
        &mut self.record_mut().decorations
    }

    /// Merges provider decorations into this tree
    ///
    /// See [`DecorationMap::merge`] for the conflict behavior.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, DecoratedTree, Decoration, DecorationKey, DecorationMap, Language};
    ///
    /// /// Marks the root of a file
    /// #[stabby::stabby]
    /// struct Root;
    ///
    /// impl Decoration for Root {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Root"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// # let source = "fn main() {}";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let mut tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    /// let root = tree.root_node().id();
    /// let mut decorations = DecorationMap::new();
    /// decorations.insert(root, Root);
    ///
    /// tree.merge_decorations(decorations);
    ///
    /// assert!(tree.root_node().get::<Root>().is_some());
    /// ```
    pub fn merge_decorations(&mut self, decorations: DecorationMap) {
        self.record_mut().decorations.merge(decorations);
    }

    /// Returns the root node wrapped as a [`DecoratedNode`]
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedTree, Language};
    ///
    /// let source = "fn main() {}";
    /// let mut parser = tree_sitter::Parser::new();
    /// parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    ///
    /// assert_eq!(tree.root_node().text(), source);
    /// ```
    pub fn root_node(&self) -> DecoratedNode<'_> {
        self.record().root_node()
    }

    /// Returns the language the tree was parsed as
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedTree, Language};
    ///
    /// let source = "fn main() {}";
    /// let mut parser = tree_sitter::Parser::new();
    /// parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    ///
    /// assert_eq!(tree.language(), Language::Rust);
    /// ```
    pub fn language(&self) -> Language {
        self.record().language
    }

    /// Returns the source text of the file
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedTree, Language};
    ///
    /// let source = "fn main() {}";
    /// let mut parser = tree_sitter::Parser::new();
    /// parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    ///
    /// assert_eq!(tree.source(), "fn main() {}");
    /// ```
    pub fn source(&self) -> &str {
        &self.record().source
    }

    /// Returns the file path
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedTree, Language};
    ///
    /// let source = "fn main() {}";
    /// let mut parser = tree_sitter::Parser::new();
    /// parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    ///
    /// assert_eq!(tree.file(), std::path::Path::new("src/main.rs"));
    /// ```
    pub fn file(&self) -> &Path {
        self.record().file.as_path()
    }

    /// Returns the record this tree owns
    fn record(&self) -> &Record {
        // SAFETY: the record lives until `drop` frees it, and a shared borrow
        // of the tree is a shared borrow of the record.
        unsafe { self.record.as_ref() }
    }

    /// Returns the record this tree owns, for a change
    fn record_mut(&mut self) -> &mut Record {
        // SAFETY: the record lives until `drop` frees it, and the tree owns it
        // alone. A unique borrow of the tree is therefore a unique borrow of
        // the record, and no node of it is alive.
        unsafe { self.record.as_mut() }
    }
}

impl Record {
    /// Returns the root of the tree as a node that reads through this record
    fn root_node(&self) -> DecoratedNode<'_> {
        DecoratedNode::builder()
            .node(TsNode::from(self.tree.root_node()))
            .context(&self.context)
            .build()
    }

    /// Parses `text` as `language`, and keeps the tree as long as this record
    ///
    /// The snippet takes this record's grammar when it is of the same
    /// language. A file of another dialect, such as TSX, therefore parses
    /// its snippets as the file itself parsed. The snippet has no
    /// decorations, and it names this record's file.
    ///
    /// The result is [`None`] when tree-sitter refuses the grammar, or
    /// stops before it finishes.
    fn parse_snippet(&self, language: Language, text: &str) -> Option<&Record> {
        let own = self.tree.language();
        let other = grammar_of(language);
        let grammar = match language == self.language {
            true => &*own,
            false => &other,
        };

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(grammar).ok()?;
        let tree = parser.parse(text, None)?;

        let snippet = DecoratedTree::builder()
            .tree(tree)
            .language(language)
            .source(text.to_owned())
            .file(self.file.clone())
            .build();
        let record = snippet.record;
        self.snippets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(snippet);

        // SAFETY: the snippet's record stays at its address until the
        // snippet drops, and the snippet drops only with this record.
        Some(unsafe { record.as_ref() })
    }
}

/// Returns the grammar the host parses `language` with by default
///
/// TypeScript takes the grammar without JSX, as `stiles_testing::parse`
/// does.
fn grammar_of(language: Language) -> tree_sitter::Language {
    match language {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    }
}

impl Drop for DecoratedTree {
    fn drop(&mut self) {
        // SAFETY: `new` made the pointer with `Box::into_raw`, and only this
        // drop frees it. No node outlives the tree, so nothing reads the
        // record afterwards.
        drop(unsafe { Box::from_raw(self.record.as_ptr()) });
    }
}

// SAFETY: the tree owns its record as a `Box` would, and the record is
// `Send`, as the assertion beside it checks.
unsafe impl Send for DecoratedTree {}
// SAFETY: a shared borrow of the tree only reads the record, or locks the
// mutex around its snippets, and the record is `Sync`.
unsafe impl Sync for DecoratedTree {}

impl std::fmt::Debug for DecoratedTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Record {
            file,
            language,
            source,
            ..
        } = self.record();

        f.debug_struct("DecoratedTree")
            .field("file", file)
            .field("language", language)
            .field("source_len", &source.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{Decoration, DecorationKey};

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Tag(u32);

    impl Decoration for Tag {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Tag"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    fn parse_tree(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn file_returns_path() {
        let source = "";
        let tree = parse_tree(source);
        let decorated = DecoratedTree::builder()
            .tree(tree)
            .language(Language::Rust)
            .source(source.into())
            .file(PathBuf::from("test.rs"))
            .build();

        assert_eq!(decorated.file(), Path::new("test.rs"));
    }

    #[test]
    fn merge_decorations_makes_decorations_visible_on_nodes() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let mut decorated = DecoratedTree::builder()
            .tree(tree)
            .language(Language::Rust)
            .source(source.into())
            .file(PathBuf::from("test.rs"))
            .build();
        let root_id = decorated.root_node().id();
        let mut decorations = DecorationMap::new();
        decorations.insert(root_id, Tag(7));

        decorated.merge_decorations(decorations);

        assert_eq!(decorated.root_node().decoration::<Tag>(), Some(&Tag(7)));
    }

    #[test]
    fn merge_decorations_twice_keeps_every_instance_in_order() {
        let decorated = merged_twice();

        let tags = decorated.root_node().decorations_of_type::<Tag>();

        assert_eq!(tags, vec![&Tag(1), &Tag(2)]);
    }

    #[test]
    fn merge_decorations_twice_keeps_first_of_a_type() {
        let decorated = merged_twice();

        let tag = decorated.root_node().decoration::<Tag>();

        assert_eq!(tag, Some(&Tag(1)));
    }

    /// Returns a tree whose root got `Tag(1)` from one merge and `Tag(2)`
    /// from a second merge
    fn merged_twice() -> DecoratedTree {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let mut decorated = DecoratedTree::builder()
            .tree(tree)
            .language(Language::Rust)
            .source(source.into())
            .file(PathBuf::from("test.rs"))
            .build();
        let root_id = decorated.root_node().id();
        let mut first = DecorationMap::new();
        first.insert(root_id, Tag(1));
        let mut second = DecorationMap::new();
        second.insert(root_id, Tag(2));

        decorated.merge_decorations(first);
        decorated.merge_decorations(second);

        decorated
    }

    #[test]
    fn root_node_kind_is_source_file() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorated = DecoratedTree::builder()
            .tree(tree)
            .language(Language::Rust)
            .source(source.into())
            .file(PathBuf::from("test.rs"))
            .build();

        assert_eq!(decorated.root_node().kind(), "source_file");
    }

    #[test]
    fn source_returns_original_text() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorated = DecoratedTree::builder()
            .tree(tree)
            .language(Language::Rust)
            .source(source.into())
            .file(PathBuf::from("test.rs"))
            .build();

        assert_eq!(decorated.source(), source);
    }

    #[test]
    fn trait_send_decorated_tree() {
        fn assert_send<T: Send>() {}
        assert_send::<DecoratedTree>();
    }

    #[test]
    fn trait_send_record() {
        fn assert_send<T: Send>() {}
        assert_send::<Record>();
    }

    #[test]
    fn trait_sync_decorated_tree() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecoratedTree>();
    }

    #[test]
    fn trait_sync_record() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Record>();
    }

    #[test]
    fn trait_unpin_decorated_tree() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecoratedTree>();
    }

    #[test]
    fn trait_unpin_record() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Record>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn source_roundtrips(source in "\\PC{0,200}") {
                let tree = parse_tree(&source);
                let decorated = DecoratedTree::builder()
                    .tree(tree)
                    .language(Language::Rust)
                    .source(source.clone())
                    .file(PathBuf::from("test.rs"))
                    .build();

                prop_assert_eq!(decorated.source(), source);
            }

            #[test]
            fn file_roundtrips(file in "[a-z]{1,10}\\.rs") {
                let tree = parse_tree("");
                let decorated = DecoratedTree::builder()
                    .tree(tree)
                    .language(Language::Rust)
                    .source(String::new())
                    .file(PathBuf::from(&file))
                    .build();

                prop_assert_eq!(decorated.file(), Path::new(&file));
            }

            #[test]
            fn root_is_source_file_or_error(source in "\\PC{0,200}") {
                let tree = parse_tree(&source);
                let decorated = DecoratedTree::builder()
                    .tree(tree)
                    .language(Language::Rust)
                    .source(source)
                    .file(PathBuf::from("test.rs"))
                    .build();

                let root = decorated.root_node();
                let kind = root.kind();
                prop_assert!(
                    kind == "source_file" || kind == "ERROR",
                    "unexpected root kind: {kind}"
                );
            }
        }
    }
}
