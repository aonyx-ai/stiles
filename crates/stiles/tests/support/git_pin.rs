//! A repository of rules as a test pins it in a configuration

/// The address of a repository of rules and the commit a test pins
///
/// Both are strings, and a call with two string arguments compiles in
/// either order. The named fields keep the address and the commit apart.
#[derive(Copy, Clone, Debug)]
pub(crate) struct GitPin<'a> {
    /// The address stiles fetches the repository from
    pub(crate) url: &'a str,

    /// The commit the configuration names
    pub(crate) rev: &'a str,
}
