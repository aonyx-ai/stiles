use std::ffi::c_void;
use std::ptr;

use stabby::result;

use crate::plugin::pass_table::{CheckNodeFn, ConfigureFn, DropFn, PassTable};
use crate::{Checked, Configured, DecoratedNode, LintPass, Panic, RuleOptions, Sink};

/// One pass a plugin built, as it crosses the boundary
///
/// An instance is a pointer to the pass's state and a pointer to the
/// `PassTable` that runs it. Both point into the plugin: the state into
/// its heap, the table into its image. The host never reads the state. It
/// hands the pointer back to an entry of the table, so only the plugin's
/// code touches the pass.
///
/// The side that allocated the state frees it. [`PassInstance::new`]
/// boxes the pass with the plugin's allocator, and dropping the instance
/// calls the table's `drop`, which frees the box in the plugin's image. A
/// plugin may therefore set its own `#[global_allocator]`. An instance
/// calls `drop` exactly once, because only its own `Drop` calls it.
///
/// The host holds the table as a raw pointer rather than a reference. A
/// plugin built against an older protocol writes a shorter table, and a
/// reference would assert that every entry of this version is there. The
/// host reads `length` first, and an entry only when `length` covers it.
///
/// The host builds a pass for each file, and files run in parallel. An
/// instance is [`Send`] because the pass it holds is: [`LintPass`]
/// requires `Send`, and the table holds only functions. It is [`Sync`]
/// because a shared reference reaches nothing. Every entry that touches
/// the state runs through `&mut self`, so one thread at a time calls into
/// a pass, and never two at once.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::PassInstance;
/// use stiles_types::{Checked, Configured, DecoratedNode, Language, LintPass, RuleOptions, Sink};
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
///     extern "C" fn check_node(
///         &mut self,
///         _node: &DecoratedNode<'_>,
///         _sink: &Sink<'_>,
///     ) -> Checked {
///         Checked::Ok(())
///     }
/// }
///
/// let instance = PassInstance::new(Quiet);
///
/// drop(instance);
/// ```
#[repr(C)]
pub struct PassInstance {
    data: *mut c_void,
    table: *const PassTable,
}

impl PassInstance {
    /// Boxes `pass` and pairs it with the table that runs its type
    ///
    /// A plugin calls this, through the adapter that `lint_pass_adapter!`
    /// writes, so the box and the table both belong to the plugin's image.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::plugin::PassInstance;
    /// use stiles_types::{
    ///     Checked, Configured, DecoratedNode, Language, LintPass, RuleOptions, Sink,
    /// };
    ///
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
    /// let instance = PassInstance::new(Quiet);
    ///
    /// drop(instance);
    /// ```
    pub fn new<P: LintPass + 'static>(pass: P) -> Self {
        Self {
            data: Box::into_raw(Box::new(pass)).cast(),
            table: ptr::from_ref(PassTable::of::<P>()),
        }
    }

    /// Runs the pass's `configure`, or returns [`None`] if its table lacks it
    pub(crate) fn configure(&mut self, options: &RuleOptions) -> Option<Configured> {
        let configure = self.configure_entry()?;

        Some(configure(self.data, options))
    }

    /// Runs the pass's `check_node`, or returns [`None`] if its table lacks it
    pub(crate) fn check_node(
        &mut self,
        node: &DecoratedNode<'_>,
        sink: &Sink<'_>,
    ) -> Option<Checked> {
        let check_node = self.check_node_entry()?;

        Some(check_node(self.data, node, sink))
    }

    /// Returns the number of hooks the plugin's table holds after `drop`
    fn length(&self) -> usize {
        // SAFETY: every protocol starts a table with `length`, and the
        // plugin's table lives in its image, which stiles never unloads.
        unsafe { (&raw const (*self.table).length).read() }
    }

    /// Returns the table's `drop`, which every version holds
    fn drop_entry(&self) -> DropFn {
        // SAFETY: every protocol puts `drop` after `length`, as a function
        // that is never null, so the read needs no `length`.
        unsafe { (&raw const (*self.table).drop).read() }
    }

    /// Returns the table's `configure`, if its `length` covers it
    fn configure_entry(&self) -> Option<ConfigureFn> {
        if self.length() < PassTable::CONFIGURE {
            return None;
        }

        // SAFETY: the plugin's `length` says that its table holds this
        // entry, at the offset this version gives it.
        unsafe { (&raw const (*self.table).configure).read() }
    }

    /// Returns the table's `check_node`, if its `length` covers it
    fn check_node_entry(&self) -> Option<CheckNodeFn> {
        if self.length() < PassTable::CHECK_NODE {
            return None;
        }

        // SAFETY: as for `configure_entry` above.
        unsafe { (&raw const (*self.table).check_node).read() }
    }
}

