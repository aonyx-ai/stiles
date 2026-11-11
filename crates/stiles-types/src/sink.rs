use stabby::abi::StableLike;
use stabby::str::Str;

use crate::Diagnostic;
use crate::sink_table::SinkTable;

/// Where a lint pass reports what it found
///
/// The host lends a sink to each call of [`LintPass::check_node`], and the
/// pass reports every diagnostic through it. A sink is a reference to a
/// table of the host's functions. [`Sink::report`] hands each part of a
/// diagnostic to one of them, and the host builds its own [`Diagnostic`]
/// from the calls. A diagnostic therefore never crosses the boundary as a
/// laid-out value, so a field added to it does not move the boundary.
///
/// A rule never sees a sink. Its `check_` methods return diagnostics, and
/// the adapter that [`lint_pass_adapter!`] writes reports each one here.
///
/// A sink crosses the plugin boundary with every check, so stabby lays it
/// out. Stabby cannot see into the table, so [`StableLike`] carries the
/// reference to it with the layout of any other reference. A sink borrows
/// the host's [`Collector`], so it cannot outlive the call it was lent to.
///
/// # Examples
///
/// ```
/// use stiles_types::{Collector, Diagnostic, RuleId, Severity, Span};
///
/// let diagnostic = Diagnostic::builder()
///     .rule_id(RuleId::new("lint.example"))
///     .severity(Severity::Warn)
///     .message("a wildcard arm")
///     .span(Span::builder().file("src/lib.rs").start(3).end(7).build())
///     .build();
/// let collector = Collector::new();
///
/// collector.lend(|sink| sink.report(&diagnostic));
///
/// let collected = collector.finish().expect("the report should be whole");
/// assert_eq!(collected[0].message(), "a wildcard arm");
/// ```
///
/// [`Collector`]: crate::Collector
/// [`LintPass::check_node`]: crate::LintPass::check_node
/// [`lint_pass_adapter!`]: crate::lint_pass_adapter
/// [`StableLike`]: stabby::abi::StableLike
#[stabby::stabby]
#[derive(Copy, Clone)]
pub struct Sink<'a> {
    table: StableLike<&'a SinkTable, &'a ()>,
}

impl<'a> Sink<'a> {
    /// Creates the sink that reports through `table`
    ///
    /// Only the host calls this, with a table whose `data` is a live
    /// collector.
    pub(crate) fn new(table: &'a SinkTable) -> Self {
        Self {
            table: StableLike::new(table),
        }
    }

