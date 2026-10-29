/// Says whether a resolved type is `Option<T>`
///
/// [`ResolvedType`] holds it, and [`ResolvedType::with_option_kind`] sets
/// it. A rule reads it out of memory the host allocated, so the discriminant
/// is a byte. That is a layout that holds from one compiler to the next.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{OptionKind, ResolvedType};
///
/// let ty = ResolvedType::new("Option<u8>".to_string()).with_option_kind(OptionKind::Option);
///
/// assert!(ty.is_option());
/// ```
///
/// [`ResolvedType`]: crate::decorations::ResolvedType
/// [`ResolvedType::with_option_kind`]: crate::decorations::ResolvedType::with_option_kind
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum OptionKind {
    /// The type is `core::option::Option`
    Option,
    /// The type is any type but `Option`
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_option_kind() {
        fn assert_send<T: Send>() {}
        assert_send::<OptionKind>();
    }

    #[test]
    fn trait_sync_option_kind() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<OptionKind>();
    }

    #[test]
    fn trait_unpin_option_kind() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<OptionKind>();
    }
}
