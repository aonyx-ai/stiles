use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use bon::bon;
use stiles_tsrs::tsrs_vfs::{Entries, FS, FileInfo};

/// A file system that serves some files from memory and the rest from disk
///
/// `stiles check --fix` changes files in memory, and writes them only when
/// the whole run succeeds. A round of fixes checks the changed text again.
/// The program of the next round therefore has to read that text and not
/// the copy on disk. The overlay holds the changed text of each such file, under the
/// absolute path that tsrs reads it by.
///
/// The overlay refuses every write, because a check writes nothing through
/// the checker.
pub(super) struct OverlayFs {
    inner: Arc<dyn FS>,
    texts: HashMap<String, String>,
}

#[bon]
impl OverlayFs {
    /// Creates a file system that reads `texts` before it reads `inner`
    #[builder]
    pub(super) fn new(inner: Arc<dyn FS>, texts: HashMap<String, String>) -> Self {
        Self { inner, texts }
    }
}

impl FS for OverlayFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.inner.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.texts.contains_key(path) || self.inner.file_exists(path)
    }

    fn read_file(&self, path: &str) -> Option<String> {
        match self.texts.get(path) {
            Some(text) => Some(text.clone()),
            None => self.inner.read_file(path),
        }
    }

    fn write_file(&self, path: &str, _data: &str) -> Result<(), String> {
        Err(read_only(path))
    }

    fn append_file(&self, path: &str, _data: &str) -> Result<(), String> {
        Err(read_only(path))
    }

    fn remove(&self, path: &str) -> Result<(), String> {
        Err(read_only(path))
    }

    fn chtimes(&self, path: &str, _a_time: SystemTime, _m_time: SystemTime) -> Result<(), String> {
        Err(read_only(path))
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.inner.directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        self.inner.get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> Option<FileInfo> {
        let info = self.inner.stat(path)?;
        let Some(text) = self.texts.get(path) else {
            return Some(info);
        };

        Some(FileInfo {
            size: i64::try_from(text.len()).unwrap_or(i64::MAX),
            ..info
        })
    }

    fn realpath(&self, path: &str) -> String {
        self.inner.realpath(path)
    }
}

/// Returns the message of a write that the overlay refuses
fn read_only(path: &str) -> String {
    format!("stiles checks {path} and does not write it")
}

#[cfg(test)]
mod tests {
    use stiles_tsrs::tsrs_vfs::osvfs;

    use super::*;

    fn overlay(directory: &tempfile::TempDir) -> (OverlayFs, String, String) {
        let changed = directory.path().join("changed.ts");
        let untouched = directory.path().join("untouched.ts");
        std::fs::write(&changed, "const a = 1;\n").expect("the file should be written");
        std::fs::write(&untouched, "const b = 2;\n").expect("the file should be written");
        let changed = changed.to_str().expect("the path is UTF-8").to_owned();
        let untouched = untouched.to_str().expect("the path is UTF-8").to_owned();
        let texts = HashMap::from([(changed.clone(), "const a = 10;\n".to_owned())]);

        (
            OverlayFs::builder()
                .inner(Arc::new(osvfs::fs()))
                .texts(texts)
                .build(),
            changed,
            untouched,
        )
    }

    #[test]
    fn read_file_of_a_changed_file_returns_the_changed_text() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let (fs, changed, _) = overlay(&directory);

        let text = fs.read_file(&changed);

        assert_eq!(text.as_deref(), Some("const a = 10;\n"));
    }

    #[test]
    fn read_file_of_another_file_returns_the_text_on_disk() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let (fs, _, untouched) = overlay(&directory);

        let text = fs.read_file(&untouched);

        assert_eq!(text.as_deref(), Some("const b = 2;\n"));
    }

    #[test]
    fn stat_of_a_changed_file_returns_the_size_of_the_changed_text() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let (fs, changed, _) = overlay(&directory);

        let info = fs.stat(&changed).expect("the file exists");

        assert_eq!(info.size, 14);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<OverlayFs>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<OverlayFs>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<OverlayFs>();
    }

    #[test]
    fn write_file_returns_an_error() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let (fs, changed, _) = overlay(&directory);

        let result = fs.write_file(&changed, "const c = 3;\n");

        assert!(result.is_err());
    }
}
