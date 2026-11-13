use std::mem::MaybeUninit;

use bon::bon;
use stabby::abi::StableLike;
use stabby::str::Str;

use crate::context::{Context, StepFn};
use crate::decoration_entry::Entry;
use crate::ts_node::TsNode;
use crate::{Decoration, DecorationKey, Language, Position, Span};

/// A tree-sitter node enriched with semantic decorations
///
/// This is the primary type that lint rules interact with. It provides
/// access to the tree-sitter node's structural information (kind, text,
/// children) and to semantic decorations attached by providers.
///
/// A node is two fields: the bytes of a tree-sitter node, and a reference
/// to the host's table of functions for its tree. Every method calls
/// through that table. The host's own tree-sitter therefore reads the
/// host's tree, and a plugin never reads the node bytes itself. Nothing in
/// a node belongs to a library that the plugin and the host each link.
///
/// This is `Copy` so that a rule walking siblings and children writes what
/// it means. Without it, a read of a node out of the `Vec` that
/// [`DecoratedNode::named_children`] returns needs a clone. A copy moves
/// the node bytes and a reference, and reaches no allocation.
///
/// A node crosses the plugin boundary with every check a pass runs, so
/// stabby lays it out. Stabby cannot see into the table, so [`StableLike`]
/// carries the reference to it with the layout of any other reference.
/// The node borrows its tree, so it cannot outlive the tree.
///
/// # Examples
///
/// ```
/// # use stiles_types::{DecoratedTree, Language};
/// # let source = "fn main() { let x = 1; }";
/// # let mut parser = tree_sitter::Parser::new();
/// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
/// # let tree = DecoratedTree::builder()
/// #     .tree(parser.parse(source, None).expect("the source should parse"))
/// #     .language(Language::Rust)
/// #     .source(source.to_string())
/// #     .file("src/main.rs")
/// #     .build();
/// let root = tree.root_node();
/// let function = root.named_child(0).expect("the file should hold a function");
///
/// assert_eq!(function.kind(), "function_item");
/// ```
///
/// [`StableLike`]: stabby::abi::StableLike
#[stabby::stabby]
#[derive(Copy, Clone)]
pub struct DecoratedNode<'a> {
    node: TsNode,
    context: StableLike<&'a Context, &'a ()>,
}

#[bon]
impl<'a> DecoratedNode<'a> {
    /// Creates the node whose bytes are `node`, read through `context`
    ///
    /// Only the host calls this, with a node of the tree that `context`
    /// reads.
    #[builder]
    pub(crate) fn new(node: TsNode, context: &'a Context) -> Self {
        Self {
            node,
            context: StableLike::new(context),
        }
    }

