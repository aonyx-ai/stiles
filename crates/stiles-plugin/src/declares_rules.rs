use stiles_types::RuleId;
use stiles_types::plugin::Reads;

/// Declares the rules a lint pass can report, and the decorations it reads
///
/// Stiles refuses a configured rule name that no loaded plugin declares,
/// and this is where a plugin says which names are its own.
///
/// A provider computes only the decorations that the loaded passes read,
/// so a pass names each decoration it looks up through
/// [`DeclaresRules::reads`]. A pass that reads none says
/// [`Reads::none`].
///
/// The trait never crosses the plugin boundary. [`export_lints!`] calls it
/// inside the plugin's own compilation. The result sits behind a function
/// pointer in the declaration, so nothing here depends on the layout of a
/// vtable. That is what lets stiles read the rules of a
/// plugin built against a newer protocol than an older one without
/// refusing either.
///
/// [`export_lints!`]: macro@crate::export_lints
///
/// # Examples
///
/// ```ignore
/// impl DeclaresRules for NoTodo {
///     fn rules(&self) -> Vec<RuleId> {
///         vec![RuleId::new("custom.no-todo")]
///     }
///
///     fn reads(&self) -> Reads {
///         Reads::none()
///     }
/// }
///
/// impl DeclaresRules for BareTry {
///     fn rules(&self) -> Vec<RuleId> {
///         vec![RuleId::new("custom.bare-try")]
///     }
///
///     fn reads(&self) -> Reads {
///         Reads::of::<FnSignature>().and::<ResolvedType>()
///     }
/// }
/// ```
pub trait DeclaresRules {
    /// Returns every rule this pass can report
    fn rules(&self) -> Vec<RuleId>;

    /// Returns the decorations this pass reads
    ///
    /// A provider computes a decoration only when a loaded pass of its
    /// language declares it. A lookup of a decoration the pass did not
    /// declare may therefore find nothing, and the rule fails open.
    ///
    /// Stiles refuses the plugin when no provider of the pass's language
    /// offers a declared decoration at the layout the plugin was built
    /// against. A changed decoration, or a changed type inside one, is
    /// therefore refused by its name.
    ///
    /// The default is [`Reads::undeclared`]. Such a pass gets every
    /// decoration of its language, as it did before passes declared their
    /// reads. That fallback is staged, and [stiles#651] removes it. Until
    /// then, such a pass also misses a changed decoration quietly.
    ///
    /// [stiles#651]: https://github.com/aonyx-ai/stiles/issues/651
    fn reads(&self) -> Reads {
        Reads::undeclared()
    }
}
