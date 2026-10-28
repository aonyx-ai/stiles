use bon::bon;
use stabby::{option, string};
use stiles_macros::Decoration;

use crate::decorations::Primitive;

/// The checker's type for a value, with the alias it was written as
///
/// The TypeScript provider attaches one to every identifier the checker
/// can type, beside its [`TypeText`]. The display is the checker's own
/// rendering. The alias is the type alias the source names for the value,
/// such as `Greeting` for a parameter written `greeting: Greeting`. The
/// primitive is what the type comes down to.
///
/// The alias and the primitive together answer the question a rendering
/// cannot. `type Greeting = string` and `type Name = string` print as
/// `string` both, and the checker accepts one where the other is expected.
/// A rule sees two different aliases over one primitive and knows a
/// caller can swap the values without an error.
///
/// [`TypeText`]: crate::decorations::TypeText
///
/// # Examples
///
/// ```
/// use stiles_typescript::decorations::{Primitive, ResolvedType};
///
/// let ty = ResolvedType::builder()
///     .display("string")
///     .alias("Greeting")
///     .primitive(Primitive::String)
///     .build();
///
/// assert_eq!(ty.alias(), Some("Greeting"));
/// assert_eq!(ty.primitive(), Primitive::String);
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct ResolvedType {
    display: string::String,
    alias: option::Option<string::String>,
    primitive: Primitive,
}

#[bon]
impl ResolvedType {
    /// Describes a type by its rendering, its written alias, and its primitive
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_typescript::decorations::{Primitive, ResolvedType};
    ///
    /// let ty = ResolvedType::builder()
    ///     .display("number")
    ///     .primitive(Primitive::Number)
    ///     .build();
    ///
    /// assert_eq!(ty.display(), "number");
    /// ```
    #[builder]
    pub fn new(display: &str, alias: Option<&str>, primitive: Primitive) -> Self {
        Self {
            display: string::String::from(display),
            alias: alias.map(string::String::from).into(),
            primitive,
        }
    }

    /// Returns the checker's rendering of the type
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_typescript::decorations::{Primitive, ResolvedType};
    ///
    /// let ty = ResolvedType::builder()
    ///     .display("string")
    ///     .alias("Name")
    ///     .primitive(Primitive::String)
    ///     .build();
    ///
    /// assert_eq!(ty.display(), "string");
    /// ```
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Returns the type alias the source names for the value, if any
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_typescript::decorations::{Primitive, ResolvedType};
    ///
    /// let ty = ResolvedType::builder()
    ///     .display("string")
    ///     .alias("Name")
    ///     .primitive(Primitive::String)
    ///     .build();
    ///
    /// assert_eq!(ty.alias(), Some("Name"));
    /// ```
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_ref().map(|alias| alias.as_str())
    }

    /// Returns the primitive the type comes down to
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_typescript::decorations::{Primitive, ResolvedType};
    ///
    /// let ty = ResolvedType::builder()
    ///     .display("{ a: number }")
    ///     .primitive(Primitive::NotPrimitive)
    ///     .build();
    ///
    /// assert_eq!(ty.primitive(), Primitive::NotPrimitive);
    /// ```
    pub fn primitive(&self) -> Primitive {
        self.primitive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_without_one_returns_none() {
        let ty = ResolvedType::builder()
            .display("string")
            .primitive(Primitive::String)
            .build();

        let alias = ty.alias();

        assert_eq!(alias, None);
    }

    #[test]
    fn builder_keeps_every_field() {
        let ty = ResolvedType::builder()
            .display("string")
            .alias("Greeting")
            .primitive(Primitive::String)
            .build();

        let fields = (ty.display(), ty.alias(), ty.primitive());

        assert_eq!(fields, ("string", Some("Greeting"), Primitive::String));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ResolvedType>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ResolvedType>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ResolvedType>();
    }
}
