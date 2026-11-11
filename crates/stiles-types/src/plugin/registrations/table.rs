//! The host's functions behind every [`Registrar`]
//!
//! Each function here is an entry of the [`Registrar`] that
//! [`Registrations::lend`] builds. The code that runs is therefore the code
//! of the image that loads the plugin. A function takes the table and
//! follows its `data` to the record. That step rests on one invariant.
//! Only the host writes a table, and only for a record that outlives it.
//!
//! A function here must not panic. It is `extern "C"`, so a panic aborts
//! the process instead of reaching the plugin that called it. A call the
//! host cannot accept is therefore kept as a fault, and the call does
//! nothing else.
//!
//! [`Registrar`]: crate::plugin::Registrar
//! [`Registrations::lend`]: crate::plugin::Registrations::lend

use std::ffi::c_void;
use std::slice;
use std::sync::MutexGuard;

use super::{RegisterLintsError, Registrations, State};
use crate::plugin::registrar::BuildFn;
use crate::plugin::{HostTable, RegisteredLint, Registrar};
use crate::{DecorationIdentity, Language, RuleId};

/// Returns the table whose functions register into the record at `data`
pub(super) fn registrar(data: *const c_void) -> Registrar {
    Registrar {
        length: Registrar::LENGTH,
        data,
        lint,
        reads,
        requires,
    }
}

/// Returns the lints that the record of `registrar` holds
fn state(registrar: &Registrar) -> MutexGuard<'_, State> {
    // SAFETY: only `Registrations::lend` writes `data`, as the address of
    // the record it borrows, and the table lives inside that call. The
    // record therefore outlives the borrow of `registrar`.
    let registrations = unsafe { &*registrar.data.cast::<Registrations>() };

    registrations.state()
}

/// Registers one lint, and returns its handle
///
/// The rule ids are copied out before the call returns, so the plugin's
/// list may go when it does. A lint whose language byte names no language
/// is kept as a fault. Its call hands back [`usize::MAX`], which names no
/// lint, and the record refuses the plugin when it finishes.
extern "C" fn lint(
    registrar: &Registrar,
    language: u8,
    factory: BuildFn,
    rules: *const RuleId,
    count: usize,
) -> usize {
    let mut state = state(registrar);
    let Some(language) = Language::from_byte(language) else {
        state.keep(RegisterLintsError::UnknownLanguage { byte: language });
        return usize::MAX;
    };
    let rules = match count {
        0 => Vec::new(),
        // SAFETY: `Registrar::lint` passes the pointer and the length of
        // one slice of rule ids, which the plugin holds for the call.
        _ => unsafe { slice::from_raw_parts(rules, count) }.to_vec(),
    };

    state.lints.push(
        RegisteredLint::builder()
            .language(language)
            .factory(factory)
            .rules(rules)
            .build(),
    );
    state.lints.len() - 1
}

/// Records that the lint behind `lint` reads the decorations of a list
///
/// The identities are copied out before the call returns, so the plugin's
/// list may go when it does. A count of zero declares that the lint reads
/// none, and reads no pointer. A handle that names no lint is kept as a
/// fault, and the record refuses the plugin when it finishes. That
/// includes [`usize::MAX`], which a refused [`lint`] hands back, so the
/// fault of that call stays the first. An identity the lint already
/// declared is kept once.
extern "C" fn reads(
    registrar: &Registrar,
    lint: usize,
    identities: *const DecorationIdentity,
    count: usize,
) {
    let mut state = state(registrar);
    let Some(registered) = state.lints.get_mut(lint) else {
        state.keep(RegisterLintsError::UnknownLint { handle: lint });
        return;
    };
    let identities = match count {
        0 => &[][..],
        // SAFETY: `Registrar::reads` passes the pointer and the length of
        // one slice of identities, which the plugin holds for the call.
        _ => unsafe { slice::from_raw_parts(identities, count) },
    };

    registered.read(identities);
}