#[cfg(test)]
impl PassInstance {
    /// Returns this instance with `table` in place of the table of its type
    ///
    /// A test hands it a table cut down the way a plugin of an older
    /// protocol writes it, from [`PassTable::shortened`].
    pub(crate) fn with_table(mut self, table: &'static PassTable) -> Self {
        self.table = ptr::from_ref(table);

        self
    }
}

impl Drop for PassInstance {
    fn drop(&mut self) {
        let drop = self.drop_entry();

        drop(self.data);
    }
}

/// What a plugin's factory hands back: nothing, or the panic that stopped it
///
/// A factory writes the [`PassInstance`] it built through an out-parameter,
/// and only an [`Ok`] says that it wrote one.
///
/// [`Ok`]: stabby::result::Result::Ok
///
/// # Examples
///
/// ```
/// use stiles_types::Panic;
/// use stiles_types::plugin::Built;
///
/// let built: Built = Panic::catch(|| panic!("boom")).into();
/// let built: Result<(), Panic> = built.into();
///
/// assert!(built.is_err());
/// ```
pub type Built = result::Result<(), Panic>;

// SAFETY: `data` holds a pass whose type implements `LintPass`, which
// requires `Send`, and only the plugin's entries touch it. The table holds
// functions and a length, and nothing writes it.
unsafe impl Send for PassInstance {}
// SAFETY: no method reaches `data` through `&self`. Every call into the
// pass takes `&mut self`, so two threads never call into one pass at once.
unsafe impl Sync for PassInstance {}

#[cfg(test)]
mod tests {
    use std::panic::{self, AssertUnwindSafe};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::{Collector, DecoratedTree, Diagnostic, Language, RuleId, Severity};

    /// Counts what a pass of this type ran, in counters of its own
    struct Counting {
        configured: Arc<AtomicUsize>,
        checked: Arc<AtomicUsize>,
        dropped: Arc<AtomicUsize>,
    }

    impl Counting {
        /// Returns a pass that counts into fresh counters, and the counters
        fn new() -> (Self, [Arc<AtomicUsize>; 3]) {
            let counters = [(); 3].map(|()| Arc::new(AtomicUsize::new(0)));
            let [configured, checked, dropped] = counters.clone();
            let pass = Self {
                configured,
                checked,
                dropped,
            };

            (pass, counters)
        }
    }

