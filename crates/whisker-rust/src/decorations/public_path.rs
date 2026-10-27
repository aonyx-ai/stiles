use std::fmt;
use std::hash::{Hash, Hasher};

use bon::bon;
use stabby::{string, vec};

/// A path through which another crate can name an item
///
/// The path starts at a crate root that the current crate depends on, and
/// every segment after it is public. A rule can therefore print it as the
/// import to write instead.
///
/// It differs from [`TypePath`], which names where a type is defined. A
/// public path names where a crate exposes an item, and that is often a
/// re-export: `facade::thing::Thing` for a `Thing` that `facade_core`
/// defines. The segments after the crate are not all modules either,
/// because the path to an enum variant passes through its enum.
///
/// The crate is the name the current crate uses for that dependency, so a
/// dependency that `Cargo.toml` renames appears under its new name.
///
/// A rule reads the path out of a decoration the host recorded, so stabby
/// lays it out. Its segments are stabby's `String`s.
///
/// # Examples
///
/// ```
/// use whisker_rust::decorations::PublicPath;
///
/// let path = PublicPath::new("facade", ["thing", "Thing"]);
///
/// assert_eq!(path.to_string(), "facade::thing::Thing");
/// ```
///
/// [`TypePath`]: crate::decorations::TypePath
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct PublicPath {
    krate: string::String,
    segments: vec::Vec<string::String>,
}

#[bon]
impl PublicPath {
    /// Creates a path from a crate name and the segments that follow it
    ///
    /// The last segment is the item's name.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::PublicPath;
    ///
    /// let path = PublicPath::new("facade", ["Shape", "Circle"]);
    ///
    /// assert_eq!(path.to_string(), "facade::Shape::Circle");
    /// ```
    #[builder(start_fn = builder)]
    pub fn new<S>(krate: &str, segments: S) -> Self
    where
        S: IntoIterator,
        S::Item: AsRef<str>,
    {
        Self {
            krate: string::String::from(krate),
            segments: segments
                .into_iter()
                .map(|segment| string::String::from(segment.as_ref()))
                .collect(),
        }
    }

    /// Returns the name of the crate the path starts at
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::PublicPath;
    ///
    /// let path = PublicPath::new("facade", ["thing", "Thing"]);
    ///
    /// assert_eq!(path.krate(), "facade");
    /// ```
    pub fn krate(&self) -> &str {
        &self.krate
    }

    /// Returns the segments after the crate, ending with the item's name
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::PublicPath;
    ///
    /// let path = PublicPath::new("facade", ["thing", "Thing"]);
    ///
    /// assert_eq!(path.segments().collect::<Vec<_>>(), vec!["thing", "Thing"]);
    /// ```
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().map(string::String::as_str)
    }
}

/// Renders the crate and the segments, joined by `::`
impl fmt::Display for PublicPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.krate)?;
        for segment in self.segments() {
            write!(f, "::{segment}")?;
        }
        Ok(())
    }
}

impl Hash for PublicPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.krate.hash(state);
        self.segments.as_slice().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn krate_returns_the_crate() {
        let path = PublicPath::new("facade", ["thing", "Thing"]);

        assert_eq!(path.krate(), "facade");
    }

    #[test]
    fn segments_returns_each_segment_in_order() {
        let path = PublicPath::new("facade", ["thing", "Thing"]);

        let segments: Vec<&str> = path.segments().collect();

        assert_eq!(segments, vec!["thing", "Thing"]);
    }

    #[test]
    fn display_joins_segments_with_double_colons() {
        let path = PublicPath::new("facade", ["thing", "Thing"]);

        let rendered = path.to_string();

        assert_eq!(rendered, "facade::thing::Thing");
    }

    /// Pins that a path with no segments does not end in `::`
    #[test]
    fn display_with_no_segments_renders_the_crate() {
        let path = PublicPath::new("facade", [] as [&str; 0]);

        let rendered = path.to_string();

        assert_eq!(rendered, "facade");
    }

    #[test]
    fn hash_agrees_with_eq() {
        let mut paths = HashSet::new();
        paths.insert(PublicPath::new("facade", ["thing", "Thing"]));

        let found = paths.contains(&PublicPath::new("facade", ["thing", "Thing"]));

        assert!(found);
    }

    #[test]
    fn hash_with_other_segments_misses() {
        let mut paths = HashSet::new();
        paths.insert(PublicPath::new("facade", ["thing", "Thing"]));

        let found = paths.contains(&PublicPath::new("facade", ["Thing"]));

        assert!(!found);
    }

    #[test]
    fn trait_send_public_path() {
        fn assert_send<T: Send>() {}
        assert_send::<PublicPath>();
    }

    #[test]
    fn trait_sync_public_path() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PublicPath>();
    }

    #[test]
    fn trait_unpin_public_path() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PublicPath>();
    }
}