    /// Returns the host's table for this node's tree
    fn context(&self) -> &'a Context {
        // SAFETY: a reference is a non-null pointer under any compiler, and
        // `StableLike::new` checked that it has the size and the alignment
        // of `&()`, whose layout stabby records in its place.
        unsafe { self.context.into_inner_unchecked() }
    }

    /// Returns the node that `find` writes, if it finds one
    ///
    /// `find` calls one entry of the table, and passes on its answer.
    fn adopt(
        &self,
        find: impl FnOnce(&'a Context, &mut MaybeUninit<TsNode>) -> bool,
    ) -> Option<Self> {
        let mut out = MaybeUninit::uninit();

        match find(self.context(), &mut out) {
            true => {
                // SAFETY: the host returns `true` only after it wrote a node
                // of this node's tree to `out`.
                let node = unsafe { out.assume_init() };
                Some(Self { node, ..*self })
            }
            false => None,
        }
    }

    /// Returns the node one `step` away from this one, if there is one
    fn step(&self, step: StepFn) -> Option<Self> {
        self.adopt(|context, out| step(context, &self.node, out))
    }

    /// Returns the `n`th decoration entry under `key` on this node, if any
    fn entry(&self, key: DecorationKey, n: usize) -> Option<&'a Entry> {
        let context = self.context();

        (context.decoration)(context, &self.node, key, n)
    }

    /// Returns the tree-sitter node kind (e.g. `"function_item"`)
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// assert_eq!(function.kind(), "function_item");
    /// ```
    pub fn kind(&self) -> &'a str {
        let context = self.context();

        (context.kind)(context, &self.node).as_str()
    }

    /// Returns the source text covered by this node
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// assert_eq!(function.text(), source);
    /// ```
    pub fn text(&self) -> &'a str {
        let context = self.context();

        (context.text)(context, &self.node).as_str()
    }

    /// Returns the tree-sitter node ID, used as the decoration map key
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// assert_ne!(function.id(), root.id());
    /// ```
    pub fn id(&self) -> usize {
        let context = self.context();

        (context.id)(context, &self.node)
    }

    /// Returns a [`Span`] covering this node's byte range
    ///
    /// The file path is reference-counted, so this is a cheap operation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// assert_eq!(function.span().end(), source.len());
    /// ```
    pub fn span(&self) -> Span {
        let context = self.context();

        Span::builder()
            .file((context.file)(context).clone())
            .start((context.start_byte)(context, &self.node))
            .end((context.end_byte)(context, &self.node))
            .build()
    }

    /// Returns the number of named children
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// assert_eq!(root.named_child_count(), 1);
    /// ```
    pub fn named_child_count(&self) -> usize {
        let context = self.context();

        (context.named_child_count)(context, &self.node)
    }

    /// Returns the named child at the given index, if it exists
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// assert!(root.named_child(1).is_none());
    /// ```
    pub fn named_child(&self, index: u32) -> Option<DecoratedNode<'a>> {
        self.adopt(|context, out| (context.named_child)(context, &self.node, index, out))
    }

    /// Returns a child node by its field name, if it exists
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// let name = function.child_by_field_name("name").expect("a function has a name");
    ///
    /// assert_eq!(name.text(), "main");
    /// ```
    pub fn child_by_field_name(&self, name: &str) -> Option<DecoratedNode<'a>> {
        self.adopt(|context, out| {
            (context.child_by_field_name)(context, &self.node, Str::new(name), out)
        })
    }

    /// Returns the total number of children (named and anonymous)
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// assert_eq!(function.child_count(), 4);
    /// ```
    pub fn child_count(&self) -> usize {
        let context = self.context();

        (context.child_count)(context, &self.node)
    }

    /// Returns the child at the given index (named or anonymous)
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// let keyword = function.child(0).expect("a function starts with a keyword");
    ///
    /// assert_eq!(keyword.kind(), "fn");
    /// ```
    pub fn child(&self, index: u32) -> Option<DecoratedNode<'a>> {
        self.adopt(|context, out| (context.child)(context, &self.node, index, out))
    }

    /// Returns whether this is a named node (as opposed to an anonymous one)
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// let keyword = function.child(0).expect("a function starts with a keyword");
    ///
    /// assert!(!keyword.is_named());
    /// ```
    pub fn is_named(&self) -> bool {
        let context = self.context();

        (context.is_named)(context, &self.node)
    }

    /// Returns the parent node, if this is not the root
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// assert!(root.parent().is_none());
    /// ```
    pub fn parent(&self) -> Option<DecoratedNode<'a>> {
        self.step(self.context().parent)
    }

    /// Retrieves the first decoration of type `T` from this node
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, DecoratedTree, Decoration, DecorationKey, Language};
    ///
    /// /// The number of bindings in a block
    /// #[stabby::stabby]
    /// struct Bindings(u32);
    ///
    /// impl Decoration for Bindings {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Bindings"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let mut tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    /// let id = tree.root_node().id();
    /// tree.decorations_mut().insert(id, Bindings(1));
    /// tree.decorations_mut().insert(id, Bindings(2));
    /// let root = tree.root_node();
    ///
    /// assert_eq!(root.decoration::<Bindings>().map(|Bindings(count)| *count), Some(1));
    /// ```
    pub fn decoration<T: Decoration>(&self) -> Option<&'a T> {
        self.entry(T::KEY, 0)?.read::<T>()
    }

    /// Retrieves all decorations of type `T` from this node
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, DecoratedTree, Decoration, DecorationKey, Language};
    ///
    /// /// The number of bindings in a block
    /// #[stabby::stabby]
    /// struct Bindings(u32);
    ///
    /// impl Decoration for Bindings {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Bindings"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let mut tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    /// let id = tree.root_node().id();
    /// tree.decorations_mut().insert(id, Bindings(1));
    /// tree.decorations_mut().insert(id, Bindings(2));
    /// let root = tree.root_node();
    ///
    /// assert_eq!(root.decorations_of_type::<Bindings>().len(), 2);
    /// ```
    pub fn decorations_of_type<T: Decoration>(&self) -> Vec<&'a T> {
        (0..)
            .map_while(|n| self.entry(T::KEY, n))
            .filter_map(Entry::read::<T>)
            .collect()
    }

    /// Reads the decoration `D` from this node
    ///
    /// The shape of the result comes from `D` itself, not from this call. A
    /// decoration declared with cardinality `one` yields [`Option`], and one
    /// declared `many` yields [`Vec`]. Prefer this over [`decoration`] and
    /// [`decorations_of_type`], which accept any type. They therefore cannot
    /// catch a rule that reads a repeated decoration as though it were
    /// singular.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, DecoratedTree, Decoration, DecorationKey, Language};
    ///
    /// /// The number of bindings in a block
    /// #[stabby::stabby]
    /// struct Bindings(u32);
    ///
    /// impl Decoration for Bindings {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Bindings"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// let mut tree = DecoratedTree::builder()
    ///     .tree(parser.parse(source, None).expect("the source should parse"))
    ///     .language(Language::Rust)
    ///     .source(source.to_string())
    ///     .file("src/main.rs")
    ///     .build();
    /// let id = tree.root_node().id();
    /// tree.decorations_mut().insert(id, Bindings(1));
    /// tree.decorations_mut().insert(id, Bindings(2));
    /// let root = tree.root_node();
    ///
    /// let first: Option<&Bindings> = root.get::<Bindings>();
    ///
    /// assert_eq!(first.map(|Bindings(count)| *count), Some(1));
    /// ```
    ///
    /// [`decoration`]: DecoratedNode::decoration
    /// [`decorations_of_type`]: DecoratedNode::decorations_of_type
    pub fn get<D: Decoration>(&self) -> D::Ref<'a> {
        D::lookup(self)
    }

    /// Returns all named children of this node as a collected vec
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    /// let function = root.named_child(0).expect("the file should hold a function");
    ///
    /// let kinds: Vec<&str> = function.named_children().iter().map(|child| child.kind()).collect();
    ///
    /// assert_eq!(kinds, ["identifier", "parameters", "block"]);
    /// ```
    pub fn named_children(&self) -> Vec<DecoratedNode<'a>> {
        (0..).map_while(|index| self.named_child(index)).collect()
    }

    /// Returns whether this node or one below it is a syntax error
    ///
    /// Tree-sitter recovers from a syntax error and still builds a tree.
    /// The tree holds an `ERROR` node, or a missing node it inserted, where
    /// the source broke the grammar. This flag says whether any such node
    /// sits at or below this one.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main( {}";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let root = tree.root_node();
    ///
    /// assert!(root.has_error());
    /// ```
    pub fn has_error(&self) -> bool {
        let context = self.context();

        (context.has_error)(context, &self.node)
    }

    /// Returns the row and the column where this node starts
    ///
    /// [`Position`] says how the two count.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn first() {}\nfn second() {}";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let second = tree.root_node().named_child(1).expect("the file should hold two functions");
    ///
    /// assert_eq!(second.start_position().row, 1);
    /// ```
    pub fn start_position(&self) -> Position {
        let context = self.context();

        (context.start_position)(context, &self.node)
    }

    /// Returns the row and the column just past the end of this node
    ///
    /// [`Position`] says how the two count.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn first() {}\nfn second() {}";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let first = tree.root_node().named_child(0).expect("the file should hold two functions");
    ///
    /// assert_eq!(first.end_position().column, 13);
    /// ```
    pub fn end_position(&self) -> Position {
        let context = self.context();

        (context.end_position)(context, &self.node)
    }

    /// Returns the node after this one under the same parent, if any
    ///
    /// The sibling may be anonymous, such as a punctuation token.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn first() {}\nfn second() {}";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let first = tree.root_node().named_child(0).expect("the file should hold two functions");
    ///
    /// let second = first.next_sibling().expect("a function follows the first");
    ///
    /// assert_eq!(second.text(), "fn second() {}");
    /// ```
    pub fn next_sibling(&self) -> Option<DecoratedNode<'a>> {
        self.step(self.context().next_sibling)
    }

    /// Returns the node before this one under the same parent, if any
    ///
    /// The sibling may be anonymous, such as a punctuation token.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn first() {}\nfn second() {}";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let first = tree.root_node().named_child(0).expect("the file should hold two functions");
    ///
    /// assert!(first.prev_sibling().is_none());
    /// ```
    pub fn prev_sibling(&self) -> Option<DecoratedNode<'a>> {
        self.step(self.context().prev_sibling)
    }

    /// Returns the named node after this one under the same parent, if any
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let function = tree.root_node().named_child(0).expect("the file should hold a function");
    /// let name = function.child_by_field_name("name").expect("a function has a name");
    ///
    /// let parameters = name.next_named_sibling().expect("the parameters follow the name");
    ///
    /// assert_eq!(parameters.kind(), "parameters");
    /// ```
    pub fn next_named_sibling(&self) -> Option<DecoratedNode<'a>> {
        self.step(self.context().next_named_sibling)
    }

    /// Returns the named node before this one under the same parent, if any
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "fn main() { let x = 1; }";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let function = tree.root_node().named_child(0).expect("the file should hold a function");
    /// let name = function.child_by_field_name("name").expect("a function has a name");
    ///
    /// assert!(name.prev_named_sibling().is_none());
    /// ```
    pub fn prev_named_sibling(&self) -> Option<DecoratedNode<'a>> {
        self.step(self.context().prev_named_sibling)
    }

    /// Parses `text` as `language` with the host's grammar, and returns its root
    ///
    /// A rule uses this to read a snippet as code, such as the body of a
    /// comment. The host parses it with the grammar of this node's tree
    /// when `language` is that tree's language, and with its default
    /// grammar for `language` otherwise.
    ///
    /// The snippet's tree lives as long as this node's tree, which keeps
    /// every snippet it parsed until the file is done. A pass that parses
    /// each comment of a file therefore holds one small tree per comment
    /// until then. The snippet's nodes carry no decorations, and their
    /// spans name this node's file. A span counts bytes from the start of
    /// `text`, not of the file.
    ///
    /// The result is [`None`] when the host has no grammar for `language`,
    /// such as a language a newer plugin knows and this host does not.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stiles_types::{DecoratedTree, Language};
    /// # let source = "// let x = 1;";
    /// # let mut parser = tree_sitter::Parser::new();
    /// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
    /// # let tree = DecoratedTree::builder()
    /// #     .tree(parser.parse(source, None).expect("the source should parse"))
    /// #     .language(Language::Rust)
    /// #     .source(source.to_string())
    /// #     .file("src/main.rs")
    /// #     .build();
    /// let comment = tree.root_node().named_child(0).expect("the file should hold a comment");
    /// let body = comment.text().trim_start_matches("//");
    ///
    /// let snippet = comment.parse(Language::Rust, body).expect("the host should parse Rust");
    ///
    /// assert_eq!(snippet.kind(), "source_file");
    /// ```
    pub fn parse(&self, language: Language, text: &str) -> Option<DecoratedNode<'a>> {
        let context = self.context();
        let mut out = MaybeUninit::uninit();

        let parsed = (context.parse)(context, language.to_byte(), Str::new(text), &mut out);

        match parsed {
            true => {
                // SAFETY: the host returns `true` only after it wrote a node
                // to `out`.
                let snippet = unsafe { out.assume_init() };
                Some(snippet)
            }
            false => None,
        }
    }
}

