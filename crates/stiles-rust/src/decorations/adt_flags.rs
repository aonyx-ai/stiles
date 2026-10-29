use stiles_macros::Decoration;

use crate::decorations::Exhaustiveness;

/// ADT-specific flags for enum types
///
/// Attached alongside [`ResolvedType`] when the type is an enum.
/// Provides additional information about the ADT that rules need for
/// decisions like whether a wildcard match arm is acceptable.
///
/// A rule reads it out of memory the host allocated, so stabby lays it
/// out. It holds an [`Exhaustiveness`] rather than a `bool`, so a call
/// site that records it names the answer it gives.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{AdtFlags, Exhaustiveness};
///
/// let flags = AdtFlags::from_exhaustiveness(Exhaustiveness::Exhaustive);
///
/// assert!(!flags.non_exhaustive_external());
/// ```
///
/// [`Exhaustiveness`]: crate::decorations::Exhaustiveness
/// [`ResolvedType`]: crate::decorations::ResolvedType
#[stabby::stabby]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct AdtFlags {
    exhaustiveness: Exhaustiveness,
}

impl AdtFlags {
    /// Creates ADT flags from how exhaustive a match on the enum can be
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
    pub fn from_exhaustiveness(exhaustiveness: Exhaustiveness) -> Self {
        Self { exhaustiveness }
    }

    /// Returns how exhaustive a match on the enum can be
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{AdtFlags, Exhaustiveness};
    ///
    /// let flags = AdtFlags::from_exhaustiveness(Exhaustiveness::NonExhaustive);
    ///
    /// assert_eq!(flags.exhaustiveness(), Exhaustiveness::NonExhaustive);
    /// ```
    pub fn exhaustiveness(&self) -> Exhaustiveness {
        self.exhaustiveness
    }

    /// Returns whether the enum is `#[non_exhaustive]` from an external crate
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{AdtFlags, Exhaustiveness};
    ///
    /// let flags = AdtFlags::from_exhaustiveness(Exhaustiveness::Exhaustive);
    ///
    /// assert!(!flags.non_exhaustive_external());
    /// ```
    pub fn non_exhaustive_external(&self) -> bool {
        match self.exhaustiveness {
            Exhaustiveness::Exhaustive => false,
            Exhaustiveness::NonExhaustive => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustiveness_returns_the_recorded_answer() {
        let flags = AdtFlags::from_exhaustiveness(Exhaustiveness::NonExhaustive);

        assert_eq!(flags.exhaustiveness(), Exhaustiveness::NonExhaustive);
    }

    #[test]
    fn from_exhaustiveness_exhaustive_leaves_the_flag_unset() {
        let flags = AdtFlags::from_exhaustiveness(Exhaustiveness::Exhaustive);

        assert!(!flags.non_exhaustive_external());
    }

    #[test]
    fn from_exhaustiveness_non_exhaustive_sets_the_flag() {
        let flags = AdtFlags::from_exhaustiveness(Exhaustiveness::NonExhaustive);

        assert!(flags.non_exhaustive_external());
    }

    #[test]
    fn trait_send_adt_flags() {
        fn assert_send<T: Send>() {}
        assert_send::<AdtFlags>();
    }

    #[test]
    fn trait_sync_adt_flags() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AdtFlags>();
    }

    #[test]
    fn trait_unpin_adt_flags() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AdtFlags>();
    }
}
