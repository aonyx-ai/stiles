use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use rakko_action::{Access, Action, Context, Name, Outcome, action_name};
use rakko_cargo::{Channel, Toolchain};
use rakko_tool::Invocation;

use crate::{HarnessError, step};

/// The channel that builds the plugin
///
/// Mise pins a stable toolchain as the default, which builds stiles, and a
/// dated nightly beside it. The nightly is the other compiler that the run
/// needs.
const NIGHTLY: &str = "nightly";

/// The rule that the example plugin reports on the scratch project
const RULE: &str = "custom.no-todo";

/// The action that loads a plugin that another toolchain built
///
/// This is the proof behind the plugin boundary: every type that crosses it
/// is laid out by stabby, so the compiler that built a plugin does not
/// matter. The run builds stiles with the default toolchain of the project,
/// and then runs it with the pinned nightly selected. Stiles passes its
/// environment to the cargo that it starts, so the nightly builds the example
/// plugin. The handshake either loads the library or ends the run, and the
/// diagnostic that the rule of the example reports on a scratch project
/// proves that the library ran.
#[derive(Copy, Clone, Debug, Default)]
pub struct TestPluginToolchain;

impl Action for TestPluginToolchain {
    type Args = ();

    fn name(&self) -> Name {
        action_name!("test-plugin-toolchain")
    }

    fn access(&self, _context: &Context, _args: &Self::Args) -> Access {
        Access::Read
    }

    async fn run(&self, context: &Context, _args: &Self::Args) -> Outcome {
        step::outcome(drive(context).await)
    }
}

/// A scratch project that the run removes again when it ends
///
/// The project lives outside the repository, because cargo would otherwise
/// find the workspace of the repository above it and refuse a package that
/// the workspace does not list.
#[derive(Debug)]
struct Scratch {
    /// The directory of the project
    directory: PathBuf,
}

impl Scratch {
    /// Writes a project whose only function calls `todo!`
    ///
    /// The configuration of stiles in the project names the example plugin
    /// of this repository as its only lint source.
    ///
    /// # Errors
    ///
    /// Returns [`Unwritable`][unwritable] when a file of the project cannot
    /// be written.
    ///
    /// [unwritable]: HarnessError::Unwritable
    fn create(plugin: &Path) -> Result<Self, HarnessError> {
        let scratch = Self {
            directory: std::env::temp_dir()
                .join(format!("stiles-plugin-toolchain-{}", process::id())),
        };

        let files = [
            (
                "Cargo.toml",
                "[package]\nname = \"toolchain_probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"
                    .to_owned(),
            ),
            (
                "src/lib.rs",
                "pub fn pending() {\n    todo!()\n}\n".to_owned(),
            ),
            (
                ".config/stiles.toml",
                format!("[[lints]]\npath = \"{}\"\n", plugin.display()),
            ),
        ];

        for (path, content) in files {
            let path = scratch.directory.join(path);
            let written = path
                .parent()
                .map_or(Ok(()), fs::create_dir_all)
                .and_then(|()| fs::write(&path, content));
            written.map_err(|source| HarnessError::Unwritable { path, source })?;
        }

        Ok(scratch)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

/// Runs the action against the project of the context
///
/// # Errors
///
/// Returns the error of the step that could not finish: the resolution of
/// cargo or the nightly toolchain, the build of stiles, the scratch
/// project, or the run of stiles.
async fn drive(context: &Context) -> Result<Outcome, HarnessError> {
    let cargo = step::cargo(context).await?;
    let nightly = Toolchain::resolve(Channel::new(NIGHTLY), context.root())
        .await
        .map_err(|source| HarnessError::UnresolvedToolchain { source })?;

    let build = step::run(cargo.args(["build", "-p", "stiles"])).await?;
    if !build.status().success() {
        return Ok(step::failed("building stiles", &build));
    }

    let root = context.root().get();
    let scratch = Scratch::create(&root.join("examples/custom_lint"))?;

    let check = step::run(
        Invocation::new(root.join("target/debug/stiles"))
            .arg("check")
            .arg(scratch.directory.as_path())
            .env("RUSTUP_TOOLCHAIN", nightly.get())
            .in_directory(root),
    )
    .await?;
    if !check.status().success() {
        return Ok(step::failed(
            "loading a plugin that the nightly toolchain built",
            &check,
        ));
    }

    if !check.stdout().to_string_lossy().contains(RULE)
        && !check.stderr().to_string_lossy().contains(RULE)
    {
        return Ok(step::failed(
            &format!("finding a diagnostic of {RULE}"),
            &check,
        ));
    }

    Ok(Outcome::Passed { summary: None })
}
