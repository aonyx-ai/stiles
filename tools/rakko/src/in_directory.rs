use std::path::Path;

use rakko_action::{
    Access, Action, Context, DirectoryPath, FilePath, Finding, Location, Name, Outcome, ProjectRoot,
};

/// An action that runs another action on one directory of the project
///
/// The Rust actions of Rakko check each workspace that cargo discovers, and
/// they leave out a package that a workspace excludes. The lint plugin that
/// the tests of stiles-rust load is such a package: stiles-rust depends on
/// it by path, so it cannot declare a workspace of its own, and the root
/// workspace excludes it so that it builds the way any plugin does. This
/// action points the inner action at the directory of that package instead,
/// as if it were a project of its own, and mounts it under a name of its own.
///
/// The inner action reports every path relative to the directory, so the
/// action puts the directory in front of each path that it reports, and every
/// path stays relative to the project root.
#[derive(Clone, Debug)]
pub struct InDirectory<A> {
    /// The name under which the harness mounts the action
    name: Name,
    /// The directory of the project that the inner action checks
    directory: &'static str,
    /// The action that runs on the directory
    action: A,
}

impl<A> InDirectory<A> {
    /// Returns an action that runs another action on one directory
    ///
    /// The directory is relative to the project root.
    pub fn new(name: Name, directory: &'static str, action: A) -> Self {
        Self {
            name,
            directory,
            action,
        }
    }

    /// Returns the context of the inner action
    fn context(&self, context: &Context) -> Context {
        let root = context.root().get().join(self.directory);

        Context::builder().root(ProjectRoot::new(root)).build()
    }
}

impl<A: Action> Action for InDirectory<A> {
    type Args = A::Args;

    fn name(&self) -> Name {
        self.name.clone()
    }

    fn access(&self, context: &Context, args: &Self::Args) -> Access {
        self.action.access(&self.context(context), args)
    }

    async fn run(&self, context: &Context, args: &Self::Args) -> Outcome {
        let outcome = self.action.run(&self.context(context), args).await;

        rebase(outcome, Path::new(self.directory))
    }
}

/// Returns an outcome whose paths start at the project root
fn rebase(outcome: Outcome, directory: &Path) -> Outcome {
    let rebase_all = |findings: Vec<Finding>| {
        findings
            .into_iter()
            .map(|finding| rebase_finding(finding, directory))
            .collect()
    };

    match outcome {
        Outcome::Changed { repairs } => Outcome::Changed {
            repairs: rebase_all(repairs),
        },
        Outcome::Failed { findings, repairs } => Outcome::Failed {
            findings: rebase_all(findings),
            repairs: rebase_all(repairs),
        },
        Outcome::Errored { source } => Outcome::Errored { source },
        Outcome::Passed { summary } => Outcome::Passed { summary },
        Outcome::Skipped { reason } => Outcome::Skipped { reason },
    }
}

/// Returns a finding whose path starts at the project root
///
/// A finding about the inner project as a whole is about the directory that
/// holds it, so the finding names that directory.
fn rebase_finding(finding: Finding, directory: &Path) -> Finding {
    let file = |path: &FilePath| {
        FilePath::try_from(directory.join(path.get())).unwrap_or_else(|_| path.clone())
    };
    let folder = |path: &Path| {
        DirectoryPath::try_from(directory.join(path))
            .ok()
            .map_or(Location::Project, |path| Location::Directory { path })
    };

    let location = match finding.location() {
        Location::Project => folder(Path::new("")),
        Location::Directory { path } => folder(path.get()),
        Location::File { path } => Location::File { path: file(path) },
        Location::Position { path, position } => Location::Position {
            path: file(path),
            position: *position,
        },
        Location::Span { path, span } => Location::Span {
            path: file(path),
            span: *span,
        },
    };

    Finding::builder()
        .message(finding.message().clone())
        .location(location)
        .build()
}

#[cfg(test)]
mod tests {
    // An assertion in a test panics by design. A `# Panics` section on every
    // test would repeat that and give the reader no information.
    #![allow(clippy::missing_panics_doc, clippy::unwrap_used)]

    use super::*;

    /// Returns a finding at a location
    fn finding(location: Location) -> Finding {
        Finding::builder()
            .message("not formatted")
            .location(location)
            .build()
    }

    #[test]
    fn rebase_finding_with_a_file_puts_the_directory_in_front() {
        let location = Location::File {
            path: FilePath::try_from("src/lib.rs").unwrap(),
        };

        let rebased = rebase_finding(finding(location), Path::new("fixtures/probes"));

        assert_eq!(
            rebased.location(),
            &Location::File {
                path: FilePath::try_from("fixtures/probes/src/lib.rs").unwrap(),
            }
        );
    }

    #[test]
    fn rebase_finding_with_the_project_names_the_directory() {
        let rebased = rebase_finding(finding(Location::Project), Path::new("fixtures/probes"));

        assert_eq!(
            rebased.location(),
            &Location::Directory {
                path: DirectoryPath::try_from("fixtures/probes").unwrap(),
            }
        );
    }

    #[test]
    fn rebase_finding_keeps_the_message() {
        let rebased = rebase_finding(finding(Location::Project), Path::new("fixtures/probes"));

        assert_eq!(rebased.message().get(), "not formatted");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<InDirectory<()>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<InDirectory<()>>();
    }
}
