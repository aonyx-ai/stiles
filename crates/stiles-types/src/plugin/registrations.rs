use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::plugin::{HostTable, RegisteredLint, Registrar};

mod register_lints_error;
mod table;

pub use self::register_lints_error::RegisterLintsError;

/// The host's record of what a plugin registers through its [`Registrar`]
///
/// The loader lends a registrar over a fresh record to the plugin's
/// `register`, and the plugin calls an entry for each lint. The record
/// keeps a copy of everything the calls carry. [`Registrations::finish`]
/// then hands the lints over in the order the plugin registered them.
///
/// The functions behind the registrar are `extern "C"`, so none of them
/// may fail or panic. A call the host cannot accept therefore does nothing
/// at the time. The record keeps the first such fault, and `finish`
/// returns it in place of the lints.
///
/// The loader registers one plugin at a time, on one thread. The lints sit
/// behind a mutex all the same, because a plugin may call the registrar
/// from threads of its own. Nothing panics while it holds the lock, so a
/// poisoned lock still holds whole lints, and the record reads through the
/// poison.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::Registrations;
///
/// let lints = Registrations::new()
///     .finish()
///     .expect("nothing should be registered");
///
/// assert!(lints.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct Registrations {
    state: Mutex<State>,
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Registrations>();
};

/// Everything a record holds: the lints, the named tables, and the first fault
#[derive(Debug, Default)]
struct State {
    lints: Vec<RegisteredLint>,
    required: Vec<HostTable>,
    fault: Option<RegisterLintsError>,
}

impl Registrations {
    /// Creates a record that holds nothing yet
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Registrations;
    ///
    /// let lints = Registrations::new()
    ///     .finish()
    ///     .expect("nothing should be registered");
    ///
    /// assert!(lints.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Lends `work` a registrar that registers into this record
    ///
    /// The registrar and the table behind it live only as long as the
    /// call, so `work` cannot keep either. The loader passes the plugin's
    /// `register` here.
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
    /// assert_eq!(lints[0].rules(), [RuleId::new("lint.quiet")]);
    /// ```
    pub fn lend<R>(&self, work: impl FnOnce(&Registrar) -> R) -> R {
        let table = self::table::registrar(ptr::from_ref(self).cast());

        work(&table)
    }

    /// Returns every lint the plugin registered, in the order it registered them
    ///
    /// # Errors
    ///
    /// Returns [`RegisterLintsError::UnknownLanguage`] if the plugin
    /// registered a lint for a language this stiles does not know.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::Registrations;
    ///
    /// let lints = Registrations::new()
    ///     .finish()
    ///     .expect("nothing should be registered");
    ///
    /// assert!(lints.is_empty());
    /// ```
    pub fn finish(self) -> Result<Vec<RegisteredLint>, RegisterLintsError> {
        let State {
            lints,
            required,
            fault,
        } = self
            .state
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);

        if let Some(fault) = fault {
            return Err(fault);
        }
        if let Some(table) = unrequired(&lints, &required) {
            return Err(RegisterLintsError::UnrequiredTable { table });
        }

        Ok(lints)
    }

    /// Returns the lints this record holds, through any poison
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Returns the first table a plugin left unnamed, if it registered lints
///
/// `export_lints!` names every table before it registers a lint. A plugin
/// that registers lints without naming one was written by hand, or built
/// against a stiles that did not know the table. Either way the host
/// cannot tell whether that plugin calls an entry the table lacks.
fn unrequired(lints: &[RegisteredLint], required: &[HostTable]) -> Option<HostTable> {
    match lints {
        [] => None,
        [_, ..] => HostTable::ALL
            .into_iter()
            .find(|table| !required.contains(table)),
    }
}

impl State {
    /// Keeps `fault`, unless the record already holds an earlier one
    fn keep(&mut self, fault: RegisterLintsError) {
        if self.fault.is_none() {
            self.fault = Some(fault);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use super::*;
    use crate::plugin::{Built, PassInstance};
    use crate::{Language, Panic, RuleId};

    /// Builds nothing, because no test here calls it
    extern "C" fn never(_out: &mut MaybeUninit<PassInstance>) -> Built {
        Panic::catch(|| unreachable!("never built")).into()
    }

    /// Registers one lint after naming the length of every table but `skipped`
    fn register_skipping(
        skipped: Option<HostTable>,
    ) -> Result<Vec<RegisteredLint>, RegisterLintsError> {
        let registrations = Registrations::new();
        registrations.lend(|registrar| {
            for table in HostTable::ALL {
                if Some(table) != skipped {
                    registrar.requires(table, table.length());
                }
            }
            // SAFETY: `never` never returns `Ok`.
            unsafe { registrar.lint(Language::Rust, never, &[RuleId::new("test.rule")]) };
        });

        registrations.finish()
    }

    #[test]
    fn finish_with_lints_and_an_unnamed_table_names_it() {
        let error = register_skipping(Some(HostTable::Sink)).expect_err("the sink is unnamed");

        assert_eq!(
            error,
            RegisterLintsError::UnrequiredTable {
                table: HostTable::Sink
            }
        );
    }

    #[test]
    fn finish_with_lints_and_every_table_named_returns_them() {
        let lints = register_skipping(None).expect("every table is named");

        assert_eq!(lints.len(), 1);
    }

    #[test]
    fn finish_with_no_lints_needs_no_table_named() {
        let registered = Registrations::new().finish();

        assert!(registered.is_ok());
    }

    #[test]
    fn trait_send_registrations() {
        fn assert_send<T: Send>() {}
        assert_send::<Registrations>();
    }

    #[test]
    fn trait_send_state() {
        fn assert_send<T: Send>() {}
        assert_send::<State>();
    }

    #[test]
    fn trait_sync_registrations() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Registrations>();
    }

    #[test]
    fn trait_sync_state() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<State>();
    }

    #[test]
    fn trait_unpin_registrations() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Registrations>();
    }

    #[test]
    fn trait_unpin_state() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<State>();
    }
}
