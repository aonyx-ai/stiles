use std::cmp::Ordering;
use std::fs;

use rakko_action::{
    Access, Action, Context, FilePath, Finding, Location, Name, Outcome, Summary, action_name,
};
use rakko_tool::Invocation;

use crate::{HarnessError, step};

/// The record of the plugin boundary, relative to the project root
const RECORD: &str = "crates/stiles-plugin/abi-surface.toml";

/// The prefix of every release tag
const TAG_PREFIX: &str = "stiles@";

/// The keys of the record that hold a fingerprint of the boundary
const FINGERPRINTS: [&str; 2] = ["types-fingerprint", "language-fingerprint"];

/// The action that fails when `ABI_VERSION` breaks the promise of a release
///
/// `crates/stiles-plugin/abi-surface.toml` records the boundary, and a test
/// in stiles-plugin keeps it equal to the code. A pull request that changes
/// the boundary therefore changes the record too, so the record alone proves
/// nothing. The run compares it with the record at the latest `stiles@` tag
/// instead.
///
/// A version is a promise about a release, so it rises once between two
/// releases. When a fingerprint moved since the latest release, the version
/// must be the one after that release's. Further moves before the next
/// release need no further rise. The version may also rise without a moved
/// fingerprint, because it guards what no fingerprint reads: the shape of
/// the declaration and the signatures of the pass trait. Either way it may
/// only rise to the next version, never past it.
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
///
/// Before 1.0 a version is `0.MINOR`. From 1.0 it is one integer, and
/// the minor is 0.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct AbiVersion {
    /// The component every release that moved the boundary raises from 1.0
    major: u32,
    /// The component every release that moved the boundary raises before 1.0
    minor: u32,
}

impl AbiVersion {
    /// Reads a version such as `0.2` or `3`
    ///
    /// A version from 1.0 has no minor, so `1.0` is not a version.
    fn parse(text: &str) -> Option<Self> {
        let Some((major, minor)) = text.split_once('.') else {
            let major = text.parse().ok()?;

            return match major {
                0 => None,
                major => Some(Self { major, minor: 0 }),
            };
        };

        let major = major.parse().ok()?;
        let minor = minor.parse().ok()?;

        match major {
            0 => Some(Self { major, minor }),
            _ => None,
        }
    }

    /// Returns the versions a release may name after one that named this
    ///
    /// Before 1.0 that is the next minor, or 1, which opens the era of
    /// integers. From 1.0 it is the next integer.
    fn successors(self) -> Vec<Self> {
        let Self { major, minor } = self;

        match major {
            0 => vec![
                Self {
                    major: 0,
                    minor: minor + 1,
                },
                Self { major: 1, minor: 0 },
            ],
            major => vec![Self {
                major: major + 1,
                minor: 0,
            }],
        }
    }
}

impl std::fmt::Display for AbiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { major, minor } = self;

        match major {
            0 => write!(f, "0.{minor}"),
            major => write!(f, "{major}"),
        }
    }
}

/// The version of a release tag, such as `0.6.0` or `0.1.0-rc.6`
///
/// The order is semver's, so a pre-release sorts before the release it
/// precedes, and a numeric identifier of a pre-release compares as a
/// number.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct ReleaseVersion {
    /// The major, minor, and patch of the release
    core: [u64; 3],
    /// The identifiers after the dash, which a full release has none of
    pre: Vec<String>,
}

impl ReleaseVersion {
    /// Reads the version of a tag such as `stiles@0.6.0`
    fn parse(tag: &str) -> Option<Self> {
        let version = tag.strip_prefix(TAG_PREFIX)?;
        let (core, pre) = match version.split_once('-') {
            Some((core, pre)) => (core, pre.split('.').map(str::to_owned).collect()),
            None => (version, Vec::new()),
        };

        let mut parts = core.split('.').map(str::parse::<u64>);
        let core = [
            parts.next()?.ok()?,
            parts.next()?.ok()?,
            parts.next()?.ok()?,
        ];
        let None = parts.next() else {
            return None;
        };

        Some(Self { core, pre })
    }
}

impl Ord for ReleaseVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => compare_pre(&self.pre, &other.pre),
            })
    }
}

impl PartialOrd for ReleaseVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compares two lists of pre-release identifiers the way semver does
fn compare_pre(left: &[String], right: &[String]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let order = match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => left.cmp(right),
        };
        if order != Ordering::Equal {
            return order;
        }
    }

    left.len().cmp(&right.len())
}

