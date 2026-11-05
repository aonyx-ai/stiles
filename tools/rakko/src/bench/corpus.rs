use std::fs;
use std::path::Path;

use rakko_tool::{Execution, Invocation};
use serde::{Deserialize, Serialize};

use crate::{HarnessError, step};

/// The configuration of stiles in this repository
///
/// The corpus runs the rules that this repository pins for itself. That
/// pin always builds against the working tree, because `check-self` would
/// fail otherwise. A pin of its own would break the benchmarks at every
/// change of the plugin interface.
pub(super) const CONFIG: &str = ".config/stiles.toml";

/// The projects that the benchmarks check, each pinned to one commit
///
/// A pin makes a change in a number a change in stiles. Each project covers
/// one way in which a run can scale. The repositories run without
/// `node_modules`, so a TypeScript import of a dependency resolves to
/// nothing, and the checker types less than it would after an install.
pub(super) const CORPUS: [Project; 4] = [
    Project {
        name: "stiles",
        remote: "https://github.com/aonyx-ai/stiles",
        rev: "ced4261a34968775357a6b7724cff5914b473add",
        path: ".",
        ignore: &[
            "/crates/stiles-tsrs/",
            "/docs/",
            "/examples/",
            "/tools/rakko/",
            "/vendor/",
            "crates/stiles-rust/tests/fixtures/",
        ],
    },
    Project {
        name: "rust-analyzer",
        remote: "https://github.com/rust-lang/rust-analyzer",
        rev: "fde5133f7d116dd81f33f71bd0d389c768c7bd58",
        path: ".",
        ignore: &["test_data/"],
    },
    Project {
        name: "typescript",
        remote: "https://github.com/microsoft/TypeScript",
        rev: "c63de15a992d37f0d6cec03ac7631872838602cb",
        path: "src/compiler",
        ignore: &[],
    },
    Project {
        name: "trpc",
        remote: "https://github.com/trpc/trpc",
        rev: "c561cd311fd1af3289fe597ec85530fa7396fe02",
        path: "packages",
        ignore: &[],
    },
];

/// One repository of the corpus, at one commit
///
/// - The stiles repository at 0.4.0 is the small and fast baseline, with
///   the ignore patterns of its own configuration.
/// - rust-analyzer is a large Rust workspace, whose load runs build scripts
///   and resolves many crates. Its `test_data` directories hold Rust files
///   that no crate reaches.
/// - The compiler of TypeScript 5.9.3 is a large TypeScript project with a
///   single `tsconfig.json`. Later versions of that repository hold the Go
///   port instead.
/// - tRPC is a monorepo with one `tsconfig.json` per package, which
///   exercises the program that stiles builds per project.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(super) struct Project {
    /// The name that the results and the `project` argument use
    pub(super) name: &'static str,
    /// The repository that holds the project
    remote: &'static str,
    /// The commit that the benchmarks check
    rev: &'static str,
    /// The directory that `stiles check` receives, relative to the checkout
    pub(super) path: &'static str,
    /// The ignore patterns that the configuration of the checkout carries
    ignore: &'static [&'static str],
}

/// The lint source that the configuration of this repository pins
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub(super) struct RulesPin {
    /// The repository of the rules
    git: String,
    /// The commit of the rules
    rev: String,
}

/// The part of the configuration of stiles that the corpus reads
#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
struct ProjectConfig {
    /// The lint sources of the configuration
    #[serde(default)]
    lints: Vec<LintEntry>,
}

/// One lint source of the configuration of stiles
#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
struct LintEntry {
    /// The repository of a git source
    git: Option<String>,
    /// The commit of a git source
    rev: Option<String>,
}

/// The configuration that a checkout of the corpus receives
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
struct CorpusConfig<'a> {
    /// The ignore patterns of the project
    ignore: &'a [&'a str],
    /// The stiles that the rules build against: this repository
    #[serde(rename = "stiles-source")]
    stiles_source: &'a Path,
    /// The one lint source of the corpus
    lints: [CorpusLint<'a>; 1],
}

/// The lint source that a checkout of the corpus receives
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
struct CorpusLint<'a> {
    /// The repository of the rules
    git: &'a str,
    /// The commit of the rules
    rev: &'a str,
}

impl Project {
    /// Returns the project of the corpus that has the name
    pub(super) fn named(name: &str) -> Option<Self> {
        CORPUS.into_iter().find(|project| project.name == name)
    }

    /// Fetches the pinned commit into `checkout`, and checks it out
    ///
    /// A checkout that is already at the commit stays as it is, so a second
    /// run of the benchmarks reaches no network for it. The fetch asks for
    /// the one commit at depth one. The checkout is forced, because the run
    /// writes the configuration of stiles into the tree, and a project such
    /// as stiles tracks a file at that path.
    ///
    /// The answer is the command of git that failed, if one did.
    ///
    /// # Errors
    ///
    /// Returns an error when git does not start, or when the directory of
    /// the checkout cannot be created.
    pub(super) async fn fetch(&self, checkout: &Path) -> Result<Option<Execution>, HarnessError> {
        let git = || Invocation::new("git").in_directory(checkout);

        if !checkout.join(".git").is_dir() {
            fs::create_dir_all(checkout).map_err(|source| HarnessError::Unwritable {
                path: checkout.to_path_buf(),
                source,
            })?;
            let init = step::run(git().args(["init", "--quiet"])).await?;
            if !init.status().success() {
                return Ok(Some(init));
            }
        }

        let head = step::run(git().args(["rev-parse", "HEAD"])).await?;
        if head.status().success() && head.stdout().to_string_lossy().trim() == self.rev {
            return Ok(None);
        }

        let fetch =
            step::run(git().args(["fetch", "--quiet", "--depth", "1", self.remote, self.rev]))
                .await?;
        if !fetch.status().success() {
            return Ok(Some(fetch));
        }

        let detach =
            step::run(git().args(["checkout", "--quiet", "--force", "--detach", self.rev])).await?;
        if !detach.status().success() {
            return Ok(Some(detach));
        }

        Ok(None)
    }

