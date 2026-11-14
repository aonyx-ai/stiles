/// What kind of definition a name refers to
///
/// A rule reads it to tell a struct from an enum variant, or a function
/// from a local binding, where the spelling cannot. `Opened` in
/// `Some(Opened(x))` and `A` in `Some(A(x))` look alike, but after
/// `use Kind::*` only one of them is a variant.
///
/// The kinds follow the definitions of rust-analyzer. A function splits by
/// where it sits and whether it takes `self`, because a rule that finds a
/// call cares how it is spelled. A const or a type alias keeps one
/// kind wherever it sits. Its [`Resolution`] names a parent when it is an
/// associated item.
///
/// A rule reads it out of memory the host allocated, so the discriminant is
/// a byte.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::DefinitionKind;
///
/// let variant_pattern = DefinitionKind::EnumVariant;
///
/// assert_ne!(variant_pattern, DefinitionKind::Struct);
/// ```
///
/// [`Resolution`]: crate::decorations::Resolution
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum DefinitionKind {
    /// The root module of a crate, as `std` in `std::io`
    Crate,
    /// A module other than a crate root
    Module,
    /// A struct
    Struct,
    /// An enum
    Enum,
    /// A union
    Union,
    /// A variant of an enum
    EnumVariant,
    /// A trait
    Trait,
    /// A type alias, or an associated type of a trait or an impl
    TypeAlias,
    /// A function that sits in a module, not in a trait or an impl
    Function,
    /// A function of a trait or an impl that takes no `self`
    AssociatedFunction,
    /// A function of a trait or an impl that takes `self`
    Method,
    /// A const, or an associated const of a trait or an impl
    Const,
    /// A static
    Static,
    /// A macro: by example, procedural, or built into the compiler
    Macro,
    /// A binding of a pattern, such as a `let`, a parameter, or a match arm
    LocalBinding,
    /// A type parameter of a generic item
    TypeParameter,
    /// A const parameter of a generic item
    ConstParameter,
    /// `Self` inside an impl
    SelfType,
    /// A primitive type, such as `u8` or `str`
    BuiltinType,
    /// An attribute the compiler defines, such as `inline` or `allow`
    BuiltinAttribute,
    /// The tool segment of a tool attribute, as `rustfmt` in `#[rustfmt::skip]`
    ToolModule,
    /// A helper attribute that a derive macro declares
    DeriveHelper,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_definition_kind() {
        fn assert_send<T: Send>() {}
        assert_send::<DefinitionKind>();
    }

    #[test]
    fn trait_sync_definition_kind() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DefinitionKind>();
    }

    #[test]
    fn trait_unpin_definition_kind() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DefinitionKind>();
    }
}
