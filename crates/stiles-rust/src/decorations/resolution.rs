use bon::bon;
use stabby::{option, string};
use stiles_macros::Decoration;

use crate::decorations::{DefinitionKind, DefinitionOrigin, TypePath};

/// Says what a name refers to: the kind, the crate, and the path of its definition
///
/// A rule reads it in place of resolving names by hand. A rule that scopes
/// `use` declarations itself goes silent at a glob import, and a list of
/// paths misses a re-export. Rust-analyzer resolves both, so the answer
/// here follows a glob, a rename, and a re-export to the definition.
///
/// The provider attaches it to the node of each name that resolves:
///
/// - `identifier`, in an expression, a type, a pattern, a path, a `use`, a
///   macro call, or an attribute
/// - `type_identifier` and `primitive_type`
/// - `scoped_identifier` and `scoped_type_identifier`, for the whole path
/// - `field_identifier`, only where it names the method of a call
/// - `shorthand_field_identifier`, as `x` in the pattern `Point { x, .. }`
///
/// A path and the name at its end get the same answer, and each qualifier
/// gets its own. In `std::io::stdout`, `std` names a crate and `std::io`
/// names a module. The keywords `crate`, `self`, and `super` get nothing,
/// though a path through them does.
///
/// A name that defines an item gets nothing, such as the name of a function
/// or the alias of `use a as b`. A name in a pattern resolves to the local
/// it binds. It resolves to a constant or a variant instead when one is in
/// scope, as `None` does.
///
/// A field gets nothing, and neither does a lifetime nor a label. The
/// tokens inside a macro call get nothing either, because the provider does
/// not look into an expansion. A name rust-analyzer cannot resolve gets no
/// decoration, so a rule that finds none keeps silent.
///
/// The lookup of a name in a body needs the inferred types of that body. A
/// run that reads this decoration therefore infers every function of every
/// file it covers.
///
/// The path names the definition, not a re-export, so `facade::thing::make`
/// appears as `facade_core::thing::make`. A variant spells its path through
/// its enum, and an associated item through its trait or its type. An item
/// of a trait impl goes through the trait, so a call of `write_all` names
/// `std::io::Write::write_all` whatever the type of its receiver.
///
/// Some definitions have no path. A crate root has only its crate, and so
/// do a local binding, a generic parameter, and a derive helper. A builtin
/// has no crate either. `Self` has the path of the type the impl is for,
/// when that type is an ADT.
///
/// A rule reads it out of memory the host allocated, so stabby lays it out.
/// The crate name is stabby's [`Option`] of stabby's `String`.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{DefinitionKind, DefinitionOrigin, Resolution, TypePath};
///
/// let enum_path = TypePath::builder().krate("sample").modules(["kinds"]).name("Kind").build();
/// let resolution = Resolution::builder()
///     .kind(DefinitionKind::EnumVariant)
///     .origin(DefinitionOrigin::SameCrate)
///     .krate("sample")
///     .path(TypePath::builder().krate("sample").modules(["kinds", "Kind"]).name("A").build())
///     .parent(enum_path)
///     .build();
///
/// assert_eq!(resolution.parent().map(ToString::to_string).as_deref(), Some("sample::kinds::Kind"));
/// ```
///
/// [`Option`]: stabby::option::Option
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct Resolution {
    kind: DefinitionKind,
    origin: DefinitionOrigin,
    krate: option::Option<string::String>,
    path: option::Option<TypePath>,
    parent: option::Option<TypePath>,
}

#[bon]
impl Resolution {
    /// Creates the decoration from what the provider found about a definition
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{DefinitionKind, DefinitionOrigin, Resolution};
    ///
    /// let binding = Resolution::builder()
    ///     .kind(DefinitionKind::LocalBinding)
    ///     .origin(DefinitionOrigin::SameCrate)
    ///     .krate("sample")
    ///     .build();
    ///
    /// assert!(binding.path().is_none());
    /// ```
    #[builder]
    pub fn new(
        kind: DefinitionKind,
        origin: DefinitionOrigin,
        krate: Option<&str>,
        path: Option<TypePath>,
        parent: Option<TypePath>,
    ) -> Self {
        Self {
            kind,
            origin,
            krate: krate.map(string::String::from).into(),
            path: path.into(),
            parent: parent.into(),
        }
    }

    /// Returns the kind of the definition
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{DefinitionKind, DefinitionOrigin, Resolution};
    ///
    /// let resolution = Resolution::builder()
    ///     .kind(DefinitionKind::BuiltinType)
    ///     .origin(DefinitionOrigin::Builtin)
    ///     .build();
    ///
    /// assert_eq!(resolution.kind(), DefinitionKind::BuiltinType);
    /// ```
    pub fn kind(&self) -> DefinitionKind {
        self.kind
    }

    /// Returns whether the crate of the file, another crate, or the language defines it
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{DefinitionKind, DefinitionOrigin, Resolution};
    ///
    /// let resolution = Resolution::builder()
    ///     .kind(DefinitionKind::Crate)
    ///     .origin(DefinitionOrigin::OtherCrate)
    ///     .krate("std")
    ///     .build();
    ///
    /// assert_eq!(resolution.origin(), DefinitionOrigin::OtherCrate);
    /// ```
    pub fn origin(&self) -> DefinitionOrigin {
        self.origin
    }

