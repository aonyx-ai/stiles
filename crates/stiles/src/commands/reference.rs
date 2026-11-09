use std::fmt;
use std::path::PathBuf;

use clawless::prelude::*;

/// Generate a reference page for a set of rules
#[derive(Debug, Args)]
pub(crate) struct ReferenceArgs {
    /// Package that re-exports every rule crate
    #[arg(long)]
    package: String,

    /// File to write the page to
    #[arg(long)]
    output: PathBuf,

    /// Path to the project that holds the package
    #[arg(default_value = ".")]
    path: PathBuf,
}

/// The page that `stiles reference` wrote
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
struct WrittenPage {
    output: String,
    rules: usize,
}

impl fmt::Display for WrittenPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { output, rules } = self;

        write!(f, "wrote {rules} rules to {output}")
    }
}

#[command]
pub(crate) async fn reference(args: ReferenceArgs, context: Context) -> CommandResult {
    let ReferenceArgs {
        package,
        output,
        path,
    } = args;

    let rules = crate::reference::generate(&path, &package, &output)
        .context("failed to generate the reference page")?;

    context
        .output()
        .artifact(WrittenPage {
            output: output.display().to_string(),
            rules,
        })
        .await
        .context("failed to report the page")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_auto_traits<T: Send + Sync + Unpin>() {}

    #[test]
    fn display_says_where_the_page_went() {
        let page = WrittenPage {
            output: "rules.html".to_owned(),
            rules: 3,
        };

        let text = page.to_string();

        assert_eq!(text, "wrote 3 rules to rules.html");
    }

    #[test]
    fn trait_auto_reference_args() {
        assert_auto_traits::<ReferenceArgs>();
    }

    #[test]
    fn trait_auto_written_page() {
        assert_auto_traits::<WrittenPage>();
    }
}
