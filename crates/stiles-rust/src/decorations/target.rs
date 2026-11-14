use bon::bon;
use stabby::{option, string};
use stiles_macros::Decoration;

use crate::decorations::{Edition, TargetKinds};

/// Says which crate and which Cargo targets compile a file
///
/// A rule reads it to tell a test, a bench, an example, or a build script
/// from the code a package ships. A guess from the path fails where a
/// manifest points a target at another file. It also fails for a module of
/// `tests` in a subdirectory.
///
/// The provider attaches it to the `source_file` node of every file it
/// covers. The kinds come from the Cargo metadata the provider loads. The
/// crate and the edition come from the crate the provider resolves the file
/// in. A file that several crates compile names one of them, while its
/// kinds cover every target.
///
/// The crate name is the one rust-analyzer reports, with underscores in
/// place of dashes. An integration test in `tests/api.rs` is the crate
/// `api`, and a build script is `build_script_build`. A crate without a
/// name has [`None`]. An empty [`TargetKinds`] means the provider knows no
/// Cargo target for the file.
///
/// Whether a single item is test code is a question for [`TestCode`].
///
/// A rule reads it out of memory the host allocated, so stabby lays it
/// out. The crate name is stabby's [`Option`] of stabby's `String`.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{Edition, Target, TargetKind, TargetKinds};
///
/// let target = Target::builder()
///     .krate("api")
///     .kinds(TargetKinds::from_iter([TargetKind::IntegrationTest]))
///     .edition(Edition::Edition2024)
///     .build();
///
/// assert!(target.kinds().contains(TargetKind::IntegrationTest));
/// ```
///
/// [`Option`]: stabby::option::Option
/// [`TestCode`]: crate::decorations::TestCode
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct Target {
    krate: option::Option<string::String>,
    kinds: TargetKinds,
    edition: Edition,
}

#[bon]
impl Target {
    /// Creates the decoration from a crate name, the target kinds, and an edition
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{Edition, Target, TargetKind, TargetKinds};
    ///
    /// let target = Target::builder()
    ///     .kinds(TargetKinds::from_iter([TargetKind::Library]))
    ///     .edition(Edition::Edition2021)
    ///     .build();
    ///
    /// assert!(target.krate().is_none());
    /// ```
    #[builder]
    pub fn new(krate: Option<&str>, kinds: TargetKinds, edition: Edition) -> Self {
        Self {
            krate: krate.map(string::String::from).into(),
            kinds,
            edition,
        }
    }

    /// Returns the name of the crate, or [`None`] for a crate without one
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{Edition, Target, TargetKind, TargetKinds};
    ///
    /// let target = Target::builder()
    ///     .krate("sample_project")
    ///     .kinds(TargetKinds::from_iter([TargetKind::Library]))
    ///     .edition(Edition::Edition2024)
    ///     .build();
    ///
    /// assert_eq!(target.krate(), Some("sample_project"));
    /// ```
    pub fn krate(&self) -> Option<&str> {
        self.krate.as_ref().map(string::String::as_str)
    }

    /// Returns the kinds of every Cargo target that compiles the file
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{Edition, Target, TargetKind, TargetKinds};
    ///
    /// let target = Target::builder()
    ///     .krate("sample_project")
    ///     .kinds(TargetKinds::from_iter([TargetKind::Library, TargetKind::Binary]))
    ///     .edition(Edition::Edition2024)
    ///     .build();
    ///
    /// assert!(target.kinds().contains(TargetKind::Binary));
    /// ```
    pub fn kinds(&self) -> TargetKinds {
        self.kinds
    }

    /// Returns the edition of the crate
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{Edition, Target, TargetKind, TargetKinds};
    ///
    /// let target = Target::builder()
    ///     .krate("sample_project")
    ///     .kinds(TargetKinds::from_iter([TargetKind::Library]))
    ///     .edition(Edition::Edition2018)
    ///     .build();
    ///
    /// assert_eq!(target.edition(), Edition::Edition2018);
    /// ```
    pub fn edition(&self) -> Edition {
        self.edition
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decorations::TargetKind;

    fn library() -> TargetKinds {
        TargetKinds::from_iter([TargetKind::Library])
    }

    #[test]
    fn edition_returns_the_recorded_edition() {
        let target = Target::builder()
            .kinds(library())
            .edition(Edition::Edition2015)
            .build();

        let edition = target.edition();

        assert_eq!(edition, Edition::Edition2015);
    }

    #[test]
    fn kinds_returns_the_recorded_kinds() {
        let target = Target::builder()
            .krate("sample")
            .kinds(library())
            .edition(Edition::Edition2024)
            .build();

        let kinds = target.kinds();

        assert_eq!(kinds, library());
    }

    #[test]
    fn krate_with_a_name_returns_it() {
        let target = Target::builder()
            .krate("sample")
            .kinds(library())
            .edition(Edition::Edition2024)
            .build();

        let krate = target.krate();

        assert_eq!(krate, Some("sample"));
    }

    #[test]
    fn krate_without_a_name_returns_none() {
        let target = Target::builder()
            .kinds(library())
            .edition(Edition::Edition2024)
            .build();

        let krate = target.krate();

        assert!(krate.is_none());
    }

    #[test]
    fn trait_send_target() {
        fn assert_send<T: Send>() {}
        assert_send::<Target>();
    }

    #[test]
    fn trait_sync_target() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Target>();
    }

    #[test]
    fn trait_unpin_target() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Target>();
    }
}
