use std::mem::MaybeUninit;

use bon::Builder;

use crate::plugin::LoadedPass;
use crate::plugin::Reads;
use crate::plugin::registrar::BuildFn;
use crate::{DecorationIdentity, Language, Panic, RuleId};

/// One lint a plugin registered, as the host keeps it
///
/// Stiles reads the language, the rules, and the decoration keys the pass
/// reads without running the factory.
/// It knows every rule of every loaded plugin before the first file. It can
/// therefore refuse a configured rule name that nothing reports, and list
/// the rules. It can also refuse two lints that report one rule on one
/// language. The walk also holds each pass to its own rules: a diagnostic
/// under any other id ends the run. A provider computes only the keys that
/// the loaded passes read, and stiles refuses a key that no provider of
/// the lint's language offers.
///
/// Passes are stateful, so the host builds a fresh one for every file
/// through [`RegisteredLint::build`]. The factory and the rule ids point
/// into the plugin's image, so the host keeps the library loaded for the
/// life of the process.
///
/// Only a [`Registrar`] creates a registered lint. Its `lint` makes the
/// plugin promise what the factory writes, and [`RegisteredLint::build`]
/// rests on that promise.
///
/// [`Registrar`]: crate::plugin::Registrar
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
/// registrations.lend(|registrar| {
///     registrar.require_host_tables();
///     // SAFETY: `build` never returns `Ok`.
///     unsafe { registrar.lint(Language::Rust, build, &[RuleId::new("lint.quiet")]) };
/// });
///
/// let lints = registrations.finish().expect("the registration should be whole");
///
/// assert_eq!(lints[0].language(), Language::Rust);
/// ```
#[derive(Clone, Debug, Builder)]
#[builder(builder_type(vis = "pub(crate)"), start_fn(vis = "pub(crate)"))]
pub struct RegisteredLint {
    language: Language,
    factory: BuildFn,
    rules: Vec<RuleId>,
    #[builder(default)]
    reads: Reads,
}

impl RegisteredLint {
    /// Returns the language whose trees the lint's pass inspects
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::{Built, PassInstance, Registrations};
    /// use stiles_types::{Language, Panic, RuleId};
    ///
    /// extern "C" fn build(_out: &mut std::mem::MaybeUninit<PassInstance>) -> Built {
    ///     Panic::catch(|| unreachable!("never built")).into()
    /// }
    ///
    /// let registrations = Registrations::new();
    /// registrations.lend(|registrar| {
    ///     registrar.require_host_tables();
    ///     // SAFETY: `build` never returns `Ok`.
    ///     unsafe { registrar.lint(Language::TypeScript, build, &[RuleId::new("lint.quiet")]) };
    /// });
    ///
    /// let lints = registrations.finish().expect("the registration should be whole");
    ///
    /// assert_eq!(lints[0].language(), Language::TypeScript);
    /// ```
    pub fn language(&self) -> Language {
        self.language
    }

    /// Returns every rule the lint's pass can report
    ///
    /// A project names the rules it runs, and stiles refuses a name that
    /// no loaded lint declares. A misspelled name would otherwise disable
    /// nothing and report nothing, which reads like a rule that found no
    /// fault.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::{Built, PassInstance, Registrations};
    /// use stiles_types::{Language, Panic, RuleId};
    ///
    /// extern "C" fn build(_out: &mut std::mem::MaybeUninit<PassInstance>) -> Built {
    ///     Panic::catch(|| unreachable!("never built")).into()
    /// }
    ///
    /// let registrations = Registrations::new();
    /// registrations.lend(|registrar| {
    ///     registrar.require_host_tables();
    ///     // SAFETY: `build` never returns `Ok`.
    ///     unsafe { registrar.lint(Language::Rust, build, &[RuleId::new("lint.quiet")]) };
    /// });
    ///
    /// let lints = registrations.finish().expect("the registration should be whole");
    ///
    /// assert_eq!(lints[0].rules(), [RuleId::new("lint.quiet")]);
    /// ```
    pub fn rules(&self) -> &[RuleId] {
        &self.rules
    }

    /// Returns the decorations the lint's pass declared that it reads
    ///
    /// A provider computes only the decorations that the loaded passes of
    /// its language declare. A pass whose reads are [`Reads::undeclared`]
    /// gets every decoration of its language. That fallback is staged, and
    /// [stiles#651] removes it.
    ///
    /// [stiles#651]: https://github.com/aonyx-ai/stiles/issues/651
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::{Built, PassInstance, Reads, Registrations};
    /// use stiles_types::{Language, Panic, RuleId};
    ///
    /// extern "C" fn build(_out: &mut std::mem::MaybeUninit<PassInstance>) -> Built {
    ///     Panic::catch(|| unreachable!("never built")).into()
    /// }
    ///
    /// let registrations = Registrations::new();
    /// registrations.lend(|registrar| {
    ///     registrar.require_host_tables();
    ///     // SAFETY: `build` never returns `Ok`.
    ///     unsafe { registrar.lint(Language::Rust, build, &[RuleId::new("lint.quiet")]) };
    /// });
    ///
    /// let lints = registrations.finish().expect("the registration should be whole");
    ///
    /// assert_eq!(lints[0].reads(), &Reads::undeclared());
    /// ```
    pub fn reads(&self) -> &Reads {
        &self.reads
    }

