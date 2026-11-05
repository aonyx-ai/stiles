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

/// The number of bytes in a gibibyte
const GIBIBYTE: f64 = 1024.0 * 1024.0 * 1024.0;

/// What the benchmarks measured on one project of the corpus
///
/// The wall time and the peak memory come from the timed runs of hyperfine.
/// The phases come from one more run with `--timings`, so they split a run
/// of the same kind, and not the mean of the timed runs.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct Measurement {
    /// The name of the project
    project: &'static str,
    /// The wall time of a run, in seconds
    wall: Statistic,
    /// The peak resident memory of a run, in bytes, where the platform
    /// reports it
    memory: Option<Statistic>,
    /// The time of each phase of the run with `--timings`, in its order
    phases: Vec<Phase>,
}

/// The summary of one metric over the timed runs, as hyperfine writes it
#[derive(Copy, Clone, PartialEq, Debug, Deserialize)]
struct Statistic {
    /// The mean of the runs
    mean: f64,
    /// The smallest value of the runs
    min: f64,
    /// The largest value of the runs
    max: f64,
    /// The standard deviation of the runs, which one run does not have
    stddev: Option<f64>,
}

/// The time of one phase, as `stiles check --timings` writes it
#[derive(Clone, PartialEq, Debug, Deserialize)]
struct Phase {
    /// The name of the span of the phase
    name: String,
    /// The summed time of the spans of the phase
    seconds: f64,
}

/// The file that `hyperfine --export-json` writes, in its second schema
#[derive(Clone, PartialEq, Debug, Deserialize)]
struct Export {
    /// One result per benchmarked command
    results: Vec<ExportResult>,
}

/// The result of one benchmarked command in an export of hyperfine
#[derive(Clone, PartialEq, Debug, Deserialize)]
struct ExportResult {
    /// The summary of each metric over the runs
    summary: Summaries,
}

/// The summaries of the metrics that the bench action reads
#[derive(Clone, PartialEq, Debug, Deserialize)]
struct Summaries {
    /// The wall time, in seconds
    time_wall_clock: Statistic,
    /// The peak resident memory, in bytes, where the platform reports it
    memory_peak_resident: Option<Statistic>,
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
/// is. A benchmark maps the name of each measure to its value, with the
/// smallest and the largest value of the runs where there were several.
#[derive(Copy, Clone, PartialEq, Debug, Serialize)]
struct Metric {
    /// The value of the measure
    value: f64,
    /// The smallest value of the runs
    #[serde(skip_serializing_if = "Option::is_none")]
    lower_value: Option<f64>,
    /// The largest value of the runs
    #[serde(skip_serializing_if = "Option::is_none")]
    upper_value: Option<f64>,
}

impl Measurement {
    /// Reads the export of hyperfine and the record of the phases of a project
    ///
    /// # Errors
    ///
    /// Returns an error when a file cannot be read, holds no JSON of the
    /// expected shape, or the export holds no result.
    pub(super) fn load(
        project: &'static str,
        export: &Path,
        timings: &Path,
    ) -> Result<Self, HarnessError> {
        let Export { results } = read_json(export)?;
        let Timings { phases } = read_json(timings)?;

        let Some(ExportResult {
            summary:
                Summaries {
                    time_wall_clock,
                    memory_peak_resident,
                },
        }) = results.into_iter().next()
        else {
            return Err(HarnessError::EmptyExport {
                path: export.to_path_buf(),
            });
        };

        Ok(Self {
            project,
            wall: time_wall_clock,
            memory: memory_peak_resident,
            phases,
        })
    }

    /// Returns one line that says what the project measured
    ///
    /// The line names the mean wall time and its spread, the mean peak
    /// memory, and the phases that took the longest.
    pub(super) fn summary(&self) -> String {
        let Self {
            project,
            wall,
            memory,
            phases,
        } = self;

        let mut line = format!("{project}: {:.2} s", wall.mean);
        if let Some(stddev) = wall.stddev {
            let _ = write!(line, " ± {stddev:.2} s");
        }
        if let Some(memory) = memory {
            let _ = write!(line, ", peak {:.2} GiB", memory.mean / GIBIBYTE);
        }

        let mut longest: Vec<&Phase> = phases.iter().collect();
        longest.sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
        for Phase { name, seconds } in longest.into_iter().take(SUMMARY_PHASES) {
            let _ = write!(line, ", {name} {seconds:.2} s");
        }

        line
    }

