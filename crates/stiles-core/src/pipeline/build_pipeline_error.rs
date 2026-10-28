use thiserror::Error;

/// The reason a pipeline could not be built
///
/// # Examples
///
/// ```
/// use stiles_core::{BuildPipelineError, Pipeline};
/// use stiles_types::Language;
///
/// let grammar = tree_sitter_rust::LANGUAGE.into();
/// let pipeline = Pipeline::builder()
///     .language(Language::Rust)
///     .grammar(&grammar)
///     .build();
///
/// assert!(pipeline.is_ok());
/// ```
#[derive(Eq, PartialEq, Debug, Error)]
pub enum BuildPipelineError {
    /// Tree-sitter refused the grammar
    #[error("failed to set the tree-sitter grammar")]
    IncompatibleGrammar {
        /// The refusal of tree-sitter, which says what it could not accept
        source: tree_sitter::LanguageError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<BuildPipelineError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<BuildPipelineError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<BuildPipelineError>();
    }
}
