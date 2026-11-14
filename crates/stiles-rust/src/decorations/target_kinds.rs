use crate::decorations::TargetKind;

/// Holds the kinds of the Cargo targets that compile one file
///
/// A file can belong to several targets at once. A module that both
/// `src/lib.rs` and `src/main.rs` declare compiles into the library and
/// into the binary. The set holds each kind once, whatever the number of
/// targets of that kind.
///
/// An empty set means the provider knows no Cargo target for the file. A
/// project that `rust-project.json` describes has none, and neither has a
/// target of a kind this set cannot name. A rule must not read an empty set
/// as a library.
///
/// A rule reads it out of memory the host allocated, so the set is one
/// byte with a bit for each [`TargetKind`].
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{TargetKind, TargetKinds};
///
/// let kinds = TargetKinds::from_iter([TargetKind::Library, TargetKind::Binary]);
///
/// assert!(kinds.contains(TargetKind::Binary));
/// ```
#[stabby::stabby]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct TargetKinds {
    bits: u8,
}

impl TargetKinds {
    /// Returns these kinds with `kind` added
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{TargetKind, TargetKinds};
    ///
    /// let kinds = TargetKinds::default().with(TargetKind::Example);
    ///
    /// assert!(kinds.contains(TargetKind::Example));
    /// ```
    #[must_use]
    pub fn with(self, kind: TargetKind) -> Self {
        Self {
            bits: self.bits | kind.bit(),
        }
    }

    /// Returns whether the set holds `kind`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{TargetKind, TargetKinds};
    ///
    /// let kinds = TargetKinds::from_iter([TargetKind::Library]);
    ///
    /// assert!(!kinds.contains(TargetKind::Bench));
    /// ```
    pub fn contains(self, kind: TargetKind) -> bool {
        self.bits & kind.bit() != 0
    }

    /// Returns whether the provider knows no target for the file
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::TargetKinds;
    ///
    /// assert!(TargetKinds::default().is_empty());
    /// ```
    pub fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// Returns each kind of the set, in the order [`TargetKind`] declares them
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{TargetKind, TargetKinds};
    ///
    /// let kinds = TargetKinds::from_iter([TargetKind::Bench, TargetKind::Library]);
    ///
    /// assert_eq!(
    ///     kinds.iter().collect::<Vec<_>>(),
    ///     [TargetKind::Library, TargetKind::Bench],
    /// );
    /// ```
    pub fn iter(self) -> impl Iterator<Item = TargetKind> {
        TargetKind::ALL
            .into_iter()
            .filter(move |kind| self.contains(*kind))
    }
}

/// Collects kinds into a set, which keeps each kind once
impl FromIterator<TargetKind> for TargetKinds {
    fn from_iter<I: IntoIterator<Item = TargetKind>>(kinds: I) -> Self {
        kinds
            .into_iter()
            .fold(Self::default(), |set, kind| set.with(kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_with_a_kind_left_out_returns_false() {
        let kinds = TargetKinds::from_iter([TargetKind::Library, TargetKind::BuildScript]);

        let holds = kinds.contains(TargetKind::Binary);

        assert!(!holds);
    }

    #[test]
    fn contains_with_an_added_kind_returns_true() {
        let kinds = TargetKinds::default().with(TargetKind::IntegrationTest);

        let holds = kinds.contains(TargetKind::IntegrationTest);

        assert!(holds);
    }

    #[test]
    fn eq_ignores_the_order_of_collection() {
        let kinds = TargetKinds::from_iter([TargetKind::Bench, TargetKind::Example]);

        let same = kinds == TargetKinds::from_iter([TargetKind::Example, TargetKind::Bench]);

        assert!(same);
    }

    #[test]
    fn from_iter_keeps_a_repeated_kind_once() {
        let kinds = TargetKinds::from_iter([TargetKind::Binary, TargetKind::Binary]);

        let listed: Vec<TargetKind> = kinds.iter().collect();

        assert_eq!(listed, [TargetKind::Binary]);
    }

    #[test]
    fn is_empty_with_a_kind_returns_false() {
        let kinds = TargetKinds::from_iter([TargetKind::Library]);

        let empty = kinds.is_empty();

        assert!(!empty);
    }

    #[test]
    fn is_empty_with_no_kind_returns_true() {
        let kinds = TargetKinds::default();

        let empty = kinds.is_empty();

        assert!(empty);
    }

    #[test]
    fn iter_lists_every_kind_in_declaration_order() {
        let kinds = TargetKinds::from_iter(TargetKind::ALL.into_iter().rev());

        let listed: Vec<TargetKind> = kinds.iter().collect();

        assert_eq!(listed, TargetKind::ALL);
    }

    #[test]
    fn trait_send_target_kinds() {
        fn assert_send<T: Send>() {}
        assert_send::<TargetKinds>();
    }

    #[test]
    fn trait_sync_target_kinds() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TargetKinds>();
    }

    #[test]
    fn trait_unpin_target_kinds() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TargetKinds>();
    }
}
