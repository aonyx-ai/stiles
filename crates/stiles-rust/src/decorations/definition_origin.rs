/// Where a definition sits relative to the crate of the file that names it
///
/// A rule reads it to tell a call into the crate's own code from a call into
/// a dependency or the standard library. The crate of the file is the one
/// the provider resolves the file in, which [`Target`] names.
///
/// A builtin belongs to no crate. The language defines a primitive type, a
/// builtin attribute, and a tool module, so none of them is in the crate or
/// outside it.
///
/// A rule reads it out of memory the host allocated, so the discriminant is
/// a byte.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::DefinitionOrigin;
///
/// let origin = DefinitionOrigin::OtherCrate;
///
/// let calls_its_own_code = match origin {
///     DefinitionOrigin::SameCrate => true,
///     DefinitionOrigin::OtherCrate | DefinitionOrigin::Builtin => false,
/// };
///
/// assert!(!calls_its_own_code);
/// ```
///
/// [`Target`]: crate::decorations::Target
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum DefinitionOrigin {
    /// The crate of the file defines it
    SameCrate,
    /// Another crate defines it, such as a dependency or `std`
    OtherCrate,
    /// The language defines it, and no crate does
    Builtin,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_definition_origin() {
        fn assert_send<T: Send>() {}
        assert_send::<DefinitionOrigin>();
    }

    #[test]
    fn trait_sync_definition_origin() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DefinitionOrigin>();
    }

    #[test]
    fn trait_unpin_definition_origin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DefinitionOrigin>();
    }
}
