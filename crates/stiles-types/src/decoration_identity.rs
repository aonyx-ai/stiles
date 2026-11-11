use crate::plugin::decoration_fingerprint;
use crate::{Decoration, DecorationKey};

/// Names a decoration type by its key and the layout behind that key
///
/// A lint declares each decoration it reads by its identity, and each
/// provider offers its decorations the same way. The key alone is not
/// enough. The derive hashes a decoration's own definition into its key,
/// but not the types that the decoration holds. A change to such a type
/// keeps the key and moves the layout. Stabby's checked downcast then
/// finds nothing where the plugin expects a value. The fingerprint is
/// [`decoration_fingerprint`]: stabby's identity of the whole type, which
/// covers every field recursively, joined with the key. The loader refuses
/// a declared read whose fingerprint differs from the one a provider offers
/// under the same key.
///
/// An identity crosses the plugin boundary, so stabby lays it out.
///
/// [`decoration_fingerprint`]: crate::plugin::decoration_fingerprint
///
/// # Examples
///
/// ```
/// use stiles_types::{DecoratedNode, Decoration, DecorationIdentity, DecorationKey};
///
/// #[stabby::stabby]
/// struct Signature(u32);
///
/// impl Decoration for Signature {
///     const KEY: DecorationKey = DecorationKey::new("my_crate::Signature");
///
///     type Ref<'a> = Option<&'a Self>;
///
///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
///         node.decoration::<Self>()
///     }
/// }
///
/// const IDENTITY: DecorationIdentity = DecorationIdentity::of::<Signature>();
///
/// assert_eq!(IDENTITY.key(), Signature::KEY);
/// ```
#[stabby::stabby]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct DecorationIdentity {
    key: DecorationKey,
    fingerprint: u64,
}

impl DecorationIdentity {
    /// Returns the identity of the decoration type `D`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::decoration_fingerprint;
    /// use stiles_types::{DecoratedNode, Decoration, DecorationIdentity, DecorationKey};
    ///
    /// #[stabby::stabby]
    /// struct Signature(u32);
    ///
    /// impl Decoration for Signature {
    ///     const KEY: DecorationKey = DecorationKey::new("my_crate::Signature");
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// let identity = DecorationIdentity::of::<Signature>();
    ///
    /// assert_eq!(identity.fingerprint(), decoration_fingerprint::<Signature>());
    /// ```
    pub const fn of<D: Decoration>() -> Self {
        Self {
            key: D::KEY,
            fingerprint: decoration_fingerprint::<D>(),
        }
    }

    /// Returns the key the decoration map stores the type under
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, Decoration, DecorationIdentity, DecorationKey};
    ///
    /// #[stabby::stabby]
    /// struct Signature(u32);
    ///
    /// impl Decoration for Signature {
    ///     const KEY: DecorationKey = DecorationKey::new("my_crate::Signature");
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// assert_eq!(DecorationIdentity::of::<Signature>().key(), Signature::KEY);
    /// ```
    pub const fn key(&self) -> DecorationKey {
        self.key
    }

    /// Returns the fingerprint of the type's layout, joined with its key
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, Decoration, DecorationIdentity, DecorationKey};
    ///
    /// #[stabby::stabby]
    /// struct Signature(u32);
    ///
    /// impl Decoration for Signature {
    ///     const KEY: DecorationKey = DecorationKey::new("my_crate::Signature");
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// assert_ne!(DecorationIdentity::of::<Signature>().fingerprint(), 0);
    /// ```
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecoratedNode;

    #[stabby::stabby]
    struct Narrow(u32);

    impl Decoration for Narrow {
        const KEY: DecorationKey = DecorationKey::new("tests::Shared");

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    struct Wide(u64);

    impl Decoration for Wide {
        const KEY: DecorationKey = DecorationKey::new("tests::Shared");

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[test]
    fn of_keeps_the_key() {
        let identity = DecorationIdentity::of::<Narrow>();

        assert_eq!(identity.key(), Narrow::KEY);
    }

    #[test]
    fn of_with_one_key_over_two_layouts_differs() {
        let narrow = DecorationIdentity::of::<Narrow>();

        let wide = DecorationIdentity::of::<Wide>();

        assert_ne!(narrow, wide);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecorationIdentity>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecorationIdentity>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorationIdentity>();
    }
}
