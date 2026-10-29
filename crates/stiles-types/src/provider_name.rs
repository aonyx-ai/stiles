/// Identifies a decoration provider in user-facing diagnostics
///
/// Providers are linked in at compile time, so the name is a static
/// string, like [`RuleId`].
///
/// # Examples
///
/// ```
/// use stiles_types::ProviderName;
///
/// assert_eq!(ProviderName::new("rust").to_string(), "rust");
/// ```
///
/// [`RuleId`]: crate::RuleId
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ProviderName(&'static str);

impl ProviderName {
    /// Creates the name of a provider from a static string
    ///
    /// The function is `const`, so a provider can hold its name in a
    /// constant.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::ProviderName;
    ///
    /// const NAME: ProviderName = ProviderName::new("rust");
    ///
    /// assert_eq!(NAME.as_str(), "rust");
    /// ```
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Returns the string representation of this provider name
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::ProviderName;
    ///
    /// assert_eq!(ProviderName::new("rust").as_str(), "rust");
    /// ```
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for ProviderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_inner_str() {
        let name = ProviderName::new("rust");

        let result = name.as_str();

        assert_eq!(result, "rust");
    }

    #[test]
    fn display_writes_name() {
        let name = ProviderName::new("rust");

        let result = name.to_string();

        assert_eq!(result, "rust");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ProviderName>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ProviderName>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ProviderName>();
    }
}
