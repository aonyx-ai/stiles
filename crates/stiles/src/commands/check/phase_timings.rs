use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use serde::Serialize;
use stiles_core::PHASE_TARGET;
use tracing::span::{Attributes, Id};
use tracing::{Metadata, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::{Context, SubscriberExt as _};
use tracing_subscriber::registry::{LookupSpan, Registry};

/// The time that each phase of a run took, summed over its spans
///
/// `stiles check --timings` installs the recorder before the run starts.
/// From then on, every span of [`PHASE_TARGET`] that closes adds its
/// duration to the phase of its name. The spans of every other target never
/// reach the recorder. The run writes the record once, after its report.
///
/// A span lasts from its creation to its close, not only while a thread is
/// inside it. The pipeline is sequential, so the two only differ for a span
/// that waits, such as the report, which waits on the terminal.
///
/// The recorder is a handle: the clone that the subscriber holds and the
/// clone that writes the record share one list.
#[derive(Clone, Debug, Default)]
pub(super) struct PhaseTimings {
    phases: Arc<Mutex<Vec<PhaseTotal>>>,
}

/// The spans of one phase, summed
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct PhaseTotal {
    name: &'static str,
    spans: u64,
    elapsed: Duration,
}

/// The instant at which a span of a phase was created
///
/// The recorder keeps it in the extensions of the span, so that the close
/// of the span finds it.
#[derive(Copy, Clone, Debug)]
struct Opened(Instant);

/// The record that `--timings` writes
///
/// The phases are in the order in which their first span closed, which is
/// the order in which a run reaches them. The fields are primitives because
/// the record leaves the process.
#[derive(Clone, PartialEq, Debug, Serialize)]
struct Record {
    phases: Vec<PhaseRecord>,
}

/// One phase in the record that `--timings` writes
#[derive(Clone, PartialEq, Debug, Serialize)]
struct PhaseRecord {
    name: &'static str,
    spans: u64,
    seconds: f64,
}

impl PhaseTimings {
    /// Installs a recorder as the subscriber of the whole process
    ///
    /// The recorder is global, not scoped to a thread, because the command
    /// runs on an async runtime that may move it between threads.
    ///
    /// # Errors
    ///
    /// Returns an error if the process already has a global subscriber.
    pub(super) fn install() -> anyhow::Result<Self> {
        let timings = Self::default();

        tracing::subscriber::set_global_default(timings.subscriber())
            .context("failed to install the recorder of the phase timings")?;

        Ok(timings)
    }

    /// Returns a subscriber that reports the spans of the phases to `self`
    fn subscriber(&self) -> impl Subscriber + Send + Sync + use<> {
        let phases = filter_fn(|metadata: &Metadata<'_>| metadata.target() == PHASE_TARGET);

        Registry::default().with(self.clone().with_filter(phases))
    }

    /// Adds the duration of one closed span to the total of its phase
    fn add(&self, name: &'static str, elapsed: Duration) {
        let mut phases = self.phases.lock().unwrap_or_else(PoisonError::into_inner);

        match phases.iter_mut().find(|phase| phase.name == name) {
            Some(phase) => {
                phase.spans += 1;
                phase.elapsed += elapsed;
            }
            None => phases.push(PhaseTotal {
                name,
                spans: 1,
                elapsed,
            }),
        }
    }

    /// Returns the record of the spans that have closed so far
    fn record(&self) -> Record {
        let phases = self.phases.lock().unwrap_or_else(PoisonError::into_inner);

        Record {
            phases: phases
                .iter()
                .map(
                    |&PhaseTotal {
                         name,
                         spans,
                         elapsed,
                     }| PhaseRecord {
                        name,
                        spans,
                        seconds: elapsed.as_secs_f64(),
                    },
                )
                .collect(),
        }
    }

    /// Writes the record to `path` as JSON
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub(super) fn write(&self, path: &Path) -> anyhow::Result<()> {
        let record = serde_json::to_string_pretty(&self.record())
            .context("failed to serialize the phase timings")?;

        std::fs::write(path, record + "\n")
            .with_context(|| format!("failed to write the phase timings to {}", path.display()))
    }
}

impl<S> Layer<S> for PhaseTimings
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, _attributes: &Attributes<'_>, id: &Id, context: Context<'_, S>) {
        let Some(span) = context.span(id) else { return };

        span.extensions_mut().insert(Opened(Instant::now()));
    }

    fn on_close(&self, id: Id, context: Context<'_, S>) {
        let Some(span) = context.span(&id) else {
            return;
        };
        let Some(&Opened(opened)) = span.extensions().get::<Opened>() else {
            return;
        };

        self.add(span.name(), opened.elapsed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `spans` under a fresh recorder and returns what it recorded
    fn recorded(spans: impl FnOnce()) -> Vec<(&'static str, u64)> {
        let timings = PhaseTimings::default();

        tracing::subscriber::with_default(timings.subscriber(), spans);

        timings
            .record()
            .phases
            .into_iter()
            .map(|PhaseRecord { name, spans, .. }| (name, spans))
            .collect()
    }

    #[test]
    fn record_counts_every_span_of_a_phase() {
        let phases = recorded(|| {
            tracing::info_span!(target: PHASE_TARGET, "parse").in_scope(|| {});
            tracing::info_span!(target: PHASE_TARGET, "parse").in_scope(|| {});
        });

        assert_eq!(phases, [("parse", 2)]);
    }

    #[test]
    fn record_ignores_a_span_of_another_target() {
        let phases = recorded(|| {
            tracing::info_span!(target: "ra_ap_hir", "parse").in_scope(|| {});
        });

        assert!(phases.is_empty());
    }

    #[test]
    fn record_keeps_phases_in_the_order_they_first_closed() {
        let phases = recorded(|| {
            tracing::info_span!(target: PHASE_TARGET, "parse").in_scope(|| {});
            tracing::info_span!(target: PHASE_TARGET, "walk").in_scope(|| {});
            tracing::info_span!(target: PHASE_TARGET, "parse").in_scope(|| {});
        });

        assert_eq!(phases, [("parse", 2), ("walk", 1)]);
    }

    #[test]
    fn record_measures_a_span_from_its_creation_to_its_close() {
        let timings = PhaseTimings::default();

        tracing::subscriber::with_default(timings.subscriber(), || {
            let span = tracing::info_span!(target: PHASE_TARGET, "report");
            std::thread::sleep(Duration::from_millis(20));
            drop(span);
        });

        assert!(timings.record().phases[0].seconds >= 0.02);
    }

    #[test]
    fn trait_send_opened() {
        fn assert_send<T: Send>() {}
        assert_send::<Opened>();
    }

    #[test]
    fn trait_send_phase_record() {
        fn assert_send<T: Send>() {}
        assert_send::<PhaseRecord>();
    }

    #[test]
    fn trait_send_phase_timings() {
        fn assert_send<T: Send>() {}
        assert_send::<PhaseTimings>();
    }

    #[test]
    fn trait_send_phase_total() {
        fn assert_send<T: Send>() {}
        assert_send::<PhaseTotal>();
    }

    #[test]
    fn trait_send_record() {
        fn assert_send<T: Send>() {}
        assert_send::<Record>();
    }

    #[test]
    fn trait_sync_opened() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Opened>();
    }

    #[test]
    fn trait_sync_phase_record() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PhaseRecord>();
    }

    #[test]
    fn trait_sync_phase_timings() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PhaseTimings>();
    }

    #[test]
    fn trait_sync_phase_total() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PhaseTotal>();
    }

    #[test]
    fn trait_sync_record() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Record>();
    }

    #[test]
    fn trait_unpin_opened() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Opened>();
    }

    #[test]
    fn trait_unpin_phase_record() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PhaseRecord>();
    }

    #[test]
    fn trait_unpin_phase_timings() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PhaseTimings>();
    }

    #[test]
    fn trait_unpin_phase_total() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PhaseTotal>();
    }

    #[test]
    fn trait_unpin_record() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Record>();
    }
}
