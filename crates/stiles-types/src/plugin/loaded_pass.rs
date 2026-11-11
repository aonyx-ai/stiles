use bon::Builder;

use crate::plugin::PassInstance;
use crate::{Checked, Configured, DecoratedNode, Language, LintPass, RuleOptions, Sink};

/// A pass a plugin built, as the host runs it
///
/// The plugin hands the host a [`PassInstance`], and this wraps it in the
/// host's [`LintPass`] trait. The walk therefore runs a plugin's pass like
/// any pass of its own. Each method calls the matching entry of the
/// plugin's table, and dropping the pass drops the instance, which frees
/// the state in the plugin's image.
///
/// A hook the plugin's table lacks does nothing. A missing `configure`
/// reads no options, and a missing `check_node` reports nothing, as a pass
/// that ignores every node would.
///
/// The language is the one the plugin registered the lint under, and the
/// host keeps it. No entry of the table answers for it.
///
/// A loaded pass is [`Send`] and [`Sync`] for the reasons its instance is.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::{Built, PassInstance, Registrations};
/// use stiles_types::{
///     Checked, Configured, DecoratedNode, Language, LintPass, Panic, RuleId, RuleOptions, Sink,
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
/// extern "C" fn build(out: &mut std::mem::MaybeUninit<PassInstance>) -> Built {
///     Panic::catch(|| {
///         out.write(PassInstance::new(Quiet));
///     })
///     .into()
/// }
///
/// let registrations = Registrations::new();
/// // SAFETY: `build` writes a pass whenever it returns `Ok`.
/// registrations.lend(|registrar| unsafe {
///     registrar.lint(Language::TypeScript, build, &[RuleId::new("lint.quiet")])
/// });
/// let lints = registrations.finish().expect("the registration should be whole");
///
/// let pass = lints[0].build().expect("the factory should not panic");
///
/// assert_eq!(pass.language(), Language::TypeScript);
/// ```
#[derive(Builder)]
#[builder(builder_type(vis = "pub(crate)"), start_fn(vis = "pub(crate)"))]
pub struct LoadedPass {
    instance: PassInstance,
    language: Language,
}

impl LintPass for LoadedPass {
    extern "C" fn language(&self) -> Language {
        self.language
    }

    extern "C" fn configure(&mut self, options: &RuleOptions) -> Configured {
        self.instance
            .configure(options)
            .unwrap_or(Configured::Ok(()))
    }

    extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>, sink: &Sink<'_>) -> Checked {
        self.instance
            .check_node(node, sink)
            .unwrap_or(Checked::Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::plugin::pass_table::PassTable;
    use crate::{Collector, DecoratedTree, Panic};

    /// Counts the calls of each hook, and panics in `check_node`
    struct Hooked {
        calls: Arc<AtomicUsize>,
    }

    impl LintPass for Hooked {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Configured::Ok(())
        }

        extern "C" fn check_node(
            &mut self,
            _node: &DecoratedNode<'_>,
            _sink: &Sink<'_>,
        ) -> Checked {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Panic::catch(|| panic!("checked")).into()
        }
    }

    /// Returns a loaded pass in `language`, and the counter of its calls
    fn loaded(language: Language) -> (LoadedPass, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let instance = PassInstance::new(Hooked {
            calls: Arc::clone(&calls),
        });

        (
            LoadedPass::builder()
                .instance(instance)
                .language(language)
                .build(),
            calls,
        )
    }

    fn tree() -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("the grammar should load");
        let tree = parser
            .parse("fn main() {}", None)
            .expect("the source should parse");

        DecoratedTree::builder()
            .tree(tree)
            .language(Language::Rust)
            .source("fn main() {}".to_owned())
            .file("test.rs")
            .build()
    }

    #[test]
    fn check_node_hands_back_the_panic_of_the_pass() {
        let (mut pass, _calls) = loaded(Language::Rust);
        let tree = tree();
        let collector = Collector::new();

        let checked: Result<(), Panic> = collector
            .lend(|sink| pass.check_node(&tree.root_node(), sink))
            .into();

        assert_eq!(
            checked.expect_err("the pass should panic").message(),
            "checked"
        );
    }

    #[test]
    fn check_node_with_a_table_short_of_it_succeeds() {
        let (pass, _calls) = loaded(Language::Rust);
        let LoadedPass { instance, language } = pass;
        let mut pass = LoadedPass::builder()
            .instance(
                instance
                    .with_table(PassTable::shortened::<Hooked, { PassTable::CHECK_NODE - 1 }>()),
            )
            .language(language)
            .build();
        let tree = tree();
        let collector = Collector::new();

        let checked: Result<(), Panic> = collector
            .lend(|sink| pass.check_node(&tree.root_node(), sink))
            .into();

        assert!(checked.is_ok());
    }

    #[test]
    fn configure_calls_the_plugin_once() {
        let (mut pass, calls) = loaded(Language::Rust);

        let _configured = pass.configure(&RuleOptions::default());

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn configure_with_a_table_short_of_it_succeeds() {
        let (pass, _calls) = loaded(Language::Rust);
        let LoadedPass { instance, language } = pass;
        let mut pass = LoadedPass::builder()
            .instance(
                instance.with_table(PassTable::shortened::<Hooked, { PassTable::CONFIGURE - 1 }>()),
            )
            .language(language)
            .build();

        let configured: Result<(), Panic> = pass.configure(&RuleOptions::default()).into();

        assert!(configured.is_ok());
    }

    #[test]
    fn language_is_the_registered_one() {
        let (pass, _calls) = loaded(Language::TypeScript);

        let language = pass.language();

        assert_eq!(language, Language::TypeScript);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LoadedPass>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LoadedPass>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LoadedPass>();
    }
}