/// Records the length of a host table that the plugin was built against
///
/// The record notes the table as named. A byte that names no table, and a
/// length past the host's own table, are each kept as a fault. The record refuses the plugin when it finishes,
/// because the plugin may call an entry that this stiles lacks.
extern "C" fn requires(registrar: &Registrar, table: u8, length: usize) {
    let mut state = state(registrar);
    let Some(table) = HostTable::from_byte(table) else {
        state.keep(RegisterLintsError::UnknownTable { byte: table });
        return;
    };

    if !state.required.contains(&table) {
        state.required.push(table);
    }
    let offered = table.length();
    if length > offered {
        state.keep(RegisterLintsError::ShorterTable {
            table,
            required: length,
            offered,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use super::*;
    use crate::plugin::{Built, PassInstance, Reads};
    use crate::{DecoratedNode, Decoration, DecorationKey, Panic};

    const RULE: RuleId = RuleId::new("test.rule");

    #[stabby::stabby]
    struct Signature(u32);

    impl Decoration for Signature {
        const KEY: DecorationKey = DecorationKey::new("test::Signature");

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    struct Resolved(u64);

    impl Decoration for Resolved {
        const KEY: DecorationKey = DecorationKey::new("test::Resolved");

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    const SIGNATURE: DecorationIdentity = DecorationIdentity::of::<Signature>();

    const RESOLVED: DecorationIdentity = DecorationIdentity::of::<Resolved>();

    /// Builds nothing, because no test here calls it
    extern "C" fn never(_out: &mut MaybeUninit<PassInstance>) -> Built {
        Panic::catch(|| unreachable!("never built")).into()
    }

    /// Runs `work` against the table of a fresh record, and finishes it
    ///
    /// The table is told every host table's length first, as
    /// `export_lints!` does.
    fn register(work: impl FnOnce(&Registrar)) -> Result<Vec<RegisteredLint>, RegisterLintsError> {
        let registrations = Registrations::new();

        registrations.lend(|registrar| {
            registrar.require_host_tables();
            work(registrar);
        });

        registrations.finish()
    }

    /// Declares `identities` for the lint behind `handle`, through the entry
    fn declare(registrar: &Registrar, handle: usize, identities: &[DecorationIdentity]) {
        reads(registrar, handle, identities.as_ptr(), identities.len());
    }

    #[test]
    fn lint_copies_the_rules() {
        let lints = register(|registrar| {
            let rules = [RULE, RuleId::new("test.other")];
            lint(
                registrar,
                Language::Rust.to_byte(),
                never,
                rules.as_ptr(),
                rules.len(),
            );
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].rules(), [RULE, RuleId::new("test.other")]);
    }

    #[test]
    fn lint_keeps_the_language() {
        let lints = register(|registrar| {
            lint(registrar, Language::TypeScript.to_byte(), never, &RULE, 1);
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].language(), Language::TypeScript);
    }

    #[test]
    fn lint_returns_a_handle_per_call() {
        let registrations = Registrations::new();

        let handles = registrations.lend(|registrar| {
            [
                lint(registrar, Language::Rust.to_byte(), never, &RULE, 1),
                lint(registrar, Language::Rust.to_byte(), never, &RULE, 1),
            ]
        });

        assert_eq!(handles, [0, 1]);
    }

    #[test]
    fn lint_with_an_unknown_language_keeps_the_first_fault() {
        let error = register(|registrar| {
            lint(registrar, u8::MAX, never, &RULE, 1);
            lint(registrar, u8::MAX - 1, never, &RULE, 1);
        })
        .expect_err("the language should be refused");

        assert_eq!(error, RegisterLintsError::UnknownLanguage { byte: u8::MAX });
    }

    #[test]
    fn lint_with_an_unknown_language_refuses_the_plugin() {
        let error = register(|registrar| {
            lint(registrar, u8::MAX, never, &RULE, 1);
        })
        .expect_err("the language should be refused");

        assert_eq!(error, RegisterLintsError::UnknownLanguage { byte: u8::MAX });
    }

    #[test]
    fn lint_with_an_unknown_language_returns_no_handle() {
        let registrations = Registrations::new();

        let handle = registrations.lend(|registrar| lint(registrar, u8::MAX, never, &RULE, 1));

        assert_eq!(handle, usize::MAX);
    }

    #[test]
    fn lint_with_no_rules_reads_no_pointer() {
        let lints = register(|registrar| {
            lint(
                registrar,
                Language::Rust.to_byte(),
                never,
                std::ptr::null(),
                0,
            );
        })
        .expect("the registration should be whole");

        assert!(lints[0].rules().is_empty());
    }

    #[test]
    fn lint_without_reads_is_undeclared() {
        let lints = register(|registrar| {
            lint(registrar, Language::Rust.to_byte(), never, &RULE, 1);
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].reads(), &Reads::undeclared());
    }

    #[test]
    fn reads_keeps_each_identity_once() {
        let lints = register(|registrar| {
            let handle = lint(registrar, Language::Rust.to_byte(), never, &RULE, 1);
            declare(registrar, handle, &[SIGNATURE, SIGNATURE]);
            declare(registrar, handle, &[SIGNATURE]);
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].reads(), &Reads::keys([SIGNATURE]));
    }

    #[test]
    fn reads_keeps_the_identities_in_order() {
        let lints = register(|registrar| {
            let handle = lint(registrar, Language::Rust.to_byte(), never, &RULE, 1);
            declare(registrar, handle, &[SIGNATURE]);
            declare(registrar, handle, &[RESOLVED]);
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].reads(), &Reads::keys([SIGNATURE, RESOLVED]));
    }

    #[test]
    fn reads_keeps_the_identities_of_each_lint_apart() {
        let lints = register(|registrar| {
            let first = lint(registrar, Language::Rust.to_byte(), never, &RULE, 1);
            let second = lint(registrar, Language::Rust.to_byte(), never, &RULE, 1);
            declare(registrar, second, &[RESOLVED]);
            declare(registrar, first, &[SIGNATURE]);
        })
        .expect("the registration should be whole");

        let reads: Vec<&Reads> = lints.iter().map(RegisteredLint::reads).collect();
        assert_eq!(reads, [&Reads::keys([SIGNATURE]), &Reads::keys([RESOLVED])]);
    }

    #[test]
    fn reads_with_an_unknown_handle_refuses_the_plugin() {
        let error = register(|registrar| {
            lint(registrar, Language::Rust.to_byte(), never, &RULE, 1);
            declare(registrar, 1, &[SIGNATURE]);
        })
        .expect_err("the handle should be refused");

        assert_eq!(error, RegisterLintsError::UnknownLint { handle: 1 });
    }

    #[test]
    fn reads_with_no_identities_declares_none_and_reads_no_pointer() {
        let lints = register(|registrar| {
            let handle = lint(registrar, Language::Rust.to_byte(), never, &RULE, 1);
            reads(registrar, handle, std::ptr::null(), 0);
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].reads(), &Reads::none());
    }

    #[test]
    fn reads_with_the_handle_of_a_refused_lint_keeps_the_first_fault() {
        let error = register(|registrar| {
            let handle = lint(registrar, u8::MAX, never, &RULE, 1);
            declare(registrar, handle, &[SIGNATURE]);
        })
        .expect_err("the language should be refused");

        assert_eq!(error, RegisterLintsError::UnknownLanguage { byte: u8::MAX });
    }

    #[test]
    fn registrar_through_the_method_declares_none() {
        let lints = register(|registrar| {
            // SAFETY: `never` never returns `Ok`.
            let handle = unsafe { registrar.lint(Language::Rust, never, &[RULE]) };
            registrar.reads(handle, &Reads::none());
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].reads(), &Reads::none());
    }

    #[test]
    fn registrar_through_the_method_declares_reads() {
        let lints = register(|registrar| {
            // SAFETY: `never` never returns `Ok`.
            let handle = unsafe { registrar.lint(Language::Rust, never, &[RULE]) };
            registrar.reads(handle, &Reads::of::<Signature>());
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].reads(), &Reads::of::<Signature>());
    }

    #[test]
    fn registrar_through_the_method_leaves_undeclared_reads_undeclared() {
        let lints = register(|registrar| {
            // SAFETY: `never` never returns `Ok`.
            let handle = unsafe { registrar.lint(Language::Rust, never, &[RULE]) };
            registrar.reads(handle, &Reads::undeclared());
        })
        .expect("the registration should be whole");

        assert_eq!(lints[0].reads(), &Reads::undeclared());
    }

    #[test]
    fn registrar_through_the_method_registers_in_order() {
        // SAFETY: `never` never returns `Ok`.
        let lints = register(|registrar| unsafe {
            registrar.lint(Language::Rust, never, &[RULE]);
            registrar.lint(Language::TypeScript, never, &[RULE]);
        })
        .expect("the registration should be whole");

        let languages: Vec<Language> = lints.iter().map(RegisteredLint::language).collect();
        assert_eq!(languages, [Language::Rust, Language::TypeScript]);
    }

    #[test]
    fn requires_with_a_longer_table_refuses_the_plugin() {
        let error = register(|registrar| {
            requires(
                registrar,
                HostTable::Context.to_byte(),
                HostTable::Context.length() + 1,
            );
        })
        .expect_err("the table should be too short");

        assert_eq!(
            error,
            RegisterLintsError::ShorterTable {
                table: HostTable::Context,
                required: HostTable::Context.length() + 1,
                offered: HostTable::Context.length(),
            }
        );
    }

    #[test]
    fn requires_with_a_shorter_table_accepts_the_plugin() {
        let registered = register(|registrar| {
            requires(registrar, HostTable::Sink.to_byte(), 1);
        });

        assert!(registered.is_ok());
    }

    #[test]
    fn requires_with_an_unknown_table_refuses_the_plugin() {
        let error = register(|registrar| {
            requires(registrar, u8::MAX, 1);
        })
        .expect_err("the table should be unknown");

        assert_eq!(error, RegisterLintsError::UnknownTable { byte: u8::MAX });
    }
}
