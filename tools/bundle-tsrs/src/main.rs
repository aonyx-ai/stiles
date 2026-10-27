//! Generates one publishable crate from the crates of tsrs
//!
//! tsrs publishes no crates, and a crate that depends on code outside the
//! registry cannot publish. This tool copies the tsrs crates that whisker
//! needs into one crate, where each of them becomes a module. It changes
//! only the paths that name a crate, so every file reads as upstream wrote
//! it. A new tsrs commit is then a new run of this tool, never a merge.
//!
//! Run it through `mise run rakko -- bundle-tsrs`, which names the submodule,
//! the output directory, and the crates whisker uses.

mod manifest;
mod metadata;
mod rewrite;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;

use crate::metadata::Metadata;
use crate::metadata::Package;
use crate::rewrite::CrateName;
use crate::rewrite::Rewriter;

fn main() -> Result<()> {
    let arguments =
        Arguments::parse(std::env::args().skip(1)).context("failed to read the arguments")?;
    bundle(&arguments).context("failed to generate the bundle")
}

/// The commit of tsrs that a run bundles
#[derive(Clone, Eq, PartialEq, Debug)]
pub(crate) struct Revision(String);

impl fmt::Display for Revision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// What one run reads and writes
struct Arguments {
    /// The checkout of tsrs, at the commit to bundle
    source: PathBuf,
    /// The directory of the generated crate, which the run replaces
    out: PathBuf,
    /// The name of the generated crate
    name: CrateName,
    /// The tsrs crates whose public API the bundle must expose
    roots: Vec<String>,
}

impl Arguments {
    fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Self> {
        let usage = "usage: bundle-tsrs <source> <out> <name> <crate>...";
        let source = arguments.next().context(usage)?;
        let out = arguments.next().context(usage)?;
        let name = arguments.next().context(usage)?;
        let roots: Vec<String> = arguments.collect();
        ensure!(!roots.is_empty(), usage);

        Ok(Self {
            source: PathBuf::from(source),
            out: PathBuf::from(out),
            name: CrateName::new(&name),
            roots,
        })
    }
}

fn bundle(arguments: &Arguments) -> Result<()> {
    let Arguments {
        source,
        out,
        name,
        roots,
    } = arguments;

    let metadata = Metadata::read(source).context("failed to read the tsrs workspace")?;
    let packages = metadata
        .closure(roots)
        .context("failed to find the tsrs crates to bundle")?;
    let crates: BTreeSet<CrateName> = packages
        .iter()
        .map(|package| CrateName::new(package.name()))
        .collect();

    let mut rewriter = Rewriter::new(crates.clone());
    for package in &packages {
        let files = rust_files(&package.src_dir())
            .with_context(|| format!("failed to list the files of {}", package.name()))?;
        for file in files {
            let text = read(&file).context("failed to read a source file")?;
            rewriter
                .collect_exported_macros(&text)
                .with_context(|| format!("failed to lex {}", file.display()))?;
        }
    }
    rewriter
        .check_exported_macros()
        .context("failed to place the exported macros")?;

    let revision = revision(source).context("failed to read the commit of tsrs")?;

    if out.exists() {
        fs::remove_dir_all(out).with_context(|| format!("failed to remove {}", out.display()))?;
    }
    fs::create_dir_all(out.join("src"))
        .with_context(|| format!("failed to create {}", out.display()))?;

    let mut build_script = None;
    for package in &packages {
        package
            .check_layout()
            .with_context(|| format!("failed to load {}", package.name()))?;
        copy_package(&mut rewriter, package, out)
            .with_context(|| format!("failed to copy {}", package.name()))?;

        let Some(script) = package.build_script() else {
            continue;
        };
        if build_script.is_some() {
            bail!("more than one tsrs crate has a build script, and a crate has only one");
        }
        build_script = Some(script.to_path_buf());
    }

    let has_build_script = match &build_script {
        Some(script) => {
            fs::copy(script, out.join("build.rs"))
                .with_context(|| format!("failed to copy {}", script.display()))?;
            manifest::BuildScript::Present
        }
        None => manifest::BuildScript::Absent,
    };

    write(&out.join("src/lib.rs"), &root_module(&crates, &revision))
        .context("failed to write the root module")?;
    let manifest = manifest::render(&manifest::Bundle {
        name,
        revision: &revision,
        packages: &packages,
        crates: &crates,
        build_script: has_build_script,
    })
    .context("failed to render the manifest")?;
    write(&out.join("Cargo.toml"), &manifest).context("failed to write the manifest")?;
    write(&out.join("README.md"), &readme(name, &revision, &crates))
        .context("failed to write the README")?;
    for notice in ["LICENSE", "NOTICE"] {
        let from = source.join(notice);
        fs::copy(&from, out.join(notice))
            .with_context(|| format!("failed to copy {}", from.display()))?;
    }

    Ok(())
}

