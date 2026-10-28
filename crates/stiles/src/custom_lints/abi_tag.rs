use std::fmt;

use bon::bon;

use super::digest::digest;
use super::handshake::AbiIdentity;

/// The target triple this binary was compiled for
///
/// Nothing at runtime reports it, so the build script reads it from cargo
/// and bakes it in.
const TARGET: &str = env!("STILES_TARGET");

/// Names the stiles binary that a prebuilt lint library has to fit
///
/// The tag holds a digest of the two fingerprints [`super::handshake`]
/// compares and the floor of the protocol it reads, then the platform. A
/// publisher of prebuilt lints puts it in the name of each archive.
/// Stiles can therefore ask for a library that fits before it downloads
/// one.
///
/// An archive under this stiles's tag passes the handshake, with one
/// exception: a later stiles built it, and the protocol grew in
/// between. The tag carries the floor, and the handshake refuses a
/// plugin newer than the stiles that reads it. Before 1.0 the floor is
/// the whole version, so a tag names one protocol exactly.
///
/// The tag leaves the compiler out, so a library any rustc built fits,
/// and one archive serves every stiles built from the same boundary. A
/// stiles that no publisher built for finds no file at all.
///
/// A small digest suffices here. The handshake still decides whether a
/// library loads, so a collision costs one wasted download.
///
/// The name is a contract with publishers, and [`AbiTag::builder`] carries a
/// test that pins it.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AbiTag(String);

#[bon]
impl AbiTag {
    /// Returns the tag of the stiles binary that is running
    ///
    /// # Examples
    ///
    /// ```ignore
    /// println!("this stiles loads lints tagged {}", AbiTag::host());
    /// ```
    pub fn host() -> Self {
        Self::builder()
            .identity(&AbiIdentity::host())
            .target(TARGET)
            .build()
    }

    /// Returns the tag of a stiles with `identity` running on `target`
    ///
    /// A newline separates the values, and each fingerprint occupies a
    /// fixed width. Two different identities therefore cannot produce one
    /// input to the digest.
    #[builder]
    pub(super) fn new(identity: &AbiIdentity, target: &str) -> Self {
        let AbiIdentity {
            abi_version,
            types_fingerprint,
            language_fingerprint,
        } = identity;

        // The floor rather than the version stiles writes. Stiles reads
        // every protocol from the floor upward, so two stiles builds sharing
        // one accept each other's archives. Before 1.0 the floor is the
        // version itself, so every release asks for its own archives.
        let floor = abi_version.floor();

        let key = digest(&format!(
            "{floor}\n{types_fingerprint:016x}\n{language_fingerprint:016x}"
        ));

        Self(format!("{key}-{target}"))
    }
}

