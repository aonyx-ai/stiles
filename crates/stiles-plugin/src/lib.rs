//! What a custom lint plugin needs to declare itself
//!
//! A plugin is a `cdylib` that stiles loads into its own process. Its
//! rules implement a language crate's lint pass trait, such as
//! [`RustLintPass`], and this crate hands them to stiles. [`export_lints!`]
//! writes the declaration stiles looks up, and [`DeclaresRules`] says
//! which rule names a pass reports.
//!
//! The crate also owns [`LANGUAGE_FINGERPRINT`], which joins the
//! fingerprints of every language crate. One plugin may export passes of
//! several languages, and stiles loads it only when every language it
//! could have been built against matches the host's. Keeping that value
//! here, rather than in a language crate, means a plugin that names one
//! language still carries the fingerprint of all of them.
//!
//! [`RustLintPass`]: stiles_rust::RustLintPass

mod declares_rules;
mod export_lints;

use stiles_types::plugin::stable_fingerprint;
pub use stiles_types::plugin::{
    ABI_VERSION, AbiVersion, Constructed, ExportedLint, LintPassFactory, Loaded, Plugin,
    PluginDeclaration, TYPES_FINGERPRINT, factory,
};
pub use stiles_types::{Language, Panic};

pub use self::declares_rules::DeclaresRules;

#[doc(hidden)]
pub mod __private {
    pub use stiles_rust;
    pub use stiles_typescript;
}

/// A fingerprint of the language support a plugin was built against
///
/// Each language crate fingerprints the part of it that shapes the
/// boundary: its generated lint pass trait and its decoration types. This
/// constant joins them in a fixed order. A plugin carries it in its
/// declaration, and stiles refuses a plugin whose value differs from its
/// own. A pass generated from another grammar would load and never fire,
/// and a decoration read at another layout is unsound.
///
/// # Examples
///
/// ```
/// use stiles_plugin::LANGUAGE_FINGERPRINT;
///
/// assert_ne!(LANGUAGE_FINGERPRINT, 0);
/// ```
pub const LANGUAGE_FINGERPRINT: u64 = stable_fingerprint(&[
    stiles_rust::LANGUAGE_FINGERPRINT,
    stiles_typescript::LANGUAGE_FINGERPRINT,
]);

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the text `abi-surface.toml` must hold for this build
    fn abi_surface() -> String {
        format!(
            "# The plugin boundary as this commit builds it. A test in stiles-plugin\n\
             # fails when these values differ from the code, and `rakko check-abi`\n\
             # fails when a fingerprint moves and the version does not.\n\
             version = \"{ABI_VERSION}\"\n\
             types-fingerprint = \"{TYPES_FINGERPRINT:016x}\"\n\
             language-fingerprint = \"{LANGUAGE_FINGERPRINT:016x}\"\n"
        )
    }

    #[test]
    fn abi_surface_matches_the_record() {
        let recorded = include_str!("../abi-surface.toml");

        let surface = abi_surface();

        assert_eq!(
            recorded, surface,
            "the plugin boundary differs from crates/stiles-plugin/abi-surface.toml. \
             If a fingerprint moved, raise ABI_VERSION. Then write this into the file:\n{surface}"
        );
    }

    #[test]
    fn language_fingerprint_covers_rust() {
        let typescript_alone = stable_fingerprint(&[stiles_typescript::LANGUAGE_FINGERPRINT]);

        let joined = LANGUAGE_FINGERPRINT;

        assert_ne!(joined, typescript_alone);
    }

    #[test]
    fn language_fingerprint_covers_typescript() {
        let rust_alone = stable_fingerprint(&[stiles_rust::LANGUAGE_FINGERPRINT]);

        let joined = LANGUAGE_FINGERPRINT;

        assert_ne!(joined, rust_alone);
    }

    #[test]
    fn language_fingerprint_is_not_zero() {
        let joined = LANGUAGE_FINGERPRINT;

        assert_ne!(joined, 0);
    }
}
