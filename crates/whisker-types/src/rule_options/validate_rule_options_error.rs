use thiserror::Error;

/// The refusal of a `[rules.options]` table that names unknown rules
///
/// An option set on a misspelled rule configures nothing, and it reads
/// exactly like a rule that took the option and found no fault. Whisker
/// therefore refuses the table and names every rule it does not know.
///
/// # Examples
///
/// ```
/// use whisker_types::ValidateRuleOptionsError;
///
/// let error = ValidateRuleOptionsError::UnknownRules {
///     unknown: vec!["lint.typo".to_owned()],
///     declared: Vec::new(),
/// };
///
/// assert!(error.to_string().ends_with("the lints loaded here report nothing"));
/// ```
#[derive(Clone, Eq, PartialEq, Debug, Error)]
pub enum ValidateRuleOptionsError {
    /// The table configures rules that no loaded lint declares
    #[error(
        "[rules.options] configures {}, which no configured lint reports; the lints loaded here \
         report {}",
        unknown.join(", "),
        list_or_nothing(declared)
    )]
    UnknownRules {
        /// The rules the table configures and no lint declares, by name
        unknown: Vec<String>,
        /// The rules that the loaded lints declare, by name
        declared: Vec<String>,
    },
}

/// Returns the names joined by commas, or "nothing" when there are none
fn list_or_nothing(names: &[String]) -> String {
    match names.is_empty() {
        true => "nothing".to_owned(),
        false => names.join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_the_unknown_and_the_declared_rules() {
        let error = ValidateRuleOptionsError::UnknownRules {
            unknown: vec!["lint.typo".to_owned()],
            declared: vec!["lint.a".to_owned(), "lint.b".to_owned()],
        };

        let message = error.to_string();

        assert_eq!(
            message,
            "[rules.options] configures lint.typo, which no configured lint reports; the lints \
             loaded here report lint.a, lint.b"
        );
    }

    #[test]
    fn list_or_nothing_without_names_says_nothing() {
        let names: Vec<String> = Vec::new();

        let listed = list_or_nothing(&names);

        assert_eq!(listed, "nothing");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ValidateRuleOptionsError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ValidateRuleOptionsError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ValidateRuleOptionsError>();
    }
}
