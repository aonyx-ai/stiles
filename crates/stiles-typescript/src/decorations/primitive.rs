/// The primitive a TypeScript type comes down to, if any
///
/// TypeScript gives a type alias of a primitive no type of its own:
/// `type Name = string` makes `Name` another spelling of `string`. Two
/// values of two such aliases are interchangeable to the checker. A rule
/// that asks whether a caller can swap them compares the primitive, not the
/// name.
///
/// A literal type counts as its primitive, so `"red"` is a
/// [`Primitive::String`]. A numeric enum is not a number here, because the
/// checker keeps an enum distinct.
///
/// # Examples
///
/// ```
/// use stiles_typescript::decorations::Primitive;
///
/// assert_ne!(Primitive::String, Primitive::NotPrimitive);
/// ```
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Primitive {
    /// `string`, or a string literal type
    String,
    /// `number`, or a number literal type
    Number,
    /// `bigint`, or a bigint literal type
    BigInt,
    /// `boolean`, `true`, or `false`
    Boolean,
    /// `symbol`
    Symbol,
    /// Any other type, such as an object, a union, or `any`
    NotPrimitive,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Primitive>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Primitive>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Primitive>();
    }
}
