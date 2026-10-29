use std::fmt;

/// The name of a language, as the generated code spells it
///
/// The name prefixes the generated trait, so `Rust` yields `RustLintPass`,
/// and the documentation of that trait names the language with it.
/// [`generate_visitor`] takes the name next to the text of a grammar. Both
/// are strings, and a type of its own keeps a call from swapping them.
///
/// # Examples
///
/// ```
/// use stiles_codegen::LanguageName;
///
/// let language = LanguageName::new("Rust");
///
/// assert_eq!(language.to_string(), "Rust");
/// ```
///
/// [`generate_visitor`]: crate::generate_visitor
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct LanguageName<'a>(&'a str);

impl<'a> LanguageName<'a> {
    /// Creates the name of a language
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_codegen::LanguageName;
    ///
    /// let language = LanguageName::new("TypeScript");
    ///
    /// assert_eq!(language.as_str(), "TypeScript");
    /// ```
    pub fn new(name: &'a str) -> Self {
        Self(name)
    }

    /// Returns the name as it was given
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_codegen::LanguageName;
    ///
    /// let language = LanguageName::new("Rust");
    ///
    /// assert_eq!(language.as_str(), "Rust");
    /// ```
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}

impl fmt::Display for LanguageName<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_writes_the_name() {
        let language = LanguageName::new("Rust");

        let text = language.to_string();

        assert_eq!(text, "Rust");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LanguageName<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LanguageName<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LanguageName<'_>>();
    }
}
