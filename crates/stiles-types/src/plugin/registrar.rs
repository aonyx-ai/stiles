use std::ffi::c_void;
use std::mem::MaybeUninit;

use stabby::result;

use crate::plugin::{Built, PassInstance};
use crate::{Language, Panic, RuleId};

/// Builds one fresh pass, and writes it to the out-parameter
pub(crate) type BuildFn = extern "C" fn(&mut MaybeUninit<PassInstance>) -> Built;

/// Registers a lint, and returns the handle that names it
pub(crate) type LintFn = extern "C" fn(&Registrar, u8, BuildFn, *const RuleId, usize) -> usize;

/// The host's table of the functions a plugin registers its lints through
///
/// The host lends a registrar to the plugin's `register`, and the plugin
/// calls an entry for each thing it exports. A plugin never assembles a
/// laid-out list of its lints. Registration therefore grows by new
/// entries, not by new fields that every plugin would have to lay out.
///
/// `lint` registers one pass. It takes the pass's language as the byte
/// that [`Language`] pins for it, then the plugin's factory. The rule ids
/// follow as a pointer and a count. It returns a handle that names the
/// lint, so that an entry a later version appends can declare more about
/// it. The host copies the rule ids before the call returns.
///
/// The table is `#[repr(C)]`, and every entry is an `extern "C"` function
/// that takes only C types: references, integers, pointers, and stabby's
/// laid-out values. The layout is not stabby's, because stabby cannot
/// describe a function whose arguments borrow for a lifetime of its own
/// choosing.
///
/// `length` counts the entries that follow it. A later version appends
/// entries, and a reader reaches only the entries its own length names.
/// Every version holds `lint`.
///
/// `data` points to the host's [`Registrations`]. Only the host's
/// functions read it.
///
/// [`Registrations`]: crate::plugin::Registrations
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::{Built, PassInstance, Registrations};
/// use stiles_types::{Language, Panic, RuleId};
///
/// /// Builds nothing, because nothing calls it
/// extern "C" fn build(_out: &mut std::mem::MaybeUninit<PassInstance>) -> Built {
///     Panic::catch(|| unreachable!("never built")).into()
/// }
///
/// let registrations = Registrations::new();
///
/// // SAFETY: `build` never returns `Ok`.
/// registrations.lend(|registrar| unsafe {
///     registrar.lint(Language::Rust, build, &[RuleId::new("lint.quiet")])
/// });
///
/// let lints = registrations.finish().expect("the registration should be whole");
/// assert_eq!(lints.len(), 1);
/// ```
#[repr(C)]
pub struct Registrar {
    pub(crate) length: usize,
    pub(crate) data: *const c_void,
    pub(crate) lint: LintFn,
}

impl Registrar {
    /// The number of function entries this version of the table holds
    pub(crate) const LENGTH: usize = 1;

    /// Registers a lint in `language` that `factory` builds and that reports `rules`
    ///
    /// The plugin calls this from its `register`, once for each lint, and
    /// `export_lints!` writes the calls. The factory is a plain `extern
    /// "C"` function of the plugin's, so the host calls it for every file
    /// without holding a closure. It writes the pass it built through its
    /// out-parameter, and returns a [`Panic`] instead if construction
    /// panicked.
    ///
    /// The handle that comes back names the lint for the entries a later
    /// version adds. The host refuses the whole plugin when it cannot
    /// register a lint, so a refused call needs no handle of its own.
    ///
    /// # Safety
    ///
    /// `factory` must write a [`PassInstance`] to its out-parameter
    /// whenever it returns [`Ok`]. The host reads the instance after every
    /// such call, and an unwritten one is uninitialized memory.
    ///
    /// [`Ok`]: stabby::result::Result::Ok
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::{Built, PassInstance, Registrations};
    /// use stiles_types::{Language, Panic, RuleId};
    ///
    /// /// Builds nothing, because nothing calls it
    /// extern "C" fn build(_out: &mut std::mem::MaybeUninit<PassInstance>) -> Built {
    ///     Panic::catch(|| unreachable!("never built")).into()
    /// }
    ///
    /// let registrations = Registrations::new();
    ///
    /// // SAFETY: `build` never returns `Ok`.
    /// let handles = registrations.lend(|registrar| unsafe {
    ///     [
    ///         registrar.lint(Language::Rust, build, &[RuleId::new("lint.first")]),
    ///         registrar.lint(Language::Rust, build, &[RuleId::new("lint.second")]),
    ///     ]
    /// });
    ///
    /// assert_eq!(handles, [0, 1]);
    /// ```
    pub unsafe fn lint(&self, language: Language, factory: BuildFn, rules: &[RuleId]) -> usize {
        (self.lint)(
            self,
            language.to_byte(),
            factory,
            rules.as_ptr(),
            rules.len(),
        )
    }
}

/// What a plugin's `register` hands back: nothing, or the panic that stopped it
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::Registered;
///
/// let registered: Registered = Registered::Ok(());
/// let registered: Result<(), stiles_types::Panic> = registered.into();
///
/// assert!(registered.is_ok());
/// ```
pub type Registered = result::Result<(), Panic>;

// SAFETY: `data` points to a `Registrations`, which keeps everything it
// holds behind a mutex and is `Send` and `Sync`, as `Registrations`
// asserts. The functions of the table touch it only through that mutex.
unsafe impl Send for Registrar {}
// SAFETY: as for `Send` above. Two threads that register through one
// table take turns at the mutex.
unsafe impl Sync for Registrar {}

#[cfg(test)]
mod tests {
    use std::mem::{offset_of, size_of};

    use super::*;

    #[test]
    fn length_counts_every_function_entry() {
        let functions = (size_of::<Registrar>() - offset_of!(Registrar, lint)) / size_of::<usize>();

        assert_eq!(functions, Registrar::LENGTH);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Registrar>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Registrar>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Registrar>();
    }
}
