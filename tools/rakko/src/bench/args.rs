use rakko_action::{
    Args, ArgsSchema, ArgsValues, Argument, ArgumentShape, ReadArgsError, argument_name,
};

use super::corpus::{CORPUS, Project};

/// The arguments that a run of the bench action reads
///
/// Without `project`, the run measures every project of the corpus, in the
/// order of the corpus. A pull request that only wants the small baseline
/// names `stiles`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct BenchArgs {
    /// The project that the run measures, or every project of the corpus
    project: Option<Project>,
}

impl BenchArgs {
    /// Returns the projects that the run measures, in their order
    pub(super) fn projects(&self) -> Vec<Project> {
        match self.project {
            Some(project) => vec![project],
            None => CORPUS.to_vec(),
        }
    }
}

impl Args for BenchArgs {
    fn schema() -> ArgsSchema {
        ArgsSchema::new([Argument::builder()
            .name(argument_name!("project"))
            .shape(ArgumentShape::Text)
            .documentation(
                "Prepare only this project of the corpus: stiles, tokio, typescript, \
                     or trpc",
            )
            .build()])
    }

    fn from_values(values: &ArgsValues) -> Result<Self, ReadArgsError> {
        let name = argument_name!("project");
        let project =
            match values.get(&name) {
                Some(value) => Some(Project::named(value.get()).ok_or_else(|| {
                    ReadArgsError::UnreadableValue {
                        name,
                        value: value.clone(),
                    }
                })?),
                None => None,
            };

        Ok(Self { project })
    }
}

#[cfg(test)]
mod tests {
    // An assertion in a test panics by design. A `# Panics` section on every
    // test would repeat that and give the reader no information.
    #![allow(clippy::missing_panics_doc, clippy::unwrap_used)]

    use rakko_action::{ArgumentName, ArgumentValue};

    use super::*;

    /// Returns the values of a run that gave the arguments
    fn values(pairs: &[(&'static str, &str)]) -> ArgsValues {
        ArgsValues::new(
            pairs
                .iter()
                .map(|&(name, value)| (ArgumentName::from_static(name), ArgumentValue::new(value))),
        )
    }

    #[test]
    fn from_values_with_a_project_measures_only_it() {
        let args = BenchArgs::from_values(&values(&[("project", "trpc")])).unwrap();

        assert_eq!(args.projects(), [Project::named("trpc").unwrap()]);
    }

    #[test]
    fn from_values_with_an_unknown_project_fails() {
        let error = BenchArgs::from_values(&values(&[("project", "linux")])).unwrap_err();

        assert!(error.to_string().contains("linux"), "{error}");
    }

    #[test]
    fn from_values_with_no_values_measures_the_whole_corpus() {
        let args = BenchArgs::from_values(&values(&[])).unwrap();

        assert_eq!(args.projects(), CORPUS);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<BenchArgs>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<BenchArgs>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<BenchArgs>();
    }
}
