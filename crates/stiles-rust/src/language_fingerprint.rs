use stiles_types::plugin::{decoration_fingerprint, stable_fingerprint};

use crate::decorations::{
    AdtFlags, EnumVariants, FnSignature, ImportSource, Reexports, ResolvedType,
};

include!(concat!(env!("OUT_DIR"), "/visitor_fingerprint.rs"));

/// A fingerprint of the Rust language support a plugin was built against
///
/// stiles-plugin joins it with every other language crate's into the
/// value a plugin's declaration carries.
///
/// Two things on this side shape the boundary. The generated lint pass
/// trait is one: its methods are named after the grammar's node kinds. A
/// plugin generated from a different grammar still loads, and its checks
/// never fire, so `VISITOR_FINGERPRINT` hashes the generated source. The
/// decoration types are the other, because a plugin reads them out of a
/// node that the host decorated. To read them at a layout that the host did
/// not write is unsound, not merely quiet.
///
/// Each decoration contributes [`decoration_fingerprint`]: the identity
/// stabby derives from its report, joined with its key. The report covers
/// the name and type of every field, recursively. A type a decoration
/// holds, such as [`TypePath`] inside [`FnSignature`], is therefore
/// covered without being named here. A plugin looks a decoration up by its
/// key. A plugin that holds another key is therefore refused rather than
/// left to find nothing. The list names every type that implements
/// [`Decoration`], and a new one belongs on it.
///
/// Everything else in this crate is deliberately absent. Hashing the
/// whole crate refuses every plugin in the tree whenever any source file
/// here changes.
///
/// # Examples
///
/// ```
/// use stiles_rust::LANGUAGE_FINGERPRINT;
///
/// assert_ne!(LANGUAGE_FINGERPRINT, 0);
/// ```
///
/// [`Decoration`]: stiles_types::Decoration
/// [`TypePath`]: crate::decorations::TypePath
pub const LANGUAGE_FINGERPRINT: u64 = stable_fingerprint(&[
    VISITOR_FINGERPRINT,
    decoration_fingerprint::<AdtFlags>(),
    decoration_fingerprint::<EnumVariants>(),
    decoration_fingerprint::<FnSignature>(),
    decoration_fingerprint::<ImportSource>(),
    decoration_fingerprint::<Reexports>(),
    decoration_fingerprint::<ResolvedType>(),
]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_covers_the_decorations() {
        let visitor_alone = stable_fingerprint(&[VISITOR_FINGERPRINT]);

        let combined = LANGUAGE_FINGERPRINT;

        assert_ne!(combined, visitor_alone);
    }

    #[test]
    fn language_fingerprint_is_not_zero() {
        let combined = LANGUAGE_FINGERPRINT;

        assert_ne!(combined, 0);
    }

    #[test]
    fn visitor_fingerprint_is_not_zero() {
        let generated = VISITOR_FINGERPRINT;

        assert_ne!(generated, 0);
    }
}
