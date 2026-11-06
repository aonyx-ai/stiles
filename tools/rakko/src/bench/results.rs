use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::HarnessError;

/// The number of phases that the summary of a project names
const SUMMARY_PHASES: usize = 3;

/// The number of nanoseconds in a second
const NANOSECONDS: f64 = 1e9;

/// What the benchmarks recorded on one project of the corpus
///
/// The phases come from one run with `--timings`, after a warm-up. The wall
/// time of a run comes from CodSpeed, which times the projects itself.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct Measurement {
    /// The name of the project
    project: &'static str,
    /// The time of each phase of the run with `--timings`, in its order
    phases: Vec<Phase>,
}

/// The time of one phase, as `stiles check --timings` writes it
#[derive(Clone, PartialEq, Debug, Deserialize)]
struct Phase {
    /// The name of the span of the phase
    name: String,
    /// The summed time of the spans of the phase
    seconds: f64,
}

/// The file that `stiles check --timings` writes
#[derive(Clone, PartialEq, Debug, Deserialize)]
struct Timings {
    /// The phases of the run, in their order
    phases: Vec<Phase>,
}

/// One measure of a benchmark in the Bencher Metric Format
///
/// The format is plain JSON, and services such as Bencher ingest it as it
/// is. A benchmark maps the name of each measure to its value.
#[derive(Copy, Clone, PartialEq, Debug, Serialize)]
struct Metric {
    /// The value of the measure
    value: f64,
}

impl Measurement {
    /// Reads the record of the phases of a project
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be read or holds no JSON of
    /// the expected shape.
    pub(super) fn load(project: &'static str, timings: &Path) -> Result<Self, HarnessError> {
        let Timings { phases } = read_json(timings)?;

        Ok(Self { project, phases })
    }

    /// Returns one line that says what the project recorded
    ///
    /// The line names the sum of the phases and the phases that took the
    /// longest.
    pub(super) fn summary(&self) -> String {
        let Self { project, phases } = self;

        let total: f64 = phases.iter().map(|phase| phase.seconds).sum();
        let mut line = format!("{project}: {total:.2} s in its phases");

        let mut longest: Vec<&Phase> = phases.iter().collect();
        longest.sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
        for Phase { name, seconds } in longest.into_iter().take(SUMMARY_PHASES) {
            let _ = write!(line, ", {name} {seconds:.2} s");
        }

        line
    }

    /// Returns the measures of the project in the Bencher Metric Format
    ///
    /// Each phase is a measure of its own, in nanoseconds, the unit that the
    /// format expects for a time. A regression then points at its phase.
    fn metrics(&self) -> BTreeMap<String, Metric> {
        let Self { project: _, phases } = self;

        phases
            .iter()
            .map(|Phase { name, seconds }| {
                (
                    format!("phase-{name}"),
                    Metric {
                        value: seconds * NANOSECONDS,
                    },
                )
            })
            .collect()
    }
}

/// The phase that a run reaches only after it walked every file
const REPORT: &str = "report";

/// Returns whether the run that wrote a record of its phases reached the report
///
/// A run that stiles could not start writes no record, which counts as a run
/// that did not reach the report.
///
/// # Errors
///
/// Returns an error when the record exists and cannot be read or parsed.
pub(super) fn reached_report(record: &Path) -> Result<bool, HarnessError> {
    if !record.exists() {
        return Ok(false);
    }

    let Timings { phases } = read_json(record)?;

    Ok(phases.iter().any(|phase| phase.name == REPORT))
}

/// Writes the measurements to `path` in the Bencher Metric Format
///
/// The file maps the name of each project to its measures.
///
/// # Errors
///
/// Returns an error when the file cannot be written.
pub(super) fn write(path: &Path, measurements: &[Measurement]) -> Result<(), HarnessError> {
    let document: BTreeMap<&str, BTreeMap<String, Metric>> = measurements
        .iter()
        .map(|measurement| (measurement.project, measurement.metrics()))
        .collect();

    let text = serde_json::to_string_pretty(&document).map_err(|source| {
        HarnessError::UnserializableJson {
            path: path.to_path_buf(),
            source,
        }
    })?;

    fs::write(path, text + "\n").map_err(|source| HarnessError::Unwritable {
        path: path.to_path_buf(),
        source,
    })
}

