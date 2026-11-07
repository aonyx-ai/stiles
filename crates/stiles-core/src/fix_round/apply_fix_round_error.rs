use std::ops::Range;
use std::path::PathBuf;

use stiles_types::RuleId;
use thiserror::Error;

/// The reason a round of fixes cannot apply to the text of a file
///
/// A fix knows its spans but not the text, so it cannot check them when a
/// rule builds it. The round checks them against the text. Each variant is
/// a mistake in the rule that built the fix, and names that rule.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use stiles_core::ApplyFixRoundError;
/// use stiles_types::RuleId;
///
/// let error = ApplyFixRoundError::SplitsCharacter {
///     rule: RuleId::new("lint.example"),
///     file: PathBuf::from("src/lib.rs"),
///     offset: 7,
/// };
///
/// assert_eq!(
///     error.to_string(),
///     "the fix of `lint.example` edits src/lib.rs at byte 7, inside a character"
/// );
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug, Error)]
pub enum ApplyFixRoundError {
    /// An edit reaches past the end of the text
    #[error(
        "the fix of `{rule}` edits bytes {range:?} of {}, which holds {length} bytes",
        file.display()
    )]
    OutOfBounds {
        /// The rule that built the fix
        rule: RuleId,
        /// The file the fix edits
        file: PathBuf,
        /// The byte range of the edit
        range: Range<usize>,
        /// The length of the text in bytes
        length: usize,
    },

    /// An edit starts or ends inside a character
    #[error(
        "the fix of `{rule}` edits {} at byte {offset}, inside a character",
        file.display()
    )]
    SplitsCharacter {
        /// The rule that built the fix
        rule: RuleId,
        /// The file the fix edits
        file: PathBuf,
        /// The offset that is not a character boundary
        offset: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_of_out_of_bounds_names_the_length() {
        let error = ApplyFixRoundError::OutOfBounds {
            rule: RuleId::new("lint.example"),
            file: PathBuf::from("src/lib.rs"),
            range: 2..9,
            length: 3,
        };

        let message = error.to_string();

        assert_eq!(
            message,
            "the fix of `lint.example` edits bytes 2..9 of src/lib.rs, which holds 3 bytes"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ApplyFixRoundError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ApplyFixRoundError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ApplyFixRoundError>();
    }
}
