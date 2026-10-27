use std::string::FromUtf8Error;

use codespan_reporting::files;
use thiserror::Error;

/// The reason [`render_to_string`] could not render the diagnostics
///
/// [`render_to_string`]: crate::render_to_string
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
///
/// use whisker_reporting::render_to_string;
///
/// let rendered = render_to_string(&[], &HashMap::new());
///
/// assert!(rendered.is_ok());
/// ```
#[derive(Debug, Error)]
pub enum RenderDiagnosticsError {
    /// Codespan could not lay out a diagnostic or write it
    #[error("failed to emit a diagnostic")]
    Unemitted {
        /// The failure that codespan reported
        source: files::Error,
    },

    /// The rendered text is not valid UTF-8
    #[error("failed to read the rendered diagnostics as UTF-8")]
    NotUtf8 {
        /// The failure of the conversion, which holds the rendered bytes
        source: FromUtf8Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RenderDiagnosticsError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RenderDiagnosticsError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RenderDiagnosticsError>();
    }
}
