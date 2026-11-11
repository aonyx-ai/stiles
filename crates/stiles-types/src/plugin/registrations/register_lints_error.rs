use thiserror::Error;

use crate::plugin::HostTable;

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

    /// The plugin declared a read for a handle that names no lint it registered
    #[error(
        "the plugin declared a decoration for the lint with handle {handle}, which it never \
         registered"
    )]
    UnknownLint {
        /// The handle the plugin passed
        handle: usize,
    },

    /// The plugin named a host table by a byte that no table of this stiles has
    #[error(
        "the plugin needs the host table with byte {byte}, which this stiles does not have; a \
         newer stiles may run it"
    )]
    UnknownTable {
        /// The byte the plugin named the table by
        byte: u8,
    },

    /// The plugin registered lints without naming the length of a host table
    #[error(
        "the plugin registered lints without naming the {table} it was built against; export \
         its lints with stiles_plugin::export_lints!"
    )]
    UnrequiredTable {
        /// The first table the plugin left unnamed
        table: HostTable,
    },

    /// The plugin was built against a longer table than this stiles has
    #[error(
        "the plugin was built against a {table} of {required} entries, and this stiles has \
         {offered}; a newer stiles may run it"
    )]
    ShorterTable {
        /// The table the plugin needs
        table: HostTable,
        /// The number of entries the plugin was built against
        required: usize,
        /// The number of entries this stiles has
        offered: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_with_a_shorter_table_names_the_table_and_both_lengths() {
        let error = RegisterLintsError::ShorterTable {
            table: HostTable::Sink,
            required: 7,
            offered: 5,
        };

        assert_eq!(
            error.to_string(),
            "the plugin was built against a sink table of 7 entries, and this stiles has 5; a \
             newer stiles may run it"
        );
    }

    #[test]
    fn display_with_an_unknown_lint_names_the_handle() {
        let error = RegisterLintsError::UnknownLint { handle: 3 };

        assert_eq!(
            error.to_string(),
            "the plugin declared a decoration for the lint with handle 3, which it never registered"
        );
    }

    #[test]
    fn display_with_an_unknown_table_names_the_byte() {
        let error = RegisterLintsError::UnknownTable { byte: 9 };

        assert_eq!(
            error.to_string(),
            "the plugin needs the host table with byte 9, which this stiles does not have; a newer \
             stiles may run it"
        );
    }

    #[test]
    fn display_with_an_unrequired_table_names_the_table() {
        let error = RegisterLintsError::UnrequiredTable {
            table: HostTable::Context,
        };

        assert_eq!(
            error.to_string(),
            "the plugin registered lints without naming the node table it was built against; \
             export its lints with stiles_plugin::export_lints!"
        );
    }

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
