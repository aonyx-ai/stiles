use thiserror::Error;

/// The reason [`Registrations`] could not accept what a plugin registered
///
/// A plugin registers through its [`Registrar`] one call at a time, and an
/// `extern "C"` function cannot fail at the boundary. The record therefore
/// keeps the first call it cannot accept, and reports it here once the
/// plugin's `register` returns.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::RegisterLintsError;
///
/// let error = RegisterLintsError::UnknownLanguage { byte: 9 };
///
/// assert_eq!(
///     error.to_string(),
///     "the plugin registered a lint for the language with byte 9, which this stiles does not \
///      know; a newer stiles may run it"
/// );
/// ```
///
/// [`Registrar`]: crate::plugin::Registrar
/// [`Registrations`]: crate::plugin::Registrations
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Error)]
pub enum RegisterLintsError {
    /// The plugin named a language by a byte that no language of this stiles has
    #[error(
        "the plugin registered a lint for the language with byte {byte}, which this stiles does \
         not know; a newer stiles may run it"
    )]
    UnknownLanguage {
        /// The byte the plugin named the language by
        byte: u8,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RegisterLintsError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RegisterLintsError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RegisterLintsError>();
    }
}
