use std::collections::HashSet;

use crate::DecorationKey;

/// The decoration types that the passes of one language read
///
/// A provider computes a decoration only when a pass can read it. The type
/// queries of a toolchain are the expensive part of a run. A provider that
/// receives a set therefore skips each target whose decorations the set
/// leaves out. An empty set still asks the provider for coverage. A file that
/// no toolchain covers stays an error, whether or not a pass reads its types.
///
/// # Examples
///
/// ```
/// use stiles_types::{DecorationKey, DecorationSet};
///
/// const SIGNATURE: DecorationKey = DecorationKey::new("example::Signature");
/// const RESOLVED: DecorationKey = DecorationKey::new("example::Resolved");
///
/// let wanted = DecorationSet::from_iter([SIGNATURE]);
///
/// assert!(wanted.contains_any(&[SIGNATURE, RESOLVED]));
/// assert!(!wanted.contains_any(&[RESOLVED]));
/// ```
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct DecorationSet {
    keys: HashSet<DecorationKey>,
}

impl DecorationSet {
    /// Returns whether the set holds at least one of `keys`
    ///
    /// A provider computes several decoration types from one query. It asks
    /// with every type that the query yields, and runs the query when a pass
    /// reads any of them.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecorationKey, DecorationSet};
    ///
    /// const SIGNATURE: DecorationKey = DecorationKey::new("example::Signature");
    ///
    /// assert!(!DecorationSet::default().contains_any(&[SIGNATURE]));
    /// ```
    pub fn contains_any(&self, keys: &[DecorationKey]) -> bool {
        keys.iter().any(|key| self.keys.contains(key))
    }

    /// Returns whether the set holds no decoration type
    ///
    /// A provider that receives an empty set answers for coverage alone.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::DecorationSet;
    ///
    /// assert!(DecorationSet::default().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

impl FromIterator<DecorationKey> for DecorationSet {
    fn from_iter<I: IntoIterator<Item = DecorationKey>>(keys: I) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: DecorationKey = DecorationKey::new("tests::First");
    const SECOND: DecorationKey = DecorationKey::new("tests::Second");

    #[test]
    fn contains_any_with_a_held_key_returns_true() {
        let set = DecorationSet::from_iter([FIRST]);

        assert!(set.contains_any(&[SECOND, FIRST]));
    }

    #[test]
    fn contains_any_with_no_keys_returns_false() {
        let set = DecorationSet::from_iter([FIRST]);

        assert!(!set.contains_any(&[]));
    }

    #[test]
    fn contains_any_without_a_held_key_returns_false() {
        let set = DecorationSet::from_iter([FIRST]);

        assert!(!set.contains_any(&[SECOND]));
    }

    #[test]
    fn is_empty_with_a_key_returns_false() {
        let set = DecorationSet::from_iter([FIRST]);

        assert!(!set.is_empty());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecorationSet>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecorationSet>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorationSet>();
    }
}