    /// Writes the configuration of stiles into the checkout
    ///
    /// The configuration names the rules of `pin`, and builds them against
    /// the stiles at `stiles_source`. Otherwise the run could take prebuilt
    /// rules that another stiles built, and the handshake would refuse them
    /// whenever the plugin interface of the working tree moved.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be written.
    pub(super) fn configure(
        &self,
        checkout: &Path,
        stiles_source: &Path,
        pin: &RulesPin,
    ) -> Result<(), HarnessError> {
        let path = checkout.join(CONFIG);
        let text =
            self.config(stiles_source, pin)
                .map_err(|source| HarnessError::UnserializableToml {
                    path: path.clone(),
                    source,
                })?;

        let directory = checkout.join(".config");
        fs::create_dir_all(&directory).map_err(|source| HarnessError::Unwritable {
            path: directory,
            source,
        })?;

        fs::write(&path, text).map_err(|source| HarnessError::Unwritable { path, source })
    }

    /// Returns the text of the configuration that the checkout receives
    fn config(&self, stiles_source: &Path, pin: &RulesPin) -> Result<String, toml::ser::Error> {
        let config = CorpusConfig {
            ignore: self.ignore,
            stiles_source,
            lints: [CorpusLint {
                git: &pin.git,
                rev: &pin.rev,
            }],
        };

        toml::to_string(&config)
    }
}

impl RulesPin {
    /// Reads the git lint source that the configuration of the root pins
    ///
    /// The answer is `None` when the configuration names no git source with
    /// a commit.
    ///
    /// # Errors
    ///
    /// Returns an error when the configuration cannot be read or parsed.
    pub(super) fn load(root: &Path) -> Result<Option<Self>, HarnessError> {
        let path = root.join(CONFIG);
        let text = fs::read_to_string(&path).map_err(|source| HarnessError::Unreadable {
            path: path.clone(),
            source,
        })?;

        Self::parse(&text).map_err(|source| HarnessError::UnparsableToml { path, source })
    }

    /// Returns the first git lint source with a commit in a configuration
    fn parse(text: &str) -> Result<Option<Self>, toml::de::Error> {
        let ProjectConfig { lints } = toml::from_str(text)?;

        Ok(lints.into_iter().find_map(|LintEntry { git, rev }| {
            Some(Self {
                git: git?,
                rev: rev?,
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    // An assertion in a test panics by design. A `# Panics` section on every
    // test would repeat that and give the reader no information.
    #![allow(clippy::missing_panics_doc, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn config_names_the_rules_and_the_stiles_source() {
        let project = Project::named("trpc").unwrap();
        let pin = RulesPin {
            git: "https://example.com/rules".to_owned(),
            rev: "abc".to_owned(),
        };

        let text = project.config(Path::new("/work/stiles"), &pin).unwrap();

        assert_eq!(
            text,
            "ignore = []\nstiles-source = \"/work/stiles\"\n\n[[lints]]\ngit = \
             \"https://example.com/rules\"\nrev = \"abc\"\n"
        );
    }

    #[test]
    fn load_with_the_configuration_of_this_repository_finds_a_pin() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

        let pin = RulesPin::load(&root).unwrap();

        assert!(pin.is_some());
    }

    #[test]
    fn named_with_an_unknown_name_returns_none() {
        let project = Project::named("linux");

        assert_eq!(project, None);
    }

    #[test]
    fn parse_skips_a_path_source() {
        let text = "[[lints]]\npath = \"rules\"\n\n[[lints]]\ngit = \"https://example.com/rules\"\nrev = \"abc\"\n";

        let pin = RulesPin::parse(text).unwrap();

        assert_eq!(
            pin,
            Some(RulesPin {
                git: "https://example.com/rules".to_owned(),
                rev: "abc".to_owned(),
            })
        );
    }

    #[test]
    fn parse_without_a_git_source_returns_none() {
        let text = "ignore = [\"/docs/\"]\n";

        let pin = RulesPin::parse(text).unwrap();

        assert_eq!(pin, None);
    }

    #[test]
    fn trait_send_project() {
        fn assert_send<T: Send>() {}
        assert_send::<Project>();
    }

    #[test]
    fn trait_send_rules_pin() {
        fn assert_send<T: Send>() {}
        assert_send::<RulesPin>();
    }

    #[test]
    fn trait_sync_project() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Project>();
    }

    #[test]
    fn trait_sync_rules_pin() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RulesPin>();
    }

    #[test]
    fn trait_unpin_project() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Project>();
    }

    #[test]
    fn trait_unpin_rules_pin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RulesPin>();
    }
}
