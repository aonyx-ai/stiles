use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term;
use stiles_types::Diagnostic;

use crate::RenderDiagnosticsError;
use crate::renderer::to_codespan;

/// Renders diagnostics against a set of source files that it indexes once
///
/// A rendered diagnostic quotes the lines it points at. The renderer
/// therefore needs the text of every file and the offset of each line.
/// Indexing a file reads all of its text. A renderer therefore indexes each
/// file when it is built. Every diagnostic that it renders afterwards reuses
/// the index. A report of thousands of diagnostics would otherwise index
/// every file once for each of them.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
///
/// use stiles_reporting::SourceRenderer;
///
/// let sources = HashMap::new();
/// let renderer = SourceRenderer::new(&sources);
///
/// assert_eq!(renderer.render(&[])?, "");
/// # Ok::<(), stiles_reporting::RenderDiagnosticsError>(())
/// ```
#[derive(Debug)]
pub struct SourceRenderer<'a> {
    /// The indexed source files, in the form that codespan renders from
    files: SimpleFiles<String, &'a str>,
    /// The identifier of each source file in `files`
    file_ids: HashMap<Arc<Path>, usize>,
}

impl<'a> SourceRenderer<'a> {
    /// Indexes `sources`, a map of file paths to their text
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::path::Path;
    /// use std::sync::Arc;
    ///
    /// use stiles_reporting::SourceRenderer;
    ///
    /// let sources = HashMap::from([(Arc::from(Path::new("lib.rs")), "fn main() {}\n".to_owned())]);
    ///
    /// let _renderer = SourceRenderer::new(&sources);
    /// ```
    pub fn new(sources: &'a HashMap<Arc<Path>, String>) -> Self {
        let mut files = SimpleFiles::new();
        let mut file_ids = HashMap::with_capacity(sources.len());

        for (path, source) in sources {
            let id = files.add(path.display().to_string(), source.as_str());
            file_ids.insert(Arc::clone(path), id);
        }

        Self { files, file_ids }
    }

    /// Renders `diagnostics` as text, one after the other
    ///
    /// A label whose file the renderer did not index is left out, so a
    /// diagnostic still renders its message and its notes.
    ///
    /// # Errors
    ///
    /// Returns [`RenderDiagnosticsError::Unemitted`] if codespan-reporting
    /// fails to render a diagnostic, and [`RenderDiagnosticsError::NotUtf8`]
    /// if the rendered text is not valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// use stiles_reporting::SourceRenderer;
    ///
    /// let sources = HashMap::new();
    ///
    /// let text = SourceRenderer::new(&sources).render(&[])?;
    ///
    /// assert!(text.is_empty());
    /// # Ok::<(), stiles_reporting::RenderDiagnosticsError>(())
    /// ```
    pub fn render(&self, diagnostics: &[Diagnostic]) -> Result<String, RenderDiagnosticsError> {
        let config = term::Config::default();
        let mut output = Vec::new();

        for diagnostic in diagnostics {
            let diagnostic = to_codespan(diagnostic, &self.file_ids);
            term::emit_to_io_write(&mut output, &config, &self.files, &diagnostic)
                .map_err(|source| RenderDiagnosticsError::Unemitted { source })?;
        }

        String::from_utf8(output).map_err(|source| RenderDiagnosticsError::NotUtf8 { source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SourceRenderer<'static>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SourceRenderer<'static>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<SourceRenderer<'static>>();
    }
}
