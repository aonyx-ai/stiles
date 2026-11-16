use std::path::PathBuf;

use thiserror::Error;

use crate::Diagnostic;

/// The reason a [`Fix`] cannot be attached to a [`Diagnostic`]
///
/// Each variant is a mistake in the rule that built the diagnostic. Each
/// variant also holds the diagnostic, as it was before the attach. A rule
/// therefore reports the finding without the fix, as [`into_diagnostic`]
/// shows.
///
/// The diagnostic sits in a [`Box`], because it is large and an error
/// travels by value.
///
/// [`Fix`]: crate::Fix
/// [`into_diagnostic`]: AttachFixError::into_diagnostic
///
/// # Examples
///
/// ```
/// use stiles_types::{AttachFixError, Diagnostic, Edit, Fix, RuleId, Severity, Span};
///
/// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
/// let diagnostic = Diagnostic::builder()
///     .rule_id(RuleId::new("lint.example"))
///     .severity(Severity::Warn)
///     .message("a wildcard arm")
///     .span(span)
///     .build();
/// let elsewhere = Span::builder().file("src/main.rs").start(0).end(1).build();
/// let fix = Fix::builder().message("rename it").edits(vec![Edit::delete(elsewhere)]).build()?;
///
/// let error = diagnostic.with_fix(fix).expect_err("the fix edits another file");
///
/// assert_eq!(
///     error.to_string(),
///     "the fix edits src/main.rs, but the diagnostic reports on src/lib.rs"
/// );
/// # Ok::<(), stiles_types::BuildFixError>(())
/// ```
#[derive(Clone, Debug, Error)]
pub enum AttachFixError {
    /// The fix edits another file than the one the diagnostic reports on
    #[error(
        "the fix edits {}, but the diagnostic reports on {}",
        fix.display(),
        diagnostic.span().file().display()
    )]
    OtherFile {
        /// The diagnostic, without the fix
        diagnostic: Box<Diagnostic>,
        /// The file that the fix edits
        fix: PathBuf,
    },

    /// The diagnostic already holds a fix
    #[error("the diagnostic already holds a fix")]
    SecondFix {
        /// The diagnostic, with the fix it held before
        diagnostic: Box<Diagnostic>,
    },
}

impl AttachFixError {
    /// Returns the diagnostic that the fix did not attach to
    ///
    /// The diagnostic is unchanged. A rule reports it without the fix, so a
    /// bug in the fix does not hide the finding.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{AttachFixError, Diagnostic, Edit, Fix, RuleId, Severity, Span};
    ///
    /// let span = Span::builder().file("src/lib.rs").start(3).end(7).build();
    /// let diagnostic = Diagnostic::builder()
    ///     .rule_id(RuleId::new("lint.example"))
    ///     .severity(Severity::Warn)
    ///     .message("a wildcard arm")
    ///     .span(span)
    ///     .build();
    /// let elsewhere = Span::builder().file("src/main.rs").start(0).end(1).build();
    /// let fix = Fix::builder().message("rename it").edits(vec![Edit::delete(elsewhere)]).build()?;
    ///
    /// let diagnostic = diagnostic.with_fix(fix).unwrap_or_else(AttachFixError::into_diagnostic);
    ///
    /// assert!(diagnostic.fix().is_none());
    /// # Ok::<(), stiles_types::BuildFixError>(())
    /// ```
    pub fn into_diagnostic(self) -> Diagnostic {
        match self {
            Self::OtherFile { diagnostic, .. } | Self::SecondFix { diagnostic } => *diagnostic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RuleId, Severity, Span};

    fn diagnostic() -> Diagnostic {
        Diagnostic::builder()
            .rule_id(RuleId::new("lint.test"))
            .severity(Severity::Warn)
            .message("test message")
            .span(Span::builder().file("a.rs").start(0).end(1).build())
            .build()
    }

    #[test]
    fn display_of_other_file_names_both_files() {
        let error = AttachFixError::OtherFile {
            diagnostic: Box::new(diagnostic()),
            fix: PathBuf::from("b.rs"),
        };

        let message = error.to_string();

        assert_eq!(
            message,
            "the fix edits b.rs, but the diagnostic reports on a.rs"
        );
    }

    #[test]
    fn into_diagnostic_of_other_file_returns_the_diagnostic() {
        let error = AttachFixError::OtherFile {
            diagnostic: Box::new(diagnostic()),
            fix: PathBuf::from("b.rs"),
        };

        let returned = error.into_diagnostic();

        assert_eq!(returned.message(), "test message");
    }

    #[test]
    fn into_diagnostic_of_second_fix_returns_the_diagnostic() {
        let error = AttachFixError::SecondFix {
            diagnostic: Box::new(diagnostic()),
        };

        let returned = error.into_diagnostic();

        assert_eq!(returned.message(), "test message");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AttachFixError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AttachFixError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AttachFixError>();
    }
}
