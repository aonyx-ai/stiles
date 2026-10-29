/// Says whether a resolved type is `Result<T, E>`
///
/// [`ResolvedType::with_result_kind`] takes it in place of a `bool`, so a call
/// site names the answer it records.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{ResultKind, ResolvedType};
///
/// let ty = ResolvedType::new("Result<(), Error>".to_string()).with_result_kind(ResultKind::Result);
///
/// assert!(ty.is_result());
/// ```
///
/// [`ResolvedType::with_result_kind`]: crate::decorations::ResolvedType::with_result_kind
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum ResultKind {
    /// The type is `core::result::Result`
    Result,
    /// The type is any type but `Result`
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_result_kind() {
        fn assert_send<T: Send>() {}
        assert_send::<ResultKind>();
    }

    #[test]
    fn trait_sync_result_kind() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ResultKind>();
    }

    #[test]
    fn trait_unpin_result_kind() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ResultKind>();
    }
}
