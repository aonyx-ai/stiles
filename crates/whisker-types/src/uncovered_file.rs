use std::path::Path;
use std::sync::Arc;

use bon::Builder;

use crate::{CoverageGap, ProviderName};

/// Error returned when no decoration provider claims a file
///
/// A pipeline returns the error as the cause of its own, and a caller can
/// match it there to report each provider's reason in detail.
///
/// # Examples
///
/// ```
/// use std::path::{Path, PathBuf};
/// use std::sync::Arc;
///
/// use whisker_types::{CoverageGap, ProviderName, UncoveredFile};
///
/// let root: Arc<Path> = Arc::from(PathBuf::from("/ws"));
/// let error = UncoveredFile::builder()
///     .file(PathBuf::from("stray.rs"))
///     .gaps(vec![(ProviderName("rust"), CoverageGap::Unreachable { root })])
///     .build();
///
/// assert_eq!(
///     error.to_string(),
///     "no decoration provider covers this file, so semantic rules cannot run\n  \
///      rust: the file is inside /ws, but nothing the toolchain loaded reaches it",
/// );
/// ```
#[derive(Clone, Eq, PartialEq, Debug, Builder)]
pub struct UncoveredFile {
    /// The file that no provider claimed
    #[builder(into)]
    file: Arc<Path>,
    /// Each provider consulted, with its reason for declining
    ///
    /// An empty list means no providers were consulted, and the error
    /// renders its own message for that case.
    gaps: Vec<(ProviderName, CoverageGap)>,
}

impl UncoveredFile {
    /// Returns the file that could not be analyzed
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns each provider consulted and its reason for declining
    ///
    /// The slice is empty when no providers were configured.
    pub fn gaps(&self) -> &[(ProviderName, CoverageGap)] {
        &self.gaps
    }
}

impl std::fmt::Display for UncoveredFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.gaps.is_empty() {
            return f
                .write_str("no decoration providers were configured, so no file can be analyzed");
        }

        f.write_str("no decoration provider covers this file, so semantic rules cannot run")?;

        for (name, gap) in &self.gaps {
            write!(f, "\n  {name}: {gap}")?;
        }

        Ok(())
    }
}

impl std::error::Error for UncoveredFile {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn root() -> Arc<Path> {
        Arc::from(PathBuf::from("/ws"))
    }

    #[test]
    fn display_with_no_gaps_reports_no_providers_configured() {
        let error = UncoveredFile::builder()
            .file(PathBuf::from("stray.rs"))
            .gaps(Vec::new())
            .build();

        let message = error.to_string();

        assert_eq!(
            message,
            "no decoration providers were configured, so no file can be analyzed"
        );
    }

    #[test]
    fn display_with_one_gap_names_provider_and_reason() {
        let error = UncoveredFile::builder()
            .file(PathBuf::from("stray.rs"))
            .gaps(vec![(
                ProviderName("rust"),
                CoverageGap::Unreachable { root: root() },
            )])
            .build();

        let message = error.to_string();

        assert_eq!(
            message,
            "no decoration provider covers this file, so semantic rules cannot run\n  \
             rust: the file is inside /ws, but nothing the toolchain loaded reaches it"
        );
    }

    #[test]
    fn display_with_two_gaps_lists_both() {
        let error = UncoveredFile::builder()
            .file(PathBuf::from("stray.rs"))
            .gaps(vec![
                (ProviderName("rust"), CoverageGap::StaleSource),
                (
                    ProviderName("other"),
                    CoverageGap::OutsideRoot { root: root() },
                ),
            ])
            .build();

        let message = error.to_string();

        assert_eq!(
            message,
            "no decoration provider covers this file, so semantic rules cannot run\n  \
             rust: the text whisker parsed differs from the text the toolchain analyzed\n  \
             other: the file is outside /ws, the root the provider loaded"
        );
    }

    #[test]
    fn file_returns_path() {
        let error = UncoveredFile::builder()
            .file(PathBuf::from("a/stray.rs"))
            .gaps(Vec::new())
            .build();

        let file = error.file();

        assert_eq!(file, Path::new("a/stray.rs"));
    }

    #[test]
    fn gaps_returns_every_provider_consulted() {
        let error = UncoveredFile::builder()
            .file(PathBuf::from("stray.rs"))
            .gaps(vec![
                (ProviderName("rust"), CoverageGap::StaleSource),
                (
                    ProviderName("other"),
                    CoverageGap::OutsideRoot { root: root() },
                ),
            ])
            .build();

        let gaps = error.gaps();

        assert_eq!(
            gaps,
            [
                (ProviderName("rust"), CoverageGap::StaleSource),
                (
                    ProviderName("other"),
                    CoverageGap::OutsideRoot { root: root() },
                ),
            ]
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<UncoveredFile>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<UncoveredFile>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<UncoveredFile>();
    }
}
