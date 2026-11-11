/// A source language stiles parses and lint passes target
///
/// A lint pass names its language at the plugin boundary, and stiles
/// runs the pass only on trees of that language. The grammars share node
/// kind names such as `identifier` and `comment`. A pass that ran on every
/// tree would therefore act on nodes it was never written for.
///
/// A language crosses the plugin boundary as a byte that this type pins
/// for each variant, never as a laid-out enum. A plugin names the language
/// of each lint it registers by that byte. A rule names the language of a
/// snippet it asks the host to parse the same way. A reader that meets a
/// byte it does not know treats the language as unknown.
///
/// # Examples
///
/// ```
/// use stiles_types::Language;
///
/// assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
/// ```
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Language {
    /// The Rust programming language
    Rust,
    /// The TypeScript programming language, including TSX
    TypeScript,
}

impl Language {
    /// Detects the language from a file extension
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::Language;
    ///
    /// assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    /// assert_eq!(Language::from_extension("tsx"), Some(Language::TypeScript));
    /// assert_eq!(Language::from_extension("py"), None);
    /// ```
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::Rust),
            "ts" | "mts" | "cts" | "tsx" => Some(Self::TypeScript),
            _ => None,
        }
    }
}

impl Language {
    /// The byte that names [`Language::Rust`] at the plugin boundary
    ///
    /// Each byte is written out rather than taken from the discriminant.
    /// No fingerprint sees a reorder of the variants, and a reorder would
    /// otherwise make a plugin and the host read one byte as two languages.
    const RUST: u8 = 1;
    /// The byte that names [`Language::TypeScript`] at the plugin boundary
    const TYPESCRIPT: u8 = 2;

    /// Returns the byte that names this language at the plugin boundary
    pub(crate) const fn to_byte(self) -> u8 {
        match self {
            Self::Rust => Self::RUST,
            Self::TypeScript => Self::TYPESCRIPT,
        }
    }

    /// Returns the language that `byte` names, or [`None`] for an unknown byte
    ///
    /// A plugin built against a newer stiles may name a language this one
    /// lacks. A byte reads that as unknown, where a [`Language`] built from
    /// it would hold a discriminant no variant has.
    pub(crate) const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            Self::RUST => Some(Self::Rust),
            Self::TYPESCRIPT => Some(Self::TypeScript),
            _ => None,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rust => f.write_str("Rust"),
            Self::TypeScript => f.write_str("TypeScript"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_with_rust_shows_rust() {
        assert_eq!(Language::Rust.to_string(), "Rust");
    }

    #[test]
    fn display_with_typescript_shows_typescript() {
        assert_eq!(Language::TypeScript.to_string(), "TypeScript");
    }

    #[test]
    fn from_byte_with_an_unknown_byte_returns_none() {
        assert_eq!(Language::from_byte(u8::MAX), None);
    }

    #[test]
    fn from_byte_with_the_byte_of_a_language_returns_it() {
        let languages = [Language::Rust, Language::TypeScript];

        let read = languages.map(|language| Language::from_byte(language.to_byte()));

        assert_eq!(read, languages.map(Some));
    }

    #[test]
    fn from_extension_with_rs_returns_rust() {
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    }

    #[test]
    fn from_extension_with_typescript_extensions_returns_typescript() {
        for extension in ["ts", "mts", "cts", "tsx"] {
            assert_eq!(
                Language::from_extension(extension),
                Some(Language::TypeScript),
                "`.{extension}` should be TypeScript"
            );
        }
    }

    #[test]
    fn from_extension_with_unknown_returns_none() {
        let languages = ["py", "js", ""].map(Language::from_extension);

        assert_eq!(languages, [None, None, None]);
    }

    /// A plugin and the host agree on these bytes whatever order the
    /// variants are declared in
    #[test]
    fn to_byte_names_each_language_by_a_fixed_byte() {
        let bytes = [Language::Rust, Language::TypeScript].map(Language::to_byte);

        assert_eq!(bytes, [1, 2]);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Language>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Language>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Language>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn unknown_extension_returns_none(ext in "[a-z]{1,10}") {
                prop_assume!(!["rs", "ts", "mts", "cts", "tsx"].contains(&ext.as_str()));
                prop_assert_eq!(Language::from_extension(&ext), None);
            }
        }
    }
}
