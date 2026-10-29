use std::sync::OnceLock;

use crate::decoration_entry::Entry;
use crate::{Decoration, DecorationLookup};

/// Storage for per-node decorations on a syntax tree
///
/// A decoration provider attaches semantic annotations to tree-sitter
/// nodes, and this map holds them. Each node (keyed by its `id()`) can
/// carry any number of decorations of different types.
///
/// The entries sit in one list in insertion order. A lookup needs them
/// grouped by node. The map therefore seals a sorted list of positions into
/// that list the first time anyone reads it. A later insertion drops the
/// seal. Sealing reads the entries and never moves them, so a shared
/// borrow is enough to build it.
///
/// A plugin searches the sealed index itself. The alternative is to hand a
/// plugin a pointer to a [`HashMap`] and a function of the host that reads
/// it. That needs a raw pointer and a cast that the compiler cannot check.
///
/// # Examples
///
/// ```
/// use stiles_types::{DecoratedNode, Decoration, DecorationKey, DecorationMap};
///
/// /// The name of a binding
/// #[stabby::stabby]
/// struct Name(u32);
///
/// impl Decoration for Name {
///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Name"));
///
///     type Ref<'a> = Option<&'a Self>;
///
///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
///         node.decoration::<Self>()
///     }
/// }
///
/// let mut map = DecorationMap::new();
/// map.insert(7, Name(1));
///
/// assert_eq!(map.get::<Name>(7).map(|Name(name)| *name), Some(1));
/// ```
///
/// [`HashMap`]: std::collections::HashMap
#[derive(Debug, Default)]
pub struct DecorationMap {
    entries: Vec<Entry>,
    sealed: OnceLock<Vec<usize>>,
}

impl DecorationMap {
    /// Creates an empty decoration map
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, Decoration, DecorationKey, DecorationMap};
    ///
    /// /// The name of a binding
    /// #[stabby::stabby]
    /// struct Name(u32);
    ///
    /// impl Decoration for Name {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Name"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// let map = DecorationMap::new();
    ///
    /// assert!(map.get::<Name>(7).is_none());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches a decoration to the node with the given tree-sitter node ID
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, Decoration, DecorationKey, DecorationMap};
    ///
    /// /// The name of a binding
    /// #[stabby::stabby]
    /// struct Name(u32);
    ///
    /// impl Decoration for Name {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Name"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// let mut map = DecorationMap::new();
    ///
    /// map.insert(7, Name(1));
    ///
    /// assert!(map.get::<Name>(7).is_some());
    /// ```
    pub fn insert<T: Decoration>(&mut self, node_id: usize, value: T) {
        self.entries
            .push(Entry::builder().node_id(node_id).value(value).build());
        self.sealed.take();
    }

    /// Retrieves the first decoration of type `T` attached to the given node
    ///
    /// Returns [`None`] if no decoration of that type exists for this node.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, Decoration, DecorationKey, DecorationMap};
    ///
    /// /// The name of a binding
    /// #[stabby::stabby]
    /// struct Name(u32);
    ///
    /// impl Decoration for Name {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Name"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// let mut map = DecorationMap::new();
    /// map.insert(7, Name(1));
    /// map.insert(7, Name(2));
    ///
    /// assert_eq!(map.get::<Name>(7).map(|Name(name)| *name), Some(1));
    /// ```
    pub fn get<T: Decoration>(&self, node_id: usize) -> Option<&T> {
        self.lookup().first::<T>(node_id)
    }

    /// Retrieves all decorations of type `T` attached to the given node
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, Decoration, DecorationKey, DecorationMap};
    ///
    /// /// The name of a binding
    /// #[stabby::stabby]
    /// struct Name(u32);
    ///
    /// impl Decoration for Name {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Name"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// let mut map = DecorationMap::new();
    /// map.insert(7, Name(1));
    /// map.insert(7, Name(2));
    ///
    /// assert_eq!(map.get_all::<Name>(7).len(), 2);
    /// ```
    pub fn get_all<T: Decoration>(&self, node_id: usize) -> Vec<&T> {
        self.lookup().get_all::<T>(node_id)
    }

    /// Merges another map into this one
    ///
    /// Entries already present win: [`DecorationMap::get`] returns the
    /// first decoration of a type for a node. [`DecorationMap::get_all`]
    /// still returns the merged entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_types::{DecoratedNode, Decoration, DecorationKey, DecorationMap};
    ///
    /// /// The name of a binding
    /// #[stabby::stabby]
    /// struct Name(u32);
    ///
    /// impl Decoration for Name {
    ///     const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Name"));
    ///
    ///     type Ref<'a> = Option<&'a Self>;
    ///
    ///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
    ///         node.decoration::<Self>()
    ///     }
    /// }
    ///
    /// let mut map = DecorationMap::new();
    /// map.insert(7, Name(1));
    /// let mut other = DecorationMap::new();
    /// other.insert(7, Name(2));
    ///
    /// map.merge(other);
    ///
    /// assert_eq!(map.get::<Name>(7).map(|Name(name)| *name), Some(1));
    /// ```
    pub fn merge(&mut self, other: DecorationMap) {
        self.entries.extend(other.entries);
        self.sealed.take();
    }

    /// Returns a handle that reads this map, sealing the index if needed
    ///
    /// The handle crosses the plugin boundary inside every
    /// [`DecoratedNode`], and it borrows this map for as long as it lives.
    ///
    /// [`DecoratedNode`]: crate::DecoratedNode
    pub(crate) fn lookup(&self) -> DecorationLookup<'_> {
        let order = self.sealed.get_or_init(|| {
            let mut order: Vec<usize> = (0..self.entries.len()).collect();
            order.sort_by_key(|&at| self.entries[at].node_id());
            order
        });

        DecorationLookup::of()
            .order(order)
            .entries(&self.entries)
            .call()
    }
}

