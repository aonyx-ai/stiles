use std::fmt;

use crate::context::Context;
use crate::plugin::Registrar;
use crate::sink_table::SinkTable;

/// A table of host functions that a plugin calls into
///
/// The host lends a plugin three tables: the [`Registrar`] that its
/// `register` receives, the table behind every node, and the table behind
/// every sink. A plugin built against a table of more entries may call an
/// entry this stiles lacks. The plugin therefore names the length of each
/// table it was built against through the registrar's `requires`. The
/// host refuses it when a table of its own is shorter.
///
/// A table crosses the boundary as a byte that this type pins for each
/// variant, never as a laid-out enum. A host that meets a byte it does not
/// know refuses the plugin, because the plugin needs a table this stiles
/// lacks.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::HostTable;
///
/// let lengths = HostTable::ALL.map(HostTable::length);
///
/// assert!(lengths.iter().all(|length| *length > 0));
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum HostTable {
    /// The [`Registrar`], through which a plugin registers its lints
    Registrar,
    /// The table behind every node, through which a pass reads a tree
    Context,
    /// The table behind every sink, through which a pass reports
    Sink,
}

impl HostTable {
    /// Every table a plugin calls into, in the order of their bytes
    ///
    /// `export_lints!` names the length of each through the registrar.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::HostTable;
    ///
    /// assert_eq!(HostTable::ALL.len(), 3);
    /// ```
    pub const ALL: [Self; 3] = [Self::Registrar, Self::Context, Self::Sink];

    /// The byte that names [`HostTable::Registrar`] at the plugin boundary
    ///
    /// Each byte is written out rather than taken from the discriminant.
    /// No fingerprint sees a reorder of the variants, and a reorder would
    /// otherwise make a plugin and the host read one byte as two tables.
    const REGISTRAR: u8 = 1;
    /// The byte that names [`HostTable::Context`] at the plugin boundary
    const CONTEXT: u8 = 2;
    /// The byte that names [`HostTable::Sink`] at the plugin boundary
    const SINK: u8 = 3;

    /// Returns the number of entries this build of the table holds
    ///
    /// A plugin reads the value of the stiles-types it compiled, and a host
    /// reads its own. The two differ when the plugin and the host were
    /// built from different versions.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::HostTable;
    ///
    /// assert!(HostTable::Sink.length() > 0);
    /// ```
    pub const fn length(self) -> usize {
        match self {
            Self::Registrar => Registrar::LENGTH,
            Self::Context => Context::LENGTH,
            Self::Sink => SinkTable::LENGTH,
        }
    }

    /// Returns the byte that names this table at the plugin boundary
    pub(crate) const fn to_byte(self) -> u8 {
        match self {
            Self::Registrar => Self::REGISTRAR,
            Self::Context => Self::CONTEXT,
            Self::Sink => Self::SINK,
        }
    }

    /// Returns the table that `byte` names, or [`None`] for an unknown byte
    ///
    /// A plugin built against a newer stiles may name a table this one
    /// lacks. A byte reads that as unknown, where a [`HostTable`] built
    /// from it would hold a discriminant no variant has.
    pub(crate) const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            Self::REGISTRAR => Some(Self::Registrar),
            Self::CONTEXT => Some(Self::Context),
            Self::SINK => Some(Self::Sink),
            _ => None,
        }
    }
}

impl fmt::Display for HostTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registrar => f.write_str("registrar"),
            Self::Context => f.write_str("node table"),
            Self::Sink => f.write_str("sink table"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_each_table() {
        let names = HostTable::ALL.map(|table| table.to_string());

        assert_eq!(names, ["registrar", "node table", "sink table"]);
    }

    #[test]
    fn from_byte_with_an_unknown_byte_returns_none() {
        assert_eq!(HostTable::from_byte(0), None);
    }

    #[test]
    fn from_byte_with_the_byte_of_a_table_returns_it() {
        let read = HostTable::ALL.map(|table| HostTable::from_byte(table.to_byte()));

        assert_eq!(read, HostTable::ALL.map(Some));
    }

    #[test]
    fn length_reads_the_length_of_each_table() {
        let lengths = HostTable::ALL.map(HostTable::length);

        assert_eq!(
            lengths,
            [Registrar::LENGTH, Context::LENGTH, SinkTable::LENGTH]
        );
    }

    /// A plugin and the host agree on these bytes whatever order the
    /// variants take
    #[test]
    fn to_byte_names_each_table_by_a_fixed_byte() {
        let bytes = HostTable::ALL.map(HostTable::to_byte);

        assert_eq!(bytes, [1, 2, 3]);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<HostTable>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<HostTable>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<HostTable>();
    }
}