    impl Drop for Counting {
        fn drop(&mut self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl LintPass for Counting {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            self.configured.fetch_add(1, Ordering::SeqCst);
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>, sink: &Sink<'_>) -> Checked {
            self.checked.fetch_add(1, Ordering::SeqCst);
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

    /// Panics in every hook and in its own drop
    struct Panicking;

    impl Drop for Panicking {
        fn drop(&mut self) {
            panic!("dropped");
        }
    }

    impl LintPass for Panicking {
        extern "C" fn language(&self) -> Language {
            Language::Rust
        }

        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Panic::catch(|| panic!("configured")).into()
        }

        extern "C" fn check_node(
            &mut self,
            _node: &DecoratedNode<'_>,
            _sink: &Sink<'_>,
        ) -> Checked {
            Panic::catch(|| panic!("checked")).into()
        }
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

    /// Checks the root of a small tree with `instance`, and returns the result
    fn check(instance: &mut PassInstance) -> Option<Checked> {
        let tree = tree();
        let collector = Collector::new();

        collector.lend(|sink| instance.check_node(&tree.root_node(), sink))
    }

    #[test]
    fn check_node_reports_through_the_sink() {
        let (pass, _counters) = Counting::new();
        let mut instance = PassInstance::new(pass);
        let tree = tree();
        let collector = Collector::new();

        let _checked = collector.lend(|sink| instance.check_node(&tree.root_node(), sink));

        let collected = collector.finish().expect("the report should be whole");
        assert_eq!(collected.len(), 1);
    }

    #[test]
    fn check_node_returns_the_panic_of_the_pass() {
        let mut instance = PassInstance::new(Panicking);

        let checked = check(&mut instance).expect("the table should hold check_node");

        let checked: Result<(), Panic> = checked.into();
        assert_eq!(
            checked.expect_err("the pass should panic").message(),
            "checked"
        );
    }

    #[test]
    fn check_node_runs_the_pass_once() {
        let (pass, [_configured, checked, _dropped]) = Counting::new();
        let mut instance = PassInstance::new(pass);

        let _checked = check(&mut instance);

        assert_eq!(checked.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn check_node_with_a_table_short_of_it_is_skipped() {
        let (pass, [_configured, checked, _dropped]) = Counting::new();
        let mut instance = PassInstance::new(pass).with_table(PassTable::shortened::<
            Counting,
            { PassTable::CHECK_NODE - 1 },
        >());

        let _checked = check(&mut instance);

        assert_eq!(checked.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn check_node_with_a_table_short_of_it_returns_none() {
        let (pass, _counters) = Counting::new();
        let mut instance = PassInstance::new(pass).with_table(PassTable::shortened::<
            Counting,
            { PassTable::CHECK_NODE - 1 },
        >());

        let checked = check(&mut instance);

        assert!(checked.is_none());
    }

    #[test]
    fn configure_returns_the_panic_of_the_pass() {
        let mut instance = PassInstance::new(Panicking);

        let configured = instance
            .configure(&RuleOptions::default())
            .expect("the table should hold configure");

        let configured: Result<(), Panic> = configured.into();
        assert_eq!(
            configured.expect_err("the pass should panic").message(),
            "configured"
        );
    }

    #[test]
    fn configure_runs_the_pass_once() {
        let (pass, [configured, _checked, _dropped]) = Counting::new();
        let mut instance = PassInstance::new(pass);

        let _configured = instance.configure(&RuleOptions::default());

        assert_eq!(configured.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn configure_with_a_table_short_of_check_node_still_runs() {
        let (pass, [configured, _checked, _dropped]) = Counting::new();
        let mut instance = PassInstance::new(pass)
            .with_table(PassTable::shortened::<Counting, { PassTable::CONFIGURE }>());

        let _configured = instance.configure(&RuleOptions::default());

        assert_eq!(configured.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn configure_with_a_table_short_of_it_is_skipped() {
        let (pass, [configured, _checked, _dropped]) = Counting::new();
        let mut instance = PassInstance::new(pass)
            .with_table(PassTable::shortened::<Counting, { PassTable::CONFIGURE - 1 }>());

        let _configured = instance.configure(&RuleOptions::default());

        assert_eq!(configured.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn configure_with_a_table_short_of_it_returns_none() {
        let (pass, _counters) = Counting::new();
        let mut instance = PassInstance::new(pass)
            .with_table(PassTable::shortened::<Counting, { PassTable::CONFIGURE - 1 }>());

        let configured = instance.configure(&RuleOptions::default());

        assert!(configured.is_none());
    }

    #[test]
    fn drop_frees_the_pass_exactly_once() {
        let (pass, [_configured, _checked, dropped]) = Counting::new();
        let instance = PassInstance::new(pass);

        drop(instance);

        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drop_of_a_pass_that_panics_returns() {
        let instance = PassInstance::new(Panicking);

        let unwound = panic::catch_unwind(AssertUnwindSafe(|| drop(instance)));

        assert!(unwound.is_ok());
    }

    #[test]
    fn drop_with_a_table_of_no_hooks_frees_the_pass() {
        let (pass, [_configured, _checked, dropped]) = Counting::new();
        let instance = PassInstance::new(pass).with_table(PassTable::shortened::<Counting, 0>());

        drop(instance);

        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PassInstance>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PassInstance>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PassInstance>();
    }
}
