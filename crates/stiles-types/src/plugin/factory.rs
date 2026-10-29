use stabby::closure::Call0Dyn as _;
use stabby::{result, vec};

use crate::{BoxedLintPass, Language, Panic, RuleId};

/// Constructs one fresh lint pass
///
/// A plugin hands over a constructor for each lint. Passes are stateful,
/// and the check command constructs a fresh set for every file.
///
/// The factory is one of stabby's boxed callables. A plugin builds it from
/// a plain closure, and the host calls it through a vtable that stabby
/// lays out. Construction runs plugin code, so a panic in it comes back
/// as a value like any other.
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_types::plugin::{Constructed, ExportedLint, Loaded, Plugin, construct, factory};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleId, RuleOptions, boxed_lint_pass,
/// };
///
/// /// Reports nothing
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let quiet = factory(|| Constructed::Ok(boxed_lint_pass(Quiet)));
///
/// let pass: Result<_, _> = construct(&quiet).into();
///
/// assert!(pass.is_ok());
/// ```
pub type LintPassFactory =
    stabby::dynptr!(stabby::boxed::Box<dyn Fn() -> Constructed + Send + Sync>);

/// What a [`LintPassFactory`] hands back: a pass, or the panic that stopped it
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_types::plugin::{Constructed, ExportedLint, Loaded, Plugin, construct, factory};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleId, RuleOptions, boxed_lint_pass,
/// };
///
/// /// Reports nothing
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let quiet = factory(|| Constructed::Ok(boxed_lint_pass(Quiet)));
///
/// let constructed: Constructed = construct(&quiet);
/// let pass: Result<_, _> = constructed.into();
///
/// assert!(pass.is_ok());
/// ```
pub type Constructed = result::Result<BoxedLintPass, Panic>;

/// Returns the factory that runs `build`
///
/// A factory is one of stabby's boxed callables, and building one means
/// naming a stabby type. This keeps that name in this crate, so a plugin
/// exports lints without depending on stabby itself.
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_types::plugin::{Constructed, ExportedLint, Loaded, Plugin, construct, factory};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleId, RuleOptions, boxed_lint_pass,
/// };
///
/// /// Reports nothing
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let quiet = factory(|| Constructed::Ok(boxed_lint_pass(Quiet)));
///
/// let pass: Result<_, _> = construct(&quiet).into();
///
/// assert!(pass.is_ok_and(|pass| pass.language() == Language::Rust));
/// ```
pub fn factory(build: impl Fn() -> Constructed + Send + Sync + 'static) -> LintPassFactory {
    stabby::boxed::Box::new(build).into()
}

/// Runs `factory` and returns the pass it built
///
/// The factory is a stabby callable, and calling one means naming a
/// stabby trait. This keeps that name in this crate, so the loader
/// constructs a pass without depending on stabby itself.
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_types::plugin::{Constructed, ExportedLint, Loaded, Plugin, construct, factory};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleId, RuleOptions, boxed_lint_pass,
/// };
///
/// /// Reports nothing
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let quiet = factory(|| Constructed::Ok(boxed_lint_pass(Quiet)));
///
/// let pass: Result<_, _> = construct(&quiet).into();
///
/// assert!(pass.is_ok());
/// ```
pub fn construct(factory: &LintPassFactory) -> Constructed {
    factory.call()
}

/// One lint a plugin exports, described before it is built
///
/// Stiles reads the language and the rules without running the factory.
/// It knows every rule of every loaded plugin before the first file. It can
/// therefore refuse a configured rule name that nothing reports, and list
/// the rules. It can also refuse two lints that report one rule on one
/// language. The walk also holds each pass to its own rules: a diagnostic
/// under any other id ends the run.
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_types::plugin::{Constructed, ExportedLint, Loaded, Plugin, construct, factory};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleId, RuleOptions, boxed_lint_pass,
/// };
///
/// /// Reports nothing
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let quiet = factory(|| Constructed::Ok(boxed_lint_pass(Quiet)));
///
/// let lint = ExportedLint {
///     factory: quiet,
///     language: Language::Rust,
///     rules: [RuleId::new("lint.quiet")].into_iter().collect(),
/// };
///
/// assert_eq!(lint.rules.len(), 1);
/// ```
#[stabby::stabby]
pub struct ExportedLint {
    /// Builds a fresh pass of the lint
    pub factory: LintPassFactory,

