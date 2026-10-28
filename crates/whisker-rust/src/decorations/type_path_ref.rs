use bon::Builder;

/// A borrowed [`TypePath`] a rule can declare as a `const`
///
/// [`TypePath`] owns its segments and cannot be a `const`. This type
/// borrows instead, so a rule declares the type it checks for at module
/// scope.
///
/// # Examples
///
/// ```
/// use whisker_rust::decorations::{TypePath, TypePathRef};
///
/// const ANYHOW_ERROR: TypePathRef<'static> = TypePathRef::builder()
///     .krate("anyhow")
///     .modules(&[])
///     .name("Error")
///     .build();
///
/// let anyhow = TypePath::builder()
///     .krate("anyhow")
///     .modules([] as [&str; 0])
///     .name("Error")
///     .build();
/// let syn = TypePath::builder()
///     .krate("syn")
///     .modules(["error"])
///     .name("Error")
///     .build();
///
/// assert!(anyhow == ANYHOW_ERROR);
/// assert!(syn != ANYHOW_ERROR);
/// ```
///
/// [`TypePath`]: crate::decorations::TypePath
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Builder)]
#[builder(const)]
pub struct TypePathRef<'a> {
    krate: &'a str,
    modules: &'a [&'a str],
    name: &'a str,
}

impl<'a> TypePathRef<'a> {
    /// Returns the name of the crate that defines the type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePathRef;
    ///
    /// let path = TypePathRef::builder().krate("syn").modules(&["error"]).name("Error").build();
    ///
    /// assert_eq!(path.krate(), "syn");
    /// ```
    pub const fn krate(&self) -> &'a str {
        self.krate
    }

    /// Returns the module segments between the crate root and the definition
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePathRef;
    ///
    /// let path = TypePathRef::builder().krate("syn").modules(&["error"]).name("Error").build();
    ///
    /// assert_eq!(path.modules(), &["error"]);
    /// ```
    pub const fn modules(&self) -> &'a [&'a str] {
        self.modules
    }

    /// Returns the item's own name
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePathRef;
    ///
    /// let path = TypePathRef::builder().krate("syn").modules(&["error"]).name("Error").build();
    ///
    /// assert_eq!(path.name(), "Error");
    /// ```
    pub const fn name(&self) -> &'a str {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn krate_returns_the_crate() {
        let path = TypePathRef::builder()
            .krate("core")
            .modules(&["io", "error"])
            .name("Error")
            .build();

        assert_eq!(path.krate(), "core");
    }

    #[test]
    fn modules_returns_the_modules() {
        let path = TypePathRef::builder()
            .krate("core")
            .modules(&["io", "error"])
            .name("Error")
            .build();

        assert_eq!(path.modules(), &["io", "error"]);
    }

    #[test]
    fn name_returns_the_name() {
        let path = TypePathRef::builder()
            .krate("core")
            .modules(&["io", "error"])
            .name("Error")
            .build();

        assert_eq!(path.name(), "Error");
    }

    #[test]
    fn trait_send_type_path_ref() {
        fn assert_send<T: Send>() {}
        assert_send::<TypePathRef<'static>>();
    }

    #[test]
    fn trait_sync_type_path_ref() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TypePathRef<'static>>();
    }

    #[test]
    fn trait_unpin_type_path_ref() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TypePathRef<'static>>();
    }
}
