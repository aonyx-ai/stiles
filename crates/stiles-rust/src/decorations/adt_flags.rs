use stiles_macros::Decoration;

use crate::decorations::Exhaustiveness;

/// ADT-specific flags for enum types
///
/// Attached alongside [`ResolvedType`] when the type is an enum.
/// Provides additional information about the ADT that rules need for
/// decisions like whether a wildcard match arm is acceptable.
///
/// A rule reads it out of memory the host allocated, so stabby lays it
/// out.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::AdtFlags;
///
/// let flags = AdtFlags::new(false);
///
/// assert!(!flags.non_exhaustive_external());
/// ```
///
/// [`ResolvedType`]: crate::decorations::ResolvedType
#[stabby::stabby]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct AdtFlags {
    non_exhaustive_external: bool,
}

impl AdtFlags {
    /// Creates ADT flags
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::AdtFlags;
    ///
    /// let flags = AdtFlags::new(true);
    ///
    /// assert!(flags.non_exhaustive_external());
    /// ```
    pub fn new(non_exhaustive_external: bool) -> Self {
        Self {
            non_exhaustive_external,
        }
    }

    /// Creates ADT flags from how exhaustive a match on the enum can be
    ///
    /// This does what [`new`] does, with a variant in place of a `bool`.
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
    /// [`new`]: Self::new
    pub fn from_exhaustiveness(exhaustiveness: Exhaustiveness) -> Self {
        Self {
            non_exhaustive_external: match exhaustiveness {
                Exhaustiveness::Exhaustive => false,
                Exhaustiveness::NonExhaustive => true,
            },
        }
    }

    /// Returns whether the enum is `#[non_exhaustive]` from an external crate
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::AdtFlags;
    ///
    /// let flags = AdtFlags::new(false);
    ///
    /// assert!(!flags.non_exhaustive_external());
    /// ```
    pub fn non_exhaustive_external(&self) -> bool {
        self.non_exhaustive_external
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn non_exhaustive_external_when_set_returns_true() {
        let flags = AdtFlags::new(true);

        assert!(flags.non_exhaustive_external());
    }

    #[test]
    fn non_exhaustive_external_when_unset_returns_false() {
        let flags = AdtFlags::new(false);

        assert!(!flags.non_exhaustive_external());
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
