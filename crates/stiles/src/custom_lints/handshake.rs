use std::fmt;

use bon::Builder;
use getset::CopyGetters;
use stiles_plugin::{self as plugin, AbiVersion, Provenance};
use stiles_types::plugin::stable_fingerprint;

/// The ABI-relevant identity of one side of the plugin boundary
///
/// Rust has no stable ABI of its own, so stabby lays out every type that
/// crosses the plugin boundary. A plugin and the stiles binary agree on
/// those layouts when the stiles source each was built from lays them
/// out the same way. Which rustc built each side does not enter into it.
/// This type holds those facts for one side, and [`validate`] compares
/// the two sides.
///
/// A published side is named by its version, such as `0.12`. An
/// unpublished side adds a short hash of its fingerprints, such as
/// `0.12-1a2b3c4d`, because one version holds more than one layout
/// between two releases.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Builder, CopyGetters)]
#[getset(get_copy = "pub")]
pub(crate) struct AbiIdentity {
    /// The version of the plugin declaration protocol
    abi_version: AbiVersion,
    /// The fingerprint of the types that cross the plugin boundary
    types_fingerprint: u64,
    /// The fingerprint of the language crate that a plugin builds against
    language_fingerprint: u64,
    /// Whether a release vouches for the version
    provenance: Provenance,
}

impl AbiIdentity {
    /// Returns the identity baked into this stiles binary
    pub(crate) fn host() -> Self {
        Self {
            abi_version: plugin::ABI_VERSION,
            types_fingerprint: plugin::TYPES_FINGERPRINT,
            language_fingerprint: plugin::LANGUAGE_FINGERPRINT,
            provenance: plugin::PROVENANCE,
        }
    }
}

impl fmt::Display for AbiIdentity {
    /// Writes the version, and for an unpublished side a short hash of
    /// its fingerprints
    ///
    /// Eight hex digits tell two builds apart in a message. The handshake
    /// compares the full fingerprints, so a collision costs a reader a
    /// second look and never a wrong load.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            abi_version,
            types_fingerprint,
            language_fingerprint,
            provenance,
        } = self;

        match provenance.is_published() {
            true => write!(f, "{abi_version}"),
            false => {
                let layout = stable_fingerprint(&[*types_fingerprint, *language_fingerprint]);

                write!(f, "{abi_version}-{:08x}", layout >> 32)
            }
        }
    }
}

/// Accepts a plugin only when its identity matches the host's
///
/// The checks run in the order in which the fields of the declaration
/// become trustworthy. The first mismatch wins, so the reported error is
/// the one whose remedy applies.
///
/// The version comes first, because it says which fields the declaration
/// has. When both sides are published, a release vouches that the version
/// names one layout, and the handshake trusts it. Otherwise the version
/// may hold another layout than the host's, and the fingerprints decide.
///
/// # Errors
///
/// Returns the first [`HandshakeMismatch`] between the two identities.
pub(crate) fn validate(host: &AbiIdentity, plugin: &AbiIdentity) -> Result<(), HandshakeMismatch> {
    if host.abi_version != plugin.abi_version {
        return Err(HandshakeMismatch::AbiVersion {
            plugin: plugin.abi_version,
            host: host.clone(),
        });
    }

    if Provenance::all(&[host.provenance, plugin.provenance]).is_published() {
        return Ok(());
    }

    if host.types_fingerprint != plugin.types_fingerprint {
        return Err(HandshakeMismatch::TypesFingerprint {
            plugin: plugin.clone(),
            host: host.clone(),
        });
    }

    if host.language_fingerprint != plugin.language_fingerprint {
        return Err(HandshakeMismatch::LanguageFingerprint {
            plugin: plugin.clone(),
            host: host.clone(),
        });
    }

    Ok(())
}