    /// The language whose trees the pass inspects
    pub language: Language,

    /// Every rule the pass can report
    ///
    /// A project names the rules it runs, and stiles refuses a name that
    /// no loaded lint declares. A misspelled name would otherwise disable
    /// nothing and report nothing, which reads like a rule that found no
    /// fault.
    pub rules: vec::Vec<RuleId>,
}

/// Everything a plugin exports, handed over in one call
///
/// Building the list runs plugin code, and one call is the whole
/// boundary. A plugin that gains something else to declare puts it here
/// rather than on a second function. The list is stabby's, because the
/// plugin allocates it and stiles frees it.
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_types::plugin::{Constructed, ExportedLint, Loaded, Plugin, construct, factory};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleId, RuleOptions, boxed_lint_pass,
/// };
///
/// /// Reports nothing
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let quiet = factory(|| Constructed::Ok(boxed_lint_pass(Quiet)));
///
/// let lint = ExportedLint {
///     factory: quiet,
///     language: Language::Rust,
///     rules: [RuleId::new("lint.quiet")].into_iter().collect(),
/// };
/// let plugin = Plugin {
///     lints: [lint].into_iter().collect(),
/// };
///
/// assert_eq!(plugin.lints.len(), 1);
/// ```
#[stabby::stabby]
pub struct Plugin {
    /// Every lint the plugin exports, in the order it named them
    pub lints: vec::Vec<ExportedLint>,
}

/// What [`PluginDeclaration::load`] hands back: the plugin, or the panic that stopped it
///
/// Building either list runs plugin code, so it carries a panic like
/// every other call into the plugin.
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use stiles_types::plugin::{Constructed, ExportedLint, Loaded, Plugin, construct, factory};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleId, RuleOptions, boxed_lint_pass,
/// };
///
/// /// Reports nothing
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::Rust
///     }
///
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let quiet = factory(|| Constructed::Ok(boxed_lint_pass(Quiet)));
///
/// let lint = ExportedLint {
///     factory: quiet,
///     language: Language::Rust,
///     rules: [RuleId::new("lint.quiet")].into_iter().collect(),
/// };
/// let loaded = Loaded::Ok(Plugin {
///     lints: [lint].into_iter().collect(),
/// });
/// let plugin: Result<Plugin, _> = loaded.into();
///
/// assert!(plugin.is_ok());
/// ```
///
/// [`PluginDeclaration::load`]: crate::plugin::PluginDeclaration::load
pub type Loaded = result::Result<Plugin, Panic>;

#[cfg(test)]
mod tests {
    use stabby::closure::Call0Dyn;
    use stabby::vec;

    use super::*;
    use crate::{Checked, Configured, DecoratedNode, Language, LintPass, RuleOptions};

    struct Quiet;

    impl LintPass for Quiet {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
            Checked::Ok(vec::Vec::new())
        }
    }

    fn quiet() -> LintPassFactory {
        stabby::boxed::Box::new(|| Constructed::Ok(crate::boxed_lint_pass(Quiet))).into()
    }

    #[test]
    fn a_factory_builds_a_pass_the_host_can_configure() {
        let factory = quiet();

        let built: Result<_, Panic> = factory.call().into();

        let mut pass = built.expect("construction should not panic");
        let configured: Result<(), Panic> = pass.configure(&RuleOptions::default()).into();
        assert!(configured.is_ok());
    }

    #[test]
    fn trait_send_exported_lint() {
        fn assert_send<T: Send>() {}
        assert_send::<ExportedLint>();
    }

    #[test]
    fn trait_send_lint_pass_factory() {
        fn assert_send<T: Send>() {}
        assert_send::<LintPassFactory>();
    }

    #[test]
    fn trait_send_plugin() {
        fn assert_send<T: Send>() {}
        assert_send::<Plugin>();
    }

    #[test]
    fn trait_sync_exported_lint() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ExportedLint>();
    }

    #[test]
    fn trait_sync_lint_pass_factory() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LintPassFactory>();
    }

    #[test]
    fn trait_sync_plugin() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Plugin>();
    }

    #[test]
    fn trait_unpin_exported_lint() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ExportedLint>();
    }

    #[test]
    fn trait_unpin_lint_pass_factory() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LintPassFactory>();
    }

    #[test]
    fn trait_unpin_plugin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Plugin>();
    }
}
