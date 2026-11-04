use stabby::IStable;

use crate::{Decoration, DecorationKey};

/// The FNV-1a offset basis that every fold here starts from
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// Returns the fingerprint of one decoration type: its layout and its key
///
/// A plugin finds a decoration by [`Decoration::KEY`] before stabby
/// checks the layout of what it found. Two images that agree on the layout
/// but not on the key therefore load each other, and every lookup of the
/// plugin returns nothing. Its rules then stop reporting, and nothing says
/// why. This joins [`IStable::ID`] with a hash of the key, so a language
/// crate that lists its decorations through it refuses such a plugin
/// instead.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::decoration_fingerprint;
/// use stiles_types::{DecoratedNode, Decoration, DecorationKey};
///
/// #[stabby::stabby]
/// struct Signature(u32);
///
/// impl Decoration for Signature {
///     const KEY: DecorationKey = DecorationKey::new("my_crate::Signature");
///
///     type Ref<'a> = Option<&'a Self>;
///
///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
///         node.decoration::<Self>()
///     }
/// }
///
/// const FINGERPRINT: u64 = decoration_fingerprint::<Signature>();
///
/// assert_ne!(FINGERPRINT, 0);
/// ```
///
/// [`IStable::ID`]: stabby::IStable::ID
pub const fn decoration_fingerprint<T: Decoration>() -> u64 {
    keyed_fingerprint(<T as IStable>::ID, T::KEY)
}

/// Joins the stabby identity of a type with the key it is stored under
const fn keyed_fingerprint(id: u64, key: DecorationKey) -> u64 {
    let key = mix(FNV_OFFSET_BASIS, key.as_str().as_bytes());

    stable_fingerprint(&[id, key])
}

/// Combines the identities stabby derived for a list of types
///
/// A type that stabby lays out carries [`IStable::ID`], a hash over its
/// report. The report names the type, its module, and the name and type of
/// every field, recursively. This folds those identities into one value,
/// so a boundary made of such types hashes to one number. The length goes
/// in first, and the order matters. Two lists that hold the same identities
/// in another order therefore hash apart, as do a list and its own prefix.
///
/// The fold is FNV-1a, a handful of const operations. It detects drift
/// between two builds of the same project, and a plugin that misreports
/// its layout can report the host's number.
///
/// # Examples
///
/// ```
/// use stabby::IStable;
/// use stiles_types::plugin::stable_fingerprint;
///
/// const A: u64 = stable_fingerprint(&[<u32 as IStable>::ID]);
/// const B: u64 = stable_fingerprint(&[<u64 as IStable>::ID]);
///
/// assert_ne!(A, B);
/// ```
///
/// [`IStable::ID`]: stabby::IStable::ID
pub const fn stable_fingerprint(ids: &[u64]) -> u64 {
    let mut hash = mix(FNV_OFFSET_BASIS, &(ids.len() as u64).to_le_bytes());

    let mut index = 0;
    while index < ids.len() {
        hash = mix(hash, &ids[index].to_le_bytes());
        index += 1;
    }

    hash
}

/// Folds a run of bytes into a running FNV-1a hash
const fn mix(hash: u64, bytes: &[u8]) -> u64 {
    let mut hash = hash;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_fingerprint_distinguishes_a_changed_id() {
        let one = keyed_fingerprint(1, DecorationKey::new("tests::Flag#00"));

        let other = keyed_fingerprint(2, DecorationKey::new("tests::Flag#00"));

        assert_ne!(one, other);
    }

    #[test]
    fn keyed_fingerprint_distinguishes_a_changed_key() {
        let one = keyed_fingerprint(1, DecorationKey::new("tests::Flag#00"));

        let other = keyed_fingerprint(1, DecorationKey::new("tests::Flag#01"));

        assert_ne!(one, other);
    }

    #[test]
    fn stable_fingerprint_distinguishes_a_changed_id() {
        let one = stable_fingerprint(&[1, 2]);

        let other = stable_fingerprint(&[1, 3]);

        assert_ne!(one, other);
    }

    #[test]
    fn stable_fingerprint_of_no_ids_is_not_zero() {
        let empty = stable_fingerprint(&[]);

        assert_ne!(empty, 0);
    }

    #[test]
    fn stable_fingerprint_of_the_same_ids_is_stable() {
        let ids = [1, 2, 3];

        let repeated = stable_fingerprint(&ids);

        assert_eq!(stable_fingerprint(&ids), repeated);
    }

    #[test]
    fn stable_fingerprint_orders_its_ids() {
        let one = stable_fingerprint(&[1, 2]);

        let other = stable_fingerprint(&[2, 1]);

        assert_ne!(one, other);
    }

    #[test]
    fn stable_fingerprint_separates_a_list_from_its_prefix() {
        let short = stable_fingerprint(&[1]);

        let long = stable_fingerprint(&[1, 0]);

        assert_ne!(short, long);
    }
}