/// A difference between the plugin's ABI identity and the host's
///
/// Every variant is a refusal to load: the two images may lay a type out
/// differently, and acting on that difference is undefined behavior. Each
/// variant names both sides, so the user can tell which one to rebuild.
/// A version mismatch names only the plugin's version, because stiles
/// reads nothing else from a declaration of another protocol.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum HandshakeMismatch {
    AbiVersion {
        plugin: AbiVersion,
        host: AbiIdentity,
    },
    TypesFingerprint {
        plugin: AbiIdentity,
        host: AbiIdentity,
    },
    LanguageFingerprint {
        plugin: AbiIdentity,
        host: AbiIdentity,
    },
}

impl fmt::Display for HandshakeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandshakeMismatch::AbiVersion { plugin, host } => write!(
                f,
                "plugin (ABI {plugin}) incompatible with stiles (ABI {host})"
            ),
            HandshakeMismatch::TypesFingerprint { plugin, host } => write!(
                f,
                "plugin (ABI {plugin}) built against another stiles-types than stiles (ABI \
                 {host}); match its stiles pin"
            ),
            HandshakeMismatch::LanguageFingerprint { plugin, host } => write!(
                f,
                "plugin (ABI {plugin}) built against another stiles-rust than stiles (ABI \
                 {host}); match its stiles pin"
            ),
        }
    }
}

impl std::error::Error for HandshakeMismatch {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns an unpublished identity at this binary's version
    fn identity() -> AbiIdentity {
        AbiIdentity {
            abi_version: plugin::ABI_VERSION,
            types_fingerprint: 0xaa,
            language_fingerprint: 0xbb,
            provenance: Provenance::UNPUBLISHED,
        }
    }

    /// Returns a published identity at this binary's version
    fn published() -> AbiIdentity {
        AbiIdentity {
            provenance: Provenance::PUBLISHED,
            ..identity()
        }
    }

    /// Returns a version that a stiles at [`plugin::ABI_VERSION`] refuses
    fn other_version() -> AbiVersion {
        AbiVersion {
            major: plugin::ABI_VERSION.major + 1,
            minor: 0,
        }
    }

    #[test]
    fn an_abi_mismatch_names_both_versions() {
        let error = HandshakeMismatch::AbiVersion {
            plugin: AbiVersion { major: 0, minor: 2 },
            host: AbiIdentity {
                abi_version: AbiVersion { major: 0, minor: 1 },
                ..published()
            },
        };

        let message = error.to_string();

        assert_eq!(
            message,
            "plugin (ABI 0.2) incompatible with stiles (ABI 0.1)"
        );
    }

    #[test]
    fn host_reports_the_baked_in_abi_version() {
        let host = AbiIdentity::host();

        assert_eq!(host.abi_version, plugin::ABI_VERSION);
    }

    #[test]
    fn host_reports_the_baked_in_language_fingerprint() {
        let host = AbiIdentity::host();

        assert_eq!(host.language_fingerprint, plugin::LANGUAGE_FINGERPRINT);
    }

    #[test]
    fn host_reports_the_baked_in_provenance() {
        let host = AbiIdentity::host();

        assert_eq!(host.provenance, plugin::PROVENANCE);
    }

    #[test]
    fn host_reports_the_baked_in_types_fingerprint() {
        let host = AbiIdentity::host();

        assert_eq!(host.types_fingerprint, plugin::TYPES_FINGERPRINT);
    }

    #[test]
    fn to_string_of_a_published_identity_is_the_version() {
        let identity = AbiIdentity {
            abi_version: AbiVersion {
                major: 0,
                minor: 12,
            },
            ..published()
        };

        let text = identity.to_string();

        assert_eq!(text, "0.12");
    }

    #[test]
    fn to_string_of_an_unpublished_identity_adds_a_hash() {
        let identity = AbiIdentity {
            abi_version: AbiVersion {
                major: 0,
                minor: 12,
            },
            ..identity()
        };

        let text = identity.to_string();

        assert_eq!(text, "0.12-7b711703");
    }

    #[test]
    fn to_string_of_unpublished_identities_with_other_layouts_differs() {
        let first = identity();
        let second = AbiIdentity {
            types_fingerprint: 0xcc,
            ..identity()
        };

        let texts = [first.to_string(), second.to_string()];

        assert_ne!(texts[0], texts[1]);
    }

