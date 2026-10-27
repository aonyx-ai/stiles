use std::fs;

use rakko_action::{
    Access, Action, Context, FilePath, Finding, Location, Name, Outcome, Summary, action_name,
};
use rakko_tool::Invocation;

use crate::{HarnessError, step};

/// The record of the plugin boundary, relative to the project root
const RECORD: &str = "crates/whisker-plugin/abi-surface.toml";

/// The branch whose record a run compares with
const BASE: &str = "main";

/// The keys of the record that hold a fingerprint of the boundary
const FINGERPRINTS: [&str; 2] = ["types-fingerprint", "language-fingerprint"];

/// The action that fails when the plugin boundary moved and its version did not
///
/// `crates/whisker-plugin/abi-surface.toml` records the boundary, and a test
/// in whisker-plugin keeps it equal to the code. A pull request that changes
/// the boundary therefore changes the record too, so the record alone proves
/// nothing. The run compares it with the record on `main` instead: a moved
/// fingerprint needs a raised version, and from 1.0 a raised major, because
/// a fingerprinted type that changed refuses every older plugin.
#[derive(Copy, Clone, Debug, Default)]
pub struct CheckAbi;

impl Action for CheckAbi {
    type Args = ();

    fn name(&self) -> Name {
        action_name!("check-abi")
    }

    fn access(&self, _context: &Context, _args: &Self::Args) -> Access {
        Access::Read
    }

    async fn run(&self, context: &Context, _args: &Self::Args) -> Outcome {
        step::outcome(drive(context).await)
    }
}

/// A version of the plugin boundary, as the record writes it
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
struct AbiVersion {
    /// The major, which carries the promise from 1.0
    major: u32,
    /// The minor, which every change raises before 1.0
    minor: u32,
}

impl AbiVersion {
    /// Reads a version such as `0.2`
    fn parse(text: &str) -> Option<Self> {
        let (major, minor) = text.split_once('.')?;

        Some(Self {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
        })
    }
}

/// What the comparison of two records found
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
enum Verdict {
    /// No fingerprint moved
    Unmoved,
    /// A fingerprint moved, and the version rose as far as it had to
    Raised {
        /// The version on the base branch
        before: AbiVersion,
        /// The version in the tree
        after: AbiVersion,
    },
    /// A fingerprint moved, and the version did not rise far enough
    Refused {
        /// Why the version is not enough
        reason: String,
    },
}

/// Runs the action against the project of the context
///
/// # Errors
///
/// Returns the error of the step that could not finish: a run of git.
async fn drive(context: &Context) -> Result<Outcome, HarnessError> {
    let root = context.root().get();
    let git = || Invocation::new("git").in_directory(root);

    let fetch = step::run(git().args(["fetch", "--quiet", "origin", BASE])).await?;
    if !fetch.status().success() {
        return Ok(step::failed(&format!("fetching {BASE}"), &fetch));
    }

    let show = step::run(git().args(["show", &format!("FETCH_HEAD:{RECORD}")])).await?;
    if !show.status().success() {
        return Ok(Outcome::Passed {
            summary: Some(Summary::new(format!(
                "{BASE} has no {RECORD}, so there is nothing to compare"
            ))),
        });
    }
    let before = show.stdout().to_string_lossy();

    let after = match fs::read_to_string(root.join(RECORD)) {
        Ok(after) => after,
        Err(error) => return Ok(refused(format!("failed to read {RECORD}: {error}"))),
    };

    Ok(match verdict(&before, &after) {
        Verdict::Unmoved => Outcome::Passed {
            summary: Some(Summary::new("the plugin boundary did not move")),
        },
        Verdict::Raised { before, after } => Outcome::Passed {
            summary: Some(Summary::new(format!(
                "the plugin boundary moved, and ABI_VERSION rose from {}.{} to {}.{}",
                before.major, before.minor, after.major, after.minor
            ))),
        },
        Verdict::Refused { reason } => refused(reason),
    })
}

