use whisker_macros::Decoration;

/// Where the module an import names comes from
///
/// The TypeScript provider attaches one to the specifier string of every
/// `import` and `export ... from` statement, as the program resolved it.
/// A rule that orders imports by origin reads this rather than guessing
/// from the specifier's spelling, because `@scope/pkg` may be a package of
/// this repository or one from the registry.
///
/// # Examples
///
/// ```
/// use whisker_typescript::decorations::ImportSource;
///
/// assert_ne!(ImportSource::Relative, ImportSource::Dependency);
/// ```
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub enum ImportSource {
    /// A path relative to the importing file, such as `./a`
    Relative,
    /// A package installed in a `node_modules` directory
    Dependency,
    /// A package of this repository, reached through a workspace link or
    /// a `paths` mapping
    Workspace,
    /// The program could not resolve the specifier
    Unresolved,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ImportSource>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ImportSource>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ImportSource>();
    }
}
