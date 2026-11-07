use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use anyhow::Context as _;
use bon::bon;
use stiles_core::FixRound;
use stiles_types::{Diagnostic, RuleId, RuleOptions};

use super::file_checks::{FileCheck, FileChecks};
use super::providers::Providers;
use crate::config::RuleFilter;
use crate::custom_lints::CustomLints;

/// The most rounds of fixes that one run may take
///
/// A fix can produce code that another rule then fixes, so a run takes one
/// round for each link of such a chain. Real chains are short. A run that
/// needs more rounds than this shows a rule whose fixes do not settle. The
/// cap is therefore fixed, and no flag raises it.
const MAX_ROUNDS: usize = 10;

/// The checks of a run after `--fix`, and the text each fixed file holds
///
/// `stiles check --fix` runs in rounds. A round applies the fixes of every
/// file, as [`FixRound`] does for one file. The round then gives the
/// providers the changed text, and checks every file of the run again. The
/// rounds stop when a round changes nothing, and the checks of the run are
/// then the checks of the fixed text.
///
/// A round checks the files it did not change as well. A change to one file
/// can change what the providers know about another, such as the type of
/// an item it imports. A check of the changed files alone would report on
/// the other files from stale decorations. Each round therefore costs about
/// as much as a check without `--fix`.
///
/// Nothing reaches the disk here. The caller writes the fixed files, and
/// only when the run succeeds.
///
/// A rule whose fix undoes the fix of another rule would loop forever. The
/// rounds remember a hash of each text that a file held. A file that comes
/// back to an earlier text fails the run at once, and the error names the
/// rules whose fixes took it around. A file that keeps changing without a
/// repeat fails the run after [`MAX_ROUNDS`] rounds.
#[derive(Debug)]
pub(super) struct FixRounds {
    checks: Vec<FileCheck>,
    texts: HashMap<PathBuf, String>,
    fixed: usize,
}

/// The history of one file across the rounds
///
/// Each entry is the hash of a text the file held, and the rules whose
/// fixes turned that text into the next one.
#[derive(Clone, Eq, PartialEq, Debug)]
struct History {
    states: Vec<(u64, Vec<RuleId>)>,
}

impl History {
    /// Starts the history of a file that holds `text`
    fn new(text: &str) -> Self {
        Self {
            states: vec![(hash(text), Vec::new())],
        }
    }

    /// Records that `rules` turned the latest text into `text`
    ///
    /// Returns the rules that took the file around, when `text` is a text
    /// the file held before.
    fn record(&mut self, text: &str, rules: &[RuleId]) -> Option<BTreeSet<RuleId>> {
        if let Some((_, latest)) = self.states.last_mut() {
            latest.extend_from_slice(rules);
        }

        let hash = hash(text);
        if let Some(repeat) = self.states.iter().position(|(state, _)| *state == hash) {
            let around = self.states[repeat..]
                .iter()
                .flat_map(|(_, rules)| rules.iter().copied())
                .collect();
            return Some(around);
        }

        self.states.push((hash, Vec::new()));
        None
    }
}

#[bon]
impl FixRounds {
    /// Fixes the files of a run, starting from the checks of the first round
    ///
    /// `checks` holds one check for each file of `files`, in their order.
    /// A diagnostic whose rule `rules` leaves out takes no part, because
    /// the run does not report it. The providers take the changed text of
    /// every round, so they hold the fixed text when this returns.
    ///
    /// # Errors
    ///
    /// Returns an error if a fix lies outside its file or splits a
    /// character. It returns one if a provider cannot take the changed
    /// text, or if a check of the fixed text cannot run. It also returns one
    /// if a file comes back to an earlier text, or if the files still change
    /// after the last round.
    #[builder]
    pub(super) fn run(
        files: &[PathBuf],
        checks: Vec<FileCheck>,
        rules: &RuleFilter,
        providers: &mut Providers,
        custom_lints: &CustomLints,
        options: &RuleOptions,
    ) -> anyhow::Result<Self> {
        let mut checks = checks;
        let mut texts: HashMap<PathBuf, String> = HashMap::new();
        let mut histories: HashMap<PathBuf, History> = HashMap::new();
        let mut fixed = 0;
        let mut last_rules: BTreeSet<RuleId> = BTreeSet::new();

        for _round in 0..MAX_ROUNDS {
            let mut changed: Vec<(usize, String)> = Vec::new();
            last_rules.clear();

            for (index, (file, check)) in files.iter().zip(&checks).enumerate() {
                let FileCheck::Checked {
                    text: Some(text),
                    result: Ok(diagnostics),
                } = check
                else {
                    continue;
                };
                let admitted: Vec<Diagnostic> = diagnostics
                    .iter()
                    .filter(|diagnostic| rules.admits(&diagnostic.rule_id()))
                    .cloned()
                    .collect();
                let round = FixRound::apply()
                    .text(text)
                    .diagnostics(&admitted)
                    .call()
                    .with_context(|| format!("fix {}", file.display()))?;
                if round.applied().is_empty() {
                    continue;
                }

                let history = histories
                    .entry(file.clone())
                    .or_insert_with(|| History::new(text));
                if let Some(around) = history.record(round.text(), round.applied()) {
                    anyhow::bail!(
                        "the fixes of {} keep undoing each other in {}, so stiles wrote nothing",
                        list(&around),
                        file.display()
                    );
                }

                fixed += round.applied().len();
                last_rules.extend(round.applied().iter().copied());
                changed.push((index, round.into_text()));
            }

            if changed.is_empty() {
                return Ok(Self {
                    checks,
                    texts,
                    fixed,
                });
            }

            let changes: Vec<(PathBuf, String)> = changed
                .into_iter()
                .map(|(index, text)| (files[index].clone(), text))
                .collect();
            providers
                .change_files(&changes)
                .context("failed to give the providers the fixed text")?;
            texts.extend(changes);

            checks = FileChecks::run()
                .files(files)
                .texts(&texts)
                .providers(providers)
                .custom_lints(custom_lints)
                .options(options)
                .call()
                .context("failed to check the fixed files")?;
        }

        anyhow::bail!(
            "the fixes did not settle after {MAX_ROUNDS} rounds, so stiles wrote nothing; the \
             fixes of {} kept changing the files",
            list(&last_rules)
        )
    }