impl fmt::Display for AbiTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use stiles_plugin::AbiVersion;

    use super::*;

    /// An identity from after 1.0, where a minor and a major differ in
    /// what they strand
    fn identity() -> AbiIdentity {
        AbiIdentity {
            abi_version: AbiVersion { major: 1, minor: 4 },
            types_fingerprint: 0x0123_4567_89ab_cdef,
            language_fingerprint: 0xfedc_ba98_7654_3210,
        }
    }

    #[test]
    fn host_names_the_platform_the_binary_was_built_for() {
        let tag = AbiTag::host().to_string();

        assert!(tag.ends_with(&format!("-{TARGET}")), "{tag}");
    }

    /// From 1.0 a minor is not part of the tag
    ///
    /// Stiles reads every protocol from the floor upward, so two
    /// stiles builds that share a major accept each other's archives. A minor
    /// must therefore not strand what a publisher already built.
    #[test]
    fn new_ignores_a_minor_from_one_point_zero_onward() {
        let other = AbiIdentity {
            abi_version: AbiVersion { major: 1, minor: 9 },
            ..identity()
        };

        assert_eq!(
            AbiTag::builder()
                .identity(&identity())
                .target("x86_64-unknown-linux-gnu")
                .build(),
            AbiTag::builder()
                .identity(&other)
                .target("x86_64-unknown-linux-gnu")
                .build()
        );
    }

    /// Pins the tag that publishers write into an archive name
    ///
    /// Stiles stops finding every archive that carries the old tag if
    /// this derivation changes. The test fails first, so whoever changes
    /// it knows to republish.
    #[test]
    fn new_is_stable_across_releases() {
        let tag = AbiTag::builder()
            .identity(&identity())
            .target("aarch64-apple-darwin")
            .build();

        assert_eq!(tag.to_string(), "b7d58f06e26e0a22-aarch64-apple-darwin");
    }

    #[test]
    fn new_separates_identities_that_differ_in_the_language_fingerprint() {
        let other = AbiIdentity {
            language_fingerprint: 1,
            ..identity()
        };

        assert_ne!(
            AbiTag::builder()
                .identity(&identity())
                .target("x86_64-unknown-linux-gnu")
                .build(),
            AbiTag::builder()
                .identity(&other)
                .target("x86_64-unknown-linux-gnu")
                .build()
        );
    }

    #[test]
    fn new_separates_identities_that_differ_in_the_major() {
        let other = AbiIdentity {
            abi_version: AbiVersion { major: 2, minor: 4 },
            ..identity()
        };

        assert_ne!(
            AbiTag::builder()
                .identity(&identity())
                .target("x86_64-unknown-linux-gnu")
                .build(),
            AbiTag::builder()
                .identity(&other)
                .target("x86_64-unknown-linux-gnu")
                .build()
        );
    }

    #[test]
    fn new_separates_identities_that_differ_in_the_types_fingerprint() {
        let other = AbiIdentity {
            types_fingerprint: 1,
            ..identity()
        };

        assert_ne!(
            AbiTag::builder()
                .identity(&identity())
                .target("x86_64-unknown-linux-gnu")
                .build(),
            AbiTag::builder()
                .identity(&other)
                .target("x86_64-unknown-linux-gnu")
                .build()
        );
    }

    /// Pins that one fingerprint cannot borrow a digit from the other
    ///
    /// At a variable width the two would run together. Two different
    /// identities would then share one tag.
    #[test]
    fn new_separates_identities_whose_fingerprints_are_shifted() {
        let first = AbiIdentity {
            types_fingerprint: 0x0000_0000_0000_0001,
            language_fingerprint: 0x0000_0000_0000_0023,
            ..identity()
        };
        let second = AbiIdentity {
            types_fingerprint: 0x0000_0000_0000_0012,
            language_fingerprint: 0x0000_0000_0000_0003,
            ..identity()
        };

        assert_ne!(
            AbiTag::builder()
                .identity(&first)
                .target("aarch64-apple-darwin")
                .build(),
            AbiTag::builder()
                .identity(&second)
                .target("aarch64-apple-darwin")
                .build()
        );
    }

    /// Before 1.0 a minor is part of the tag
    ///
    /// Nothing is promised there, so a plugin loads only on the stiles
    /// it was built for.
    #[test]
    fn new_separates_minors_before_one_point_zero() {
        let first = AbiIdentity {
            abi_version: AbiVersion { major: 0, minor: 1 },
            ..identity()
        };
        let second = AbiIdentity {
            abi_version: AbiVersion { major: 0, minor: 2 },
            ..identity()
        };

        assert_ne!(
            AbiTag::builder()
                .identity(&first)
                .target("x86_64-unknown-linux-gnu")
                .build(),
            AbiTag::builder()
                .identity(&second)
                .target("x86_64-unknown-linux-gnu")
                .build()
        );
    }

    #[test]
    fn new_separates_platforms() {
        assert_ne!(
            AbiTag::builder()
                .identity(&identity())
                .target("aarch64-apple-darwin")
                .build(),
            AbiTag::builder()
                .identity(&identity())
                .target("x86_64-unknown-linux-gnu")
                .build()
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AbiTag>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AbiTag>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AbiTag>();
    }
}
