use std::sync::Arc;

use crate::DecorationKey;

/// The decoration keys that one lint's pass declared it reads
///
/// The walk hands these to [`DecoratedTree::confine`] around each call of
/// the pass. A lookup of any other key then finds nothing, and `confine`
/// returns the first such key. The keys are shared, so a clone per call
/// costs one count and no copy.
///
/// A pass declares a few keys at most, so a lookup scans them in order.
/// That beats hashing the key on the path of every lookup.
///
/// [`DecoratedTree::confine`]: crate::DecoratedTree::confine
///
/// # Examples
///
/// ```
/// use stiles_types::{DecorationKey, ReadKeys};
///
/// const SIGNATURE: DecorationKey = DecorationKey::new("my_crate::Signature");
///
/// let keys = ReadKeys::from_iter([SIGNATURE]);
///
/// assert!(keys.contains(SIGNATURE));
/// ```
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ReadKeys {
    keys: Arc<[DecorationKey]>,
}

impl ReadKeys {
    /// Returns whether the pass declared that it reads `key`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecorationKey, ReadKeys};
    ///
    /// let keys = ReadKeys::from_iter([]);
    ///
    /// assert!(!keys.contains(DecorationKey::new("my_crate::Signature")));
    /// ```
    pub fn contains(&self, key: DecorationKey) -> bool {
        self.keys.contains(&key)
    }
}

impl FromIterator<DecorationKey> for ReadKeys {
    fn from_iter<I: IntoIterator<Item = DecorationKey>>(keys: I) -> Self {
        let mut unique = Vec::new();
        for key in keys {
            if !unique.contains(&key) {
                unique.push(key);
            }
        }

        Self {
            keys: unique.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: DecorationKey = DecorationKey::new("tests::First");
    const SECOND: DecorationKey = DecorationKey::new("tests::Second");

    #[test]
    fn contains_with_a_declared_key_returns_true() {
        let keys = ReadKeys::from_iter([FIRST, SECOND]);

        assert!(keys.contains(SECOND));
    }

    #[test]
    fn contains_with_another_key_returns_false() {
        let keys = ReadKeys::from_iter([FIRST]);

        assert!(!keys.contains(SECOND));
    }

    #[test]
    fn from_iter_keeps_each_key_once() {
        let keys = ReadKeys::from_iter([FIRST, FIRST]);

        assert_eq!(keys, ReadKeys::from_iter([FIRST]));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ReadKeys>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ReadKeys>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ReadKeys>();
    }
}
