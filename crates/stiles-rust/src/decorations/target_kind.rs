/// Names the kind of a Cargo target that compiles a file
///
/// Cargo builds each package as a set of targets, and each target is a
/// crate of its own. A rule reads the kind to tell a test or a build script
/// from the code a package ships. A path such as `tests/` or `build.rs`
/// proves nothing, because a manifest can point a target at any file.
///
/// A proc-macro crate counts as a [`Library`]. A cdylib, a staticlib, and
/// any other library type count as one too.
///
/// A rule reads it out of memory the host allocated, so the discriminant
/// is a byte.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::TargetKind;
///
/// let kind = TargetKind::IntegrationTest;
///
/// let ships = match kind {
///     TargetKind::Library | TargetKind::Binary => true,
///     TargetKind::IntegrationTest
///     | TargetKind::Bench
///     | TargetKind::Example
///     | TargetKind::BuildScript => false,
/// };
///
/// assert!(!ships);
/// ```
///
/// [`Library`]: TargetKind::Library
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum TargetKind {
    /// A library target, such as `src/lib.rs`
    Library,
    /// A binary target, such as `src/main.rs` or a file in `src/bin`
    Binary,
    /// An integration test, such as a file in `tests`
    IntegrationTest,
    /// A benchmark, such as a file in `benches`
    Bench,
    /// An example, such as a file in `examples`
    Example,
    /// The build script of a package, such as `build.rs`
    BuildScript,
}

impl TargetKind {
    /// Every kind, in the order of declaration
    pub(crate) const ALL: [TargetKind; 6] = [
        TargetKind::Library,
        TargetKind::Binary,
        TargetKind::IntegrationTest,
        TargetKind::Bench,
        TargetKind::Example,
        TargetKind::BuildScript,
    ];

    /// Returns the bit that stands for this kind in a set of kinds
    pub(crate) const fn bit(self) -> u8 {
        match self {
            TargetKind::Library => 1,
            TargetKind::Binary => 1 << 1,
            TargetKind::IntegrationTest => 1 << 2,
            TargetKind::Bench => 1 << 3,
            TargetKind::Example => 1 << 4,
            TargetKind::BuildScript => 1 << 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn bit_differs_for_every_kind() {
        let bits: HashSet<u8> = TargetKind::ALL.iter().map(|kind| kind.bit()).collect();

        assert_eq!(bits.len(), TargetKind::ALL.len());
    }

    #[test]
    fn trait_send_target_kind() {
        fn assert_send<T: Send>() {}
        assert_send::<TargetKind>();
    }

    #[test]
    fn trait_sync_target_kind() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TargetKind>();
    }

    #[test]
    fn trait_unpin_target_kind() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TargetKind>();
    }
}
