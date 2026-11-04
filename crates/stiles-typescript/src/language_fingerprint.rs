use stabby::IStable;
use stiles_types::plugin::{decoration_fingerprint, stable_fingerprint};

use crate::decorations::{
    FnSignature, ImportSource, Parameter, Primitive, ResolvedType, StringUnion, TypeText,
};

include!(concat!(env!("OUT_DIR"), "/visitor_fingerprint.rs"));

/// A fingerprint of the TypeScript language support a plugin was built against
///
/// stiles-plugin joins it with every other language crate's into the
/// value a plugin's declaration carries.
///
/// Two things on this side shape the boundary. The generated lint pass
/// trait is one: its methods are named after the grammars' node kinds. A
/// plugin generated from a different grammar still loads, and its checks
/// never fire, so `VISITOR_FINGERPRINT` hashes the generated source. The
/// decoration types are the other. A plugin reads them out of a node the
/// host decorated, and reading them at another layout is unsound rather
/// than merely quiet. Each contributes [`decoration_fingerprint`]: the
/// identity stabby derives from its report, joined with the key a plugin
/// looks it up by. A plugin that holds another key is therefore refused
/// rather than left to find nothing. [`Primitive`] and [`Parameter`] are
/// not decorations, so they contribute their stabby identity alone. The
/// list names every type that implements [`Decoration`], and a new one
/// belongs on it.
///
/// Everything else in this crate is deliberately absent. Hashing the
/// whole crate refuses every plugin in the tree whenever any source file
/// here changes.
///
/// # Examples
///
/// ```
/// use stiles_typescript::LANGUAGE_FINGERPRINT;
///
/// assert_ne!(LANGUAGE_FINGERPRINT, 0);
/// ```
///
/// [`Decoration`]: stiles_types::Decoration
pub const LANGUAGE_FINGERPRINT: u64 = stable_fingerprint(&[
    VISITOR_FINGERPRINT,
    decoration_fingerprint::<TypeText>(),
    decoration_fingerprint::<ResolvedType>(),
    <Primitive as IStable>::ID,
    decoration_fingerprint::<FnSignature>(),
    <Parameter as IStable>::ID,
    decoration_fingerprint::<ImportSource>(),
    decoration_fingerprint::<StringUnion>(),
]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_covers_the_decorations() {
        let visitor_alone = stable_fingerprint(&[VISITOR_FINGERPRINT]);

        let fingerprint = LANGUAGE_FINGERPRINT;

        assert_ne!(fingerprint, visitor_alone);
    }

    #[test]
    fn language_fingerprint_is_not_zero() {
        let fingerprint = LANGUAGE_FINGERPRINT;

        assert_ne!(fingerprint, 0);
    }

    #[test]
    fn visitor_fingerprint_is_not_zero() {
        let generated = VISITOR_FINGERPRINT;

        assert_ne!(generated, 0);
    }
}