    /// Records that the lint's pass reads `identities`, each once
    pub(crate) fn read(&mut self, identities: &[DecorationIdentity]) {
        self.reads = std::mem::take(&mut self.reads).extend(identities);
    }

    /// Runs the plugin's factory, and returns the fresh pass it built
    ///
    /// Construction runs plugin code, so a panic in it comes back as a
    /// value like any other.
    ///
    /// # Errors
    ///
    /// Returns the [`Panic`] that stopped the factory.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::{Built, PassInstance, Registrations};
    /// use stiles_types::{Language, Panic, RuleId};
    ///
    /// extern "C" fn build(_out: &mut std::mem::MaybeUninit<PassInstance>) -> Built {
    ///     Panic::catch(|| panic!("no pass today")).into()
    /// }
    ///
    /// let registrations = Registrations::new();
    /// registrations.lend(|registrar| {
    ///     registrar.require_host_tables();
    ///     // SAFETY: `build` never returns `Ok`.
    ///     unsafe { registrar.lint(Language::Rust, build, &[RuleId::new("lint.quiet")]) };
    /// });
    /// let lints = registrations.finish().expect("the registration should be whole");
    ///
    /// let error = lints[0].build().err().expect("the factory should panic");
    ///
    /// assert_eq!(error.message(), "no pass today");
    /// ```
    pub fn build(&self) -> Result<LoadedPass, Panic> {
        let mut out = MaybeUninit::uninit();

        let built: Result<(), Panic> = (self.factory)(&mut out).into();
        built?;

        // SAFETY: `Registrar::lint` is the only way to create a registered
        // lint, and its caller promised that the factory writes an
        // instance whenever it returns `Ok`.
        let instance = unsafe { out.assume_init() };

        Ok(LoadedPass::builder()
            .instance(instance)
            .language(self.language)
            .build())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::plugin::{Built, PassInstance, Registrations};
    use crate::{Checked, Configured, DecoratedNode, LintPass, RuleOptions, Sink};

    /// Counts the passes that `counting` built
    static BUILT: AtomicUsize = AtomicUsize::new(0);

    /// Reports nothing
    struct Quiet;

    impl LintPass for Quiet {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(
            &mut self,
            _node: &DecoratedNode<'_>,
            _sink: &Sink<'_>,
        ) -> Checked {
            Checked::Ok(())
        }
    }

    extern "C" fn quiet(out: &mut MaybeUninit<PassInstance>) -> Built {
        Panic::catch(|| {
            out.write(PassInstance::new(Quiet));
        })
        .into()
    }

    extern "C" fn counting(out: &mut MaybeUninit<PassInstance>) -> Built {
        BUILT.fetch_add(1, Ordering::SeqCst);

        quiet(out)
    }

    extern "C" fn panicking(_out: &mut MaybeUninit<PassInstance>) -> Built {
        Panic::catch(|| panic!("no pass today")).into()
    }

    /// Returns the one lint that `factory` builds, in TypeScript
    fn registered(factory: BuildFn) -> RegisteredLint {
        let registrations = Registrations::new();
        registrations.lend(|registrar| {
            registrar.require_host_tables();
            // SAFETY: every factory of this module writes a pass whenever
            // it returns `Ok`.
            unsafe { registrar.lint(Language::TypeScript, factory, &[RuleId::new("test.rule")]) };
        });

        let mut lints = registrations
            .finish()
            .expect("the registration should be whole");

        lints.remove(0)
    }

    #[test]
    fn build_returns_a_pass_in_the_registered_language() {
        let lint = registered(quiet);

        let pass = lint.build().expect("the factory should not panic");

        assert_eq!(pass.language(), Language::TypeScript);
    }

    #[test]
    fn build_runs_the_factory_each_time() {
        let lint = registered(counting);

        let _first = lint.build();
        let _second = lint.build();

        assert_eq!(BUILT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn build_with_a_factory_that_panics_returns_the_panic() {
        let lint = registered(panicking);

        let error = lint.build().err().expect("the factory should panic");

        assert_eq!(error.message(), "no pass today");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RegisteredLint>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RegisteredLint>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RegisteredLint>();
    }
}
