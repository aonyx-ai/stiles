use crate::plugin::{AbiVersion, Provenance, Registered, Registrar};

/// The entry point a custom lint plugin exports
///
/// A plugin exports exactly one static of this type under the symbol name
/// `stiles_plugin_declaration`. The `export_lints!` macro in stiles-rust
/// writes it. The loader reads the fields in declaration order and stops
/// at the first mismatch. Each field needs more agreement between the two
/// images than the field before it:
///
/// - [`abi_version`] is a pair of bare integers at offset zero of a
///   `#[repr(C)]` struct. The loader reads it whatever else changed.
/// - [`types_fingerprint`] and [`language_fingerprint`] are plain `u64`,
///   readable under any pair of compilers.
/// - [`provenance`] says whether the plugin was built from published
///   crates. When it and the host both were, the version alone vouches
///   for the layout, and the fingerprints are not compared.
/// - [`register`] is an `extern "C"` function pointer that takes the
///   host's [`Registrar`]. Calling it is sound once the version or the
///   fingerprints prove that both images lay out the boundary alike.
///
/// The fields are public rather than accessed through getters, because
/// this struct is a wire format. The exporting macro constructs it in a
/// `const` context, and the loader consumes it field by field.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::{
///     ABI_VERSION, PROVENANCE, PluginDeclaration, Registered, Registrar, TYPES_FINGERPRINT,
/// };
///
/// /// Registers no lints
/// extern "C" fn register(_registrar: &Registrar) -> Registered {
///     Registered::Ok(())
/// }
///
/// static DECLARATION: PluginDeclaration = PluginDeclaration {
///     abi_version: ABI_VERSION,
///     types_fingerprint: TYPES_FINGERPRINT,
///     language_fingerprint: 0,
///     register,
///     provenance: PROVENANCE,
/// };
///
/// assert_eq!(DECLARATION.abi_version, ABI_VERSION);
/// ```
///
/// [`abi_version`]: PluginDeclaration::abi_version
/// [`language_fingerprint`]: PluginDeclaration::language_fingerprint
/// [`Registrar`]: crate::plugin::Registrar
/// [`provenance`]: PluginDeclaration::provenance
/// [`register`]: PluginDeclaration::register
/// [`types_fingerprint`]: PluginDeclaration::types_fingerprint
#[repr(C)]
pub struct PluginDeclaration {
    /// The plugin's copy of [`ABI_VERSION`]
    ///
    /// [`ABI_VERSION`]: crate::plugin::ABI_VERSION
    pub abi_version: AbiVersion,

    /// The plugin's copy of [`TYPES_FINGERPRINT`]
    ///
    /// [`TYPES_FINGERPRINT`]: crate::plugin::TYPES_FINGERPRINT
    pub types_fingerprint: u64,

    /// The plugin's fingerprint of the language crate it was built against
    ///
    /// For Rust lints this is stiles-rust's fingerprint, which also covers
    /// the lint pass trait generated from the grammar's node types. A
    /// plugin whose dispatch was generated from a different grammar would
    /// not crash, but its checks would quietly never fire. The handshake
    /// turns that into a refusal.
    pub language_fingerprint: u64,

    /// Registers everything the plugin exports through the host's registrar
    ///
    /// The host calls this once, after the handshake, and keeps what the
    /// plugin registers for the life of the process. The plugin calls an
    /// entry of the [`Registrar`] for each lint, with its language, its
    /// factory, and its rules.
    ///
    /// This is the only function the declaration needs. After `dlopen` the
    /// host holds an address and a name, and no table. The first call must
    /// therefore be one whose convention both images agree on with no
    /// prior agreement. It hands over the host's table, and everything
    /// after it goes through tables.
    ///
    /// Anything a plugin declares later is a new entry of the registrar,
    /// not a new field here or a new field of a laid-out list. A
    /// `#[repr(C)]` table has offsets stiles can reason about per protocol,
    /// and a reader reaches only the entries its own length names.
    ///
    /// [`Registrar`]: crate::plugin::Registrar
    pub register: extern "C" fn(&Registrar) -> Registered,

    /// Whether every boundary crate of the plugin was published
    ///
    /// stiles-plugin joins the [`PROVENANCE`] of every crate it builds
    /// on, so a plugin that patched one of them is unpublished. The field
    /// follows [`register`], because the version that precedes it says
    /// which fields a declaration has.
    ///
    /// [`PROVENANCE`]: crate::plugin::PROVENANCE
    /// [`register`]: PluginDeclaration::register
    pub provenance: Provenance,
}

#[cfg(test)]
mod tests {
    use std::mem::offset_of;

    use super::*;

    #[test]
    fn abi_version_sits_at_offset_zero() {
        assert_eq!(offset_of!(PluginDeclaration, abi_version), 0);
    }

    /// A field appended later goes after this one, for the reason
    /// [`register_sits_after_every_field_before_it`] gives
    #[test]
    fn provenance_sits_last() {
        let provenance = offset_of!(PluginDeclaration, provenance);

        let before = [
            offset_of!(PluginDeclaration, abi_version),
            offset_of!(PluginDeclaration, types_fingerprint),
            offset_of!(PluginDeclaration, language_fingerprint),
            offset_of!(PluginDeclaration, register),
        ]
        .map(|offset| offset < provenance);

        assert_eq!(before, [true; 4]);
    }

    /// Pins that the fields keep the order the protocol was written in
    ///
    /// Stiles reads each field at the offset this struct gives. A field
    /// inserted among them would move the rest, and stiles would read one
    /// plugin's data as another field. That is silent: the fields are
    /// integers and pointers, and a wrong one is a wrong answer rather than
    /// a crash. A new field goes last, and a test pins each field behind
    /// the ones before it.
    #[test]
    fn register_sits_after_every_field_before_it() {
        let register = offset_of!(PluginDeclaration, register);

        let before = [
            offset_of!(PluginDeclaration, abi_version),
            offset_of!(PluginDeclaration, types_fingerprint),
            offset_of!(PluginDeclaration, language_fingerprint),
        ]
        .map(|offset| offset < register);

        assert_eq!(before, [true; 3]);
    }

    /// Ties the fields of the declaration to the version that names them
    ///
    /// No fingerprint reads the declaration, and two published sides
    /// compare only their versions. A field appended without a new version
    /// would make stiles read past the end of an older plugin's
    /// declaration. The pattern below names every field, so an added or
    /// removed one fails to compile here, next to the version it needs.
    #[test]
    fn the_fields_belong_to_the_abi_version() {
        fn fields(declaration: &PluginDeclaration) {
            let PluginDeclaration {
                abi_version: _,
                types_fingerprint: _,
                language_fingerprint: _,
                register: _,
                provenance: _,
            } = declaration;
        }
        let _ = fields;

        assert_eq!(
            crate::plugin::ABI_VERSION,
            AbiVersion {
                major: 0,
                minor: 12
            },
            "the fields of the declaration moved; bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PluginDeclaration>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PluginDeclaration>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PluginDeclaration>();
    }
}
