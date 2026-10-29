use bon::Builder;

use crate::config::{GitRev, GitUrl, LintPath};

/// Where one configured set of custom lints comes from
///
/// A project either keeps its lints beside its code or shares them with
/// other projects through a repository. The two arrive at the same place: a
/// directory of cargo packages that stiles builds and loads. They differ
/// only in how that directory comes to exist. A path is already there, and
/// a git source is fetched into the cache first.
///
/// # Examples
///
/// ```ignore
/// let source = LintSource::Path(LintPath::new("lints/no_todo"));
///
/// assert_eq!(source.to_string(), "lints/no_todo");
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub(crate) enum LintSource {
    /// Lints in a directory of the project or of the machine
    Path(LintPath),

    /// Lints in a repository, pinned to one commit
    Git(GitLintSource),
}

impl std::fmt::Display for LintSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(path) => path.fmt(f),
            Self::Git(git) => git.fmt(f),
        }
    }
}

/// A repository and the commit stiles checks its lints out at
///
/// Both halves are required, and the [`GitRev`] is a full hash, so the
/// source names exactly one tree. That is what lets stiles treat a checkout
/// as permanent once it exists. The cache never has to ask whether the
/// remote moved, because this source cannot follow it.
///
/// # Examples
///
/// ```ignore
/// let source = GitLintSource::builder()
///     .url(GitUrl::new("https://github.com/aonyx-ai/stiles-aonyx-rules")?)
///     .rev(GitRev::new("0123456789abcdef0123456789abcdef01234567")?)
///     .build();
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Builder)]
pub(crate) struct GitLintSource {
    url: GitUrl,
    rev: GitRev,
}

impl GitLintSource {
    /// Returns the remote the lints are fetched from
    ///
    /// # Examples
    ///
    /// ```ignore
    /// println!("{}", source.url());
    /// ```
    pub(crate) fn url(&self) -> &GitUrl {
        &self.url
    }

    /// Returns the commit the lints are checked out at
    ///
    /// # Examples
    ///
    /// ```ignore
    /// println!("{}", source.rev());
    /// ```
    pub(crate) fn rev(&self) -> &GitRev {
        &self.rev
    }
}

impl std::fmt::Display for GitLintSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}", self.url, self.rev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REV: &str = "0123456789abcdef0123456789abcdef01234567";

    fn git_source() -> GitLintSource {
        GitLintSource::builder()
            .url(GitUrl::new("https://example.com/rules").expect("the remote should be accepted"))
            .rev(GitRev::new(REV).expect("the revision should be accepted"))
            .build()
    }

    #[test]
    fn display_of_a_git_source_names_the_remote_and_the_commit() {
        let source = LintSource::Git(git_source());

        let displayed = source.to_string();

        assert_eq!(displayed, format!("https://example.com/rules at {REV}"));
    }

    #[test]
    fn display_of_a_path_source_matches_the_path() {
        let source = LintSource::Path(LintPath::new("lints/no_todo"));

        assert_eq!(source.to_string(), "lints/no_todo");
    }

    #[test]
    fn rev_returns_the_configured_commit() {
        let source = git_source();

        assert_eq!(source.rev().as_str(), REV);
    }

    #[test]
    fn trait_send_git_lint_source() {
        fn assert_send<T: Send>() {}
        assert_send::<GitLintSource>();
    }

    #[test]
    fn trait_send_lint_source() {
        fn assert_send<T: Send>() {}
        assert_send::<LintSource>();
    }

    #[test]
    fn trait_sync_git_lint_source() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GitLintSource>();
    }

    #[test]
    fn trait_sync_lint_source() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LintSource>();
    }

    #[test]
    fn trait_unpin_git_lint_source() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<GitLintSource>();
    }

    #[test]
    fn trait_unpin_lint_source() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LintSource>();
    }

    #[test]
    fn url_returns_the_configured_remote() {
        let source = git_source();

        assert_eq!(source.url().to_string(), "https://example.com/rules");
    }
}
