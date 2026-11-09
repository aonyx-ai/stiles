//! Generates a reference page for a set of rules
//!
//! A project that loads rules has no single place to read what each one
//! checks, so this builds one from the rustdoc of the rules. A package
//! re-exports every rule crate. Each of those crates holds one lint pass,
//! whose docs describe the rule, and one rule id constant beside it.
//!
//! The page comes from rustdoc rather than from the loaded plugins, so
//! generating it changes nothing at the plugin boundary and compiles no
//! plugin.

mod page;
mod rustdoc;
mod rustdoc_json;
mod sections;

use std::path::Path;

use anyhow::Context as _;
use stiles_types::Language;

/// One rule, as its crate documents it
#[derive(Clone, Eq, PartialEq, Debug)]
struct DocumentedRule {
    id: String,
    languages: Vec<Language>,
    docs: String,
}

/// Writes the reference page of a package and returns how many rules it has
///
/// # Errors
///
/// Returns an error if the rules cannot be read or the page cannot be
/// written.
pub(crate) fn generate(directory: &Path, package: &str, output: &Path) -> anyhow::Result<usize> {
    let rules = self::rustdoc::load_rules(directory, package)
        .with_context(|| format!("failed to read the rules of `{package}`"))?;
    let page = self::page::render(package, &rules);

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(output, page)
        .with_context(|| format!("failed to write {}", output.display()))?;

    Ok(rules.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_auto_documented_rule() {
        fn assert_auto_traits<T: Send + Sync + Unpin>() {}
        assert_auto_traits::<DocumentedRule>();
    }
}
