//! The spans that mark the phases of a run
//!
//! A run spends its time in a few distinct phases, and a change to stiles
//! usually moves one of them. The pipeline marks each phase of each file
//! with a span, so that a subscriber can sum the spans and say which phase
//! moved. Nothing records the spans unless a subscriber is installed, and
//! then a span costs one atomic load.

/// The target of every span that marks a phase of a run
///
/// The pipeline opens a span of this target for each phase of each file:
/// `parse`, `decorate`, and `walk`. The check command opens spans of the
/// same target for the phases of the whole run. A subscriber that keeps only
/// this target sees where a run spent its time. It sees none of the spans
/// that rust-analyzer and the other libraries of a run open.
///
/// # Examples
///
/// ```
/// let span = tracing::info_span!(target: stiles_core::PHASE_TARGET, "report");
///
/// span.in_scope(|| {});
/// ```
pub const PHASE_TARGET: &str = "stiles::phase";
