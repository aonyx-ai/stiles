use std::path::Path;

use bon::bon;

use crate::FilePath;

/// A byte range within a source file
///
/// Spans identify a contiguous region of source text by file path and byte
/// offsets. The range is half-open: `[start, end)`. The file path is
/// reference-counted so that creating spans from a shared source is cheap.
///
/// A span crosses the plugin boundary inside every [`Diagnostic`], so
/// stabby lays it out. Its file is a [`FilePath`], for the reason that
/// type gives.
///
/// [`Diagnostic`]: crate::Diagnostic
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Span {
    file: FilePath,
    start: usize,
    end: usize,
}

#[bon]
impl Span {
    /// Creates a span covering `[start, end)` in the given file
    ///
    /// # Panics
    ///
    /// Panics if `start > end`.
    #[builder]
    pub fn new(file: impl Into<FilePath>, start: usize, end: usize) -> Self {
        let file = file.into();
        assert!(
            start <= end,
            "span start ({start}) must not exceed end ({end})"
        );
        Self { file, start, end }
    }

    /// Returns the file path this span belongs to
    pub fn file(&self) -> &Path {
        self.file.as_path()
    }

    /// Returns the shared file path
    ///
    /// Clone it to build another span in the same file without copying
    /// the path.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::Span;
    ///
    /// let span = Span::builder().file("src/lib.rs").start(10).end(20).build();
    ///
    /// let narrower = Span::builder().file(span.file_path().clone()).start(12).end(14).build();
    ///
    /// assert_eq!(narrower.file(), span.file());
    /// ```
    pub fn file_path(&self) -> &FilePath {
        &self.file
    }

    /// Returns the start byte offset (inclusive)
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the end byte offset (exclusive)
    pub fn end(&self) -> usize {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn empty_span_is_allowed() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(5)
            .end(5)
            .build();
        assert_eq!(span.start(), span.end());
    }

    #[test]
    fn end_returns_the_end() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(10)
            .end(20)
            .build();

        assert_eq!(span.end(), 20);
    }

    #[test]
    fn file_path_is_shared_by_a_clone() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(0)
            .end(10)
            .build();

        let copy = span.clone();

        assert!(std::ptr::eq(copy.file(), span.file()));
    }

    #[test]
    fn file_returns_the_file() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(10)
            .end(20)
            .build();

        assert_eq!(span.file(), Path::new("test.rs"));
    }

    #[test]
    fn new_accepts_a_shared_file_path() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(0)
            .end(10)
            .build();

        let narrower = Span::builder()
            .file(span.file_path().clone())
            .start(2)
            .end(4)
            .build();

        assert_eq!(
            narrower,
            Span::builder()
                .file(PathBuf::from("test.rs"))
                .start(2)
                .end(4)
                .build()
        );
    }

    #[test]
    #[should_panic(expected = "span start")]
    fn new_with_inverted_range_panics() {
        Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(20)
            .end(10)
            .build();
    }

    #[test]
    fn start_returns_the_start() {
        let span = Span::builder()
            .file(PathBuf::from("test.rs"))
            .start(10)
            .end(20)
            .build();

        assert_eq!(span.start(), 10);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Span>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Span>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Span>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn new_roundtrips_all_fields(
                file in "[a-z]{1,10}\\.rs",
                start in 0..=1000usize,
                delta in 0..=1000usize,
            ) {
                let end = start + delta;
                let span = Span::builder().file(PathBuf::from(&file)).start(start).end(end).build();

                prop_assert_eq!(span.file(), Path::new(&file));
                prop_assert_eq!(span.start(), start);
                prop_assert_eq!(span.end(), end);
            }

            #[test]
            fn start_never_exceeds_end(
                file in "[a-z]+\\.rs",
                start in 0..=1000usize,
                delta in 0..=1000usize,
            ) {
                let end = start + delta;
                let span = Span::builder().file(PathBuf::from(&file)).start(start).end(end).build();
                prop_assert!(span.start() <= span.end());
            }
        }
    }
}