    #[test]
    fn trait_send_abi_identity() {
        fn assert_send<T: Send>() {}
        assert_send::<AbiIdentity>();
    }

    #[test]
    fn trait_send_handshake_mismatch() {
        fn assert_send<T: Send>() {}
        assert_send::<HandshakeMismatch>();
    }

    #[test]
    fn trait_sync_abi_identity() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AbiIdentity>();
    }

    #[test]
    fn trait_sync_handshake_mismatch() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<HandshakeMismatch>();
    }

    #[test]
    fn trait_unpin_abi_identity() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AbiIdentity>();
    }

    #[test]
    fn trait_unpin_handshake_mismatch() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<HandshakeMismatch>();
    }

    #[test]
    fn validate_matching_identities_succeeds() {
        validate(&identity(), &identity()).expect("should match");
    }

    /// A release vouches for the layout of its version, so two published
    /// sides do not compare fingerprints
    #[test]
    fn validate_published_sides_with_differing_fingerprints_succeeds() {
        let plugin = AbiIdentity {
            types_fingerprint: 0xcc,
            language_fingerprint: 0xdd,
            ..published()
        };

        validate(&published(), &plugin).expect("should trust the version");
    }

    #[test]
    fn validate_reports_a_differing_language_fingerprint() {
        let plugin = AbiIdentity {
            language_fingerprint: 0xcc,
            ..identity()
        };

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(
            error,
            HandshakeMismatch::LanguageFingerprint {
                plugin,
                host: identity(),
            }
        );
    }

    #[test]
    fn validate_reports_a_differing_types_fingerprint() {
        let plugin = AbiIdentity {
            types_fingerprint: 0xcc,
            ..identity()
        };

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(
            error,
            HandshakeMismatch::TypesFingerprint {
                plugin,
                host: identity(),
            }
        );
    }

    /// A version says which fields a declaration has, so even two
    /// published sides must agree on it
    #[test]
    fn validate_reports_a_differing_version_between_published_sides() {
        let plugin = AbiIdentity {
            abi_version: other_version(),
            ..published()
        };

        let error = validate(&published(), &plugin).expect_err("should mismatch");

        assert_eq!(
            error,
            HandshakeMismatch::AbiVersion {
                plugin: other_version(),
                host: published(),
            }
        );
    }

    /// A version outside the one stiles reads is reported before anything
    /// else. The fields that the rest reads are only readable once the shape
    /// of the declaration is known
    #[test]
    fn validate_reports_an_unsupported_abi_version_first() {
        let plugin = AbiIdentity {
            abi_version: other_version(),
            types_fingerprint: 0xcc,
            ..identity()
        };

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(
            error,
            HandshakeMismatch::AbiVersion {
                plugin: other_version(),
                host: identity(),
            }
        );
    }

    /// One unpublished side is enough to make the fingerprints decide,
    /// because its version may hold another layout
    #[test]
    fn validate_unpublished_host_compares_a_published_plugin_s_fingerprints() {
        let plugin = AbiIdentity {
            types_fingerprint: 0xcc,
            ..published()
        };

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(
            error,
            HandshakeMismatch::TypesFingerprint {
                plugin,
                host: identity(),
            }
        );
    }

    #[test]
    fn validate_unpublished_plugin_on_a_published_host_compares_fingerprints() {
        let plugin = AbiIdentity {
            language_fingerprint: 0xcc,
            ..identity()
        };

        let error = validate(&published(), &plugin).expect_err("should mismatch");

        assert_eq!(
            error,
            HandshakeMismatch::LanguageFingerprint {
                plugin,
                host: published(),
            }
        );
    }

    #[test]
    fn validate_with_a_differing_language_fingerprint_names_the_language_crate() {
        let plugin = AbiIdentity {
            language_fingerprint: 0xcc,
            ..identity()
        };

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert!(error.to_string().contains("stiles-rust"));
    }

    #[test]
    fn validate_with_a_differing_types_fingerprint_names_the_types_crate() {
        let plugin = AbiIdentity {
            types_fingerprint: 0xcc,
            ..identity()
        };

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert!(error.to_string().contains("stiles-types"));
    }
}
