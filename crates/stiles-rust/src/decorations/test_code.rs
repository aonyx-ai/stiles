use stiles_macros::Decoration;

/// Says that only a test build compiles an item, and why
///
/// A rule reads it to spare tests what it asks of shipped code, such as a
/// ban on `unwrap` or on printing to the terminal. Each rule used to walk
/// the attributes itself, so each one drew the line in its own place.
///
/// The provider attaches it to every item that is test code. An item
/// counts when it carries `#[test]` or `#[bench]`, under any path such as
/// `#[tokio::test]`. It counts too when it carries a `cfg` whose predicate
/// holds only in a test build, such as `#[cfg(test)]` or
/// `#[cfg(all(test, unix))]`. A `cfg_attr` never counts, because it adds an
/// attribute and does not remove the item. A predicate the provider cannot
/// read does not count either.
///
/// The mark can sit in front of the item, or open its body as an inner
/// attribute, as in `mod tests { #![cfg(test)] }`. Every item nested in a
/// marked item counts as well. So does every item of a file whose own
/// module is declared as test code, as `#[cfg(test)] mod tests;` declares
/// `tests.rs`. A file that opens with `#![cfg(test)]` counts the same way.
///
/// The provider decides from the syntax of the file, plus the declarations
/// of its module, which rust-analyzer finds. A `cfg` on a statement, a
/// field, or a variant is not an item and gets nothing.
///
/// The items are functions, modules, impls, structs, enums, unions, traits,
/// type aliases, consts, statics, uses, extern crates, extern blocks, and
/// macro definitions. Function signatures and associated types count too.
/// A macro call counts where it stands in place of an item, at the top of
/// a module. Other nodes get nothing, so the cost grows with the items of a
/// file. A rule that asks about any node climbs to the nearest item.
///
/// A rule reads it out of memory the host allocated, so the discriminant
/// is a byte.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::TestCode;
///
/// let test_code = TestCode::Enclosed;
///
/// let marked_here = match test_code {
///     TestCode::Marked => true,
///     TestCode::Enclosed => false,
/// };
///
/// assert!(!marked_here);
/// ```
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub enum TestCode {
    /// The item itself carries a test attribute or a `cfg` that implies `test`
    Marked,
    /// An enclosing item, block, module, or file is test code
    Enclosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_test_code() {
        fn assert_send<T: Send>() {}
        assert_send::<TestCode>();
    }

    #[test]
    fn trait_sync_test_code() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TestCode>();
    }

    #[test]
    fn trait_unpin_test_code() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TestCode>();
    }
}
