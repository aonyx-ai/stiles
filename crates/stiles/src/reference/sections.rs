/// The heading that opens the examples of a pass
const EXAMPLES_HEADING: &str = "# Examples";

/// The marker line that opens a negative example
const EXAMPLE_MARKER: &str = "// DON'T";

/// The marker line that opens the fix of a negative example
const FIX_MARKER: &str = "// DO";

/// The parts of a pass's docs that a reference entry shows
///
/// The examples section keeps only its negative example and fix. Its other
/// code blocks are doctests, which drive a test harness and teach a reader
/// nothing about the rule.
#[derive(Clone, Eq, PartialEq, Debug)]
pub(super) struct Sections {
    pub(super) summary: String,
    pub(super) body: String,
    pub(super) pair: Option<Pair>,
}

/// A negative example and its fix
#[derive(Clone, Eq, PartialEq, Debug)]
pub(super) struct Pair {
    pub(super) example: String,
    pub(super) fix: String,
}

/// Where a doc line sits against the code fences around it
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Fence {
    /// The line opens, closes, or sits inside a code fence
    Inside,
    /// The line is prose or a heading
    Outside,
}

impl Sections {
    /// Splits the docs of a pass into the parts of an entry
    ///
    /// A link definition in the examples section moves to the body, because
    /// the prose above may use it.
    pub(super) fn parse(docs: &str) -> Self {
        let lines: Vec<&str> = docs.lines().collect();
        let fences = classify(&lines);
        let unfenced = |index: &usize| fences[*index] == Fence::Outside;

        let summary_end = (0..lines.len())
            .filter(unfenced)
            .find(|index| lines[*index].trim().is_empty())
            .unwrap_or(lines.len());
        let examples_start = (0..lines.len())
            .filter(unfenced)
            .find(|index| lines[*index].trim() == EXAMPLES_HEADING)
            .unwrap_or(lines.len());
        let examples_end = (examples_start + 1..lines.len())
            .filter(unfenced)
            .find(|index| lines[*index].starts_with("# "))
            .unwrap_or(lines.len());

        let examples = lines.get(examples_start + 1..examples_end).unwrap_or(&[]);
        let definitions = examples
            .iter()
            .copied()
            .filter(|line| is_link_definition(line));
        let body: Vec<&str> = lines[summary_end.min(examples_start)..examples_start]
            .iter()
            .chain(&lines[examples_end..])
            .copied()
            .chain(definitions)
            .collect();

        Self {
            summary: lines[..summary_end].join("\n"),
            body: body.join("\n").trim().to_owned(),
            pair: parse_pair(examples),
        }
    }
}

/// Returns where each line sits against the code fences
fn classify(lines: &[&str]) -> Vec<Fence> {
    let mut open = None;

    lines
        .iter()
        .map(|line| {
            let marker = fence_marker(line);
            let fence = match open.or(marker) {
                Some(_) => Fence::Inside,
                None => Fence::Outside,
            };
            match (open, marker) {
                (None, Some(_)) => open = marker,
                (Some(_), Some(_)) if open == marker => open = None,
                _ => {}
            }
            fence
        })
        .collect()
}

/// Returns the character of the code fence that a line opens or closes
fn fence_marker(line: &str) -> Option<char> {
    let line = line.trim_start();

    ['`', '~']
        .into_iter()
        .find(|marker| line.starts_with(&marker.to_string().repeat(3)))
}

/// Returns whether a line defines a Markdown link
fn is_link_definition(line: &str) -> bool {
    line.starts_with('[') && line.contains("]: ")
}

/// Returns the first negative example and fix in a run of lines
fn parse_pair(section: &[&str]) -> Option<Pair> {
    let mut lines = section.iter().copied();

    while let Some(line) = lines.next() {
        let Some(marker) = fence_marker(line) else {
            continue;
        };
        let block: Vec<&str> = lines
            .by_ref()
            .take_while(|line| fence_marker(line) != Some(marker))
            .collect();
        if let Some(pair) = split_pair(&block) {
            return Some(pair);
        }
    }

    None
}

/// Returns the code under the two markers of a fenced block
fn split_pair(block: &[&str]) -> Option<Pair> {
    let example_start = block
        .iter()
        .position(|line| line.trim() == EXAMPLE_MARKER)?;
    let after_example = &block[example_start + 1..];
    let fix_start = after_example
        .iter()
        .position(|line| line.trim() == FIX_MARKER)?;

    Some(Pair {
        example: after_example[..fix_start].join("\n").trim().to_owned(),
        fix: after_example[fix_start + 1..].join("\n").trim().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_auto_traits<T: Send + Sync + Unpin>() {}

    const DOCS: &str = "Flags a thing\n\nA thing hides intent.\n\n# Examples\n\n```text\n// DON'T\nthing();\n\n// DO\nother();\n```\n\n```\nlet doctest = 1;\n```\n";

    #[test]
    fn parse_drops_the_doctests() {
        let sections = Sections::parse(DOCS);

        assert!(!sections.body.contains("doctest"));
    }

    #[test]
    fn parse_keeps_a_section_after_the_examples() {
        let docs = format!("{DOCS}\n# Options\n\nDepth.\n");

        let sections = Sections::parse(&docs);

        assert_eq!(
            sections.body,
            "A thing hides intent.\n\n# Options\n\nDepth."
        );
    }

    #[test]
    fn parse_moves_a_link_definition_to_the_body() {
        let docs = "Flags a thing\n\nSee [TOML].\n\n# Examples\n\n[TOML]: https://toml.io\n";

        let sections = Sections::parse(docs);

        assert!(sections.body.ends_with("[TOML]: https://toml.io"));
    }

    #[test]
    fn parse_of_a_pair_in_a_tilde_fence_keeps_backticks_inside() {
        let docs = "S\n\n# Examples\n\n~~~text\n// DON'T\n```\n// DO\nb\n~~~\n";

        let sections = Sections::parse(docs);

        assert_eq!(
            sections.pair.map(|pair| pair.example),
            Some("```".to_owned())
        );
    }

    #[test]
    fn parse_reads_a_summary_over_two_lines() {
        let sections = Sections::parse("Flags a\nthing\n\nBody");

        assert_eq!(sections.summary, "Flags a\nthing");
    }

    #[test]
    fn parse_reads_the_example() {
        let sections = Sections::parse(DOCS);

        assert_eq!(
            sections.pair.map(|pair| pair.example),
            Some("thing();".to_owned())
        );
    }

    #[test]
    fn parse_reads_the_fix() {
        let sections = Sections::parse(DOCS);

        assert_eq!(
            sections.pair.map(|pair| pair.fix),
            Some("other();".to_owned())
        );
    }

    #[test]
    fn parse_with_a_heading_in_a_fence_ignores_it() {
        let sections = Sections::parse("S\n\n```\n# Examples\n```\n\nAfter");

        assert!(sections.body.contains("After"));
    }

    #[test]
    fn parse_without_a_body_leaves_it_empty() {
        let sections = Sections::parse("Flags a thing\n\n# Examples\n");

        assert_eq!(sections.body, "");
    }

    #[test]
    fn parse_without_markers_has_no_pair() {
        let sections = Sections::parse("S\n\n# Examples\n\n```\nplain\n```\n");

        assert!(sections.pair.is_none());
    }

    #[test]
    fn trait_auto_fence() {
        assert_auto_traits::<Fence>();
    }

    #[test]
    fn trait_auto_pair() {
        assert_auto_traits::<Pair>();
    }

    #[test]
    fn trait_auto_sections() {
        assert_auto_traits::<Sections>();
    }
}
