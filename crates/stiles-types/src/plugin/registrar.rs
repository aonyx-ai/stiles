use std::ffi::c_void;
use std::mem::MaybeUninit;

use stabby::result;

use crate::plugin::{Built, HostTable, PassInstance, Reads};
use crate::{DecorationIdentity, Language, Panic, RuleId};

/// Builds one fresh pass, and writes it to the out-parameter
pub(crate) type BuildFn = extern "C" fn(&mut MaybeUninit<PassInstance>) -> Built;

/// Registers a lint, and returns the handle that names it
pub(crate) type LintFn = extern "C" fn(&Registrar, u8, BuildFn, *const RuleId, usize) -> usize;

/// Declares the decorations that the lint behind a handle reads
pub(crate) type ReadsFn = extern "C" fn(&Registrar, usize, *const DecorationIdentity, usize);

/// Names the length of a host table that the plugin was built against
pub(crate) type RequiresFn = extern "C" fn(&Registrar, u8, usize);

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
/// lint, so that a later entry can declare more about it. The host
/// copies the rule ids before the call returns.
///
/// `reads` declares the decorations that the pass of a lint reads, by the
/// handle that `lint` returned. They follow as a pointer to
/// [`DecorationIdentity`] values and a count, and a count of zero declares
/// that the pass reads none. A provider computes only what the loaded
/// passes declare. A lint that never calls `reads` gets every decoration
/// of its language. That fallback is staged, and [stiles#651] removes it.
///
/// `requires` names a [`HostTable`] by its byte, and the number of entries
/// the plugin was built against. The host refuses the plugin when its own
/// table is shorter, because the plugin may call an entry the host lacks.
/// The check is coarse: it refuses the plugin even when the plugin never
/// calls the entries this stiles lacks. The host also refuses a plugin
/// that registers lints without naming every table it knows.
///
/// [stiles#651]: https://github.com/aonyx-ai/stiles/issues/651
///
/// The table is `#[repr(C)]`, and every entry is an `extern "C"` function
/// that takes only C types: references, integers, pointers, and stabby's
/// laid-out values. The layout is not stabby's, because stabby cannot
/// describe a function whose arguments borrow for a lifetime of its own
/// choosing.
///
/// `length` counts the entries that follow it. A later version appends
/// entries, and a reader reaches only the entries its own length names.
/// Every version holds `lint`, `reads`, and `requires`.
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
/// registrations.lend(|registrar| {
///     registrar.require_host_tables();
///     // SAFETY: `build` never returns `Ok`.
///     unsafe { registrar.lint(Language::Rust, build, &[RuleId::new("lint.quiet")]) };
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
    pub(crate) reads: ReadsFn,
    pub(crate) requires: RequiresFn,
}

impl Registrar {
    /// The number of function entries this version of the table holds
    pub(crate) const LENGTH: usize = 3;

    /// Registers a lint in `language` that `factory` builds and that reports `rules`
    ///
    /// The plugin calls this from its `register`, once for each lint, and
    /// `export_lints!` writes the calls. The factory is a plain `extern
    /// "C"` function of the plugin's, so the host calls it for every file
    /// without holding a closure. It writes the pass it built through its
    /// out-parameter, and returns a [`Panic`] instead if construction
    /// panicked.
    ///
    /// The handle that comes back names the lint for [`Registrar::reads`],
    /// and for the entries a later version adds. The host refuses the whole
    /// plugin when it cannot register a lint, so a refused call needs no
    /// handle of its own.
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

    /// Declares that the pass of the lint behind `lint` reads `reads`
    ///
    /// `export_lints!` calls this once for each lint, with what its
    /// `DeclaresRules::reads` returns and the handle that
    /// [`Registrar::lint`] returned for it. A provider computes only the
    /// decorations that the loaded passes declare, so a pass that reads one
    /// it never declared may find nothing.
    ///
    /// [`Reads::undeclared`] calls nothing, and the lint gets every
    /// decoration of its language. That fallback is staged, and
    /// [stiles#651] removes it. [`Reads::none`] declares that the pass
    /// reads no decoration.
    ///
    /// The host refuses the whole plugin, once its `register` returns, when
    /// no provider of the lint's language offers a declared decoration at
    /// its layout. It refuses it too when `lint` names no registered lint.
    /// The host copies the identities before the call returns.
    ///
    /// [stiles#651]: https://github.com/aonyx-ai/stiles/issues/651
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::{Built, PassInstance, Reads, Registrations};
    /// use stiles_types::{Language, Panic, RuleId};
    ///
    /// /// Builds nothing, because nothing calls it
    /// extern "C" fn build(_out: &mut std::mem::MaybeUninit<PassInstance>) -> Built {
    ///     Panic::catch(|| unreachable!("never built")).into()
    /// }
    ///
    /// let registrations = Registrations::new();
    ///
    /// registrations.lend(|registrar| {
    ///     registrar.require_host_tables();
    ///     // SAFETY: `build` never returns `Ok`.
    ///     let lint = unsafe { registrar.lint(Language::Rust, build, &[RuleId::new("lint.quiet")]) };
    ///     registrar.reads(lint, &Reads::none());
    /// });
    ///
    /// let lints = registrations.finish().expect("the registration should be whole");
    /// assert_eq!(lints[0].reads(), &Reads::none());
    /// ```
    pub fn reads(&self, lint: usize, reads: &Reads) {
        let Some(identities) = reads.declared() else {
            return;
        };

        (self.reads)(self, lint, identities.as_ptr(), identities.len());
    }

    /// Names the `length` of `table` that the plugin was built against
    ///
    /// [`Registrar::require_host_tables`] calls this for every table. The
    /// host refuses the whole plugin, once its `register` returns, when its
    /// own table is shorter.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::{HostTable, Registrations};
    ///
    /// let registrations = Registrations::new();
    ///
    /// registrations.lend(|registrar| registrar.requires(HostTable::Sink, usize::MAX));
    ///
    /// let error = registrations.finish().expect_err("no sink table is that long");
    /// assert!(error.to_string().contains("sink table"));
    /// ```
    pub fn requires(&self, table: HostTable, length: usize) {
        (self.requires)(self, table.to_byte(), length);
    }

    /// Names the length of every host table that this build calls into
    ///
    /// `export_lints!` calls this first, so each length is the one that
    /// [`HostTable::length`] reads in the plugin's own build. The host
    /// refuses a plugin that registers lints without naming each table.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Registrations;
    ///
    /// let registrations = Registrations::new();
    ///
    /// registrations.lend(|registrar| registrar.require_host_tables());
    ///
    /// let lints = registrations.finish().expect("this build knows its own tables");
    /// assert!(lints.is_empty());
    /// ```
    pub fn require_host_tables(&self) {
        for table in HostTable::ALL {
            self.requires(table, table.length());
        }
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