/// What the comparison of two records found
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
enum Verdict {
    /// The fingerprints and the version are the release's
    Unmoved,
    /// The version is the one after the release's
    Raised {
        /// The version of the release
        before: AbiVersion,
        /// The version in the tree
        after: AbiVersion,
    },
    /// The version breaks the promise of the release
    Refused {
        /// Why the version is wrong
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

    let pattern = format!("refs/tags/{TAG_PREFIX}*");
    let listed =
        step::run(git().args(["ls-remote", "--tags", "--refs", "origin", &pattern])).await?;
    if !listed.status().success() {
        return Ok(step::failed("listing the release tags", &listed));
    }

    let Some(tag) = latest_release(&listed.stdout().to_string_lossy()) else {
        return Ok(Outcome::Passed {
            summary: Some(Summary::new(
                "no release names an ABI version yet, so there is nothing to compare",
            )),
        });
    };

    let fetch = step::run(git().args([
        "fetch",
        "--quiet",
        "--no-tags",
        "origin",
        &format!("refs/tags/{tag}"),
    ]))
    .await?;
    if !fetch.status().success() {
        return Ok(step::failed(&format!("fetching {tag}"), &fetch));
    }

    let show = step::run(git().args(["show", &format!("FETCH_HEAD:{RECORD}")])).await?;
    if !show.status().success() {
        return Ok(Outcome::Passed {
            summary: Some(Summary::new(format!(
                "{tag} has no {RECORD}, so there is nothing to compare"
            ))),
        });
    }
    let before = show.stdout().to_string_lossy();

    let after = match fs::read_to_string(root.join(RECORD)) {
        Ok(after) => after,
        Err(error) => return Ok(refused(format!("failed to read {RECORD}: {error}"))),
    };

    Ok(match verdict(&tag, &before, &after) {
        Verdict::Unmoved => Outcome::Passed {
            summary: Some(Summary::new(format!(
                "the plugin boundary is the one {tag} released"
            ))),
        },
        Verdict::Raised { before, after } => Outcome::Passed {
            summary: Some(Summary::new(format!(
                "ABI_VERSION rose from {before}, which {tag} released, to {after}"
            ))),
        },
        Verdict::Refused { reason } => refused(reason),
    })
}

/// Returns the latest release among the tags that `git ls-remote` listed
///
/// Each line holds an object name, a tab, and a ref. A ref that names no
/// release version is skipped.
fn latest_release(listing: &str) -> Option<String> {
    listing
        .lines()
        .filter_map(|line| line.split_once('\t')?.1.strip_prefix("refs/tags/"))
        .filter_map(|tag| Some((ReleaseVersion::parse(tag)?, tag)))
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, tag)| tag.to_owned())
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

/// Compares the record of the release `tag` with the record of the tree
fn verdict(tag: &str, before: &str, after: &str) -> Verdict {
    let version = |record: &str| value("version", record).and_then(AbiVersion::parse);
    let (Some(old), Some(new)) = (version(before), version(after)) else {
        return Verdict::Refused {
            reason: format!(
                "{RECORD} holds no version in the form 0.MINOR, or MAJOR from 1 onward"
            ),
        };
    };

    let moved = FINGERPRINTS
        .iter()
        .any(|key| value(key, before) != value(key, after));

    match (moved, new == old) {
        (false, true) => Verdict::Unmoved,
        (true, true) => Verdict::Refused {
            reason: format!(
                "the plugin boundary moved since {tag}, which released ABI {old}, so ABI_VERSION \
                 must rise. Set ABI_VERSION in crates/stiles-types/src/plugin.rs to {}.",
                next(old)
            ),
        },
        (false | true, false) if old.successors().contains(&new) => Verdict::Raised {
            before: old,
            after: new,
        },
        (false | true, false) => Verdict::Refused {
            reason: format!(
                "{tag} released ABI {old}, so ABI_VERSION is either {old} or {}, not {new}. \
                     Set ABI_VERSION in crates/stiles-types/src/plugin.rs.",
                next(old)
            ),
        },
    }
}

