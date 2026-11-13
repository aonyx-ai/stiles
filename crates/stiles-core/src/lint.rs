use bon::bon;
use stiles_types::{
    Checked, Collector, ConfineReadsError, DecoratedNode, DecoratedTree, Diagnostic, Language,
    LintPass, Panic, ReadKeys, RuleId,
};

use crate::{PassPanic, UndeclaredRead, UndeclaredRule, WalkLintsError};

/// A lint pass, and the rules and decorations its plugin declared for it
///
/// A plugin declares the rule ids each of its passes reports, and stiles
/// knows them before it walks a file. That is what lets a project turn a
/// rule off, hand it options, or list it. A pass that reports an id it did
/// not declare would make each of those wrong without a trace. The walk
/// therefore refuses such a diagnostic with an [`UndeclaredRule`] error.
///
/// A plugin also declares the decorations each pass reads, and a provider
/// computes only those. A pass that reads another would find nothing, and
/// its rule would report nothing. The walk confines each call of the pass
/// to the declared keys, and refuses any other lookup with an
/// [`UndeclaredRead`] error.
///
/// # Examples
///
/// ```ignore
/// let lint = Lint::builder()
///     .pass(pass)
///     .rules(vec![RuleId::new("lint.todo-comment")])
///     .reads(ReadKeys::from_iter([]))
///     .build();
///
/// assert_eq!(lint.rules(), [RuleId::new("lint.todo-comment")]);
/// ```
pub struct Lint<'pass> {
    pass: Box<dyn LintPass + 'pass>,
    declared: Declared,
}

/// What a [`Lint`] knows about the rules its pass reports and the keys it reads
enum Declared {
    /// The plugin declared these, and the walk holds the pass to them
    Plugin { rules: Vec<RuleId>, reads: ReadKeys },
    /// Nothing declared them, so the walk accepts any rule and any read
    Unchecked,
}

#[bon]
impl<'pass> Lint<'pass> {
    /// Pairs `pass` with the rules and the reads its plugin declared for it
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let lint = Lint::builder()
    ///     .pass(pass)
    ///     .rules(vec![RuleId::new("lint.todo-comment")])
    ///     .reads(ReadKeys::from_iter([]))
    ///     .build();
    /// ```
    #[builder]
    pub fn new(pass: Box<dyn LintPass + 'pass>, rules: Vec<RuleId>, reads: ReadKeys) -> Self {
        Self {
            pass,
            declared: Declared::Plugin { rules, reads },
        }
    }

    /// Wraps a pass that no plugin declared, and accepts any rule and read
    ///
    /// A test that runs a rule's pass directly has the pass and nothing
    /// that declares its rules or its reads. The pass may be borrowed,
    /// which is what the `'pass` lifetime allows. Stiles itself never
    /// builds one of these, because every pass it runs came from a plugin.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let lint = Lint::unchecked(pass);
    ///
    /// assert!(lint.rules().is_empty());
    /// ```
    pub fn unchecked(pass: Box<dyn LintPass + 'pass>) -> Self {
        Self {
            pass,
            declared: Declared::Unchecked,
        }
    }

    /// Returns the language whose trees the pass inspects
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(lint.language(), Language::Rust);
    /// ```
    pub fn language(&self) -> Language {
        self.pass.language()
    }

    /// Returns the rules the pass may report
    ///
    /// An [unchecked] lint declares none, and reports any.
    ///
    /// [unchecked]: Lint::unchecked
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(lint.rules(), [RuleId::new("lint.todo-comment")]);
    /// ```
    pub fn rules(&self) -> &[RuleId] {
        match &self.declared {
            Declared::Plugin { rules, .. } => rules,
            Declared::Unchecked => &[],
        }
    }

    /// Checks one node of `tree`, and holds the pass to what it declared
    ///
    /// The pass reports through a sink over a [`Collector`] of this call's
    /// own. A collector therefore never outlives one node of one file, and
    /// files that walk in parallel never share one. What a pass reported
    /// before it panicked is dropped with the collector.
    ///
    /// The call runs inside [`DecoratedTree::confine`], with the keys the
    /// plugin declared. A walk visits the nodes of one file on one thread,
    /// so the confinement covers this pass alone.
    ///
    /// # Errors
    ///
    /// Returns [`WalkLintsError::UndeclaredRead`] if the pass read a
    /// decoration it did not declare. That comes first, because the pass
    /// then ran on a lookup that found nothing. Returns
    /// [`WalkLintsError::Panicked`] if the pass panicked, and
    /// [`WalkLintsError::Reported`] if the host cannot build what it
    /// reported. Returns [`WalkLintsError::Undeclared`] if it reported a
    /// rule it did not declare.
    pub(crate) fn check_node(
        &mut self,
        tree: &DecoratedTree,
        node: &DecoratedNode<'_>,
    ) -> Result<Vec<Diagnostic>, WalkLintsError> {
        let collector = Collector::new();
        let checked = self.check_confined(tree, node, &collector)?;
        let checked: Result<(), Panic> = checked.into();
        checked.map_err(|panic| WalkLintsError::Panicked {
            source: PassPanic::builder()
                .kind(node.kind())
                .span(node.span())
                .panic(panic)
                .build(),
        })?;
        let found = collector
            .finish()
            .map_err(|source| WalkLintsError::Reported {
                span: node.span(),
                source,
            })?;

        let rules = match &self.declared {
            Declared::Plugin { rules, .. } => rules,
            Declared::Unchecked => return Ok(found),
        };
        if let Some(stray) = found
            .iter()
            .find(|diagnostic| !rules.contains(&diagnostic.rule_id()))
        {
            return Err(WalkLintsError::Undeclared {
                source: UndeclaredRule::builder()
                    .rule(stray.rule_id())
                    .declared(rules.clone())
                    .span(node.span())
                    .build(),
            });
        }

        Ok(found)
    }

    /// Runs the pass on `node`, confined to the keys its plugin declared
    ///
    /// # Errors
    ///
    /// Returns [`WalkLintsError::UndeclaredRead`] if the pass read a
    /// decoration it did not declare.
    fn check_confined(
        &mut self,
        tree: &DecoratedTree,
        node: &DecoratedNode<'_>,
        collector: &Collector,
    ) -> Result<Checked, WalkLintsError> {
        let pass = &mut self.pass;
        let (rules, reads) = match &self.declared {
            Declared::Plugin { rules, reads } => (rules, reads),
            Declared::Unchecked => return Ok(collector.lend(|sink| pass.check_node(node, sink))),
        };

        tree.confine(reads, || collector.lend(|sink| pass.check_node(node, sink)))
            .map_err(
                |ConfineReadsError::Undeclared { key }| WalkLintsError::UndeclaredRead {
                    source: UndeclaredRead::builder()
                        .rules(rules.clone())
                        .key(key)
                        .span(node.span())
                        .build(),
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_declared() {
        fn assert_send<T: Send>() {}
        assert_send::<Declared>();
    }

    #[test]
    fn trait_send_lint() {
        fn assert_send<T: Send>() {}
        assert_send::<Lint<'static>>();
    }

    #[test]
    fn trait_sync_declared() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Declared>();
    }

    #[test]
    fn trait_sync_lint() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Lint<'static>>();
    }

    #[test]
    fn trait_unpin_declared() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Declared>();
    }

    #[test]
    fn trait_unpin_lint() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Lint<'static>>();
    }
}
