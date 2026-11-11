use std::ffi::c_void;

use stabby::str::Str;

use crate::{RuleId, Severity, Span};

/// Starts a diagnostic, and returns the handle that names it
pub(crate) type ReportFn =
    for<'n> extern "C" fn(&SinkTable, &RuleId, Severity, Str<'n>, &Span) -> usize;

/// Attaches a span and a text to the diagnostic behind a handle
pub(crate) type AttachFn = for<'n> extern "C" fn(&SinkTable, usize, &Span, Str<'n>);

/// Attaches a text to the diagnostic behind a handle
pub(crate) type NameFn = for<'n> extern "C" fn(&SinkTable, usize, Str<'n>);

/// The host's table of the functions that collect what a pass reports
///
/// A [`Sink`] is a reference to this table. A pass reports each
/// diagnostic through its entries, one call for each part, and the host
/// builds its own [`Diagnostic`] from the calls. No diagnostic therefore
/// crosses as a laid-out value, and a field added to one changes nothing
/// that a plugin reads.
///
/// The table is `#[repr(C)]`, and every entry is an `extern "C"` function
/// that takes only C types: references, integers, stabby's strings, and
/// stabby's laid-out values. `report` hands back a handle, and every
/// other entry names the diagnostic it adds to by that handle. The layout
/// is not stabby's, because stabby cannot describe a function whose
/// arguments borrow for a lifetime of its own choosing.
///
/// `length` counts the entries that follow it. A later version appends
/// entries, and a reader reaches only the entries its own length names.
///
/// `data` points to the host's [`Collector`]. Only the host's functions
/// read it.
///
/// The type is public because stabby names it in the layout it records
/// for [`Sink`]. Its module is not, so nothing outside this crate can
/// name it, and it stays out of the documentation.
///
/// [`Collector`]: crate::Collector
/// [`Diagnostic`]: crate::Diagnostic
/// [`Sink`]: crate::Sink
#[doc(hidden)]
#[repr(C)]
pub struct SinkTable {
    pub(crate) length: usize,
    pub(crate) data: *const c_void,
    pub(crate) report: ReportFn,
    pub(crate) origin: AttachFn,
    pub(crate) related: AttachFn,
    pub(crate) fix_message: NameFn,
    pub(crate) edit: AttachFn,
}

impl SinkTable {
    /// The number of function entries this version of the table holds
    pub(crate) const LENGTH: usize = 5;
}

// SAFETY: `data` points to a `Collector`, which keeps everything it
// collects behind a mutex and is `Send` and `Sync`, as `Collector`
// asserts. The functions of the table touch the collector only through
// that mutex.
unsafe impl Send for SinkTable {}
// SAFETY: as for `Send` above. Two threads that report through one table
// take turns at the collector's mutex.
unsafe impl Sync for SinkTable {}

#[cfg(test)]
mod tests {
    use std::mem::{offset_of, size_of};

    use super::*;

    #[test]
    fn length_counts_every_function_entry() {
        let functions =
            (size_of::<SinkTable>() - offset_of!(SinkTable, report)) / size_of::<usize>();

        assert_eq!(functions, SinkTable::LENGTH);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SinkTable>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SinkTable>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<SinkTable>();
    }
}