/// Returns the outcome of a run that refuses the record
fn refused(reason: String) -> Outcome {
    let location =
        FilePath::try_from(RECORD).map_or(Location::Project, |path| Location::File { path });

    Outcome::Failed {
        findings: vec![
            Finding::builder()
                .message(reason)
                .location(location)
                .build(),
        ],
        repairs: Vec::new(),
    }
}

/// Compares the record of the base branch with the record of the tree
fn verdict(before: &str, after: &str) -> Verdict {
    if FINGERPRINTS
        .iter()
        .all(|key| value(key, before) == value(key, after))
    {
        return Verdict::Unmoved;
    }

    let version = |record: &str| value("version", record).and_then(AbiVersion::parse);
    let (Some(old), Some(new)) = (version(before), version(after)) else {
        return Verdict::Refused {
            reason: format!("{RECORD} holds no version in the form MAJOR.MINOR"),
        };
    };

    if new <= old {
        return Verdict::Refused {
            reason: format!(
                "the plugin boundary moved, but ABI_VERSION did not rise above {}.{}. Raise \
                 ABI_VERSION in crates/whisker-types/src/plugin.rs.",
                old.major, old.minor
            ),
        };
    }

    if old.major >= 1 && new.major == old.major {
        return Verdict::Refused {
            reason: format!(
                "a fingerprinted type changed, which refuses every plugin of ABI {}. Raise the \
                 major of ABI_VERSION, not the minor.",
                old.major
            ),
        };
    }

    Verdict::Raised {
        before: old,
        after: new,
    }
}

/// Returns the value of a key in a record, which writes each as `key = "value"`
fn value<'a>(key: &str, record: &'a str) -> Option<&'a str> {
    record.lines().find_map(|line| {
        line.strip_prefix(key)?
            .trim_start()
            .strip_prefix('=')?
            .trim()
            .strip_prefix('"')?
            .strip_suffix('"')
    })
}

#[cfg(test)]
mod tests {
    // An assertion in a test panics by design. A `# Panics` section on every
    // test would repeat that and give the reader no information.
    #![allow(clippy::missing_panics_doc)]

    use super::*;

    /// Returns a record with a version and a types fingerprint
    fn record(version: &str, types: &str) -> String {
        format!(
            "# The plugin boundary\nversion = \"{version}\"\ntypes-fingerprint = \
             \"{types}\"\nlanguage-fingerprint = \"6ddbcd1ad0a3f382\"\n"
        )
    }

    #[test]
    fn value_with_a_key_of_the_record_returns_its_value() {
        let text = record("0.2", "c24935f2378f4aba");

        let found = value("types-fingerprint", &text);

        assert_eq!(found, Some("c24935f2378f4aba"));
    }

    #[test]
    fn value_with_a_key_that_prefixes_another_returns_nothing() {
        let text = "versions = \"0.2\"\n";

        let found = value("version", text);

        assert_eq!(found, None);
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_and_a_raised_minor_passes() {
        let found = verdict(&record("0.2", "a"), &record("0.3", "b"));

        assert_eq!(
            found,
            Verdict::Raised {
                before: AbiVersion { major: 0, minor: 2 },
                after: AbiVersion { major: 0, minor: 3 },
            }
        );
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_and_the_same_version_refuses() {
        let found = verdict(&record("0.2", "a"), &record("0.2", "b"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_after_one_and_a_raised_minor_refuses() {
        let found = verdict(&record("1.2", "a"), &record("1.3", "b"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_after_one_and_a_raised_major_passes() {
        let found = verdict(&record("1.2", "a"), &record("2.0", "b"));

        assert!(matches!(found, Verdict::Raised { .. }));
    }

    #[test]
    fn verdict_with_a_ten_after_a_nine_compares_numbers() {
        let found = verdict(&record("0.9", "a"), &record("0.10", "b"));

        assert!(matches!(found, Verdict::Raised { .. }));
    }

    #[test]
    fn verdict_with_unmoved_fingerprints_passes_whatever_the_version() {
        let found = verdict(&record("0.2", "a"), &record("0.2", "a"));

        assert_eq!(found, Verdict::Unmoved);
    }

    #[test]
    fn verdict_with_an_unreadable_version_refuses() {
        let found = verdict(&record("0.2", "a"), &record("two", "b"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }
}
