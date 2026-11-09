use stiles_macros::Decoration;

/// Says whether another crate can name an item
///
/// A rule sees one file, and the `mod seal;` that hides a `pub` item often
/// sits in another. The `pub` keyword therefore proves nothing about the
/// item. A rule that asks for a doctest needs this answer, because a
/// doctest compiles as an outside crate.
///
/// The provider attaches it to a `function_item`, `struct_item`,
/// `enum_item`, `union_item`, `trait_item`, `type_item`, `const_item`, or
/// `static_item` that sits directly in a module. An associated item, a
/// method, a member of an `extern` block, and an item in a function body
/// get none. Neither does an item rust-analyzer cannot resolve, so an
/// absent decoration proves nothing either way.
///
/// The answer comes from the import map of the crate that defines the
/// item. That map follows `pub` names from the crate root, so it sees a
/// re-export from a private module. A `const _` has no name, so it is
/// [`Internal`].
///
/// A `#[doc(hidden)]` mark does not change the answer. The mark removes a
/// path from the documentation, and the compiler still accepts that path.
/// A doctest can name such an item, so the item is [`Public`]. The
/// [`Reexports`] decoration differs on purpose, because it lists only the
/// paths that an author offers.
///
/// Every crate counts as a library, because rust-analyzer does not record
/// the kind of a target. A `pub` function at the root of a binary is
/// therefore [`Public`], although no crate can depend on a binary. Analysis
/// enables the `test` cfg, so a `#[cfg(test)] pub mod` also counts as a
/// public path.
///
/// A rule reads it out of memory the host allocated, so the discriminant
/// is a byte.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::ItemExposure;
///
/// let exposure = ItemExposure::Internal;
///
/// let wants_doctest = match exposure {
///     ItemExposure::Public => true,
///     ItemExposure::Internal => false,
/// };
///
/// assert!(!wants_doctest);
/// ```
///
/// [`Internal`]: ItemExposure::Internal
/// [`Public`]: ItemExposure::Public
/// [`Reexports`]: crate::decorations::Reexports
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub enum ItemExposure {
    /// A dependent crate can name the item through some public path
    Public,
    /// No path from outside the defining crate reaches the item
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_item_exposure() {
        fn assert_send<T: Send>() {}

        assert_send::<ItemExposure>();
    }

    #[test]
    fn trait_sync_item_exposure() {
        fn assert_sync<T: Sync>() {}

        assert_sync::<ItemExposure>();
    }

    #[test]
    fn trait_unpin_item_exposure() {
        fn assert_unpin<T: Unpin>() {}

        assert_unpin::<ItemExposure>();
    }
}
