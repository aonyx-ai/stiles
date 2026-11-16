/// The line break that a file writes at the end of a line
///
/// A fix that adds a line writes it with the line ending of the file, so
/// that a file with CRLF endings keeps them. [`FileText::line_ending`]
/// reads it.
///
/// [`FileText::line_ending`]: crate::FileText::line_ending
///
/// # Examples
///
/// ```
/// use stiles_types::LineEnding;
///
/// assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum LineEnding {
    /// A line feed alone, `\n`
    Lf,
    /// A carriage return and a line feed, `\r\n`
    CrLf,
}

impl LineEnding {
    /// Returns the text of the line break
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::LineEnding;
    ///
    /// assert_eq!(LineEnding::Lf.as_str(), "\n");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_of_crlf_returns_a_carriage_return_and_a_line_feed() {
        assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
    }

    #[test]
    fn as_str_of_lf_returns_a_line_feed() {
        assert_eq!(LineEnding::Lf.as_str(), "\n");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LineEnding>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LineEnding>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LineEnding>();
    }
}
