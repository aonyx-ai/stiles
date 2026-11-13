include!(concat!(env!("OUT_DIR"), "/visitor_fingerprint.rs"));

/// A fingerprint of the Rust language support a plugin was built against
///
/// stiles-plugin joins it with every other language crate's into the
/// value a plugin's declaration carries.
///
/// The generated lint pass trait is what it covers. The trait's methods
/// are named after the grammar's node kinds. A plugin generated from a
/// different grammar still loads, and its checks never fire, so
/// `VISITOR_FINGERPRINT` hashes the generated source.
///
/// The decoration types are deliberately absent, so adding one refuses no
/// plugin. A lint declares each decoration it reads by its key and its
/// [`decoration_fingerprint`], which covers every type the decoration
/// holds. Stiles refuses a read that no provider offers at that key and
/// layout, so a changed decoration is refused by its name. Stabby's
/// checked downcast still compares every layout a plugin reads, so a
/// lookup finds nothing rather than reading the wrong bytes.
///
/// Everything else in this crate is absent too. Hashing the whole crate
/// refuses every plugin in the tree whenever any source file here changes.
///
/// [`decoration_fingerprint`]: stiles_types::plugin::decoration_fingerprint
///
/// # Examples
///
/// ```
/// use stiles_rust::LANGUAGE_FINGERPRINT;
///
/// assert_ne!(LANGUAGE_FINGERPRINT, 0);
/// ```
pub const LANGUAGE_FINGERPRINT: u64 = VISITOR_FINGERPRINT;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_is_not_zero() {
        let combined = LANGUAGE_FINGERPRINT;

        assert_ne!(combined, 0);
    }
}
