mod decorate_tree_error;

pub use self::decorate_tree_error::DecorateTreeError;
use crate::{Coverage, DecoratedTree, ProviderName};

/// Attaches semantic decorations to a parsed syntax tree
///
/// Decoration providers bridge between a language toolchain (e.g.
/// rust-analyzer) and the platform's decoration system. A provider reads
/// the syntax tree, queries the toolchain, and returns decorations
/// together with a [`Coverage`] verdict.
///
/// The verdict tells the caller whether the provider could analyze the
/// file. An empty decoration set is ambiguous: a file with nothing to
/// decorate and an unknown file produce the same output.
///
/// # Examples
///
/// ```
/// use stiles_types::{
///     Coverage, CoverageGap, DecorateTreeError, DecoratedTree, DecorationProvider, Language,
///     ProviderName,
/// };
///
/// /// Declines every file, because it has no toolchain to ask
/// struct Offline;
///
/// impl DecorationProvider for Offline {
///     fn name(&self) -> ProviderName {
///         ProviderName::new("offline")
///     }
///
///     fn decorate(&self, _tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError> {
///         Ok(Coverage::NotCovered(CoverageGap::NoProject))
///     }
/// }
///
/// # let source = "fn main() {}";
/// # let mut parser = tree_sitter::Parser::new();
/// # parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("the grammar should load");
/// # let tree = DecoratedTree::builder()
/// #     .tree(parser.parse(source, None).expect("the source should parse"))
/// #     .language(Language::Rust)
/// #     .source(source.to_string())
/// #     .file("src/main.rs")
/// #     .build();
/// let verdict = Offline.decorate(&tree)?;
///
/// let declined = match verdict {
///     Coverage::Covered(_) => false,
///     Coverage::NotCovered(_) => true,
/// };
///
/// assert!(declined);
/// # Ok::<(), DecorateTreeError>(())
/// ```
pub trait DecorationProvider: Send + Sync {
    /// Returns the name that identifies this provider in diagnostics
    fn name(&self) -> ProviderName;

    /// Decorates the file, or explains why the provider declines it
    ///
    /// The provider cannot mutate the tree, so a declined file carries no
    /// partial state.
    ///
    /// # Errors
    ///
    /// Returns a [`DecorateTreeError`] only when the toolchain itself
    /// malfunctions, for example a poisoned lock. Lack of information about a
    /// file is not an error; the provider reports [`Coverage::NotCovered`]
    /// instead.
    fn decorate(&self, tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecorationMap;

    struct Stub;

    impl DecorationProvider for Stub {
        fn name(&self) -> ProviderName {
            ProviderName::new("stub")
        }

        fn decorate(&self, _tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError> {
            Ok(Coverage::Covered(DecorationMap::new()))
        }
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Stub>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Stub>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Stub>();
    }
}
