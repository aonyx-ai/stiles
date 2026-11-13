use crate::{Decoration, DecorationIdentity};

/// The decorations that a lint's pass reads, as the lint declares them
///
/// A provider computes only the decorations that the loaded passes read,
/// so a pass names each one it looks up. [`Reads::none`] declares that the
/// pass reads no decoration. [`Reads::of`], [`Reads::keys`], and
/// [`Reads::and`] declare exactly the decorations it reads.
///
/// A pass reads only what it declares. The walk ends the run when a pass
/// looks up a decoration it did not declare, and names the rule and the
/// decoration. Without that, the lookup would find nothing, and the rule
/// would report nothing.
///
/// Each decoration is named by its [`DecorationIdentity`], so the loader
/// can refuse a read whose layout differs from the one this stiles offers.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::Reads;
/// use stiles_types::{DecoratedNode, Decoration, DecorationKey};
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
/// let reads = Reads::of::<Signature>();
///
/// assert_eq!(reads.declared().len(), 1);
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Reads {
    identities: Vec<DecorationIdentity>,
}

impl Reads {
    /// Returns the reads of a pass that reads no decoration
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Reads;
    ///
    /// assert!(Reads::none().declared().is_empty());
    /// ```
    pub fn none() -> Self {
        Self {
            identities: Vec::new(),
        }
    }

    /// Returns the reads of a pass that reads exactly `identities`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Reads;
    ///
    /// assert_eq!(Reads::keys([]), Reads::none());
    /// ```
    pub fn keys(identities: impl IntoIterator<Item = DecorationIdentity>) -> Self {
        identities
            .into_iter()
            .fold(Self::none(), |reads, identity| reads.with(identity))
    }

    /// Returns the reads of a pass that reads only the decoration `D`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Reads;
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
    /// assert_eq!(
    ///     Reads::of::<Signature>(),
    ///     Reads::keys([DecorationIdentity::of::<Signature>()])
    /// );
    /// ```
    pub fn of<D: Decoration>() -> Self {
        Self::none().and::<D>()
    }

    /// Returns these reads with the decoration `D` added
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Reads;
    /// use stiles_types::{DecoratedNode, Decoration, DecorationKey};
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
    /// assert_eq!(Reads::none().and::<Signature>(), Reads::of::<Signature>());
    /// ```
    #[must_use]
    pub fn and<D: Decoration>(self) -> Self {
        self.with(DecorationIdentity::of::<D>())
    }

    /// Returns the declared decorations, in the order they were declared
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Reads;
    ///
    /// assert!(Reads::none().declared().is_empty());
    /// ```
    pub fn declared(&self) -> &[DecorationIdentity] {
        &self.identities
    }

    /// Returns these reads with every one of `identities` added
    ///
    /// The registrar calls this once per `reads` call.
    pub(crate) fn extend(self, identities: &[DecorationIdentity]) -> Self {
        identities
            .iter()
            .fold(self, |reads, identity| reads.with(*identity))
    }

    /// Returns these reads with `identity` added, unless they hold it
    fn with(mut self, identity: DecorationIdentity) -> Self {
        if !self.identities.contains(&identity) {
            self.identities.push(identity);
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecoratedNode, DecorationKey};

    #[stabby::stabby]
    struct First(u32);

    impl Decoration for First {
        const KEY: DecorationKey = DecorationKey::new("tests::First");

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    struct Second(u32);

    impl Decoration for Second {
        const KEY: DecorationKey = DecorationKey::new("tests::Second");

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[test]
    fn and_keeps_each_decoration_once() {
        let reads = Reads::of::<First>().and::<First>();

        assert_eq!(reads, Reads::of::<First>());
    }

    #[test]
    fn and_keeps_the_order() {
        let reads = Reads::of::<Second>().and::<First>();

        assert_eq!(
            reads.declared(),
            [
                DecorationIdentity::of::<Second>(),
                DecorationIdentity::of::<First>()
            ]
        );
    }

    #[test]
    fn extend_keeps_what_was_declared() {
        let reads = Reads::of::<First>().extend(&[DecorationIdentity::of::<Second>()]);

        assert_eq!(reads, Reads::of::<First>().and::<Second>());
    }

    #[test]
    fn keys_keeps_each_identity_once() {
        let first = DecorationIdentity::of::<First>();

        let reads = Reads::keys([first, first]);

        assert_eq!(reads, Reads::of::<First>());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Reads>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Reads>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Reads>();
    }
}