    /// Returns the checks of the fixed text, the fixed text of each changed
    /// file, and the number of fixes that applied
    ///
    /// The checks hold one check for each file of the run, in its order.
    pub(super) fn into_parts(self) -> (Vec<FileCheck>, HashMap<PathBuf, String>, usize) {
        let Self {
            checks,
            texts,
            fixed,
        } = self;
        (checks, texts, fixed)
    }
}

/// Returns a hash of `text`, to tell its states apart across rounds
fn hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Returns the rules as a list a reader can scan, such as `` `a` and `b` ``
///
/// A list of three or more takes a comma before its last rule.
fn list(rules: &BTreeSet<RuleId>) -> String {
    let names: Vec<String> = rules.iter().map(|rule| format!("`{rule}`")).collect();
    match names.as_slice() {
        [] => "no rule".to_owned(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &'static str) -> RuleId {
        RuleId::new(name)
    }

    #[test]
    fn list_of_one_rule_names_it() {
        let rules = BTreeSet::from([rule("lint.a")]);

        let text = list(&rules);

        assert_eq!(text, "`lint.a`");
    }

    #[test]
    fn list_of_three_rules_joins_the_last_with_a_comma_and_and() {
        let rules = BTreeSet::from([rule("lint.a"), rule("lint.b"), rule("lint.c")]);

        let text = list(&rules);

        assert_eq!(text, "`lint.a`, `lint.b`, and `lint.c`");
    }

    #[test]
    fn list_of_two_rules_joins_them_with_and() {
        let rules = BTreeSet::from([rule("lint.a"), rule("lint.b")]);

        let text = list(&rules);

        assert_eq!(text, "`lint.a` and `lint.b`");
    }

    #[test]
    fn record_of_a_new_text_returns_none() {
        let mut history = History::new("a");

        let around = history.record("b", &[rule("lint.a")]);

        assert_eq!(around, None);
    }

    #[test]
    fn record_of_a_text_seen_two_rounds_ago_leaves_out_the_rules_before_it() {
        let mut history = History::new("a");
        history.record("b", &[rule("lint.first")]);
        history.record("c", &[rule("lint.forth")]);

        let around = history.record("b", &[rule("lint.back")]);

        assert_eq!(
            around,
            Some(BTreeSet::from([rule("lint.back"), rule("lint.forth")]))
        );
    }

    #[test]
    fn record_of_an_earlier_text_names_the_rules_that_took_it_around() {
        let mut history = History::new("a");
        history.record("b", &[rule("lint.forth")]);

        let around = history.record("a", &[rule("lint.back")]);

        assert_eq!(
            around,
            Some(BTreeSet::from([rule("lint.back"), rule("lint.forth")]))
        );
    }

    #[test]
    fn trait_send_fix_rounds() {
        fn assert_send<T: Send>() {}
        assert_send::<FixRounds>();
    }

    #[test]
    fn trait_send_history() {
        fn assert_send<T: Send>() {}
        assert_send::<History>();
    }

    #[test]
    fn trait_sync_fix_rounds() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FixRounds>();
    }

    #[test]
    fn trait_sync_history() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<History>();
    }

    #[test]
    fn trait_unpin_fix_rounds() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FixRounds>();
    }

    #[test]
    fn trait_unpin_history() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<History>();
    }
}
