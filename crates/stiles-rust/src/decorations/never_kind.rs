/// Says whether a resolved type is the never type (`!`)
///
/// [`ResolvedType::with_never_kind`] takes it in place of a `bool`, so a call
/// site names the answer it records.
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
/// [`ResolvedType::with_never_kind`]: crate::decorations::ResolvedType::with_never_kind
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