#[cfg(test)]
mod tests {
    use stabby::string;

    use super::*;
    use crate::{DecoratedNode, DecorationKey};

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct TypeInfo(string::String);

    impl Decoration for TypeInfo {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::TypeInfo"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Scope(u32);

    impl Decoration for Scope {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Scope"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[test]
    fn get_all_on_missing_node_returns_empty() {
        let map = DecorationMap::new();

        let results = map.get_all::<TypeInfo>(99);

        assert!(results.is_empty());
    }

    #[test]
    fn get_all_returns_multiple_same_type() {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("i32".into()));
        map.insert(1, TypeInfo("u64".into()));

        let results = map.get_all::<TypeInfo>(1);

        assert_eq!(results, [&TypeInfo("i32".into()), &TypeInfo("u64".into())]);
    }

    #[test]
    fn get_on_empty_map_returns_none() {
        let map = DecorationMap::new();
        assert!(map.get::<TypeInfo>(0).is_none());
    }

    #[test]
    fn get_retrieves_a_zero_sized_decoration() {
        #[stabby::stabby]
        #[derive(Eq, PartialEq, Debug)]
        struct Present;

        impl Decoration for Present {
            const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Present"));

            type Ref<'a> = Option<&'a Self>;

            fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
                node.decoration::<Self>()
            }
        }

        let mut map = DecorationMap::new();
        map.insert(7, Present);

        assert_eq!(map.get::<Present>(7), Some(&Present));
    }

    #[test]
    fn get_retrieves_inserted_decoration() {
        let mut map = DecorationMap::new();
        map.insert(42, TypeInfo("bool".into()));

        let result = map.get::<TypeInfo>(42).expect("should find decoration");
        assert_eq!(result.0, "bool");
    }

    #[test]
    fn get_with_wrong_node_returns_none() {
        let mut map = DecorationMap::new();
        map.insert(42, TypeInfo("bool".into()));

        assert!(map.get::<TypeInfo>(99).is_none());
    }

    #[test]
    fn get_with_wrong_type_returns_none() {
        let mut map = DecorationMap::new();
        map.insert(42, TypeInfo("bool".into()));

        assert!(map.get::<Scope>(42).is_none());
    }

    #[test]
    fn merge_with_conflicting_type_keeps_both_in_order() {
        let map = merged_conflict();

        let all = map.get_all::<TypeInfo>(1);

        assert_eq!(all, [&TypeInfo("first".into()), &TypeInfo("second".into())]);
    }

    #[test]
    fn merge_with_conflicting_type_keeps_first() {
        let map = merged_conflict();

        let first = map.get::<TypeInfo>(1);

        assert_eq!(first, Some(&TypeInfo("first".into())));
    }

    #[test]
    fn merge_with_disjoint_nodes_keeps_other_node() {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("i32".into()));
        let mut other = DecorationMap::new();
        other.insert(2, TypeInfo("u64".into()));

        map.merge(other);