    /// Returns the measures of the project in the Bencher Metric Format
    ///
    /// Times are in nanoseconds and memory in bytes, the units that the
    /// format expects for latency. Each phase is a measure of its own, so a
    /// regression points at the phase that it is in.
    fn metrics(&self) -> BTreeMap<String, Metric> {
        let Self {
            project: _,
            wall,
            memory,
            phases,
        } = self;

        let mut metrics = BTreeMap::new();
        metrics.insert("latency".to_owned(), wall.metric(NANOSECONDS));
        if let Some(memory) = memory {
            metrics.insert("peak-memory".to_owned(), memory.metric(1.0));
        }
        for Phase { name, seconds } in phases {
            metrics.insert(
                format!("phase-{name}"),
                Metric {
                    value: seconds * NANOSECONDS,
                    lower_value: None,
                    upper_value: None,
                },
            );
        }

        metrics
    }
}

impl Statistic {
    /// Returns the statistic as a measure, with every value times `scale`
    fn metric(self, scale: f64) -> Metric {
        Metric {
            value: self.mean * scale,
            lower_value: Some(self.min * scale),
            upper_value: Some(self.max * scale),
        }
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

    /// An export of hyperfine 2 with three runs, cut to what the action reads
    const EXPORT: &str = r#"{
        "schema_version": 2,
        "results": [{
            "command": "stiles check --keep-going .",
            "summary": {
                "time_wall_clock": {"unit": "second", "count": 3, "mean": 2.5, "stddev": 0.25, "median": 2.5, "min": 2.0, "max": 3.0},
                "memory_peak_resident": {"unit": "byte", "count": 3, "mean": 2147483648.0, "stddev": 0.0, "median": 2147483648.0, "min": 2147483648.0, "max": 2147483648.0}
            }
        }]
    }"#;

    /// A record of `stiles check --timings` with four phases
    const TIMINGS: &str = r#"{"phases": [
        {"name": "plugins", "spans": 1, "seconds": 0.5},
        {"name": "load", "spans": 1, "seconds": 0.25},
        {"name": "decorate", "spans": 9, "seconds": 1.5},
        {"name": "walk", "spans": 9, "seconds": 0.125}
    ]}"#;

    /// Returns the measurement of the export and the record above
    fn measurement() -> Measurement {
        let directory = tempfile::tempdir().unwrap();
        let export = directory.path().join("hyperfine.json");
        let timings = directory.path().join("timings.json");
        fs::write(&export, EXPORT).unwrap();
        fs::write(&timings, TIMINGS).unwrap();

        Measurement::load("stiles", &export, &timings).unwrap()
    }

    #[test]
    fn load_with_an_empty_export_fails() {
        let directory = tempfile::tempdir().unwrap();
        let export = directory.path().join("hyperfine.json");
        let timings = directory.path().join("timings.json");
        fs::write(&export, r#"{"results": []}"#).unwrap();
        fs::write(&timings, TIMINGS).unwrap();

        let error = Measurement::load("stiles", &export, &timings).unwrap_err();

        assert!(error.to_string().contains("hyperfine.json"), "{error}");
    }

    #[test]
    fn metrics_give_the_latency_in_nanoseconds() {
        let metrics = measurement().metrics();

        assert_eq!(
            metrics["latency"],
            Metric {
                value: 2.5e9,
                lower_value: Some(2.0e9),
                upper_value: Some(3.0e9),
            }
        );
    }

    #[test]
    fn metrics_give_each_phase_a_measure() {
        let metrics = measurement().metrics();

        let names: Vec<&str> = metrics.keys().map(String::as_str).collect();

        assert_eq!(
            names,
            [
                "latency",
                "peak-memory",
                "phase-decorate",
                "phase-load",
                "phase-plugins",
                "phase-walk"
            ]
        );
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
            "stiles: 2.50 s ± 0.25 s, peak 2.00 GiB, decorate 1.50 s, plugins 0.50 s, load 0.25 s"
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
