/// Whether a build of the plugin boundary comes from published crates
///
/// An ABI version is a promise about a release. `rakko check-abi` keeps
/// the promise: the boundary of a release matches the version that
/// release names. Between two releases, one version holds more than one
/// layout, so a build from unpublished source cannot vouch for its
/// version. Stiles trusts the version only when both sides are
/// published, and compares the fingerprints otherwise.
///
/// Cargo writes `.cargo_vcs_info.json` into every crate that it packages,
/// and nowhere else. A build script of each boundary crate looks for that
/// file. A git source, a path source, and a patched crate are therefore
/// unpublished.
///
/// The value is a bare integer, because stiles reads it from a library
/// before it trusts anything else there. Any value other than
/// [`Provenance::PUBLISHED`] reads as unpublished, which is the side that
/// checks more.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::Provenance;
///
/// let joined = Provenance::all(&[Provenance::PUBLISHED, Provenance::UNPUBLISHED]);
///
/// assert_eq!(joined, Provenance::UNPUBLISHED);
/// ```
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Provenance(u32);

impl Provenance {
    /// A build from crates that cargo packaged
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Provenance;
    ///
    /// assert!(Provenance::PUBLISHED.is_published());
    /// ```
    pub const PUBLISHED: Self = Self(1);

    /// A build from source that no release vouches for
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Provenance;
    ///
    /// assert!(!Provenance::UNPUBLISHED.is_published());
    /// ```
    pub const UNPUBLISHED: Self = Self(0);

    /// Returns the provenance of a build that joins several crates
    ///
    /// The build is published only when every crate in it is published.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Provenance;
    ///
    /// let joined = Provenance::all(&[Provenance::PUBLISHED, Provenance::PUBLISHED]);
    ///
    /// assert_eq!(joined, Provenance::PUBLISHED);
    /// ```
    pub const fn all(parts: &[Self]) -> Self {
        let mut index = 0;
        while index < parts.len() {
            if parts[index].0 != Self::PUBLISHED.0 {
                return Self::UNPUBLISHED;
            }
            index += 1;
        }

        Self::PUBLISHED
    }

    /// Reports whether a release vouches for this build
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Provenance;
    ///
    /// assert!(Provenance::PUBLISHED.is_published());
    /// assert!(!Provenance::UNPUBLISHED.is_published());
    /// ```
    pub const fn is_published(self) -> bool {
        self.0 == Self::PUBLISHED.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_of_no_parts_is_published() {
        let joined = Provenance::all(&[]);

        assert_eq!(joined, Provenance::PUBLISHED);
    }

    #[test]
    fn all_with_an_unpublished_part_is_unpublished() {
        let joined = Provenance::all(&[Provenance::PUBLISHED, Provenance::UNPUBLISHED]);

        assert_eq!(joined, Provenance::UNPUBLISHED);
    }

    #[test]
    fn all_with_only_published_parts_is_published() {
        let joined = Provenance::all(&[Provenance::PUBLISHED, Provenance::PUBLISHED]);

        assert_eq!(joined, Provenance::PUBLISHED);
    }

    /// A value that no build writes reads as unpublished, so a corrupt
    /// declaration gets the stricter check
    #[test]
    fn is_published_with_an_unknown_value_returns_false() {
        let unknown = Provenance(7);

        assert!(!unknown.is_published());
    }

    #[test]
    fn the_value_is_four_bytes() {
        assert_eq!(size_of::<Provenance>(), 4);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Provenance>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Provenance>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Provenance>();
    }
}
