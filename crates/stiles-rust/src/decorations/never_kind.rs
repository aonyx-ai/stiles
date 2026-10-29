/// Says whether a resolved type is the never type (`!`)
///
/// [`ResolvedType`] holds it, and [`ResolvedType::with_never_kind`] sets it.
/// A rule reads it out of memory the host allocated, so the discriminant
/// is a byte. That is a layout that holds from one compiler to the next.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{NeverKind, ResolvedType};
///
/// let ty = ResolvedType::new("!".to_string()).with_never_kind(NeverKind::Never);
///
/// assert!(ty.is_never());
/// ```
///
/// [`ResolvedType`]: crate::decorations::ResolvedType
/// [`ResolvedType::with_never_kind`]: crate::decorations::ResolvedType::with_never_kind
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum NeverKind {
    /// The type is `!`
    Never,
    /// The type is any type but `!`
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_never_kind() {
        fn assert_send<T: Send>() {}
        assert_send::<NeverKind>();
    }

    #[test]
    fn trait_sync_never_kind() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<NeverKind>();
    }

    #[test]
    fn trait_unpin_never_kind() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<NeverKind>();
    }
}