    /// Returns the name of the crate that holds the definition
    ///
    /// The name has underscores in place of dashes, as in [`TypePath`]. A
    /// builtin has [`None`], and so has a definition in a crate without a
    /// name. An item of a trait impl names the crate of the impl, which can
    /// differ from the crate of its path.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{DefinitionKind, DefinitionOrigin, Resolution};
    ///
    /// let resolution = Resolution::builder()
    ///     .kind(DefinitionKind::Crate)
    ///     .origin(DefinitionOrigin::OtherCrate)
    ///     .krate("tint_core")
    ///     .build();
    ///
    /// assert_eq!(resolution.krate(), Some("tint_core"));
    /// ```
    ///
    /// [`TypePath`]: crate::decorations::TypePath
    pub fn krate(&self) -> Option<&str> {
        self.krate.as_ref().map(string::String::as_str)
    }

    /// Returns the path of the definition, or [`None`] for one without a path
    ///
    /// Its [`Display`] renders `crate::module::Name`, which a rule can
    /// compare with a configured string. A rule compares it with a
    /// [`TypePathRef`] to test identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{DefinitionKind, DefinitionOrigin, Resolution, TypePath};
    ///
    /// let resolution = Resolution::builder()
    ///     .kind(DefinitionKind::Function)
    ///     .origin(DefinitionOrigin::OtherCrate)
    ///     .krate("facade_core")
    ///     .path(TypePath::builder().krate("facade_core").modules(["thing"]).name("make").build())
    ///     .build();
    ///
    /// assert_eq!(
    ///     resolution.path().map(ToString::to_string).as_deref(),
    ///     Some("facade_core::thing::make"),
    /// );
    /// ```
    ///
    /// [`Display`]: std::fmt::Display
    /// [`TypePathRef`]: crate::decorations::TypePathRef
    pub fn path(&self) -> Option<&TypePath> {
        self.path.as_ref()
    }

    /// Returns the definition that holds this one, or [`None`] for a top-level one
    ///
    /// A variant names its enum. An associated item names its trait, or
    /// the type of its inherent impl. An inherent impl of a type that is
    /// not an ADT, such as `str`, gives no parent and no path.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{DefinitionKind, DefinitionOrigin, Resolution, TypePath};
    ///
    /// let resolution = Resolution::builder()
    ///     .kind(DefinitionKind::Method)
    ///     .origin(DefinitionOrigin::OtherCrate)
    ///     .krate("std")
    ///     .path(TypePath::builder().krate("std").modules(["io", "Write"]).name("write_all").build())
    ///     .parent(TypePath::builder().krate("std").modules(["io"]).name("Write").build())
    ///     .build();
    ///
    /// assert_eq!(resolution.parent().map(ToString::to_string).as_deref(), Some("std::io::Write"));
    /// ```
    pub fn parent(&self) -> Option<&TypePath> {
        self.parent.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant() -> Resolution {
        Resolution::builder()
            .kind(DefinitionKind::EnumVariant)
            .origin(DefinitionOrigin::SameCrate)
            .krate("sample")
            .path(
                TypePath::builder()
                    .krate("sample")
                    .modules(["kinds", "Kind"])
                    .name("A")
                    .build(),
            )
            .parent(
                TypePath::builder()
                    .krate("sample")
                    .modules(["kinds"])
                    .name("Kind")
                    .build(),
            )
            .build()
    }

    fn binding() -> Resolution {
        Resolution::builder()
            .kind(DefinitionKind::LocalBinding)
            .origin(DefinitionOrigin::SameCrate)
            .krate("sample")
            .build()
    }

    #[test]
    fn eq_with_another_kind_is_false() {
        let resolution = binding();

        let matches = resolution
            == Resolution::builder()
                .kind(DefinitionKind::TypeParameter)
                .origin(DefinitionOrigin::SameCrate)
                .krate("sample")
                .build();

        assert!(!matches);
    }

    #[test]
    fn kind_returns_the_recorded_kind() {
        let resolution = variant();

        let kind = resolution.kind();

        assert_eq!(kind, DefinitionKind::EnumVariant);
    }

    #[test]
    fn krate_of_a_builtin_is_none() {
        let resolution = Resolution::builder()
            .kind(DefinitionKind::BuiltinType)
            .origin(DefinitionOrigin::Builtin)
            .build();

        let krate = resolution.krate();

        assert!(krate.is_none());
    }

    #[test]
    fn krate_returns_the_recorded_crate() {
        let resolution = variant();

        let krate = resolution.krate();

        assert_eq!(krate, Some("sample"));
    }

    #[test]
    fn origin_returns_the_recorded_origin() {
        let resolution = variant();

        let origin = resolution.origin();

        assert_eq!(origin, DefinitionOrigin::SameCrate);
    }

    #[test]
    fn parent_of_a_binding_is_none() {
        let resolution = binding();

        let parent = resolution.parent();

        assert!(parent.is_none());
    }

    #[test]
    fn parent_renders_the_enum_of_a_variant() {
        let resolution = variant();

        let parent = resolution.parent().map(ToString::to_string);

        assert_eq!(parent.as_deref(), Some("sample::kinds::Kind"));
    }

    #[test]
    fn path_of_a_binding_is_none() {
        let resolution = binding();

        let path = resolution.path();

        assert!(path.is_none());
    }

    #[test]
    fn path_renders_a_variant_through_its_enum() {
        let resolution = variant();

        let path = resolution.path().map(ToString::to_string);

        assert_eq!(path.as_deref(), Some("sample::kinds::Kind::A"));
    }

    #[test]
    fn trait_send_resolution() {
        fn assert_send<T: Send>() {}
        assert_send::<Resolution>();
    }

    #[test]
    fn trait_sync_resolution() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Resolution>();
    }

    #[test]
    fn trait_unpin_resolution() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Resolution>();
    }
}
