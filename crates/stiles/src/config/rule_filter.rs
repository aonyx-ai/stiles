use std::collections::BTreeSet;

use bon::bon;
use stiles_types::RuleId;

/// Which of a source's rules a project runs
///
/// A `[[lints]]` entry loads every rule its source provides, because the
/// libraries arrive as one unit and stiles cannot ask a plugin what it
/// reports. A project that wants some of them says so here, and stiles
/// drops the diagnostics of the rest.
///
/// A project adopting stiles names the rules it is ready for, and adds
/// to the list as it fixes what they find. A project that wants all but
/// a few names those few instead. Naming both is a contradiction rather
/// than a combination, and the configuration is refused.
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub enum RuleFilter {
    /// Every rule a source provides
    #[default]
    All,

    /// Only the rules named
    Only(BTreeSet<String>),

    /// Every rule except those named
    Except(BTreeSet<String>),
}

#[bon]
impl RuleFilter {
    /// Builds the filter that `enable` and `disable` describe
    ///
    /// # Errors
    ///
    /// Returns an error if both name something, because a project that
    /// says which rules run has already said which do not.
    #[builder]
    pub(crate) fn new(enable: Vec<String>, disable: Vec<String>) -> anyhow::Result<Self> {
        match (enable.is_empty(), disable.is_empty()) {
            (true, true) => Ok(Self::All),
            (false, true) => Ok(Self::Only(enable.into_iter().collect())),
            (true, false) => Ok(Self::Except(disable.into_iter().collect())),
            (false, false) => Err(anyhow::anyhow!(
                "[rules] names both enable and disable; naming the rules that run already \
                 says which do not"
            )),
        }
    }

    /// Reports whether a diagnostic from `rule` reaches the report
    pub fn admits(&self, rule: &RuleId) -> bool {
        match self {
            Self::All => true,
            Self::Only(named) => named.contains(rule.as_str()),
            Self::Except(named) => !named.contains(rule.as_str()),
        }
    }

    /// Reports the names this filter uses that no loaded rule declares
    ///
    /// # Errors
    ///
    /// Returns an error naming every one of them. A misspelled rule
    /// disables nothing and reads exactly like a rule that found no
    /// fault, so stiles refuses the file rather than the reader having
    /// to notice.
    pub(crate) fn validate(&self, declared: &BTreeSet<String>) -> anyhow::Result<()> {
        let unknown: Vec<&str> = self
            .named()
            .iter()
            .filter(|name| !declared.contains(*name))
            .map(String::as_str)
            .collect();

        anyhow::ensure!(
            unknown.is_empty(),
            "[rules] names {}, which no configured lint reports; the lints loaded here report {}",
            unknown.join(", "),
            match declared.is_empty() {
                true => "nothing".to_owned(),
                false => declared
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
            }
        );

        Ok(())
    }

    /// Returns the rules this filter names, which a run may never see
    ///
    /// A name that no loaded rule reports is worth saying out loud. It is
    /// a typo, or a rule that reports nothing here, and stiles cannot
    /// tell those apart: a plugin declares no rule ids, so the only ones
    /// stiles learns are the ones it is handed in a diagnostic.
    pub fn named(&self) -> &BTreeSet<String> {
        static NONE: BTreeSet<String> = BTreeSet::new();

        match self {
            Self::All => &NONE,
            Self::Only(named) | Self::Except(named) => named,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &'static str) -> RuleId {
        RuleId::new(id)
    }

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn admits_everything_but_a_disabled_rule() {
        let filter = RuleFilter::builder()
            .enable(Vec::new())
            .disable(names(&["lint.noisy"]))
            .build()
            .expect("should build");

        let admitted = ["lint.noisy", "lint.other"].map(|name| filter.admits(&rule(name)));

        assert_eq!(admitted, [false, true]);
    }

    #[test]
    fn admits_everything_when_nothing_is_named() {
        let filter = RuleFilter::builder()
            .enable(Vec::new())
            .disable(Vec::new())
            .build()
            .expect("should build");

        assert!(filter.admits(&rule("lint.anything")));
    }

    #[test]
    fn admits_only_an_enabled_rule() {
        let filter = RuleFilter::builder()
            .enable(names(&["lint.wanted"]))
            .disable(Vec::new())
            .build()
            .expect("should build");

        let admitted = ["lint.wanted", "lint.other"].map(|name| filter.admits(&rule(name)));

        assert_eq!(admitted, [true, false]);
    }

    #[test]
    fn named_returns_nothing_for_an_unfiltered_project() {
        let filter = RuleFilter::builder()
            .enable(Vec::new())
            .disable(Vec::new())
            .build()
            .expect("should build");

        assert!(filter.named().is_empty());
    }

    #[test]
    fn named_returns_the_rules_a_project_wrote() {
        let filter = RuleFilter::builder()
            .enable(Vec::new())
            .disable(names(&["lint.noisy"]))
            .build()
            .expect("should build");

        let named = filter.named();

        assert_eq!(named, &BTreeSet::from(["lint.noisy".to_owned()]));
    }

    #[test]
    fn new_with_both_lists_returns_error() {
        let error = RuleFilter::builder()
            .enable(names(&["lint.a"]))
            .disable(names(&["lint.b"]))
            .build()
            .expect_err("should fail");

        assert!(format!("{error:#}").contains("both"), "{error:#}");
    }

    /// Returns the error of a filter that disables `lint.typo` in a run
    /// whose lints declare only `lint.known`
    fn undeclared_name_error() -> String {
        let filter = RuleFilter::builder()
            .enable(Vec::new())
            .disable(names(&["lint.typo"]))
            .build()
            .expect("should build");
        let declared = BTreeSet::from(["lint.known".to_owned()]);

        let error = filter.validate(&declared).expect_err("should fail");

        format!("{error:#}")
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RuleFilter>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RuleFilter>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RuleFilter>();
    }

    #[test]
    fn validate_accepts_a_name_a_lint_declares() {
        let filter = RuleFilter::builder()
            .enable(Vec::new())
            .disable(names(&["lint.known"]))
            .build()
            .expect("should build");
        let declared = BTreeSet::from(["lint.known".to_owned()]);

        filter.validate(&declared).expect("should accept");
    }

    #[test]
    fn validate_accepts_an_unfiltered_project() {
        let filter = RuleFilter::builder()
            .enable(Vec::new())
            .disable(Vec::new())
            .build()
            .expect("should build");

        filter.validate(&BTreeSet::new()).expect("should accept");
    }

    #[test]
    fn validate_names_the_declared_rules() {
        let error = undeclared_name_error();

        assert!(error.contains("lint.known"), "{error}");
    }

    #[test]
    fn validate_refuses_a_name_no_lint_declares() {
        let error = undeclared_name_error();

        assert!(error.contains("lint.typo"), "{error}");
    }
}
