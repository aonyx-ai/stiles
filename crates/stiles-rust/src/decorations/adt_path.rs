use stiles_macros::Decoration;

use crate::decorations::TypePath;

/// Where the enum, struct, or union of a value is defined
///
/// A rule reads it to tell one ADT from another by identity, not by its
/// rendered name. A configuration can then name an enum by its path, and a
/// message can print that path.
///
/// The provider attaches it beside [`AdtFlags`]: on a match scrutinee, and
/// on a value that decides a branch. It attaches for every ADT, so a struct
/// gets one too. The provider looks through references, so a `&Kind` gets
/// the path of `Kind`.
///
/// The path is the definition's, not a re-export's. A crate can re-export
/// an item under any path, and a dependency can move that re-export
/// without moving the item. The defining path is the one name every
/// re-export shares. The crate segment is the name rust-analyzer reports,
/// with underscores in place of dashes.
///
/// A rule reads it out of memory the host allocated, so stabby lays it
/// out. It is a decoration of its own, not a field of [`AdtFlags`]. The key
/// of [`AdtFlags`] therefore stays, and a lint built against it still loads.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{AdtPath, TypePath};
///
/// let path = TypePath::builder().krate("tar").modules(["entry_type"]).name("EntryType").build();
/// let adt = AdtPath::new(path);
///
/// assert_eq!(adt.path().to_string(), "tar::entry_type::EntryType");
/// ```
///
/// [`AdtFlags`]: crate::decorations::AdtFlags
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct AdtPath {
    path: TypePath,
}

impl AdtPath {
    /// Creates the decoration from the path of the ADT's definition
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{AdtPath, TypePath};
    ///
    /// let path = TypePath::builder().krate("facade_core").modules(["tint"]).name("Tint").build();
    ///
    /// let adt = AdtPath::new(path.clone());
    ///
    /// assert_eq!(adt.path(), &path);
    /// ```
    pub fn new(path: TypePath) -> Self {
        Self { path }
    }

    /// Returns the path of the ADT's definition
    ///
    /// Its [`Display`] renders `crate::module::Name`, which a rule can
    /// compare with a configured string.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{AdtPath, TypePath};
    ///
    /// let path = TypePath::builder().krate("sample").modules([] as [&str; 0]).name("Color").build();
    /// let adt = AdtPath::new(path);
    ///
    /// assert_eq!(adt.path().to_string(), "sample::Color");
    /// ```
    ///
    /// [`Display`]: std::fmt::Display
    pub fn path(&self) -> &TypePath {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tint_path() -> TypePath {
        TypePath::builder()
            .krate("facade_core")
            .modules(["tint"])
            .name("Tint")
            .build()
    }

    #[test]
    fn eq_with_another_path_is_false() {
        let adt = AdtPath::new(tint_path());

        let matches = adt
            == AdtPath::new(
                TypePath::builder()
                    .krate("facade")
                    .modules([] as [&str; 0])
                    .name("Tint")
                    .build(),
            );

        assert!(!matches);
    }

    #[test]
    fn path_renders_crate_modules_and_name() {
        let adt = AdtPath::new(tint_path());

        let rendered = adt.path().to_string();

        assert_eq!(rendered, "facade_core::tint::Tint");
    }

    #[test]
    fn path_returns_the_recorded_path() {
        let adt = AdtPath::new(tint_path());

        let path = adt.path();

        assert_eq!(path, &tint_path());
    }

    #[test]
    fn trait_send_adt_path() {
        fn assert_send<T: Send>() {}
        assert_send::<AdtPath>();
    }

    #[test]
    fn trait_sync_adt_path() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AdtPath>();
    }

    #[test]
    fn trait_unpin_adt_path() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AdtPath>();
    }
}
