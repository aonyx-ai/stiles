//! Whether this build of stiles-typescript comes from published source
//!
//! stiles-plugin joins this value with the provenance of the other
//! boundary crates. `Provenance` in stiles-types says what it licenses.

use stiles_types::plugin::Provenance;

/// Whether this crate is a build of published source
///
/// The build script sets the flag when cargo packaged this crate.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::Provenance;
///
/// let joined = Provenance::all(&[stiles_typescript::PROVENANCE]);
///
/// assert_eq!(joined, stiles_typescript::PROVENANCE);
/// ```
#[cfg(stiles_published)]
pub const PROVENANCE: Provenance = Provenance::PUBLISHED;

/// Whether this crate is a build of published source
///
/// The build script sets the flag when cargo packaged this crate.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::Provenance;
///
/// let joined = Provenance::all(&[stiles_typescript::PROVENANCE]);
///
/// assert_eq!(joined, stiles_typescript::PROVENANCE);
/// ```
#[cfg(not(stiles_published))]
pub const PROVENANCE: Provenance = Provenance::UNPUBLISHED;

#[cfg(test)]
mod tests {
    use super::*;

    /// A test builds the crate from its source tree, which cargo never
    /// packaged
    #[test]
    fn provenance_of_a_source_tree_is_unpublished() {
        let provenance = PROVENANCE;

        assert_eq!(provenance, Provenance::UNPUBLISHED);
    }
}