impl std::fmt::Debug for DecoratedNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let span = self.span();

        f.debug_struct("DecoratedNode")
            .field("kind", &self.kind())
            .field("start_byte", &span.start())
            .field("end_byte", &span.end())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecoratedTree, DecorationMap};

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct TestDeco(u32);

    impl Decoration for TestDeco {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::TestDeco"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Missing;

    impl Decoration for Missing {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Missing"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Value(u64);

    impl Decoration for Value {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Value"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    /// Stores a `TestDeco` under the key of `Value`, as a plugin with a stale
    /// definition would
    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Impostor(u64);

    impl Decoration for Impostor {
        const KEY: DecorationKey = Value::KEY;

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

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

    /// Returns a tree whose root holds `decorations`
    fn decorated(
        source: &str,
        decorations: impl FnOnce(usize, &mut DecorationMap),
    ) -> DecoratedTree {
        let mut tree = tree_of(source);
        let id = tree.root_node().id();
        decorations(id, tree.decorations_mut());
        tree
    }

    #[test]
    fn child_by_field_name_with_an_unknown_field_returns_none() {
        let tree = tree_of("fn main() {}");
        let function = tree.root_node().named_child(0).unwrap();

        let found = function.child_by_field_name("no_such_field");

        assert!(found.is_none());
    }

    #[test]
    fn decoration_on_another_node_returns_none() {
        let tree = decorated("fn main() {}", |id, map| map.insert(id, TestDeco(1)));
        let function = tree.root_node().named_child(0).unwrap();

        let deco = function.decoration::<TestDeco>();

        assert!(deco.is_none());
    }

    #[test]
    fn decoration_returns_attached_value() {
        let tree = decorated("fn main() {}", |id, map| map.insert(id, TestDeco(42)));

        let deco = tree.root_node().decoration::<TestDeco>();

        assert_eq!(deco, Some(&TestDeco(42)));
    }

    #[test]
    fn decoration_returns_none_when_missing() {
        let tree = decorated("fn main() {}", |id, map| map.insert(id, TestDeco(1)));

        let deco = tree.root_node().decoration::<Missing>();

        assert!(deco.is_none());
    }

    #[test]
    fn decoration_returns_the_first_of_several() {
        let tree = decorated("fn main() {}", |id, map| {
            map.insert(id, Value(9));
            map.insert(id, TestDeco(1));
            map.insert(id, TestDeco(2));
        });

        let deco = tree.root_node().decoration::<TestDeco>();

        assert_eq!(deco, Some(&TestDeco(1)));
    }

    #[test]
    fn decoration_with_another_type_under_its_key_returns_none() {
        let tree = decorated("fn main() {}", |id, map| map.insert(id, Value(7)));

        let deco = tree.root_node().decoration::<Impostor>();

        assert!(deco.is_none());
    }

    #[test]
    fn decorations_of_type_returns_every_value_in_order() {
        let tree = decorated("fn main() {}", |id, map| {
            map.insert(id, TestDeco(1));
            map.insert(id, Value(9));
            map.insert(id, TestDeco(2));
        });

        let found = tree.root_node().decorations_of_type::<TestDeco>();

        assert_eq!(found, vec![&TestDeco(1), &TestDeco(2)]);
    }

    #[test]
    fn decorations_of_type_when_missing_returns_nothing() {
        let tree = tree_of("fn main() {}");

        let found = tree.root_node().decorations_of_type::<TestDeco>();

        assert!(found.is_empty());
    }

    #[test]
    fn end_position_counts_the_row_from_zero() {
        let tree = tree_of("fn first() {}\nfn second() {}\n");
        let second = tree.root_node().named_child(1).unwrap();

        let end = second.end_position();

        assert_eq!(end, Position { row: 1, column: 14 });
    }

    #[test]
    fn has_error_with_a_broken_source_returns_true() {
        let tree = tree_of("fn main( {}");

        let broken = tree.root_node().has_error();

        assert!(broken);
    }

    #[test]
    fn has_error_with_a_valid_source_returns_false() {
        let tree = tree_of("fn main() {}");

        let broken = tree.root_node().has_error();

        assert!(!broken);
    }

    #[test]
    fn kind_returns_node_kind() {
        let tree = tree_of("fn main() {}");

        let kind = tree.root_node().kind();

        assert_eq!(kind, "source_file");
    }

    #[test]
    fn named_child_returns_first_item() {
        let tree = tree_of("fn main() {}");

        let first_child = tree
            .root_node()
            .named_child(0)
            .expect("should have a child");

        assert_eq!(first_child.kind(), "function_item");
    }

    #[test]
    fn next_named_sibling_skips_anonymous_nodes() {
        let tree = tree_of("fn main() { let x = 1; }");
        let function = tree.root_node().named_child(0).unwrap();
        let keyword = function.child(0).unwrap();

        let next = keyword
            .next_named_sibling()
            .expect("the name follows the keyword");

        assert_eq!(next.text(), "main");
    }

    #[test]
    fn next_sibling_of_the_last_child_returns_none() {
        let tree = tree_of("fn main() {}");
        let function = tree.root_node().named_child(0).unwrap();
        let body = function.child(3).unwrap();

        let next = body.next_sibling();

        assert!(next.is_none());
    }

    #[test]
    fn next_sibling_returns_an_anonymous_node() {
        let tree = tree_of("fn main() {}");
        let function = tree.root_node().named_child(0).unwrap();
        let keyword = function.child(0).unwrap();
        let name = keyword.next_sibling().unwrap();
        let parameters = name.next_sibling().unwrap();
        let open = parameters.child(0).unwrap();

        let close = open.next_sibling().expect("the parenthesis closes");

        assert_eq!(close.text(), ")");
    }

    #[test]
    fn parent_of_a_child_returns_the_node() {
        let tree = tree_of("fn main() {}");
        let root = tree.root_node();
        let function = root.named_child(0).unwrap();

        let parent = function.parent().expect("a child has a parent");

        assert_eq!(parent.id(), root.id());
    }

    #[test]
    fn parse_from_a_snippet_parses_again() {
        let tree = tree_of("// fn main() {}");
        let comment = tree.root_node().named_child(0).unwrap();
        let snippet = comment.parse(Language::Rust, "fn outer() {}").unwrap();

        let nested = snippet.parse(Language::Rust, "fn inner() {}").unwrap();

        assert_eq!(nested.text(), "fn inner() {}");
    }

    #[test]
    fn parse_keeps_every_snippet_alive() {
        let tree = tree_of("fn main() {}");
        let root = tree.root_node();
        let first = root.parse(Language::Rust, "fn first() {}").unwrap();

        for _ in 0..64 {
            root.parse(Language::Rust, "fn later() {}").unwrap();
        }

        assert_eq!(first.text(), "fn first() {}");
    }

    #[test]
    fn parse_with_another_language_uses_its_grammar() {
        let tree = tree_of("fn main() {}");

        let snippet = tree
            .root_node()
            .parse(Language::TypeScript, "let x = 1;")
            .unwrap();

        assert_eq!(snippet.kind(), "program");
    }

    #[test]
    fn parse_with_broken_text_returns_a_tree_with_an_error() {
        let tree = tree_of("fn main() {}");

        let snippet = tree
            .root_node()
            .parse(Language::Rust, "fn main( {}")
            .unwrap();

        assert!(snippet.has_error());
    }

    #[test]
    fn parse_with_the_tree_language_returns_the_snippet_root() {
        let tree = tree_of("fn main() {}");

        let snippet = tree
            .root_node()
            .parse(Language::Rust, "let x = 1;")
            .unwrap();

        assert_eq!(snippet.kind(), "source_file");
    }

    #[test]
    fn parse_yields_nodes_without_decorations() {
        let tree = decorated("fn main() {}", |id, map| map.insert(id, TestDeco(1)));

        let snippet = tree
            .root_node()
            .parse(Language::Rust, "fn main() {}")
            .unwrap();

        assert!(snippet.decoration::<TestDeco>().is_none());
    }

    #[test]
    fn parse_yields_spans_in_the_same_file() {
        let tree = tree_of("fn main() {}");

        let snippet = tree
            .root_node()
            .parse(Language::Rust, "fn main() {}")
            .unwrap();

        assert_eq!(
            snippet.span().file_path(),
            tree.root_node().span().file_path()
        );
    }

    #[test]
    fn parse_yields_spans_within_the_snippet() {
        let tree = tree_of("fn main() {}\n// let x = 1;");
        let comment = tree.root_node().named_child(1).unwrap();
        let snippet = comment.parse(Language::Rust, "let x = 1;").unwrap();

        let span = snippet.named_child(0).unwrap().span();

        assert_eq!(span.start()..span.end(), 0..10);
    }

    #[test]
    fn prev_named_sibling_skips_anonymous_nodes() {
        let tree = tree_of("fn main() {}");
        let function = tree.root_node().named_child(0).unwrap();
        let parameters = function.child_by_field_name("parameters").unwrap();

        let previous = parameters.prev_named_sibling().expect("the name precedes");

        assert_eq!(previous.text(), "main");
    }

    #[test]
    fn prev_sibling_returns_an_anonymous_node() {
        let tree = tree_of("fn main() {}");
        let function = tree.root_node().named_child(0).unwrap();
        let name = function.child_by_field_name("name").unwrap();

        let previous = name.prev_sibling().expect("the keyword precedes the name");

        assert_eq!(previous.text(), "fn");
    }

    #[test]
    fn span_covers_node_range() {
        let source = "fn main() {}";
        let tree = tree_of(source);

        let span = tree.root_node().span();

        assert_eq!(span.start()..span.end(), 0..source.len());
    }

    #[test]
    fn start_position_counts_the_column_in_bytes() {
        let tree = tree_of("const S: &str = \"É\"; fn main() {}");
        let function = tree.root_node().named_child(1).unwrap();

        let start = function.start_position();

        assert_eq!(start, Position { row: 0, column: 22 });
    }

    #[test]
    fn start_position_counts_the_row_from_zero() {
        let tree = tree_of("fn first() {}\nfn second() {}\n");
        let second = tree.root_node().named_child(1).unwrap();

        let start = second.start_position();

        assert_eq!(start, Position { row: 1, column: 0 });
    }

    #[test]
    fn text_returns_source_slice() {
        let source = "fn main() {}";
        let tree = tree_of(source);

        let text = tree.root_node().text();

        assert_eq!(text, source);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecoratedNode<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecoratedNode<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecoratedNode<'_>>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn root_text_equals_source(source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}") {
                let tree = tree_of(&source);

                prop_assert_eq!(tree.root_node().text(), source.as_str());
            }

            #[test]
            fn root_span_covers_full_source(source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}") {
                let tree = tree_of(&source);
                let span = tree.root_node().span();

                prop_assert_eq!(span.start()..span.end(), 0..source.len());
            }

            #[test]
            fn named_child_count_matches_named_children_len(source in "\\PC{0,200}") {
                let tree = tree_of(&source);
                let root = tree.root_node();

                prop_assert_eq!(
                    root.named_child_count(),
                    root.named_children().len()
                );
            }

            #[test]
            fn named_child_out_of_bounds_returns_none(
                index in 1000..=u32::MAX,
            ) {
                let tree = tree_of("fn main() {}");

                prop_assert!(tree.root_node().named_child(index).is_none());
            }

            #[test]
            fn decoration_roundtrips_through_node(value: u64) {
                let tree = decorated("fn main() {}", |id, map| map.insert(id, Value(value)));

                prop_assert_eq!(tree.root_node().decoration::<Value>(), Some(&Value(value)));
            }

            #[test]
            fn parse_text_equals_the_snippet(snippet in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}") {
                let tree = tree_of("fn main() {}");

                let parsed = tree.root_node().parse(Language::Rust, &snippet).unwrap();

                prop_assert_eq!(parsed.text(), snippet.as_str());
            }
        }
    }
}
