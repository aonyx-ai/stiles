use std::ffi::c_void;
use std::mem;

use crate::{Checked, Configured, DecoratedNode, LintPass, Panic, RuleOptions, Sink};

/// Frees the state of a pass, in the image that allocated it
pub(crate) type DropFn = extern "C" fn(*mut c_void);

/// Applies the project's options to the state of a pass
pub(crate) type ConfigureFn = extern "C" fn(*mut c_void, &RuleOptions) -> Configured;

/// Inspects one node with the state of a pass, and reports through the sink
pub(crate) type CheckNodeFn = extern "C" fn(*mut c_void, &DecoratedNode<'_>, &Sink<'_>) -> Checked;

/// A plugin's table of the functions that run one kind of pass
///
/// A [`PassInstance`] is a pointer to a pass's state and a reference to
/// this table. The plugin writes one table for each type of pass, from
/// functions in its own image, and the table lives in that image. The host
/// calls the pass only through the entries, so no trait object and no
/// vtable cross the boundary.
///
/// The table is `#[repr(C)]`, and every entry is an `extern "C"` function.
/// Each takes the state as a bare pointer, then only C types: references
/// and stabby's laid-out values. Each entry catches a panic and hands it
/// back as a [`Panic`], because an unwind must not cross the boundary. The
/// layout is not stabby's, because stabby cannot describe a function whose
/// arguments borrow for a lifetime of its own choosing.
///
/// `drop` comes first and every version holds it, because the host must
/// free every instance it receives. `length` counts the hooks that follow
/// `drop`. Each hook is optional, and the host calls one only when the
/// plugin's `length` covers it and the entry is set. A hook added later,
/// such as `finish_file`, goes at the end. A plugin built before it then
/// writes a shorter table, and the host skips the hook. The host reads a
/// table through a raw pointer, so it never asserts that the entries past
/// `length` exist.
///
/// The type is public because [`PassInstance`] points to it. Its module is
/// not, so nothing outside this crate can name it, and it stays out of the
/// documentation.
///
/// [`PassInstance`]: crate::plugin::PassInstance
#[doc(hidden)]
#[repr(C)]
pub struct PassTable {
    pub(crate) length: usize,
    pub(crate) drop: DropFn,
    pub(crate) configure: Option<ConfigureFn>,
    pub(crate) check_node: Option<CheckNodeFn>,
}

impl PassTable {
    /// The number of hooks this version of the table holds after `drop`
    pub(crate) const LENGTH: usize = 2;

    /// The `length` a table needs for the host to read `configure`
    pub(crate) const CONFIGURE: usize = 1;

    /// The `length` a table needs for the host to read `check_node`
    pub(crate) const CHECK_NODE: usize = 2;

    /// Returns the table whose entries run a pass of type `P`
    ///
    /// The table is a constant, so it lives in the image that names `P`.
    /// For a plugin that is the plugin's own image, which stiles keeps
    /// loaded for the life of the process.
    pub(crate) fn of<P: LintPass + 'static>() -> &'static Self {
        const {
            &Self {
                length: Self::LENGTH,
                drop: drop::<P>,
                configure: Some(configure::<P>),
                check_node: Some(check_node::<P>),
            }
        }
    }
}

#[cfg(test)]
impl PassTable {
    /// Returns the table of `P` with only `LENGTH` hooks
    ///
    /// The table is cut down the way a plugin of an older protocol writes
    /// it, and it is a constant, as a plugin's table is.
    pub(crate) fn shortened<P: LintPass + 'static, const LENGTH: usize>() -> &'static Self {
        const {
            &Self {
                length: LENGTH,
                drop: drop::<P>,
                configure: Some(configure::<P>),
                check_node: Some(check_node::<P>),
            }
        }
    }
}

/// Frees the pass of type `P` that `data` points to
///
/// The box comes back here, in the image that allocated it, so the
/// allocator that built the pass frees it. A panic in the pass's own
/// `Drop` stops here too. The panic hook has already reported it, and
/// the host has nothing left to do with the pass.
extern "C" fn drop<P: LintPass + 'static>(data: *mut c_void) {
    // SAFETY: only `PassInstance::new` pairs this entry with a `data`, and
    // it writes the address of a `Box<P>` that it leaked. The instance
    // calls `drop` once, from its own `Drop`, and never reads `data` again.
    let pass = unsafe { Box::from_raw(data.cast::<P>()) };

    let _unreported = Panic::catch(move || mem::drop(pass));
}

/// Applies `options` to the pass of type `P` that `data` points to
extern "C" fn configure<P: LintPass + 'static>(
    data: *mut c_void,
    options: &RuleOptions,
) -> Configured {
    // SAFETY: `data` is the `Box<P>` of `PassInstance::new`, and it lives
    // until `drop`. The instance calls this entry only through `&mut self`,
    // so no other reference to the pass exists while it runs.
    let pass = unsafe { &mut *data.cast::<P>() };

    match Panic::catch(|| pass.configure(options)) {
        Ok(configured) => configured,
        Err(panic) => Configured::Err(panic),
    }
}

/// Checks `node` with the pass of type `P` that `data` points to
extern "C" fn check_node<P: LintPass + 'static>(
    data: *mut c_void,
    node: &DecoratedNode<'_>,
    sink: &Sink<'_>,
) -> Checked {
    // SAFETY: as for `configure` above.
    let pass = unsafe { &mut *data.cast::<P>() };

    match Panic::catch(|| pass.check_node(node, sink)) {
        Ok(checked) => checked,
        Err(panic) => Checked::Err(panic),
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{offset_of, size_of};

    use super::*;

    #[test]
    fn check_node_is_the_second_hook() {
        let second =
            (offset_of!(PassTable, check_node) - offset_of!(PassTable, drop)) / size_of::<usize>();

        assert_eq!(second, PassTable::CHECK_NODE);
    }

    #[test]
    fn configure_is_the_first_hook() {
        let first =
            (offset_of!(PassTable, configure) - offset_of!(PassTable, drop)) / size_of::<usize>();

        assert_eq!(first, PassTable::CONFIGURE);
    }

    #[test]
    fn length_counts_every_hook() {
        let hooks =
            (size_of::<PassTable>() - offset_of!(PassTable, configure)) / size_of::<usize>();

        assert_eq!(hooks, PassTable::LENGTH);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PassTable>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PassTable>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PassTable>();
    }
}
