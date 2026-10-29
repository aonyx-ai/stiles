//! The maintenance commands of this repository
//!
//! This binary is the harness of the Stiles repository: the one place that
//! states which maintenance actions run here. It mounts the bundles that this
//! repository uses, and the command line that it builds turns each of them
//! into a command. A bundle carries a set of actions that projects adopt
//! together, so the harness names the bundle instead of each action in it.
//!
//! Some checks of Stiles reach past what a bundle of Rakko covers, so the
//! library of this package defines them, and the harness mounts them next to
//! the bundles.
//!
//! It also mounts commands, for a maintenance activity that no single action
//! describes. A command comes from a crate, as an action does, and this file
//! names it. When a command runs actions, this file names those actions as
//! well and gives them to the command.
//!
//! Run it with `mise run rakko`, or with `rakko` where the environment
//! supplies the shortcut.

use harness::{
    BundleTsrs, CheckAbi, CheckBundleTsrs, CheckFeatures, CheckSelf, InDirectory,
    TestPluginToolchain,
};
use rakko_action::{ErasedAction, action_name};
use rakko_cli::ErasedGroup;
use rakko_format_json::FormatJson;
use rakko_format_markdown::FormatMarkdown;
use rakko_format_rust::FormatRust;
use rakko_format_toml::FormatToml;
use rakko_format_yaml::FormatYaml;
use rakko_lint_github_actions::LintGitHubActions;
use rakko_lint_markdown::LintMarkdown;
use rakko_lint_rust::LintRust;
use rakko_lint_yaml::LintYaml;
use rakko_pre_commit::PreCommit;
use rakko_test_rust::TestRust;
use rakko_test_rust_docs::TestRustDocs;

/// The lint plugin that the tests of stiles-rust load
///
/// The root workspace excludes the package, so the Rust actions of Rakko do
/// not reach it, and the harness runs them on its directory instead.
const DECORATION_PROBES: &str = "crates/stiles-rust/tests/fixtures/lints/decoration_probes";

/// Builds the command line of this repository and runs it
///
/// The call ends the process itself, so a harness stays a `main` that names
/// what the repository mounts and returns nothing.
///
/// Stiles publishes binaries and no crate, so the harness mounts the binary
/// bundle and not the library bundle. The binary bundle leaves out the tests
/// of the documentation examples, which the crates of this repository carry,
/// so the harness mounts that action on its own.
fn main() {
    let pre_commit = PreCommit::new(actions_that_write().into_iter().chain(actions_that_read()));

    rakko_cli::builder()
        .mount(rakko_baseline::bundle())
        .mount(rakko_rust_binary::bundle())
        .mount(stiles())
        .mount_groups([Box::new(pre_commit) as Box<dyn ErasedGroup>])
        .run();
}

/// Returns the actions that only this repository mounts
fn stiles() -> Vec<Box<dyn ErasedAction>> {
    vec![
        Box::new(BundleTsrs),
        Box::new(CheckAbi),
        Box::new(CheckBundleTsrs),
        Box::new(CheckFeatures),
        Box::new(CheckSelf),
        format_fixture_lint(),
        test_fixture_lint(),
        Box::new(TestPluginToolchain),
        Box::new(TestRustDocs),
    ]
}

/// Returns the action that formats the lint plugin of the tests
fn format_fixture_lint() -> Box<dyn ErasedAction> {
    Box::new(InDirectory::new(
        action_name!("format-fixture-lint"),
        DECORATION_PROBES,
        FormatRust,
    ))
}

/// Returns the action that runs the tests of the lint plugin of the tests
fn test_fixture_lint() -> Box<dyn ErasedAction> {
    Box::new(InDirectory::new(
        action_name!("test-fixture-lint"),
        DECORATION_PROBES,
        TestRust,
    ))
}

/// Returns the actions that write to the tree before a commit, in their order
///
/// The order is the one in which this repository formatted before the harness
/// existed. The formatters overlap in the files that they write, so the last
/// formatter that writes a file decides its content.
fn actions_that_write() -> Vec<Box<dyn ErasedAction>> {
    vec![
        Box::new(FormatJson),
        Box::new(FormatMarkdown),
        Box::new(FormatYaml),
        Box::new(FormatToml),
        Box::new(FormatRust),
        format_fixture_lint(),
    ]
}

/// Returns the actions that only read the tree before a commit, in their order
///
/// The list is shorter than the list that the harness mounts. An action such
/// as `check-latest-deps`, which resolves the dependencies of the project
/// again, belongs in CI and not in front of every commit. The run of stiles
/// on this repository comes last, as it did before the harness existed.
fn actions_that_read() -> Vec<Box<dyn ErasedAction>> {
    vec![
        Box::new(LintGitHubActions),
        Box::new(LintMarkdown),
        Box::new(LintRust),
        Box::new(LintYaml),
        Box::new(TestRust),
        Box::new(TestRustDocs),
        Box::new(CheckSelf),
    ]
}
