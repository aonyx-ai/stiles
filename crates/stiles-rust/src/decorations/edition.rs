/// Names the Rust edition that a crate compiles with
///
/// An edition changes what some code means. A rule that suggests a fix
/// reads it, because a fix that one edition accepts can fail to compile in
/// an older one. The variants follow their release order, so a rule can
/// compare two editions.
///
/// A rule reads it out of memory the host allocated, so the discriminant
/// is a byte.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::Edition;
///
/// let edition = Edition::Edition2021;
///
/// assert!(edition < Edition::Edition2024);
/// ```
#[stabby::stabby]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Edition {
    /// The 2015 edition, the default of a manifest that names none
    Edition2015,
    /// The 2018 edition
    Edition2018,
    /// The 2021 edition
    Edition2021,
    /// The 2024 edition
    Edition2024,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmp_orders_editions_by_release() {
        let mut editions = [
            Edition::Edition2024,
            Edition::Edition2015,
            Edition::Edition2021,
            Edition::Edition2018,
        ];

        editions.sort();

        assert_eq!(
            editions,
            [
                Edition::Edition2015,
                Edition::Edition2018,
                Edition::Edition2021,
                Edition::Edition2024,
            ]
        );
    }

    #[test]
    fn trait_send_edition() {
        fn assert_send<T: Send>() {}
        assert_send::<Edition>();
    }

    #[test]
    fn trait_sync_edition() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Edition>();
    }

    #[test]
    fn trait_unpin_edition() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Edition>();
    }
}
