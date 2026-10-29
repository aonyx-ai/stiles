use std::fmt;
use std::path::PathBuf;

use bon::bon;
use clawless::exit::Exit;
use clawless::prelude::*;
use stiles_types::Language;

use super::closing_text::ClosingText;
use crate::config::StilesConfig;
use crate::custom_lints::{CustomLints, DeclaredRule};

/// List the rules the project's configured lints declare
#[derive(Debug, Args)]
pub struct RulesArgs {
    /// Path to the target project directory
    #[arg(default_value = ".")]
    path: PathBuf,
}

/// One rule as `stiles rules` lists it
///
/// A rule can run on several languages, each through its own lint, and the
/// listing names them all. `enabled` says whether the project's `[rules]`
/// let the rule's diagnostics through, so a reader sees the effect of the
/// configuration and not just what the plugins offer.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
struct ListedRule {
    rule: &'static str,
    languages: Vec<&'static str>,
    enabled: bool,
}

#[bon]
impl ListedRule {
    /// Describes `declared` under the project's `config`
    #[builder]
    fn new(declared: &DeclaredRule, config: &StilesConfig) -> Self {
        let DeclaredRule { rule, languages } = declared;

        Self {
            rule: rule.as_str(),
            languages: languages.iter().copied().map(language_name).collect(),
            enabled: config.rules().admits(rule),
        }
    }
}

impl fmt::Display for ListedRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.rule, self.languages.join(", "))?;

        match self.enabled {
            true => Ok(()),
            false => f.write_str(", off"),
        }
    }
}

/// Returns the lowercase name a reader of the listing sees for `language`
///
/// The names match the groups of `export_lints!`, where a rule author
/// writes them.
fn language_name(language: Language) -> &'static str {
    match language {
        Language::Rust => "rust",
        Language::TypeScript => "typescript",
    }
}

#[command]
pub async fn rules(args: RulesArgs, context: Context) -> CommandResult<Exit> {
    let mut closing = ClosingText::default();

    match list(args, &context, &mut closing).await {
        Ok(()) => Ok(closing.into_exit(ExitCode::SUCCESS)),
        Err(error) => Ok(closing.into_failure(&error)),
    }
}

/// Lists the rules, and leaves the notices of the lint loading in `closing`
///
/// # Errors
///
/// Returns an error if the configuration or the lints cannot be loaded, or
/// if a rule cannot be reported.
async fn list(args: RulesArgs, context: &Context, closing: &mut ClosingText) -> anyhow::Result<()> {
    let RulesArgs { path } = args;

    anyhow::ensure!(path.exists(), "{} does not exist", path.display());

    let config = StilesConfig::load(&path).context("failed to load the stiles configuration")?;
    let mut notices = Vec::new();
    let custom_lints = CustomLints::load()
        .config(&config)
        .notices(&mut notices)
        .call();
    closing.extend(notices);
    let custom_lints = custom_lints.context("failed to load the project's custom lints")?;

    for declared in custom_lints.rules() {
        context
            .output()
            .artifact(
                ListedRule::builder()
                    .declared(&declared)
                    .config(&config)
                    .build(),
            )
            .await
            .context("failed to report a rule")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(enabled: bool) -> ListedRule {
        ListedRule {
            rule: "lint.todo-comment",
            languages: vec!["rust", "typescript"],
            enabled,
        }
    }

    #[test]
    fn display_names_the_rule_and_its_languages() {
        let text = listed(true).to_string();

        assert_eq!(text, "lint.todo-comment (rust, typescript)");
    }

    #[test]
    fn display_with_a_rule_turned_off_says_so() {
        let text = listed(false).to_string();

        assert_eq!(text, "lint.todo-comment (rust, typescript), off");
    }

    #[test]
    fn json_has_the_rule_its_languages_and_whether_it_runs() {
        let json = serde_json::to_value(listed(true)).expect("should serialize");

        assert_eq!(
            json,
            serde_json::json!({
                "rule": "lint.todo-comment",
                "languages": ["rust", "typescript"],
                "enabled": true,
            })
        );
    }

    #[test]
    fn language_name_matches_the_export_lints_groups() {
        let names = [Language::Rust, Language::TypeScript].map(language_name);

        assert_eq!(names, ["rust", "typescript"]);
    }

    #[test]
    fn trait_send_listed_rule() {
        fn assert_send<T: Send>() {}
        assert_send::<ListedRule>();
    }

    #[test]
    fn trait_send_rules_args() {
        fn assert_send<T: Send>() {}
        assert_send::<RulesArgs>();
    }

    #[test]
    fn trait_sync_listed_rule() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ListedRule>();
    }

    #[test]
    fn trait_sync_rules_args() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RulesArgs>();
    }

    #[test]
    fn trait_unpin_listed_rule() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ListedRule>();
    }

    #[test]
    fn trait_unpin_rules_args() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RulesArgs>();
    }
}
