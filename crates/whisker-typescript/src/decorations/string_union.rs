use stabby::{string, vec};
use whisker_macros::Decoration;

/// The members of a union of string literals
///
/// The TypeScript provider attaches one to an identifier whose type is a
/// union of string literals, such as `type Color = "red" | "green"`. The
/// union is closed: a `switch` that names every member needs no `default`,
/// and one that has a `default` hides a member added later. A rule reads
/// the members here. They come in the checker's order, which is not the
/// order the source wrote them in.
///
/// # Examples
///
/// ```
/// use whisker_typescript::decorations::StringUnion;
///
/// let color = StringUnion::new(vec!["red", "green"]);
///
/// assert_eq!(color.members().collect::<Vec<_>>(), ["red", "green"]);
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct StringUnion {
    members: vec::Vec<string::String>,
}

impl StringUnion {
    /// Describes the union of the string literals `members`
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::StringUnion;
    ///
    /// let size = StringUnion::new(vec!["s", "m", "l"]);
    ///
    /// assert_eq!(size.members().count(), 3);
    /// ```
    pub fn new<'a>(members: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            members: members.into_iter().map(string::String::from).collect(),
        }
    }

    /// Returns the members, in the checker's order
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::StringUnion;
    ///
    /// let size = StringUnion::new(vec!["s", "m"]);
    ///
    /// assert_eq!(size.members().next(), Some("s"));
    /// ```
    pub fn members(&self) -> impl Iterator<Item = &str> {
        self.members.iter().map(|member| member.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_keep_their_order() {
        let union = StringUnion::new(vec!["b", "a"]);

        let members: Vec<&str> = union.members().collect();

        assert_eq!(members, ["b", "a"]);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<StringUnion>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<StringUnion>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<StringUnion>();
    }
}
