use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::{Diagnostic, Edit, Fix, Location, RuleId, Severity, Sink, Span};

mod collect_diagnostics_error;
mod table;

pub use self::collect_diagnostics_error::CollectDiagnosticsError;

/// The host's record of what a lint pass reports through its [`Sink`]
///
/// The host lends a sink over a collector to a pass, and the pass reports
/// each diagnostic through the sink one part at a time. The collector keeps
/// the parts. [`Collector::finish`] then builds each diagnostic in the
/// host's own layout, with the checks that a fix gets anywhere else.
///
/// The functions behind the sink are `extern "C"`, so none of them may
/// fail or panic. A call that names a diagnostic the sink never started
/// therefore does nothing at the time. The collector keeps the first such
/// fault, and `finish` returns it in place of the diagnostics.
///
/// The walk of a file builds a collector for each call of a pass, on the
/// thread that walks the file. Files that run in parallel therefore never
/// share one. The parts sit behind a mutex all the same. A pass may hand its sink
/// to threads of its own, and the sink must stay sound then too. Nothing
/// panics while it holds the lock, so a poisoned lock still holds whole
/// parts, and the collector reads through the poison.
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
/// assert_eq!(collected[0].rule_id(), RuleId::new("lint.example"));
/// ```
#[derive(Debug, Default)]
pub struct Collector {
    state: Mutex<State>,
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Collector>();
};

/// Everything a collector holds: the started diagnostics and the first fault
#[derive(Debug, Default)]
struct State {
    pending: Vec<Pending>,
    fault: Option<CollectDiagnosticsError>,
}

/// The parts of one diagnostic, as the pass reported them so far
#[derive(Debug)]
struct Pending {
    rule: RuleId,
    severity: Severity,
    message: String,
    span: Span,
    origins: Vec<Location>,
    related: Vec<Location>,
    fix_message: Option<String>,
    edits: Vec<Edit>,
}

impl Collector {
    /// Creates a collector that holds nothing yet
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::Collector;
    ///
    /// let collected = Collector::new().finish().expect("nothing should be reported");
    ///
    /// assert!(collected.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Lends `work` a sink that reports into this collector
    ///
    /// The sink and the table behind it live only as long as the call, so
    /// `work` cannot keep either. A handle the sink hands out names the
    /// same diagnostic across every lend of one collector.
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
    /// collector.lend(|sink| sink.report(&diagnostic));
    ///
    /// let collected = collector.finish().expect("both reports should be whole");
    /// assert_eq!(collected.len(), 2);
    /// ```
    pub fn lend<R>(&self, work: impl FnOnce(&Sink<'_>) -> R) -> R {
        let table = self::table::table(ptr::from_ref(self).cast());

        work(&Sink::new(&table))
    }

    /// Builds each diagnostic the pass reported, in the order it started them
    ///
    /// # Errors
    ///
    /// Returns [`CollectDiagnosticsError::UnknownHandle`] if the pass named
    /// a diagnostic its sink never started, and
    /// [`CollectDiagnosticsError::AttachFix`] if it named a fix twice.
    /// Returns [`CollectDiagnosticsError::UnnamedFix`] for edits without a
    /// fix message. Returns [`CollectDiagnosticsError::BuildFix`] or
    /// [`CollectDiagnosticsError::AttachFix`] for a fix that
    /// [`Fix::builder`] or [`Diagnostic::with_fix`] refuses.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::Collector;
    ///
    /// let collected = Collector::new().finish().expect("nothing should be reported");
    ///
    /// assert!(collected.is_empty());
    /// ```
    pub fn finish(self) -> Result<Vec<Diagnostic>, CollectDiagnosticsError> {
        let State { pending, fault } = self
            .state
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(fault) = fault {
            return Err(fault);
        }

        pending.into_iter().map(Pending::finish).collect()
    }

    /// Returns the parts this collector holds, through any poison
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    /// Starts a diagnostic from `pending`, and returns its handle
    fn start(&mut self, pending: Pending) -> usize {
        self.pending.push(pending);
        self.pending.len() - 1
    }

    /// Returns the diagnostic behind `handle`, or keeps the fault if none is
    fn at(&mut self, handle: usize) -> Option<&mut Pending> {
        let started = self.pending.len();
        if handle >= started {
            self.keep(CollectDiagnosticsError::UnknownHandle { handle, started });
            return None;
        }

        self.pending.get_mut(handle)
    }

    /// Keeps `fault`, unless the collector already holds an earlier one
    fn keep(&mut self, fault: CollectDiagnosticsError) {
        if self.fault.is_none() {
            self.fault = Some(fault);
        }
    }
}

impl Pending {
    /// Builds the diagnostic, with its fix when the pass named one
    fn finish(self) -> Result<Diagnostic, CollectDiagnosticsError> {
        let Self {
            rule,
            severity,
            message,
            span,
            origins,
            related,
            fix_message,
            edits,
        } = self;
        let diagnostic = Diagnostic::builder()
            .rule_id(rule)
            .severity(severity)
            .message(message)
            .span(span)
            .build();
        let diagnostic = origins
            .into_iter()
            .fold(diagnostic, Diagnostic::with_origin);
        let diagnostic = related
            .into_iter()
            .fold(diagnostic, Diagnostic::with_related);

        let Some(fix_message) = fix_message else {
            return match edits.is_empty() {
                true => Ok(diagnostic),
                false => Err(CollectDiagnosticsError::UnnamedFix { rule }),
            };
        };
        let fix = Fix::builder()
            .message(fix_message)
            .edits(edits)
            .build()
            .map_err(|source| CollectDiagnosticsError::BuildFix { rule, source })?;

        diagnostic
            .with_fix(fix)
            .map_err(|source| CollectDiagnosticsError::AttachFix { rule, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_collector() {
        fn assert_send<T: Send>() {}
        assert_send::<Collector>();
    }

    #[test]
    fn trait_send_pending() {
        fn assert_send<T: Send>() {}
        assert_send::<Pending>();
    }

    #[test]
    fn trait_send_state() {
        fn assert_send<T: Send>() {}
        assert_send::<State>();
    }

    #[test]
    fn trait_sync_collector() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Collector>();
    }

    #[test]
    fn trait_sync_pending() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Pending>();
    }

    #[test]
    fn trait_sync_state() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<State>();
    }

    #[test]
    fn trait_unpin_collector() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Collector>();
    }

    #[test]
    fn trait_unpin_pending() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Pending>();
    }

    #[test]
    fn trait_unpin_state() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<State>();
    }
}
