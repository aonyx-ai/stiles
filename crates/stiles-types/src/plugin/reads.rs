use crate::{Decoration, DecorationIdentity};

/// The decorations that a lint's pass reads, as the lint declares them
///
/// A provider computes only the decorations that the loaded passes read,
/// so a pass names each one it looks up. A lint declares one of three
/// things:
///
/// - [`Reads::none`]: the pass reads no decoration, and adds nothing to
///   what a provider computes.
/// - [`Reads::of`], [`Reads::keys`], and [`Reads::and`]: the pass reads
///   exactly these decorations.
/// - [`Reads::undeclared`], the default: the pass said nothing, and it gets
///   every decoration of its language. This fallback is staged, and
///   [stiles#651] removes it. A lint that declares [`Reads::none`] or its
///   keys today means the same after that change.
///
/// Each decoration is named by its [`DecorationIdentity`], so the loader
/// can refuse a read whose layout differs from the one this stiles offers.
///
/// [stiles#651]: https://github.com/aonyx-ai/stiles/issues/651
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
/// assert_eq!(reads.declared().map(<[_]>::len), Some(1));
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct Reads {
    declared: Declared,
}

/// Whether a lint said what it reads, and what
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
enum Declared {
    /// The lint said nothing, the fallback that stiles#651 removes
    #[default]
    Undeclared,
    /// The lint reads exactly these decorations, possibly none
    Identities(Vec<DecorationIdentity>),
}

impl Reads {
    /// Returns the reads of a pass that reads no decoration
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Reads;
    ///
    /// assert_eq!(Reads::none().declared(), Some(&[][..]));
    /// ```
    pub fn none() -> Self {
        Self {
            declared: Declared::Identities(Vec::new()),
        }
    }

    /// Returns the reads of a pass that has not declared what it reads
    ///
    /// Such a pass gets every decoration of its language. This fallback is
    /// staged, and [stiles#651] removes it.
    ///
    /// [stiles#651]: https://github.com/aonyx-ai/stiles/issues/651
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Reads;
    ///
    /// assert_eq!(Reads::undeclared().declared(), None);
    /// ```
    pub fn undeclared() -> Self {
        Self::default()
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
    /// An undeclared lint that adds a decoration declares exactly that one.
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
    /// assert_eq!(Reads::undeclared().and::<Signature>(), Reads::of::<Signature>());
    /// ```
    #[must_use]
    pub fn and<D: Decoration>(self) -> Self {
        self.with(DecorationIdentity::of::<D>())
    }

    /// Returns the declared decorations, or [`None`] for an undeclared lint
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Reads;
    ///
    /// assert_eq!(Reads::undeclared().declared(), None);
    /// ```
    pub fn declared(&self) -> Option<&[DecorationIdentity]> {
        match &self.declared {
            Declared::Undeclared => None,
            Declared::Identities(identities) => Some(identities),
        }
    }

    /// Returns these reads with `identity` added, unless they hold it
    fn with(self, identity: DecorationIdentity) -> Self {
        let mut identities = match self.declared {
            Declared::Undeclared => Vec::new(),
            Declared::Identities(identities) => identities,
        };
        if !identities.contains(&identity) {
            identities.push(identity);
        }

        Self {
            declared: Declared::Identities(identities),
        }
    }

    /// Returns these reads with every one of `identities` added
    ///
    /// The registrar calls this once per `reads` call. An empty list still
    /// marks the lint as declared.
    pub(crate) fn extend(self, identities: &[DecorationIdentity]) -> Self {
        identities
            .iter()
            .fold(self.with_nothing(), |reads, identity| reads.with(*identity))
    }

    /// Returns these reads marked as declared, with nothing added
    fn with_nothing(self) -> Self {
        match self.declared {
            Declared::Undeclared => Self::none(),
            Declared::Identities(identities) => Self {
                declared: Declared::Identities(identities),
            },
        }
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
            Some(
                &[
                    DecorationIdentity::of::<Second>(),
                    DecorationIdentity::of::<First>()
                ][..]
            )
        );
    }

    #[test]
    fn default_is_undeclared() {
        let reads = Reads::default();

        assert_eq!(reads, Reads::undeclared());
    }

    #[test]
    fn extend_keeps_what_was_declared() {
        let reads = Reads::of::<First>().extend(&[DecorationIdentity::of::<Second>()]);

        assert_eq!(reads, Reads::of::<First>().and::<Second>());
    }

    #[test]
    fn extend_of_an_undeclared_lint_with_nothing_declares_none() {
        let reads = Reads::undeclared().extend(&[]);

        assert_eq!(reads, Reads::none());
    }

    #[test]
    fn none_differs_from_undeclared() {
        let none = Reads::none();

        assert_ne!(none, Reads::undeclared());
    }

    #[test]
    fn trait_send_declared() {
        fn assert_send<T: Send>() {}
        assert_send::<Declared>();
    }

    #[test]
    fn trait_send_reads() {
        fn assert_send<T: Send>() {}
        assert_send::<Reads>();
    }

    #[test]
    fn trait_sync_declared() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Declared>();
    }

    #[test]
    fn trait_sync_reads() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Reads>();
    }

    #[test]
    fn trait_unpin_declared() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Declared>();
    }

    #[test]
    fn trait_unpin_reads() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Reads>();
    }
}
