use stabby::str::Str;

/// Identifies a decoration type across separately compiled crate graphs
///
/// The decoration map stores type-erased values and must decide, at
/// retrieval, whether a stored value is of the requested type. [`TypeId`]
/// cannot make that call, because it includes the metadata that the
/// compiler keeps per crate. One source compiled into the stiles binary and
/// into a custom lint plugin therefore produces two different ids. A lookup
/// in a plugin would then silently miss every decoration that the host
/// recorded. A key compared by string content is identical in both images
/// whenever both were compiled from one source, which the plugin handshake
/// enforces.
///
/// A key selects the entry, and stabby decides whether it holds the type
/// the caller asked for. Stabby compares the identity and the layout report
/// of the stored type before it casts. Two types that share a key therefore
/// give each other [`None`], not each other's memory. A key should still
/// name exactly one type definition, because a shared key costs a lookup
/// its answer. The derive macro builds one from the module path of the
/// type, its name, and a hash of its definition. Two types therefore stay
/// apart even where a module path and a name coincide.
///
/// The name is held as a [`Str`], stabby's string slice, because a key
/// crosses the plugin boundary with every lookup a plugin makes. Std
/// promises no layout for a string slice that holds from one compiler to
/// the next.
///
/// # Examples
///
/// ```
/// use stiles_types::DecorationKey;
///
/// const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Signature"));
///
/// assert!(KEY.as_str().ends_with("::Signature"));
/// ```
///
/// [`Decoration`]: crate::Decoration
/// [`Str`]: stabby::str::Str
/// [`TypeId`]: std::any::TypeId
#[stabby::stabby]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct DecorationKey(Str<'static>);

impl DecorationKey {
    /// Creates a key from the name that identifies a decoration type
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::DecorationKey;
    ///
    /// const KEY: DecorationKey = DecorationKey::new("my_crate::Signature");
    /// ```
    pub const fn new(name: &'static str) -> Self {
        Self(Str::new(name))
    }

    /// Returns the name this key was created from
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::DecorationKey;
    ///
    /// let key = DecorationKey::new("my_crate::Signature");
    ///
    /// assert_eq!(key.as_str(), "my_crate::Signature");
    /// ```
    pub const fn as_str(&self) -> &'static str {
        self.0.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_compares_content_not_address() {
        let literal = DecorationKey::new("tests::Marker");
        let concatenated = DecorationKey::new(concat!("tests", "::Marker"));

        assert_eq!(literal, concatenated);
    }

    #[test]
    fn keys_with_different_names_are_unequal() {
        let first = DecorationKey::new("tests::First");
        let second = DecorationKey::new("tests::Second");

        assert_ne!(first, second);
    }

    #[test]
    fn new_roundtrips_through_as_str() {
        let key = DecorationKey::new("tests::Marker");

        assert_eq!(key.as_str(), "tests::Marker");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecorationKey>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecorationKey>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorationKey>();
    }
}
