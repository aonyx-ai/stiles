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
use crate::plugin::{RegisteredLint, Registrar};
use crate::{Language, RuleId};

/// Returns the table whose functions register into the record at `data`
pub(super) fn registrar(data: *const c_void) -> Registrar {
    Registrar {
        length: Registrar::LENGTH,
        data,
        lint,
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

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use super::*;
    use crate::Panic;
    use crate::plugin::{Built, PassInstance};

    const RULE: RuleId = RuleId::new("test.rule");

    /// Builds nothing, because no test here calls it
    extern "C" fn never(_out: &mut MaybeUninit<PassInstance>) -> Built {
        Panic::catch(|| unreachable!("never built")).into()
    }

    /// Runs `work` against the table of a fresh record, and finishes it
    fn register(work: impl FnOnce(&Registrar)) -> Result<Vec<RegisteredLint>, RegisterLintsError> {
        let registrations = Registrations::new();

        registrations.lend(work);

        registrations.finish()
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
}
