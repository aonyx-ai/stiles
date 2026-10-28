use std::fmt::Display;
use std::process::ExitCode;

use clawless::exit::Exit;

/// The text a command leaves for the standard error
///
/// A command reports its result through the presenter, which writes to the
/// standard output. Its errors and notices belong on the standard error,
/// out of the way of a caller that reads the result. A command never takes
/// that stream itself. It collects the lines here and returns them in its
/// [`Exit`], and `main` writes them after every other line of output.
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub(crate) struct ClosingText {
    text: String,
}

impl ClosingText {
    /// Appends `line` and ends it with a line break
    pub(crate) fn push(&mut self, line: impl Display) {
        self.text.push_str(&line.to_string());
        self.text.push('\n');
    }

    /// Appends every line of `lines`, each ending in a line break
    pub(crate) fn extend(&mut self, lines: impl IntoIterator<Item = impl Display>) {
        for line in lines {
            self.push(line);
        }
    }

    /// Returns the exit with `code` that writes the collected lines
    pub(crate) fn into_exit(self, code: ExitCode) -> Exit {
        match self.text.is_empty() {
            true => Exit::from(code),
            false => Exit::builder().code(code).text(self.text).build(),
        }
    }

    /// Returns the exit of a command that failed with `error`
    ///
    /// The collected lines come first, and the error last, in the words
    /// clawless uses for an error that a command returns.
    pub(crate) fn into_failure(self, error: &anyhow::Error) -> Exit {
        let failed = Exit::from_error(error);
        let text = format!("{}{}", self.text, failed.text().unwrap_or_default());

        Exit::builder().code(failed.code()).text(text).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extend_appends_every_line() {
        let mut closing = ClosingText::default();

        closing.extend(["warning: one", "note: two"]);
        let exit = closing.into_exit(ExitCode::SUCCESS);

        assert_eq!(exit.text(), Some("warning: one\nnote: two\n"));
    }

    #[test]
    fn into_exit_with_lines_writes_each_on_its_own_line() {
        let mut closing = ClosingText::default();
        closing.push("note: one");
        closing.push("error: two");

        let exit = closing.into_exit(ExitCode::FAILURE);

        assert_eq!(exit.text(), Some("note: one\nerror: two\n"));
    }

    #[test]
    fn into_exit_without_lines_writes_nothing() {
        let closing = ClosingText::default();

        let exit = closing.into_exit(ExitCode::SUCCESS);

        assert_eq!(exit, Exit::from(ExitCode::SUCCESS));
    }

    #[test]
    fn into_failure_writes_the_error_after_the_lines() {
        let mut closing = ClosingText::default();
        closing.push("note: one");

        let exit = closing.into_failure(&anyhow::anyhow!("the file is missing"));

        assert_eq!(exit.text(), Some("note: one\nError: the file is missing\n"));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ClosingText>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ClosingText>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ClosingText>();
    }
}
