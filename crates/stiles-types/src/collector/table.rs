//! The host's functions behind every [`Sink`]
//!
//! Each function here is an entry of the [`SinkTable`] that
//! [`Collector::lend`] builds. The code that runs is therefore the code of
//! the image that collects. A function takes the table and follows its
//! `data` to the collector. That step rests on one invariant. Only the
//! host writes a table, and only for a collector that outlives it.
//!
//! A function here must not panic. It is `extern "C"`, so a panic aborts
//! the process instead of reaching the pass that called it. A handle that
//! names no diagnostic is therefore kept as a fault, and the call does
//! nothing else.
//!
//! [`Collector::lend`]: crate::Collector::lend
//! [`Sink`]: crate::Sink

use std::ffi::c_void;
use std::sync::MutexGuard;

use stabby::str::Str;

use super::{Collector, Pending, State};
use crate::sink_table::SinkTable;
use crate::{AttachFixError, CollectDiagnosticsError, Edit, Location, RuleId, Severity, Span};

/// Returns the table whose functions collect into the collector at `data`
pub(super) fn table(data: *const c_void) -> SinkTable {
    SinkTable {
        length: SinkTable::LENGTH,
        data,
        report,
        origin,
        related,
        fix_message,
        edit,
    }
}

/// Returns the parts that the collector of `table` holds
fn state(table: &SinkTable) -> MutexGuard<'_, State> {
    // SAFETY: only `Collector::lend` writes `data`, as the address of the
    // collector it borrows, and the table lives inside that call. The
    // collector therefore outlives the borrow of `table`.
    let collector = unsafe { &*table.data.cast::<Collector>() };

    collector.state()
}

extern "C" fn report(
    table: &SinkTable,
    rule: &RuleId,
    severity: u8,
    message: Str<'_>,
    span: &Span,
) -> usize {
    let mut state = state(table);
    let severity = match Severity::from_byte(severity) {
        Some(severity) => severity,
        None => {
            state.keep(CollectDiagnosticsError::UnknownSeverity {
                rule: *rule,
                byte: severity,
            });
            Severity::Error
        }
    };

    state.start(Pending {
        rule: *rule,
        severity,
        message: message.as_str().to_owned(),
        span: span.clone(),
        origins: Vec::new(),
        related: Vec::new(),
        fix_message: None,
        edits: Vec::new(),
    })
}

extern "C" fn origin(table: &SinkTable, handle: usize, span: &Span, message: Str<'_>) {
    let mut state = state(table);
    let Some(pending) = state.at(handle) else {
        return;
    };

    pending.origins.push(location(span, message));
}

extern "C" fn related(table: &SinkTable, handle: usize, span: &Span, message: Str<'_>) {
    let mut state = state(table);
    let Some(pending) = state.at(handle) else {
        return;
    };

    pending.related.push(location(span, message));
}

/// Names the fix of a diagnostic, which starts it
///
/// A diagnostic holds one fix, so a second name is a fault, as a second
/// [`Diagnostic::with_fix`] is.
///
/// [`Diagnostic::with_fix`]: crate::Diagnostic::with_fix
extern "C" fn fix_message(table: &SinkTable, handle: usize, message: Str<'_>) {
    let mut state = state(table);
    let Some(pending) = state.at(handle) else {
        return;
    };

    match pending.fix_message {
        None => pending.fix_message = Some(message.as_str().to_owned()),
        Some(_) => {
            let rule = pending.rule;
            state.keep(CollectDiagnosticsError::AttachFix {
                rule,
                source: AttachFixError::SecondFix,
            });
        }
    }
}

extern "C" fn edit(table: &SinkTable, handle: usize, span: &Span, replacement: Str<'_>) {
    let mut state = state(table);
    let Some(pending) = state.at(handle) else {
        return;
    };

    pending.edits.push(
        Edit::builder()
            .span(span.clone())
            .replacement(replacement.as_str())
            .build(),
    );
}