/// Copies the sources of one tsrs crate into the bundle, with its paths rewritten
///
/// The crate keeps its layout under a directory of its own name. A path that
/// a file names relative to itself, such as the argument of `include_str!`,
/// then still reaches the same file.
fn copy_package(rewriter: &mut Rewriter, package: &Package, out: &Path) -> Result<()> {
    let crate_dir = package.dir();
    let target_dir = out.join(package.name());
    let module = CrateName::new(package.name());
    let mut includes = BTreeMap::new();

    let files = rust_files(&package.src_dir()).context("failed to list the source files")?;
    for file in files {
        let text = read(&file).context("failed to read a source file")?;
        let rewritten = rewriter
            .rewrite(&text, &module)
            .with_context(|| format!("failed to rewrite {}", file.display()))?;

        let relative = file
            .strip_prefix(&crate_dir)
            .with_context(|| format!("{} is not inside {}", file.display(), crate_dir.display()))?;
        write(&target_dir.join(relative), rewritten.text())
            .context("failed to write a rewritten file")?;

        let directory = file.parent().context("a source file has no directory")?;
        for include in rewritten.includes() {
            let path = normalize(&directory.join(include));
            includes.insert(path, include.clone());
        }
    }

    for (path, include) in includes {
        if path.starts_with(package.src_dir()) {
            continue;
        }
        let relative = path
            .strip_prefix(&crate_dir)
            .with_context(|| format!("`{include}` reaches outside {}", crate_dir.display()))?;
        let target = target_dir.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(&path, &target).with_context(|| format!("failed to copy {}", path.display()))?;
    }

    Ok(())
}

/// Returns the module tree of the bundle, one module for each tsrs crate
fn root_module(crates: &BTreeSet<CrateName>, revision: &Revision) -> String {
    let mut text = format!(
        "//! The crates of [tsrs] at commit {revision}, one module each\n\
         //!\n\
         //! `tools/bundle-tsrs` generates this crate. Do not edit it: change\n\
         //! the tool or the submodule, and run `mise run rakko -- bundle-tsrs`.\n\
         //!\n\
         //! [tsrs]: https://github.com/oxc-project/tsrs\n\
         \n\
         // The code is upstream's, and its lints are upstream's to fix.\n\
         #![allow(warnings, clippy::all, clippy::pedantic, clippy::nursery)]\n"
    );
    for name in crates {
        text.push_str(&format!(
            "\n#[path = \"../{name}/src/lib.rs\"]\npub mod {name};\n"
        ));
    }
    text
}

fn readme(name: &CrateName, revision: &Revision, crates: &BTreeSet<CrateName>) -> String {
    let list: Vec<String> = crates.iter().map(|name| format!("- `{name}`")).collect();
    format!(
        "# {name}\n\
         \n\
         The TypeScript checker of [tsrs], repackaged as one crate.\n\
         \n\
         tsrs publishes no crates. This crate holds the tsrs crates that\n\
         whisker uses, each as a module of the same name, from commit\n\
         `{revision}`:\n\
         \n\
         {}\n\
         \n\
         A tool generates the crate, and the only change to the code is to the\n\
         paths that name a crate. The code is the work of the tsrs authors,\n\
         under the license and the notice beside this file.\n\
         \n\
         [tsrs]: https://github.com/oxc-project/tsrs\n",
        list.join("\n")
    )
}

fn revision(source: &Path) -> Result<Revision> {
    let output = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("failed to run git")?;
    ensure!(
        output.status.success(),
        "failed to read the commit of {}",
        source.display()
    );
    let revision = String::from_utf8(output.stdout).context("failed to read the commit")?;
    Ok(Revision(revision.trim().to_owned()))
}

/// Returns every `.rs` file under `dir`, in a stable order
fn rust_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries =
            fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .with_context(|| format!("failed to read {}", path.display()))?;
            if kind.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Resolves `.` and `..` in `path` without asking the file system
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::Normal(_) => normalized.push(component),
        }
    }
    normalized
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_parse_without_a_crate_fails() {
        let arguments = ["vendor/tsrs", "out", "bundle"].map(str::to_owned);

        let error = Arguments::parse(arguments.into_iter()).err();

        assert!(error.is_some());
    }

    #[test]
    fn normalize_resolves_parent_and_current_directories() {
        let path = Path::new("a/b/./c/../../libs/x.d.ts");

        let normalized = normalize(path);

        assert_eq!(normalized, PathBuf::from("a/libs/x.d.ts"));
    }

    #[test]
    fn trait_auto() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Arguments>();
        assert_auto::<Revision>();
    }
}