    /// Returns the host's table behind this sink
    pub(crate) fn table(&self) -> &'a SinkTable {
        // SAFETY: a reference is a non-null pointer under any compiler, and
        // `StableLike::new` checked that it has the size and the alignment
        // of `&()`, whose layout stabby records in its place.
        unsafe { self.table.into_inner_unchecked() }
    }

    /// Reports `diagnostic` to the host, one part at a time
    ///
    /// The diagnostic itself stays with the caller. The host copies its
    /// rule, severity, message, span, origins, related locations, and fix
    /// into a diagnostic of its own. The host checks what it collected
    /// only after the pass returns, so a call here never fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{Collector, Diagnostic, RuleId, Severity, Span};
    ///
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(Span::builder().file("src/lib.rs").start(3).end(7).build())
    ///     .build();
    /// let collector = Collector::new();
    ///
    /// collector.lend(|sink| sink.report(&diagnostic));
    ///
    /// let collected = collector.finish().expect("the report should be whole");
    /// assert_eq!(collected.len(), 1);
    /// ```
    pub fn report(&self, diagnostic: &Diagnostic) {
        let table = self.table();
        let handle = (table.report)(
            table,
            &diagnostic.rule_id(),
            diagnostic.severity(),
            Str::new(diagnostic.message()),
            diagnostic.span(),
        );

        for origin in diagnostic.origins() {
            (table.origin)(table, handle, origin.span(), Str::new(origin.message()));
        }
        for related in diagnostic.related() {
            (table.related)(table, handle, related.span(), Str::new(related.message()));
        }
        let Some(fix) = diagnostic.fix() else {
            return;
        };
        (table.fix_message)(table, handle, Str::new(fix.message()));
        for edit in fix.edits() {
            (table.edit)(table, handle, edit.span(), Str::new(edit.replacement()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::*;
    use crate::{Collector, Edit, Fix, Location, RuleId, Severity, Span};

    fn span(file: &str, bytes: Range<usize>) -> Span {
        let Range { start, end } = bytes;

        Span::builder().file(file).start(start).end(end).build()
    }

    /// Returns a diagnostic on `test.rs` with an origin, a related location, and a fix
    fn whole() -> Diagnostic {
        let fix = Fix::builder()
            .message("rename")
            .edits(vec![
                Edit::builder()
                    .span(span("test.rs", 1..2))
                    .replacement("x")
                    .build(),
                Edit::builder()
                    .span(span("test.rs", 3..4))
                    .replacement("y")
                    .build(),
            ])
            .build()
            .expect("the edits should form a fix");

        Diagnostic::builder()
            .rule_id(RuleId::new("test.rule"))
            .severity(Severity::Error)
            .message("found")
            .span(span("test.rs", 0..4))
            .build()
            .with_origin(
                Location::builder()
                    .span(span("other.rs", 5..9))
                    .message("set here")
                    .build(),
            )
            .with_related(
                Location::builder()
                    .span(span("third.rs", 1..3))
                    .message("used here")
                    .build(),
            )
            .with_fix(fix)
            .expect("the fix should edit the file of the diagnostic")
    }

    /// Reports `diagnostic` through a sink, and returns what the host built
    fn round_trip(diagnostic: &Diagnostic) -> Diagnostic {
        let collector = Collector::new();
        collector.lend(|sink| sink.report(diagnostic));

        let mut collected = collector.finish().expect("the report should be whole");

        collected.remove(0)
    }

    #[test]
    fn report_carries_the_fix() {
        let diagnostic = whole();

        let collected = round_trip(&diagnostic);

        assert_eq!(collected.fix(), diagnostic.fix());
    }

    #[test]
    fn report_carries_the_message() {
        let diagnostic = whole();

        let collected = round_trip(&diagnostic);

        assert_eq!(collected.message(), diagnostic.message());
    }

    #[test]
    fn report_carries_the_origins() {
        let diagnostic = whole();

        let collected = round_trip(&diagnostic);

        assert_eq!(collected.origins(), diagnostic.origins());
    }

    #[test]
    fn report_carries_the_related_locations() {
        let diagnostic = whole();

        let collected = round_trip(&diagnostic);

        assert_eq!(collected.related(), diagnostic.related());
    }

    #[test]
    fn report_carries_the_rule() {
        let diagnostic = whole();

        let collected = round_trip(&diagnostic);

        assert_eq!(collected.rule_id(), diagnostic.rule_id());
    }

    #[test]
    fn report_carries_the_severity() {
        let diagnostic = whole();

        let collected = round_trip(&diagnostic);

        assert_eq!(collected.severity(), diagnostic.severity());
    }

    #[test]
    fn report_carries_the_span() {
        let diagnostic = whole();

        let collected = round_trip(&diagnostic);

        assert_eq!(collected.span(), diagnostic.span());
    }

    #[test]
    fn report_of_two_diagnostics_keeps_their_order() {
        let first = whole();
        let second = whole().with_severity(Severity::Help);
        let collector = Collector::new();
        collector.lend(|sink| {
            sink.report(&first);
            sink.report(&second);
        });

        let collected = collector.finish().expect("both reports should be whole");

        let severities: Vec<Severity> = collected.iter().map(Diagnostic::severity).collect();
        assert_eq!(severities, [Severity::Error, Severity::Help]);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Sink<'static>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Sink<'static>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Sink<'static>>();
    }
}
