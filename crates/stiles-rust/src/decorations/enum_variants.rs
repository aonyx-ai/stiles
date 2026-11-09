use std::hash::{Hash, Hasher};

use stabby::{string, vec};
use stiles_macros::Decoration;

/// The names of the variants of an enum, in declaration order
///
/// A rule reads it to tell a predicate of a variant from any other method
/// whose name starts with `is_`. On an enum with an `Error` variant,
/// `is_error` is such a predicate. A rule can also name the variants in its
/// message.
///
/// The provider attaches it beside [`ResolvedType`] and [`AdtFlags`] on a
/// value that decides a branch. Such a value is the value of an `if let` or
/// a let-else, the receiver of an `is_` method, or an operand of `==` or
/// `!=`. The `matches!` macro that tests a value counts as well. The
/// provider looks through references, so the names of `Kind` also reach a
/// `&Kind`.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::EnumVariants;
///
/// let variants = EnumVariants::new(["Error", "Warning"]);
///
/// assert_eq!(variants.names().collect::<Vec<_>>(), ["Error", "Warning"]);
/// ```
///
/// [`AdtFlags`]: crate::decorations::AdtFlags
/// [`ResolvedType`]: crate::decorations::ResolvedType
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct EnumVariants {
    names: vec::Vec<string::String>,
}

impl EnumVariants {
    /// Creates the decoration from the names of the variants
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::EnumVariants;
    ///
    /// let variants = EnumVariants::new(["Red"]);
    ///
    /// assert_eq!(variants.names().count(), 1);
    /// ```
    pub fn new<N, S>(names: N) -> Self
    where
        N: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            names: names
                .into_iter()
                .map(|name| string::String::from(name.as_ref()))
                .collect(),
        }
    }

    /// Returns the names of the variants, in declaration order
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::EnumVariants;
    ///
    /// let variants = EnumVariants::new(["Red", "Blue"]);
    ///
    /// assert_eq!(variants.names().last(), Some("Blue"));
    /// ```
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.as_slice().iter().map(|name| name.as_str())
    }
}

impl Hash for EnumVariants {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.names.as_slice().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_return_every_name_in_order() {
        let variants = EnumVariants::new(["Error", "Warning", "Note"]);

        let names: Vec<&str> = variants.names().collect();

        assert_eq!(names, ["Error", "Warning", "Note"]);
    }

    #[test]
    fn trait_send_enum_variants() {
        fn assert_send<T: Send>() {}

        assert_send::<EnumVariants>();
    }

    #[test]
    fn trait_sync_enum_variants() {
        fn assert_sync<T: Sync>() {}

        assert_sync::<EnumVariants>();
    }

    #[test]
    fn trait_unpin_enum_variants() {
        fn assert_unpin<T: Unpin>() {}

        assert_unpin::<EnumVariants>();
    }
}