/// Reads a JSON file that a tool of the run wrote
///
/// # Errors
///
/// Returns an error when the file cannot be read or holds no JSON of the
/// expected shape.
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, HarnessError> {
    let text = fs::read_to_string(path).map_err(|source| HarnessError::Unreadable {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&text).map_err(|source| HarnessError::UnparsableJson {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    // An assertion in a test panics by design. A `# Panics` section on every
    // test would repeat that and give the reader no information.
    #![allow(clippy::missing_panics_doc, clippy::unwrap_used)]

    use super::*;

    /// A record of `stiles check --timings` with four phases
    const TIMINGS: &str = r#"{"phases": [
        {"name": "plugins", "spans": 1, "seconds": 0.5},
        {"name": "load", "spans": 1, "seconds": 0.25},
        {"name": "decorate", "spans": 9, "seconds": 1.5},
        {"name": "walk", "spans": 9, "seconds": 0.125}
    ]}"#;

    /// Returns the measurement of the record above
    fn measurement() -> Measurement {
        let directory = tempfile::tempdir().unwrap();
        let timings = directory.path().join("timings.json");
        fs::write(&timings, TIMINGS).unwrap();

        Measurement::load("stiles", &timings).unwrap()
    }

    #[test]
    fn load_without_a_record_fails() {
        let directory = tempfile::tempdir().unwrap();
        let timings = directory.path().join("timings.json");

        let error = Measurement::load("stiles", &timings).unwrap_err();

        assert!(error.to_string().contains("timings.json"), "{error}");
    }

    #[test]
    fn metrics_give_each_phase_a_measure() {
        let metrics = measurement().metrics();

        let names: Vec<&str> = metrics.keys().map(String::as_str).collect();

        assert_eq!(
            names,
            [
                "phase-decorate",
                "phase-load",
                "phase-plugins",
                "phase-walk"
            ]
        );
    }

    #[test]
    fn metrics_give_a_phase_in_nanoseconds() {
        let metrics = measurement().metrics();

        assert_eq!(metrics["phase-decorate"], Metric { value: 1.5e9 });
    }

    #[test]
    fn reached_report_with_a_record_that_stops_early_returns_false() {
        let directory = tempfile::tempdir().unwrap();
        let record = directory.path().join("timings.json");
        fs::write(
            &record,
            r#"{"phases": [{"name": "plugins", "spans": 1, "seconds": 0.5}]}"#,
        )
        .unwrap();

        let reached = reached_report(&record).unwrap();

        assert!(!reached);
    }

    #[test]
    fn reached_report_with_a_record_that_reports_returns_true() {
        let directory = tempfile::tempdir().unwrap();
        let record = directory.path().join("timings.json");
        fs::write(
            &record,
            r#"{"phases": [{"name": "report", "spans": 1, "seconds": 0.5}]}"#,
        )
        .unwrap();

        let reached = reached_report(&record).unwrap();

        assert!(reached);
    }

    #[test]
    fn reached_report_without_a_record_returns_false() {
        let directory = tempfile::tempdir().unwrap();

        let reached = reached_report(&directory.path().join("timings.json")).unwrap();

        assert!(!reached);
    }

    #[test]
    fn summary_names_the_three_longest_phases() {
        let summary = measurement().summary();

        assert_eq!(
            summary,
            "stiles: 2.38 s in its phases, decorate 1.50 s, plugins 0.50 s, load 0.25 s"
        );
    }

    #[test]
    fn trait_send_measurement() {
        fn assert_send<T: Send>() {}
        assert_send::<Measurement>();
    }

    #[test]
    fn trait_sync_measurement() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Measurement>();
    }

    #[test]
    fn trait_unpin_measurement() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Measurement>();
    }
}