/// Names the versions a release may name after `old`, for a message
fn next(old: AbiVersion) -> String {
    old.successors()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" or ")
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

    /// The release every verdict compares with
    const TAG: &str = "stiles@0.6.0";

    /// Returns a record with a version and a types fingerprint
    fn record(version: &str, types: &str) -> String {
        format!(
            "# The plugin boundary\nversion = \"{version}\"\ntypes-fingerprint = \
             \"{types}\"\nlanguage-fingerprint = \"6ddbcd1ad0a3f382\"\n"
        )
    }

    #[test]
    fn abi_version_parse_with_a_major_from_one_onward_reads_one_integer() {
        let found = AbiVersion::parse("3");

        assert_eq!(found, Some(AbiVersion { major: 3, minor: 0 }));
    }

    #[test]
    fn abi_version_parse_with_a_minor_from_one_onward_returns_nothing() {
        let found = AbiVersion::parse("1.0");

        assert_eq!(found, None);
    }

    #[test]
    fn abi_version_parse_with_a_minor_reads_both() {
        let found = AbiVersion::parse("0.12");

        assert_eq!(
            found,
            Some(AbiVersion {
                major: 0,
                minor: 12
            })
        );
    }

    #[test]
    fn abi_version_parse_with_a_zero_alone_returns_nothing() {
        let found = AbiVersion::parse("0");

        assert_eq!(found, None);
    }

    #[test]
    fn abi_version_to_string_from_one_onward_writes_one_integer() {
        let text = AbiVersion { major: 2, minor: 0 }.to_string();

        assert_eq!(text, "2");
    }

    #[test]
    fn latest_release_skips_a_ref_that_names_no_release() {
        let listing = "a\trefs/tags/stiles@next\nb\trefs/tags/stiles@0.2.0\n";

        let found = latest_release(listing);

        assert_eq!(found.as_deref(), Some("stiles@0.2.0"));
    }

    #[test]
    fn latest_release_with_a_ten_after_a_nine_compares_numbers() {
        let listing = "a\trefs/tags/stiles@0.10.0\nb\trefs/tags/stiles@0.9.0\n";

        let found = latest_release(listing);

        assert_eq!(found.as_deref(), Some("stiles@0.10.0"));
    }

    #[test]
    fn latest_release_with_no_tags_returns_nothing() {
        let found = latest_release("");

        assert_eq!(found, None);
    }

    #[test]
    fn release_version_cmp_puts_a_pre_release_before_its_release() {
        let order =
            ReleaseVersion::parse("stiles@0.1.0-rc.6").cmp(&ReleaseVersion::parse("stiles@0.1.0"));

        assert_eq!(order, Ordering::Less);
    }

    #[test]
    fn release_version_cmp_with_numeric_pre_releases_compares_numbers() {
        let order = ReleaseVersion::parse("stiles@0.1.0-rc.10")
            .cmp(&ReleaseVersion::parse("stiles@0.1.0-rc.9"));

        assert_eq!(order, Ordering::Greater);
    }

    #[test]
    fn release_version_parse_with_four_components_returns_nothing() {
        let found = ReleaseVersion::parse("stiles@0.1.0.0");

        assert_eq!(found, None);
    }

    #[test]
    fn release_version_parse_without_the_prefix_returns_nothing() {
        let found = ReleaseVersion::parse("v0.1.0-rc.5");

        assert_eq!(found, None);
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
    fn verdict_with_a_moved_fingerprint_after_one_and_the_next_integer_passes() {
        let found = verdict(TAG, &record("2", "a"), &record("3", "b"));

        assert_eq!(
            found,
            Verdict::Raised {
                before: AbiVersion { major: 2, minor: 0 },
                after: AbiVersion { major: 3, minor: 0 },
            }
        );
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_after_one_and_a_skipped_integer_refuses() {
        let found = verdict(TAG, &record("2", "a"), &record("4", "b"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_and_one_after_a_minor_passes() {
        let found = verdict(TAG, &record("0.12", "a"), &record("1", "b"));

        assert_eq!(
            found,
            Verdict::Raised {
                before: AbiVersion {
                    major: 0,
                    minor: 12
                },
                after: AbiVersion { major: 1, minor: 0 },
            }
        );
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_and_a_skipped_minor_refuses() {
        let found = verdict(TAG, &record("0.2", "a"), &record("0.4", "b"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_and_the_next_minor_passes() {
        let found = verdict(TAG, &record("0.2", "a"), &record("0.3", "b"));

        assert_eq!(
            found,
            Verdict::Raised {
                before: AbiVersion { major: 0, minor: 2 },
                after: AbiVersion { major: 0, minor: 3 },
            }
        );
    }

    /// The version names the release that comes next, so it is the same
    /// when the boundary moves again before that release
    #[test]
    fn verdict_with_a_moved_fingerprint_and_the_release_s_version_refuses() {
        let found = verdict(TAG, &record("0.2", "a"), &record("0.2", "b"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }

    #[test]
    fn verdict_with_a_moved_fingerprint_and_the_same_version_names_the_release() {
        let found = verdict(TAG, &record("0.2", "a"), &record("0.2", "b"));

        let Verdict::Refused { reason } = found else {
            panic!("the verdict should refuse, not {found:?}");
        };
        assert!(reason.contains("stiles@0.6.0"), "{reason}");
    }

    #[test]
    fn verdict_with_a_ten_after_a_nine_compares_numbers() {
        let found = verdict(TAG, &record("0.9", "a"), &record("0.10", "b"));

        assert!(matches!(found, Verdict::Raised { .. }));
    }

    /// The version guards the declaration and the pass trait too, which
    /// no fingerprint reads
    #[test]
    fn verdict_with_unmoved_fingerprints_and_the_next_version_passes() {
        let found = verdict(TAG, &record("0.2", "a"), &record("0.3", "a"));

        assert!(matches!(found, Verdict::Raised { .. }));
    }

    #[test]
    fn verdict_with_unmoved_fingerprints_and_a_lowered_version_refuses() {
        let found = verdict(TAG, &record("0.2", "a"), &record("0.1", "a"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }

    #[test]
    fn verdict_with_unmoved_fingerprints_and_a_skipped_version_refuses() {
        let found = verdict(TAG, &record("0.2", "a"), &record("0.4", "a"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }

    #[test]
    fn verdict_with_unmoved_fingerprints_and_the_same_version_passes() {
        let found = verdict(TAG, &record("0.2", "a"), &record("0.2", "a"));

        assert_eq!(found, Verdict::Unmoved);
    }

    #[test]
    fn verdict_with_an_unreadable_version_refuses() {
        let found = verdict(TAG, &record("0.2", "a"), &record("two", "b"));

        assert!(matches!(found, Verdict::Refused { .. }));
    }
}
