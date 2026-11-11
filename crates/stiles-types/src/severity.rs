/// Severity level for a diagnostic
///
/// The discriminant is a byte. A severity crosses the plugin boundary
/// inside every [`Diagnostic`], and an undeclared representation has no
/// layout that holds from one compiler to the next.
///
/// # Examples
///
/// ```
/// use stiles_types::Severity;
///
/// assert!(Severity::Error > Severity::Warn);
/// ```
///
/// [`Diagnostic`]: crate::Diagnostic
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Severity {
    /// A help message
    Help,
    /// An informational note
    Info,
    /// A warning that should be addressed
    Warn,
    /// An error that must be addressed
    Error,
}

impl Severity {
    /// The byte that names [`Severity::Help`] at the plugin boundary
    ///
    /// Each byte is written out rather than taken from the discriminant.
    /// A plugin built against a newer stiles may name a severity this one
    /// lacks. The host must read that as unknown, not as a variant.
    const HELP: u8 = 1;
    /// The byte that names [`Severity::Info`] at the plugin boundary
    const INFO: u8 = 2;
    /// The byte that names [`Severity::Warn`] at the plugin boundary
    const WARN: u8 = 3;
    /// The byte that names [`Severity::Error`] at the plugin boundary
    const ERROR: u8 = 4;

    /// Returns the byte that names this severity at the plugin boundary
    pub(crate) const fn to_byte(self) -> u8 {
        match self {
            Self::Help => Self::HELP,
            Self::Info => Self::INFO,
            Self::Warn => Self::WARN,
            Self::Error => Self::ERROR,
        }
    }

    /// Returns the severity that `byte` names, or [`None`] for an unknown byte
    pub(crate) const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            Self::HELP => Some(Self::Help),
            Self::INFO => Some(Self::Info),
            Self::WARN => Some(Self::Warn),
            Self::ERROR => Some(Self::Error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_byte_with_an_unknown_byte_returns_none() {
        let read = Severity::from_byte(0);

        assert_eq!(read, None);
    }

    #[test]
    fn from_byte_with_the_byte_of_a_severity_returns_it() {
        let severities = [
            Severity::Help,
            Severity::Info,
            Severity::Warn,
            Severity::Error,
        ];

        let read = severities.map(|severity| Severity::from_byte(severity.to_byte()));

        assert_eq!(read, severities.map(Some));
    }

    #[test]
    fn ordering_puts_error_above_warn() {
        assert!(Severity::Error > Severity::Warn);
    }

    #[test]
    fn ordering_puts_info_above_help() {
        assert!(Severity::Info > Severity::Help);
    }

    #[test]
    fn ordering_puts_warn_above_info() {
        assert!(Severity::Warn > Severity::Info);
    }

    /// A plugin and the host agree on these bytes whatever order the
    /// variants are declared in
    #[test]
    fn to_byte_names_each_severity_by_a_fixed_byte() {
        let bytes = [
            Severity::Help,
            Severity::Info,
            Severity::Warn,
            Severity::Error,
        ]
        .map(Severity::to_byte);

        assert_eq!(bytes, [1, 2, 3, 4]);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Severity>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Severity>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Severity>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        fn arb_severity() -> impl Strategy<Value = Severity> {
            prop_oneof![
                Just(Severity::Help),
                Just(Severity::Info),
                Just(Severity::Warn),
                Just(Severity::Error),
            ]
        }

        proptest! {
            #[test]
            fn equal_variants_are_equal(a in arb_severity()) {
                #[allow(clippy::eq_op)]
                let is_eq = a == a;
                prop_assert!(is_eq);
            }

            #[test]
            fn ordering_is_transitive(
                a in arb_severity(),
                b in arb_severity(),
                c in arb_severity(),
            ) {
                if a <= b && b <= c {
                    prop_assert!(a <= c);
                }
            }

            #[test]
            fn equality_is_reflexive(a in arb_severity()) {
                prop_assert_eq!(a, a);
            }
        }
    }
}
