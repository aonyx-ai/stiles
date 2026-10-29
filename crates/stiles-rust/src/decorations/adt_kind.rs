/// Says whether a resolved type is an enum
///
/// [`ResolvedType`] holds it, and [`ResolvedType::with_adt_kind`] sets it.
/// A rule reads it out of memory the host allocated, so the discriminant
/// is a byte. That is a layout that holds from one compiler to the next.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{AdtKind, ResolvedType};
///
/// let ty = ResolvedType::new("Shape".to_string()).with_adt_kind(AdtKind::Enum);
///
/// assert!(ty.is_enum());
/// ```
///
/// [`ResolvedType`]: crate::decorations::ResolvedType
/// [`ResolvedType::with_adt_kind`]: crate::decorations::ResolvedType::with_adt_kind
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum AdtKind {
    /// The type is an enum
    Enum,
    /// The type is a struct, a union, or no algebraic data type at all
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_adt_kind() {
        fn assert_send<T: Send>() {}
        assert_send::<AdtKind>();
    }

    #[test]
    fn trait_sync_adt_kind() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AdtKind>();
    }

    #[test]
    fn trait_unpin_adt_kind() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AdtKind>();
    }
}
