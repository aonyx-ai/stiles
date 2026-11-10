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
/// The tag holds a digest of the version and the two fingerprints of
/// [`super::handshake`], then the platform. A publisher of prebuilt lints
/// puts it in the name of each archive. Stiles can therefore ask for a
/// library that fits before it downloads one, and an archive under this
/// stiles's tag passes the handshake.
///
/// The tag holds the fingerprints even though two published sides compare
/// only their versions. A release fixes the fingerprints of its version,
/// so the tag of a published stiles names the same archives either way.
/// A stiles built from unpublished source asks for archives of its own
/// layout, which no publisher of a release built.
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
pub(crate) struct AbiTag(String);

#[bon]
impl AbiTag {
    /// Returns the tag of the stiles binary that is running
    ///
    /// # Examples
    ///
    /// ```ignore
    /// println!("this stiles loads lints tagged {}", AbiTag::host());
    /// ```
    pub(crate) fn host() -> Self {
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
    ///
    /// Stiles loads a plugin only on its own version, so every version
    /// that moved the boundary asks for archives of its own.
    #[builder]
    pub(super) fn new(identity: &AbiIdentity, target: &str) -> Self {
        let version = identity.abi_version();
        let types_fingerprint = identity.types_fingerprint();
        let language_fingerprint = identity.language_fingerprint();

        let key = digest(&format!(
            "{version}\n{types_fingerprint:016x}\n{language_fingerprint:016x}"
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
    use stiles_plugin::{AbiVersion, Provenance};

    use super::*;

    /// An identity from after 1.0, where a version is one integer
    fn identity() -> AbiIdentity {
        AbiIdentity::builder()
            .abi_version(AbiVersion { major: 1, minor: 0 })
            .types_fingerprint(0x0123_4567_89ab_cdef)
            .language_fingerprint(0xfedc_ba98_7654_3210)
            .provenance(Provenance::PUBLISHED)
            .build()
    }

    #[test]
    fn host_names_the_platform_the_binary_was_built_for() {
        let tag = AbiTag::host().to_string();

        assert!(tag.ends_with(&format!("-{TARGET}")), "{tag}");
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

        assert_eq!(tag.to_string(), "6bbaccb767bb687c-aarch64-apple-darwin");
    }

    #[test]
    fn new_separates_identities_that_differ_in_the_language_fingerprint() {
        let other = AbiIdentity::builder()
            .abi_version(AbiVersion { major: 1, minor: 0 })
            .types_fingerprint(0x0123_4567_89ab_cdef)
            .language_fingerprint(1)
            .provenance(Provenance::PUBLISHED)
            .build();

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
        let other = AbiIdentity::builder()
            .abi_version(AbiVersion { major: 2, minor: 0 })
            .types_fingerprint(0x0123_4567_89ab_cdef)
            .language_fingerprint(0xfedc_ba98_7654_3210)
            .provenance(Provenance::PUBLISHED)
            .build();

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
        let other = AbiIdentity::builder()
            .abi_version(AbiVersion { major: 1, minor: 0 })
            .types_fingerprint(1)
            .language_fingerprint(0xfedc_ba98_7654_3210)
            .provenance(Provenance::PUBLISHED)
            .build();

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
        let first = AbiIdentity::builder()
            .abi_version(AbiVersion { major: 1, minor: 0 })
            .types_fingerprint(0x0000_0000_0000_0001)
            .language_fingerprint(0x0000_0000_0000_0023)
            .provenance(Provenance::PUBLISHED)
            .build();
        let second = AbiIdentity::builder()
            .abi_version(AbiVersion { major: 1, minor: 0 })
            .types_fingerprint(0x0000_0000_0000_0012)
            .language_fingerprint(0x0000_0000_0000_0003)
            .provenance(Provenance::PUBLISHED)
            .build();

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
    /// A plugin loads only on the version it was built for, and before
    /// 1.0 the minor is what tells two versions apart.
    #[test]
    fn new_separates_minors_before_one_point_zero() {
        let first = AbiIdentity::builder()
            .abi_version(AbiVersion { major: 0, minor: 1 })
            .types_fingerprint(0x0123_4567_89ab_cdef)
            .language_fingerprint(0xfedc_ba98_7654_3210)
            .provenance(Provenance::PUBLISHED)
            .build();
        let second = AbiIdentity::builder()
            .abi_version(AbiVersion { major: 0, minor: 2 })
            .types_fingerprint(0x0123_4567_89ab_cdef)
            .language_fingerprint(0xfedc_ba98_7654_3210)
            .provenance(Provenance::PUBLISHED)
            .build();

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