        assert_eq!(map.get::<TypeInfo>(2), Some(&TypeInfo("u64".into())));
    }

    /// Returns a map that holds `first` on node 1, merged with one that
    /// holds `second` on the same node
    fn merged_conflict() -> DecorationMap {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("first".into()));
        let mut other = DecorationMap::new();
        other.insert(1, TypeInfo("second".into()));

        map.merge(other);

        map
    }

    #[test]
    fn merge_with_disjoint_nodes_keeps_own_node() {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("i32".into()));
        let mut other = DecorationMap::new();
        other.insert(2, TypeInfo("u64".into()));

        map.merge(other);

        assert_eq!(map.get::<TypeInfo>(1), Some(&TypeInfo("i32".into())));
    }

    #[test]
    fn merge_with_empty_other_is_noop() {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("i32".into()));

        map.merge(DecorationMap::new());

        assert_eq!(map.get_all::<TypeInfo>(1), [&TypeInfo("i32".into())]);
    }

    #[test]
    fn merge_with_same_node_different_types_keeps_other_type() {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("i32".into()));
        let mut other = DecorationMap::new();
        other.insert(1, Scope(3));

        map.merge(other);

        assert_eq!(map.get::<Scope>(1), Some(&Scope(3)));
    }

    #[test]
    fn merge_with_same_node_different_types_keeps_own_type() {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("i32".into()));
        let mut other = DecorationMap::new();
        other.insert(1, Scope(3));

        map.merge(other);

        assert_eq!(map.get::<TypeInfo>(1), Some(&TypeInfo("i32".into())));
    }

    #[test]
    fn multiple_types_on_same_node_keep_the_first_type() {
        let mut map = DecorationMap::new();

        map.insert(1, TypeInfo("i32".into()));
        map.insert(1, Scope(3));

        assert_eq!(map.get::<TypeInfo>(1), Some(&TypeInfo("i32".into())));
    }

    #[test]
    fn multiple_types_on_same_node_keep_the_second_type() {
        let mut map = DecorationMap::new();

        map.insert(1, TypeInfo("i32".into()));
        map.insert(1, Scope(3));

        assert_eq!(map.get::<Scope>(1), Some(&Scope(3)));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecorationMap>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecorationMap>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorationMap>();
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        #[stabby::stabby]
        #[derive(Eq, PartialEq, Debug)]
        struct Value(u64);

        impl Decoration for Value {
            const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Value"));

            type Ref<'a> = Option<&'a Self>;

            fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
                node.decoration::<Self>()
            }
        }

        #[stabby::stabby]
        #[derive(Eq, PartialEq, Debug)]
        struct Count(u32);

        impl Decoration for Count {
            const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Count"));

            type Ref<'a> = Option<&'a Self>;

            fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
                node.decoration::<Self>()
            }
        }

        #[stabby::stabby]
        #[derive(Eq, PartialEq, Debug)]
        struct Signed(i64);

        impl Decoration for Signed {
            const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Signed"));

            type Ref<'a> = Option<&'a Self>;

            fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
                node.decoration::<Self>()
            }
        }

        proptest! {
            #[test]
            fn insert_then_get_roundtrips(node_id: usize, value: u64) {
                let mut map = DecorationMap::new();
                map.insert(node_id, Value(value));

                let result = map.get::<Value>(node_id);
                prop_assert_eq!(result, Some(&Value(value)));
            }

            #[test]
            fn get_on_different_node_returns_none(
                insert_id: usize,
                query_id: usize,
                value: u64,
            ) {
                prop_assume!(insert_id != query_id);
                let mut map = DecorationMap::new();
                map.insert(insert_id, Value(value));

                prop_assert!(map.get::<Value>(query_id).is_none());
            }

            #[test]
            fn get_with_wrong_type_returns_none(node_id: usize, value: u64) {
                let mut map = DecorationMap::new();
                map.insert(node_id, Value(value));

                prop_assert!(map.get::<Count>(node_id).is_none());
            }

            #[test]
            fn get_all_count_matches_insert_count(
                node_id: usize,
                values in proptest::collection::vec(any::<u32>(), 0..=20),
            ) {
                let mut map = DecorationMap::new();
                for v in &values {
                    map.insert(node_id, Count(*v));
                }

                let results = map.get_all::<Count>(node_id);
                prop_assert_eq!(results.len(), values.len());
            }

            #[test]
            fn get_all_preserves_insertion_order(
                node_id: usize,
                values in proptest::collection::vec(any::<u32>(), 1..=10),
            ) {
                let mut map = DecorationMap::new();
                for v in &values {
                    map.insert(node_id, Count(*v));
                }

                let results = map.get_all::<Count>(node_id);
                let result_values: Vec<u32> = results.into_iter().map(|c| c.0).collect();
                prop_assert_eq!(result_values, values);
            }

            #[test]
            fn merge_preserves_total_entry_count(
                left in proptest::collection::vec((any::<usize>(), any::<u32>()), 0..=20),
                right in proptest::collection::vec((any::<usize>(), any::<u32>()), 0..=20),
            ) {
                fn total(map: &DecorationMap) -> usize {
                    map.entries.len()
                }

                let mut map = DecorationMap::new();
                for (node_id, value) in &left {
                    map.insert(*node_id, Count(*value));
                }
                let mut other = DecorationMap::new();
                for (node_id, value) in &right {
                    other.insert(*node_id, Count(*value));
                }

                map.merge(other);

                prop_assert_eq!(total(&map), left.len() + right.len());
            }

            #[test]
            fn different_types_do_not_interfere(node_id: usize, a: u32, b: i64) {
                let mut map = DecorationMap::new();
                map.insert(node_id, Count(a));
                map.insert(node_id, Signed(b));

                prop_assert_eq!(map.get::<Count>(node_id), Some(&Count(a)));
                prop_assert_eq!(map.get::<Signed>(node_id), Some(&Signed(b)));
            }
        }
    }
}
