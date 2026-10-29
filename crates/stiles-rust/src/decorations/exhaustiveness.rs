/// Says whether a match on an enum can list every variant
///
/// An enum that another crate marks `#[non_exhaustive]` may grow a variant
/// in any release. A match on it outside that crate therefore needs a
/// wildcard arm. Inside its own crate the attribute changes nothing.
/// [`AdtFlags::from_exhaustiveness`] takes it in place of a `bool`.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{AdtFlags, Exhaustiveness};
///
/// let flags = AdtFlags::from_exhaustiveness(Exhaustiveness::NonExhaustive);
///
/// assert!(flags.non_exhaustive_external());
/// ```
///
/// [`AdtFlags::from_exhaustiveness`]: crate::decorations::AdtFlags::from_exhaustiveness
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Exhaustiveness {
    /// A match can list every variant: the enum is exhaustive, or the
    /// matching crate defines it
    Exhaustive,
    /// The enum is `#[non_exhaustive]` and another crate defines it, so a
    /// match needs a wildcard arm
    NonExhaustive,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_exhaustiveness() {
        fn assert_send<T: Send>() {}
        assert_send::<Exhaustiveness>();
    }

    #[test]
    fn trait_sync_exhaustiveness() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Exhaustiveness>();
    }

    #[test]
    fn trait_unpin_exhaustiveness() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Exhaustiveness>();
    }
}
