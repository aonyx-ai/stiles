/// Defines the adapter that bridges a language's lint pass trait to [`LintPass`]
///
/// Every language crate generates a trait with one method per node kind,
/// such as `RustLintPass`. It also generates a `dispatch` function that
/// routes a node to its method. The adapter around it is the same for every
/// language except for three names, so a language crate writes it with this
/// macro:
///
/// ```ignore
/// stiles_types::lint_pass_adapter!(RustLintPassAdapter, RustLintPass, crate::dispatch, Rust);
/// ```
///
/// The arguments are the adapter's name, the generated trait, the generated
/// `dispatch` function, and the [`Language`] variant the adapter names. The
/// adapter answers [`LintPass::language`] with that variant, so the walker
/// hands it only trees in that language.
///
/// The adapter is also the plugin's edge. A rule may panic, and a panic must
/// not unwind into the host. Each call therefore runs under
/// [`Panic::catch`] and comes back as a value. `instance` pairs an adapter
/// with the table of `extern "C"` entries that run it, which is what
/// crosses the boundary. A rule keeps writing plain
/// Rust and returning std's `Vec` of diagnostics. The adapter reports each
/// one through the [`Sink`] that the host lent the check, so no diagnostic
/// crosses the boundary as a laid-out value. A rule that panics reports
/// nothing for that node.
///
/// [`Language`]: crate::Language
/// [`LintPass`]: crate::LintPass
/// [`LintPass::language`]: crate::LintPass::language
/// [`Panic::catch`]: crate::Panic::catch
/// [`Sink`]: crate::Sink
#[macro_export]
macro_rules! lint_pass_adapter {
    ($adapter:ident, $pass:ident, $dispatch:path, $language:ident) => {
        #[doc = concat!(
            "Bridges a [`", stringify!($pass), "`] into the platform's [`LintPass`] trait"
        )]
        #[doc = ""]
        #[doc = concat!(
            "The adapter names [`Language::", stringify!($language), "`] as its language, so ",
            "the walker hands it only trees in that language. It routes each node to the ",
            "method for its kind, and returns a panic as a value rather than unwinding into ",
            "the host. [`lint_pass_adapter!`] defines it."
        )]
        #[doc = ""]
        #[doc = "# Examples"]
        #[doc = ""]
        #[doc = "```ignore"]
        #[doc = concat!("struct Quiet;")]
        #[doc = concat!("impl ", stringify!($pass), " for Quiet {}")]
        #[doc = ""]
        #[doc = concat!(
            "let pass: Box<dyn LintPass> = Box::new(", stringify!($adapter), "::new(Quiet));"
        )]
        #[doc = "```"]
        #[doc = ""]
        #[doc = concat!("[`Language::", stringify!($language), "`]: stiles_types::Language")]
        #[doc = "[`LintPass`]: stiles_types::LintPass"]
        #[doc = "[`lint_pass_adapter!`]: stiles_types::lint_pass_adapter"]
        pub struct $adapter<P: $pass> {
            inner: P,
        }

        impl<P: $pass> $adapter<P> {
            #[doc = concat!("Wraps a `", stringify!($pass), "` implementation for the pipeline")]
            #[doc = ""]
            #[doc = "# Examples"]
            #[doc = ""]
            #[doc = "```ignore"]
            #[doc = concat!("let pass = ", stringify!($adapter), "::new(Quiet);")]
            #[doc = "```"]
            pub fn new(pass: P) -> Self {
                Self { inner: pass }
            }

            /// Wraps a rule as a pass instance for the plugin boundary
            ///
            /// A factory that stiles-plugin's `export_lints!` writes hands this
            /// to stiles. The instance holds the adapter and the table of
            /// `extern "C"` entries that run it, both in the plugin's image.
            /// The host therefore calls the rule without knowing its type.
            ///
            /// # Examples
            ///
            /// ```ignore
            #[doc = concat!("let instance = ", stringify!($adapter), "::instance(Quiet);")]
            /// ```
            pub fn instance(pass: P) -> $crate::plugin::PassInstance
            where
                P: 'static,
            {
                $crate::plugin::PassInstance::new(Self::new(pass))
            }
        }

        impl<P: $pass> $crate::LintPass for $adapter<P> {
            extern "C" fn language(&self) -> $crate::Language {
                $crate::Language::$language
            }

            extern "C" fn configure(&mut self, options: &$crate::RuleOptions) -> $crate::Configured {
                $crate::Panic::catch(|| self.inner.configure(options)).into()
            }

            extern "C" fn check_node(
                &mut self,
                node: &$crate::DecoratedNode<'_>,
                sink: &$crate::Sink<'_>,
            ) -> $crate::Checked {
                $crate::Panic::catch(|| $dispatch(&mut self.inner, node))
                    .map(|found| found.iter().for_each(|diagnostic| sink.report(diagnostic)))
                    .into()
            }
        }
    };
}
