//! Size bounded collections.

use ahash::{AHashMap, AHashSet};
use std::{hash::Hash, ops::Deref};

/// A HashMap that is bounded in growth.
pub struct BoundedHashMap<K, V> {
    map: AHashMap<K, V>,
    limit: usize,
}

impl<K, V> BoundedHashMap<K, V>
where
    K: Hash + Eq,
{
    pub fn new(limit: usize) -> Self {
        Self::with_capacity(2, limit)
    }

    pub fn with_capacity(cap: usize, limit: usize) -> Self {
        assert!(cap <= limit, "capacity must not be larger than limit");

        Self {
            map: AHashMap::with_capacity(cap),
            limit,
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        assert!(
            self.try_insert(key, value).is_none(),
            "failed to insert, at limit"
        );
    }

    pub fn try_insert(&mut self, key: K, value: V) -> Option<(K, V)> {
        if self.map.len() + 1 > self.limit {
            return Some((key, value));
        }

        self.map.insert(key, value);
        None
    }
}

impl<K, V> Deref for BoundedHashMap<K, V> {
    type Target = AHashMap<K, V>;
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

/// A HashSet that is bounded in growth.
pub struct BoundedHashSet<V> {
    map: AHashSet<V>,
    limit: usize,
}

impl<V> BoundedHashSet<V>
where
    V: Hash + Eq,
{
    pub fn new(limit: usize) -> Self {
        Self::with_capacity(2, limit)
    }

    pub fn with_capacity(cap: usize, limit: usize) -> Self {
        assert!(cap <= limit, "capacity must not be larger than limit");

        Self {
            map: AHashSet::with_capacity(cap),
            limit,
        }
    }

    pub fn insert(&mut self, value: V) {
        assert!(
            self.try_insert(value).is_none(),
            "failed to insert, at limit"
        );
    }

    pub fn try_insert(&mut self, value: V) -> Option<V> {
        if self.map.len() + 1 > self.limit {
            return Some(value);
        }

        self.map.insert(value);
        None
    }
}

impl<V> Deref for BoundedHashSet<V> {
    type Target = AHashSet<V>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}
