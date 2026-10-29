use std::fmt;

use super::asset_name::AssetName;
use crate::config::GitLintSource;

/// What the reader hears about a lookup of prebuilt lints that found none
///
/// A lookup that finds nothing is never an error, because stiles then
/// compiles the source, which is what it did before prebuilt lints
/// existed. It still costs a compile of every rule in the source, on every
/// machine and every build agent. Those minutes look like stiles being
/// slow rather than like an absent archive. The lookup therefore hands a
/// notice to its caller, and the command shows it.
///
/// A repository stiles cannot see produces no notice. That is what a
/// private one looks like without a token, and nobody reading it can do
/// anything about it.
///
/// # Examples
///
/// ```ignore
/// let mut notices = Vec::new();
/// let directory = prebuilt::fetch(&source, &AbiTag::host(), &mut notices)?;
///
/// for notice in notices {
///     closing.push(notice);
/// }
/// ```
#[derive(Debug)]
pub(crate) enum PrebuiltNotice {
    /// The releases were listed, and nobody published the archive
    ///
    /// Nothing is wrong, and the check reports exactly what it would have
    /// reported anyway. Silence would read the same as a warm cache, so
    /// the one case a publisher can act on says so. The notice names the
    /// archive rather than describing it, so a publisher knows the exact file
    /// to produce. The name carries the commit and this stiles's ABI tag,
    /// which decide whether an archive is found.
    Unpublished {
        /// The lint source that stiles looked up
        lints: GitLintSource,
        /// The archive that nobody published
        name: AssetName,
    },

    /// The lookup failed, and it might succeed on the next run
    ///
    /// Nothing is broken. The check goes on and reports the same
    /// diagnostics, more slowly.
    Unusable {
        /// The lint source that stiles looked up
        lints: GitLintSource,
        /// What stopped the lookup
        error: anyhow::Error,
    },
}

impl fmt::Display for PrebuiltNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unpublished { lints, name } => write!(
                f,
                "note: no prebuilt lints published for {lints} as {name}; stiles builds them \
                 from source instead"
            ),
            Self::Unusable { lints, error } => write!(
                f,
                "warning: stiles cannot use the prebuilt lints for {lints}: {error:#}; it builds \
                 them from source instead"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GitRev, GitUrl};
    use crate::custom_lints::AbiTag;

    const REV: &str = "0123456789abcdef0123456789abcdef01234567";

    fn lints() -> GitLintSource {
        GitLintSource::builder()
            .url(GitUrl::new("https://example.com/rules").expect("the remote should be accepted"))
            .rev(GitRev::new(REV).expect("the revision should be accepted"))
            .build()
    }

    /// Returns the notice that no archive is published for [`lints`], and
    /// the name of the archive it looked for
    fn unpublished() -> (PrebuiltNotice, AssetName) {
        let lints = lints();
        let name = AssetName::builder()
            .rev(lints.rev())
            .tag(&AbiTag::host())
            .build();
        let notice = PrebuiltNotice::Unpublished {
            lints,
            name: name.clone(),
        };
        (notice, name)
    }

    #[test]
    fn display_of_unpublished_names_the_archive() {
        let (notice, name) = unpublished();

        let text = notice.to_string();

        assert!(text.contains(name.as_str()), "{text}");
    }

    #[test]
    fn display_of_unpublished_starts_with_a_note() {
        let (notice, _) = unpublished();

        let text = notice.to_string();

        assert!(
            text.starts_with("note: no prebuilt lints published for "),
            "{text}"
        );
    }

    #[test]
    fn display_of_unusable_names_the_cause() {
        let notice = PrebuiltNotice::Unusable {
            lints: lints(),
            error: anyhow::anyhow!("the digest does not match"),
        };

        let text = notice.to_string();

        assert!(text.contains("the digest does not match"), "{text}");
    }

    #[test]
    fn display_of_unusable_starts_with_a_warning() {
        let notice = PrebuiltNotice::Unusable {
            lints: lints(),
            error: anyhow::anyhow!("the digest does not match"),
        };

        let text = notice.to_string();

        assert!(
            text.starts_with("warning: stiles cannot use the prebuilt lints for "),
            "{text}"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PrebuiltNotice>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PrebuiltNotice>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PrebuiltNotice>();
    }
}
