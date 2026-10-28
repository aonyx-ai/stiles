use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::slice;
use std::sync::Arc;

use bon::bon;
use serde::Serialize;
use whisker_types::{Diagnostic, Severity};

/// One diagnostic as `whisker check` reports it
///
/// A finding is the artifact the check command sends to its presenter, one
/// per diagnostic. In text mode the presenter prints the rendered snippet,
/// the same text whisker always printed. With `--json` it prints the fields
/// below as one JSON object per line, so a clean run prints nothing in
/// either mode and a caller reads the findings one line at a time.
///
/// `start` and `end` are byte offsets into the file. `line` and `column`
/// count from one, and the column counts characters, the way an editor
/// shows a position.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub(crate) struct Finding {
    rule: &'static str,
    severity: &'static str,
    message: String,
    file: PathBuf,
    start: usize,
    end: usize,
    line: usize,
    column: usize,
    #[serde(skip)]
    rendered: String,
}

#[bon]
impl Finding {
    /// Describes `diagnostic`, reading its position from `sources`
    ///
    /// A file missing from `sources` reports line and column one. The check
    /// command records the source of every file that produced a diagnostic,
    /// so that happens only to a diagnostic whose span names another file.
    ///
    /// # Errors
    ///
    /// Returns an error if the diagnostic cannot be rendered.
    #[builder]
    pub(crate) fn new(
        diagnostic: &Diagnostic,
        sources: &HashMap<Arc<Path>, String>,
    ) -> anyhow::Result<Self> {
        let rendered = whisker_reporting::render_to_string(slice::from_ref(diagnostic), sources)?;
        let span = diagnostic.span();
        let (line, column) = match sources.get(span.file()) {
            Some(source) => position(source, span.start()),
            None => (1, 1),
        };

        Ok(Self {
            rule: diagnostic.rule_id().as_str(),
            severity: severity_name(diagnostic.severity()),
            message: diagnostic.message().to_owned(),
            file: span.file().to_path_buf(),
            start: span.start(),
            end: span.end(),
            line,
            column,
            rendered,
        })
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered = self.rendered.strip_suffix('\n').unwrap_or(&self.rendered);

        f.write_str(rendered)
    }
}

/// Returns the name a JSON reader sees for `severity`
///
/// The names match the headers of the rendered text, such as `warning` in
/// `warning[lint.todo-comment]`.
fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Help => "help",
        Severity::Info => "note",
        Severity::Warn => "warning",
        Severity::Error => "error",
    }
}

/// Returns the line and the column of byte `offset` in `source`
///
/// Both count from one. An offset past the end of `source`, or inside a
/// character, counts up to the last whole character before it.
fn position(source: &str, offset: usize) -> (usize, usize) {
    let mut end = offset.min(source.len());
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    let before = &source[..end];

    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    let column = before[line_start..].chars().count() + 1;

    (line, column)
}

#[cfg(test)]
mod tests {
    use whisker_types::{RuleId, Span};

    use super::*;

    fn finding(source: &str, start: usize, end: usize) -> Finding {
        let path: Arc<Path> = Path::new("src/lib.rs").into();
        let sources = HashMap::from([(path, source.to_owned())]);
        let diagnostic = Diagnostic::new(
            RuleId::new("lint.test"),
            Severity::Warn,
            "test message".into(),
            Span::new(PathBuf::from("src/lib.rs"), start, end),
        );

        Finding::builder()
            .diagnostic(&diagnostic)
            .sources(&sources)
            .build()
            .expect("should render")
    }

    #[test]
    fn display_shows_the_location() {
        let finding = finding("fn a() {}\n", 3, 4);

        let text = finding.to_string();

        assert!(text.contains("src/lib.rs:1:4"));
    }

    #[test]
    fn display_shows_the_rendered_snippet() {
        let finding = finding("fn a() {}\n", 3, 4);

        let text = finding.to_string();

        assert!(text.starts_with("warning[lint.test]: test message"));
    }

    #[test]
    fn display_drops_one_trailing_newline() {
        let finding = finding("fn a() {}\n", 3, 4);

        let text = finding.to_string();

        assert!(!text.ends_with("\n\n"));
    }

    #[test]
    fn json_has_the_fields_a_reader_needs() {
        let finding = finding("fn a() {}\nfn b() {}\n", 13, 14);

        let json = serde_json::to_value(&finding).expect("should serialize");

        assert_eq!(
            json,
            serde_json::json!({
                "rule": "lint.test",
                "severity": "warning",
                "message": "test message",
                "file": "src/lib.rs",
                "start": 13,
                "end": 14,
                "line": 2,
                "column": 4,
            })
        );
    }

    #[test]
    fn new_with_a_file_missing_from_the_sources_reports_line_one() {
        let sources = HashMap::new();
        let diagnostic = Diagnostic::new(
            RuleId::new("lint.test"),
            Severity::Error,
            "test message".into(),
            Span::new(PathBuf::from("other.rs"), 5, 6),
        );

        let finding = Finding::builder()
            .diagnostic(&diagnostic)
            .sources(&sources)
            .build()
            .expect("should render");

        assert_eq!((finding.line, finding.column), (1, 1));
    }

    #[test]
    fn position_counts_characters_not_bytes() {
        let source = "é = 1\n";

        let column = position(source, 3).1;

        assert_eq!(column, 3);
    }

    #[test]
    fn position_inside_a_character_counts_the_whole_characters_before_it() {
        let source = "aé";

        let column = position(source, 2).1;

        assert_eq!(column, 2);
    }

    #[test]
    fn position_past_the_end_stops_at_the_end() {
        let source = "ab\ncd";

        let position = position(source, 99);

        assert_eq!(position, (2, 3));
    }

    #[test]
    fn severity_name_matches_the_rendered_header() {
        let names = [
            Severity::Help,
            Severity::Info,
            Severity::Warn,
            Severity::Error,
        ]
        .map(severity_name);

        assert_eq!(names, ["help", "note", "warning", "error"]);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Finding>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Finding>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Finding>();
    }
}