/// Returns the location of `span` that `message` describes
fn location(span: &Span, message: Str<'_>) -> Location {
    Location::builder()
        .span(span.clone())
        .message(message.as_str())
        .build()
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::ptr;

    use super::*;
    use crate::{BuildFixError, Diagnostic, Fix};

    const RULE: RuleId = RuleId::new("test.rule");

    fn span(file: &str, bytes: Range<usize>) -> Span {
        let Range { start, end } = bytes;

        Span::builder().file(file).start(start).end(end).build()
    }

    /// Runs `work` against the table of a fresh collector, and finishes it
    fn collect(work: impl FnOnce(&SinkTable)) -> Result<Vec<Diagnostic>, CollectDiagnosticsError> {
        let collector = Collector::new();

        collector.lend(|sink| work(sink.table()));

        collector.finish()
    }

    /// Reports one diagnostic on bytes 0 to 4 of `test.rs`, and returns its handle
    fn start(table: &SinkTable) -> usize {
        report(
            table,
            &RULE,
            Severity::Warn.to_byte(),
            Str::new("found"),
            &span("test.rs", 0..4),
        )
    }

    /// Returns the fix of the only diagnostic that `work` reports
    fn fix_of(work: impl FnOnce(&SinkTable)) -> Fix {
        let collected = collect(work).expect("the report should be whole");

        collected[0]
            .fix()
            .expect("the diagnostic should hold a fix")
            .clone()
    }

    #[test]
    fn edit_adds_an_edit_to_the_fix() {
        let fix = fix_of(|table| {
            let handle = start(table);
            fix_message(table, handle, Str::new("rename"));
            edit(table, handle, &span("test.rs", 1..2), Str::new("x"));
        });

        assert_eq!(
            fix.edits(),
            [Edit::builder()
                .span(span("test.rs", 1..2))
                .replacement("x")
                .build()]
        );
    }

    #[test]
    fn edit_in_another_file_refuses_the_fix() {
        let error = collect(|table| {
            let handle = start(table);
            fix_message(table, handle, Str::new("rename"));
            edit(table, handle, &span("other.rs", 1..2), Str::new("x"));
        })
        .expect_err("a fix must edit the file of its diagnostic");

        assert_eq!(
            error,
            CollectDiagnosticsError::AttachFix {
                rule: RULE,
                source: AttachFixError::OtherFile {
                    diagnostic: "test.rs".into(),
                    fix: "other.rs".into(),
                },
            }
        );
    }

    #[test]
    fn edit_with_an_unknown_handle_keeps_the_fault() {
        let error = collect(|table| {
            edit(table, 0, &span("test.rs", 1..2), Str::new("x"));
        })
        .expect_err("no diagnostic was started");

        assert_eq!(
            error,
            CollectDiagnosticsError::UnknownHandle {
                handle: 0,
                started: 0,
            }
        );
    }

    #[test]
    fn edit_without_a_fix_message_refuses_the_diagnostic() {
        let error = collect(|table| {
            let handle = start(table);
            edit(table, handle, &span("test.rs", 1..2), Str::new("x"));
        })
        .expect_err("a fix needs a message");

        assert_eq!(error, CollectDiagnosticsError::UnnamedFix { rule: RULE });
    }

    #[test]
    fn edits_in_two_files_refuse_the_fix() {
        let error = collect(|table| {
            let handle = start(table);
            fix_message(table, handle, Str::new("rename"));
            edit(table, handle, &span("test.rs", 1..2), Str::new("x"));
            edit(table, handle, &span("other.rs", 3..4), Str::new("y"));
        })
        .expect_err("a fix edits one file");

        assert_eq!(
            error,
            CollectDiagnosticsError::BuildFix {
                rule: RULE,
                source: BuildFixError::SeveralFiles {
                    first: "test.rs".into(),
                    other: "other.rs".into(),
                },
            }
        );
    }

    #[test]
    fn edits_that_overlap_refuse_the_fix() {
        let error = collect(|table| {
            let handle = start(table);
            fix_message(table, handle, Str::new("rename"));
            edit(table, handle, &span("test.rs", 0..3), Str::new("x"));
            edit(table, handle, &span("test.rs", 2..4), Str::new("y"));
        })
        .expect_err("the edits of a fix must not overlap");

        assert_eq!(
            error,
            CollectDiagnosticsError::BuildFix {
                rule: RULE,
                source: BuildFixError::OverlappingEdits {
                    file: "test.rs".into(),
                    first: 0..3,
                    second: 2..4,
                },
            }
        );
    }

    #[test]
    fn fix_message_names_the_fix() {
        let fix = fix_of(|table| {
            let handle = start(table);
            fix_message(table, handle, Str::new("rename"));
            edit(table, handle, &span("test.rs", 1..2), Str::new("x"));
        });

        assert_eq!(fix.message(), "rename");
    }

    #[test]
    fn fix_message_twice_refuses_the_diagnostic() {
        let error = collect(|table| {
            let handle = start(table);
            fix_message(table, handle, Str::new("rename"));
            fix_message(table, handle, Str::new("rename again"));
            edit(table, handle, &span("test.rs", 1..2), Str::new("x"));
        })
        .expect_err("a diagnostic holds one fix");

        assert_eq!(
            error,
            CollectDiagnosticsError::AttachFix {
                rule: RULE,
                source: AttachFixError::SecondFix,
            }
        );
    }

    #[test]
    fn fix_message_with_an_unknown_handle_keeps_the_fault() {
        let error = collect(|table| {
            start(table);
            fix_message(table, 1, Str::new("rename"));
        })
        .expect_err("only diagnostic 0 was started");

        assert_eq!(
            error,
            CollectDiagnosticsError::UnknownHandle {
                handle: 1,
                started: 1,
            }
        );
    }

    #[test]
    fn fix_message_without_edits_refuses_the_fix() {
        let error = collect(|table| {
            let handle = start(table);
            fix_message(table, handle, Str::new("rename"));
        })
        .expect_err("a fix needs an edit");

        assert_eq!(
            error,
            CollectDiagnosticsError::BuildFix {
                rule: RULE,
                source: BuildFixError::NoEdits,
            }
        );
    }

    #[test]
    fn origin_adds_an_origin_in_another_file() {
        let collected = collect(|table| {
            let handle = start(table);
            origin(table, handle, &span("other.rs", 5..9), Str::new("set here"));
        })
        .expect("an origin may sit in another file");

        assert_eq!(
            collected[0].origins(),
            [Location::builder()
                .span(span("other.rs", 5..9))
                .message("set here")
                .build()]
        );
    }

    #[test]
    fn origin_with_an_unknown_handle_keeps_the_fault() {
        let error = collect(|table| {
            origin(
                table,
                usize::MAX,
                &span("test.rs", 0..1),
                Str::new("set here"),
            );
        })
        .expect_err("no diagnostic was started");

        assert_eq!(
            error,
            CollectDiagnosticsError::UnknownHandle {
                handle: usize::MAX,
                started: 0,
            }
        );
    }

    #[test]
    fn related_adds_a_related_location_in_another_file() {
        let collected = collect(|table| {
            let handle = start(table);
            related(
                table,
                handle,
                &span("other.rs", 5..9),
                Str::new("used here"),
            );
        })
        .expect("a related location may sit in another file");

        assert_eq!(
            collected[0].related(),
            [Location::builder()
                .span(span("other.rs", 5..9))
                .message("used here")
                .build()]
        );
    }

    #[test]
    fn related_with_an_unknown_handle_keeps_the_fault() {
        let error = collect(|table| {
            related(table, 3, &span("test.rs", 0..1), Str::new("used here"));
        })
        .expect_err("no diagnostic was started");

        assert_eq!(
            error,
            CollectDiagnosticsError::UnknownHandle {
                handle: 3,
                started: 0,
            }
        );
    }

    #[test]
    fn report_hands_out_handles_in_order() {
        let collector = Collector::new();

        let handles = collector.lend(|sink| (start(sink.table()), start(sink.table())));

        assert_eq!(handles, (0, 1));
    }

    #[test]
    fn report_keeps_the_message() {
        let collected = collect(|table| {
            start(table);
        })
        .expect("the report should be whole");

        assert_eq!(collected[0].message(), "found");
    }

    #[test]
    fn report_keeps_the_rule() {
        let collected = collect(|table| {
            start(table);
        })
        .expect("the report should be whole");

        assert_eq!(collected[0].rule_id(), RULE);
    }

    #[test]
    fn report_keeps_the_severity() {
        let collected = collect(|table| {
            start(table);
        })
        .expect("the report should be whole");

        assert_eq!(collected[0].severity(), Severity::Warn);
    }

    #[test]
    fn report_keeps_the_span() {
        let collected = collect(|table| {
            start(table);
        })
        .expect("the report should be whole");

        assert_eq!(collected[0].span(), &span("test.rs", 0..4));
    }

    /// A plugin built against a newer stiles may send a severity that this
    /// one lacks, and the host refuses it rather than guess its weight
    #[test]
    fn report_with_an_unknown_severity_byte_keeps_the_fault() {
        let error = collect(|table| {
            report(
                table,
                &RULE,
                u8::MAX,
                Str::new("found"),
                &span("test.rs", 0..4),
            );
        })
        .expect_err("the severity is unknown");

        assert_eq!(
            error,
            CollectDiagnosticsError::UnknownSeverity {
                rule: RULE,
                byte: u8::MAX,
            }
        );
    }

    #[test]
    fn report_without_a_fix_holds_none() {
        let collected = collect(|table| {
            start(table);
        })
        .expect("the report should be whole");

        assert!(collected[0].fix().is_none());
    }

    #[test]
    fn table_counts_its_function_entries() {
        let table = table(ptr::null());

        assert_eq!(table.length, SinkTable::LENGTH);
    }

    #[test]
    fn unknown_handles_keep_the_first_fault() {
        let error = collect(|table| {
            let handle = start(table);
            origin(table, 7, &span("test.rs", 0..1), Str::new("set here"));
            fix_message(table, handle, Str::new("rename"));
            fix_message(table, handle, Str::new("rename again"));
        })
        .expect_err("the stray handle should end the collection");

        assert_eq!(
            error,
            CollectDiagnosticsError::UnknownHandle {
                handle: 7,
                started: 1,
            }
        );
    }
}
