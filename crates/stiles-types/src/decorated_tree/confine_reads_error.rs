use thiserror::Error;

use crate::DecorationKey;

/// The reason a call that [`DecoratedTree::confine`] ran broke its confinement
///
/// [`DecoratedTree::confine`]: crate::DecoratedTree::confine
///
/// # Examples
///
/// ```
/// use stiles_types::{ConfineReadsError, DecorationKey};
///
/// let error = ConfineReadsError::Undeclared {
///     key: DecorationKey::new("my_crate::Signature"),
/// };
///
/// assert_eq!(
///     error.to_string(),
///     "the pass read the decoration `my_crate::Signature`, which it does not declare"
/// );
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Error)]
pub enum ConfineReadsError {
    /// The call looked up a decoration under a key outside the declared ones
    #[error(
        "the pass read the decoration `{}`, which it does not declare",
        .key.as_str()
    )]
    Undeclared {
        /// The first undeclared key the call looked up
        key: DecorationKey,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ConfineReadsError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ConfineReadsError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ConfineReadsError>();
    }
}
