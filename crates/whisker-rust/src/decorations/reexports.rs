use std::hash::{Hash, Hasher};

use stabby::vec;
use whisker_macros::Decoration;

use crate::decorations::PublicPath;

/// The other crates that publicly expose the item a `use` path imports
///
/// Two imports in one file do not prove that one crate re-exports the
/// other. `serde` and `serde_json` sit side by side in many files, and
/// `serde_json` re-exports nothing of `serde`. A rule that wants to say
/// "import this through the facade" needs rust-analyzer's proof that the
/// item really is reachable there, and this decoration carries it.
///
/// The provider attaches it to each path a `use_declaration` imports: the
/// `scoped_identifier` or `identifier` of `use a::B;`, of `use a::B as C;`,
/// and of each name in `use a::{B, c::D};`. A wildcard and a `self` in a
/// list get none. Neither does a path rust-analyzer cannot resolve, so an
/// absent decoration proves nothing either way.
///
/// The list holds one path for each direct dependency of the current crate
/// that publicly exposes the item, other than the crate that defines it. A
/// dependency appears at most once, with its shortest path. An item the
/// current crate defines has an empty list, as has an item no other
/// dependency exposes. The list holds the crate the `use` already goes
/// through when that crate re-exports the item, so `use facade::Thing` lists
/// `facade::Thing`.
///
/// A path that `#[doc(hidden)]` marks, on the re-export or on a module it
/// passes through, is not listed. An associated item is never listed,
/// because a `use` cannot import one from a trait or a type.
///
/// The sysroot crates are dependencies like any other, so an item of
/// `core` lists its `std` and `alloc` paths.
///
/// A rule reads it out of memory the host allocated, so stabby lays it out.
///
/// # Examples
///
/// ```
/// use whisker_rust::decorations::{PublicPath, Reexports};
///
/// let reexports = Reexports::new([PublicPath::new("facade", ["thing", "Thing"])]);
///
/// let path = reexports.through("facade").expect("facade exposes the item");
///
/// assert_eq!(path.to_string(), "facade::thing::Thing");
/// assert!(reexports.through("serde").is_none());
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct Reexports {
    paths: vec::Vec<PublicPath>,
}

impl Reexports {
    /// Creates the decoration from the public paths to one item
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::{PublicPath, Reexports};
    ///
    /// let reexports = Reexports::new([PublicPath::new("facade", ["Shape"])]);
    ///
    /// assert_eq!(reexports.paths().len(), 1);
    /// ```
    pub fn new<P>(paths: P) -> Self
    where
        P: IntoIterator<Item = PublicPath>,
    {
        Self {
            paths: paths.into_iter().collect(),
        }
    }

    /// Returns every public path to the item, one per exposing crate
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::Reexports;
    ///
    /// let reexports = Reexports::new([]);
    ///
    /// assert!(reexports.paths().is_empty());
    /// ```
    pub fn paths(&self) -> &[PublicPath] {
        self.paths.as_slice()
    }

    /// Returns the public path to the item through the crate named `krate`
    ///
    /// `krate` is the name the current crate uses for the dependency, which
    /// is the first segment of a `use` path into it.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::{PublicPath, Reexports};
    ///
    /// let reexports = Reexports::new([PublicPath::new("facade", ["Shape"])]);
    ///
    /// assert!(reexports.through("facade").is_some());
    /// assert!(reexports.through("facade_core").is_none());
    /// ```
    pub fn through(&self, krate: &str) -> Option<&PublicPath> {
        self.paths().iter().find(|path| path.krate() == krate)
    }
}

impl Hash for Reexports {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.paths().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_return_every_path_in_order() {
        let first = PublicPath::new("facade", ["Shape"]);
        let second = PublicPath::new("other_facade", ["shapes", "Shape"]);

        let reexports = Reexports::new([first.clone(), second.clone()]);

        assert_eq!(reexports.paths(), &[first, second]);
    }

    /// Pins that the crate name is compared whole, not as a prefix
    ///
    /// `facade` must not answer for `facade_core`, or a rule would take a
    /// constituent crate for the facade that wraps it.
    #[test]
    fn through_with_a_crate_sharing_a_prefix_returns_none() {
        let reexports = Reexports::new([PublicPath::new("facade_core", ["Shape"])]);

        let path = reexports.through("facade");

        assert!(path.is_none());
    }

    #[test]
    fn through_with_a_listed_crate_returns_its_path() {
        let reexports = Reexports::new([
            PublicPath::new("facade", ["Shape"]),
            PublicPath::new("other_facade", ["shapes", "Shape"]),
        ]);

        let path = reexports
            .through("other_facade")
            .expect("other_facade is listed");

        assert_eq!(path.to_string(), "other_facade::shapes::Shape");
    }

    #[test]
    fn through_with_no_paths_returns_none() {
        let reexports = Reexports::new([]);

        let path = reexports.through("facade");

        assert!(path.is_none());
    }

    #[test]
    fn trait_send_reexports() {
        fn assert_send<T: Send>() {}
        assert_send::<Reexports>();
    }

    #[test]
    fn trait_sync_reexports() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Reexports>();
    }

    #[test]
    fn trait_unpin_reexports() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Reexports>();
    }
}
