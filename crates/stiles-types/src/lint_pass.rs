use stabby::result;

use crate::{DecoratedNode, Language, Panic, RuleOptions, Sink};

/// A lint rule that inspects decorated syntax tree nodes
///
/// This is the platform-level trait that the tree walker dispatches to.
/// Language-specific SDKs generate more ergonomic traits (e.g.
/// `RustLintPass`) that refine this into per-node-kind methods, but the
/// core pipeline operates on this common interface.
///
/// A pass that a plugin exports never crosses the boundary as this trait.
/// It crosses as a [`PassInstance`]: a pointer to its state and a table of
/// `extern "C"` entries, which a later version can append to. The host
/// wraps the instance in a [`LoadedPass`] that implements this trait
/// again, so the walk and every in-process pass share one interface.
///
/// Each method stays `extern "C"`, so a panic that escapes one aborts
/// rather than unwinds. A method catches its own panic and hands it back
/// as a [`Panic`], and the host reports what it caught.
///
/// [`LoadedPass`]: crate::plugin::LoadedPass
/// [`PassInstance`]: crate::plugin::PassInstance
///
/// # Examples
///
/// ```
/// use stiles_types::{Checked, Configured, DecoratedNode, Language, LintPass, RuleOptions, Sink};
///
/// /// Reports nothing, on Rust trees only
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
///     extern "C" fn check_node(
///         &mut self,
///         _node: &DecoratedNode<'_>,
///         _sink: &Sink<'_>,
///     ) -> Checked {
///         Checked::Ok(())
///     }
/// }
///
/// assert_eq!(Quiet.language(), Language::Rust);
/// ```
pub trait LintPass: Send + Sync {
    /// Returns the language whose trees this pass inspects
    ///
    /// The tree walker hands a pass only the nodes of trees in this
    /// language. A language adapter answers for the trait it adapts. A rule
    /// written against one grammar therefore never sees the nodes of another
    /// grammar that happen to share a kind name.
    extern "C" fn language(&self) -> Language;

    /// Applies the project's options to this pass
    ///
    /// Called once on every pass, after it is constructed and before it
    /// sees a node. A pass the project configured nothing for is still
    /// called, with a table that holds nothing for it.
    ///
    /// The method carries no default, so a language adapter that forwards
    /// nothing does not compile. Configuration that silently does nothing
    /// reads exactly like configuration that works.
    extern "C" fn configure(&mut self, options: &RuleOptions) -> Configured;

    /// Inspects a single node and reports any diagnostics through `sink`
    ///
    /// Called by the tree walker for every named node in the syntax tree.
    /// Implementations report nothing for nodes they do not care about.
    /// The host builds its own diagnostics from what the pass reports, so
    /// no diagnostic crosses the boundary as a laid-out value.
    extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>, sink: &Sink<'_>) -> Checked;
}

/// What [`LintPass::configure`] hands back: nothing, or the panic that stopped it
///
/// # Examples
///
/// ```
/// use stiles_types::{Configured, Panic};
///
/// let configured: Configured = Configured::Ok(());
/// let configured: Result<(), Panic> = configured.into();
///
/// assert!(configured.is_ok());
/// ```
pub type Configured = result::Result<(), Panic>;

/// What [`LintPass::check_node`] hands back: nothing, or the panic that stopped it
///
/// The diagnostics go through the [`Sink`] instead, so this carries only
/// whether the check ran to its end.
///
/// # Examples
///
/// ```
/// use stiles_types::{Checked, Panic};
///
/// let checked: Checked = Checked::Ok(());
/// let checked: Result<(), Panic> = checked.into();
///
/// assert!(checked.is_ok());
/// ```
pub type Checked = result::Result<(), Panic>;

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    impl LintPass for Dummy {
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

    #[test]
    fn trait_send_dummy() {
        fn assert_send<T: Send>() {}
        assert_send::<Dummy>();
    }

    #[test]
    fn trait_sync_dummy() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Dummy>();
    }

    #[test]
    fn trait_unpin_dummy() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Dummy>();
    }
}
