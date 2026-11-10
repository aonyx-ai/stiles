use std::fmt;

/// The version of the plugin declaration protocol
///
/// A plugin carries the version it was built for at offset zero of its
/// declaration, and stiles compares it against its own [`ABI_VERSION`].
/// Stiles loads a plugin only on the version it was built for, in either
/// era. The eras differ in how a version is written:
///
/// - Before 1.0 a version is a major of 0 and a minor, such as `0.12`.
///   Every release that moved the boundary raises the minor.
/// - From 1.0 a version is one integer, such as `2`. The minor stays 0,
///   and every release that moved the boundary raises the major.
///
/// The fields are public rather than read through getters, for the
/// reason [`PluginDeclaration`]'s are. This is the head of a wire
/// format. The exporting macro writes it in a `const` context, and the
/// loader reads it field by field.
///
/// This type's own layout is the one thing no version can ever change.
/// Stiles reads these eight bytes before it knows anything else about a
/// library, so it reads them the same way whatever the plugin says. A
/// protocol that laid them out differently would be read as though it
/// had not, and the version stiles compared would be a fiction.
///
/// [`ABI_VERSION`]: crate::plugin::ABI_VERSION
/// [`PluginDeclaration`]: crate::plugin::PluginDeclaration
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::AbiVersion;
///
/// let version = AbiVersion { major: 0, minor: 12 };
///
/// assert_eq!(version.to_string(), "0.12");
/// ```
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct AbiVersion {
    /// The component every release that moved the boundary raises from 1.0
    pub major: u32,

    /// The component every release that moved the boundary raises before 1.0
    pub minor: u32,
}

impl fmt::Display for AbiVersion {
    /// Writes `0.12` before 1.0 and `2` from 1.0
    ///
    /// A minor from 1.0 is not a version stiles writes. Such a value is
    /// still written in full, so that a message never hides the part
    /// that differs.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { major, minor } = self;

        match (major, minor) {
            (0, minor) => write!(f, "0.{minor}"),
            (major, 0) => write!(f, "{major}"),
            (major, minor) => write!(f, "{major}.{minor}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::offset_of;

    use super::*;

    /// Pins the layout every protocol is read through
    ///
    /// Stiles reads these bytes before it knows which protocol wrote them.
    /// A field that moved would make a plugin claim a version it never
    /// named.
    #[test]
    fn major_sits_first() {
        assert_eq!(offset_of!(AbiVersion, major), 0);
    }

    #[test]
    fn minor_sits_after_the_major() {
        assert_eq!(offset_of!(AbiVersion, minor), 4);
    }

    #[test]
    fn the_pair_is_eight_bytes() {
        assert_eq!(size_of::<AbiVersion>(), 8);
    }

    #[test]
    fn to_string_before_one_point_zero_writes_the_minor() {
        let version = AbiVersion {
            major: 0,
            minor: 34,
        };

        let text = version.to_string();

        assert_eq!(text, "0.34");
    }

    #[test]
    fn to_string_from_one_point_zero_writes_one_integer() {
        let version = AbiVersion {
            major: 12,
            minor: 0,
        };

        let text = version.to_string();

        assert_eq!(text, "12");
    }

    /// No stiles writes such a version, so a message that met one should
    /// show all of it
    #[test]
    fn to_string_with_a_minor_from_one_point_zero_writes_both() {
        let version = AbiVersion { major: 1, minor: 3 };

        let text = version.to_string();

        assert_eq!(text, "1.3");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AbiVersion>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AbiVersion>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AbiVersion>();
    }
}
