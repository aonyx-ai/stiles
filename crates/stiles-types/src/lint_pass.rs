use stabby::result;

use crate::{DecoratedNode, Language, Panic, RuleOptions, Sink};

/// A lint rule that inspects decorated syntax tree nodes
///
/// This is the platform-level trait that the tree walker dispatches to.
/// Language-specific SDKs generate more ergonomic traits (e.g.
/// `RustLintPass`) that refine this into per-node-kind methods, but the
/// core pipeline operates on this common interface.
///
/// A pass lives in a plugin and the host calls it, so stabby generates the
/// vtable and every method is `extern "C"`. Rust's own calling convention
/// is a promise only one compiler makes, and so is the vtable std would
/// build for a trait object. Each method hands back a result whose error
/// is a [`Panic`]. A plugin catches a panic at its edge, because an unwind
/// must not cross the boundary, and the host reports what it caught.
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
#[stabby::stabby]
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

/// A pass boxed for the boundary: a stabby box with a stabby vtable
///
/// A plugin builds one from a pass it owns, and the host calls through it
/// without knowing the pass's type. The host can also treat it as a
/// [`LintPass`] of its own, because this type implements the trait by
/// forwarding through the vtable. The rest of stiles therefore keeps its
/// `Box<dyn LintPass>`.
///
/// # Examples
///
/// ```
/// use stiles_types::{
///     BoxedLintPass, Checked, Configured, DecoratedNode, Language, LintPass, RuleOptions, Sink,
///     boxed_lint_pass,
/// };
///
/// /// Reports nothing, on TypeScript trees only
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn language(&self) -> Language {
///         Language::TypeScript
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
/// let pass: BoxedLintPass = boxed_lint_pass(Quiet);
///
/// assert_eq!(pass.language(), Language::TypeScript);
/// ```
pub type BoxedLintPass = stabby::dynptr!(stabby::boxed::Box<dyn LintPass + Send + Sync + 'static>);

/// Boxes a pass for the boundary
///
/// # Examples
///
/// ```
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, RuleOptions, Sink, boxed_lint_pass,
/// };
///
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
/// let mut pass = boxed_lint_pass(Quiet);
///
/// let configured: Result<(), _> = pass.configure(&RuleOptions::default()).into();
/// assert!(configured.is_ok());
/// assert_eq!(pass.language(), Language::Rust);
/// ```
pub fn boxed_lint_pass(pass: impl LintPass + 'static) -> BoxedLintPass {
    stabby::boxed::Box::new(pass).into()
}

impl LintPass for BoxedLintPass {
    extern "C" fn language(&self) -> Language {
        LintPassDyn::language(self)
    }

    extern "C" fn configure(&mut self, options: &RuleOptions) -> Configured {
        LintPassDynMut::configure(self, options)
    }

    extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>, sink: &Sink<'_>) -> Checked {
        LintPassDynMut::check_node(self, node, sink)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::{Collector, DecoratedTree, Diagnostic, RuleId, Severity};

    fn rust_tree(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("the grammar should load");

        parser.parse(source, None).expect("the source should parse")
    }

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
    fn a_boxed_pass_forwards_configure_to_the_pass() {
        static CONFIGURED: AtomicU32 = AtomicU32::new(0);
        struct Counting;

        impl LintPass for Counting {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                CONFIGURED.fetch_add(1, Ordering::SeqCst);
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

        let mut pass: Box<dyn LintPass> = Box::new(boxed_lint_pass(Counting));

        let _configured = pass.configure(&RuleOptions::default());

        assert_eq!(CONFIGURED.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_boxed_pass_forwards_its_language() {
        let pass: Box<dyn LintPass> = Box::new(boxed_lint_pass(Dummy));

        let language = pass.language();

        assert_eq!(language, Language::Rust);
    }

    #[test]
    fn a_boxed_pass_forwards_the_result_of_configure() {
        let mut pass: Box<dyn LintPass> = Box::new(boxed_lint_pass(Dummy));

        let configured: Result<(), Panic> = pass.configure(&RuleOptions::default()).into();

        assert!(configured.is_ok());
    }

    #[test]
    fn a_boxed_pass_forwards_the_sink_to_the_pass() {
        struct Reporting;

        impl LintPass for Reporting {
            extern "C" fn language(&self) -> Language {
                Language::Rust
            }

            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(
                &mut self,
                node: &DecoratedNode<'_>,
                sink: &Sink<'_>,
            ) -> Checked {
                sink.report(
                    &Diagnostic::builder()
                        .rule_id(RuleId::new("test.rule"))
                        .severity(Severity::Warn)
                        .message("found")
                        .span(node.span())
                        .build(),
                );
                Checked::Ok(())
            }
        }

        let tree = DecoratedTree::builder()
            .tree(rust_tree("fn main() {}"))
            .language(Language::Rust)
            .source("fn main() {}".to_owned())
            .file("test.rs")
            .build();
        let mut pass: Box<dyn LintPass> = Box::new(boxed_lint_pass(Reporting));
        let collector = Collector::new();

        let _checked = collector.lend(|sink| pass.check_node(&tree.root_node(), sink));

        let collected = collector.finish().expect("the report should be whole");
        assert_eq!(collected.len(), 1);
    }

    #[test]
    fn trait_send_boxed_lint_pass() {
        fn assert_send<T: Send>() {}
        assert_send::<BoxedLintPass>();
    }

    #[test]
    fn trait_send_dummy() {
        fn assert_send<T: Send>() {}
        assert_send::<Dummy>();
    }

    #[test]
    fn trait_sync_boxed_lint_pass() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<BoxedLintPass>();
    }

    #[test]
    fn trait_sync_dummy() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Dummy>();
    }

    #[test]
    fn trait_unpin_boxed_lint_pass() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<BoxedLintPass>();
    }

    #[test]
    fn trait_unpin_dummy() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Dummy>();
    }
}
