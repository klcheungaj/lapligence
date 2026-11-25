//! Maps and sets keyed by ids.
//!
//! The hash of an id is one multiplication where the standard hasher runs a
//! keyed function over its bytes, which profiles showed as several percent of
//! a compilation. It is the same in every run, so an order taken from it would
//! look stable while depending on table sizes; neither type therefore offers
//! an iteration in its own order ([`IdMap::sorted`], [`IdSet::sorted`]).

use std::borrow::Borrow;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hash, Hasher};

/// Hashes the integers an id consists of.
///
/// The keys are made by the program, so nothing chooses them to collide.
#[derive(Clone, Copy, Default)]
pub(crate) struct IdHasher(u64);

/// Odd, with bits set at all scales: a product spreads every bit of the id
/// over the bits above it.
const SPREAD: u64 = 0xf135_7aea_2e62_a9c5;
/// Moves the well spread upper bits to where a table takes its index from.
const ROTATION: u32 = 26;

impl Hasher for IdHasher {
    fn write_u64(&mut self, value: u64) {
        self.0 = self.0.wrapping_add(value).wrapping_mul(SPREAD);
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(std::mem::size_of::<u64>()) {
            let mut word = [0; std::mem::size_of::<u64>()];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }

    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }

    fn write_isize(&mut self, value: isize) {
        self.write_u64(value as u64);
    }

    fn finish(&self) -> u64 {
        self.0.rotate_left(ROTATION)
    }
}

type IdState = BuildHasherDefault<IdHasher>;

/// A map whose keys are ids.
#[derive(Clone, Debug)]
pub(crate) struct IdMap<K, V>(HashMap<K, V, IdState>);

impl<K, V> Default for IdMap<K, V> {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

impl<K: Eq + Hash, V> IdMap<K, V> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.0.get(key)
    }

    pub(crate) fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.0.get_mut(key)
    }

    pub(crate) fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.0.contains_key(key)
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    pub(crate) fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.0.remove(key)
    }

    pub(crate) fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        self.0.entry(key)
    }

    /// The entries by ascending key.
    pub(crate) fn sorted(&self) -> Vec<(&K, &V)>
    where
        K: Ord,
    {
        let mut entries = self.0.iter().collect::<Vec<_>>();
        entries.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
        entries
    }

    /// The entries in no order, for a result that has none either: a count, a
    /// test, another map or set, or a list that is sorted before it is used.
    pub(crate) fn unordered(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter()
    }
}

impl<K: Eq + Hash, V> FromIterator<(K, V)> for IdMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(entries: I) -> Self {
        Self(entries.into_iter().collect())
    }
}

impl<K: Eq + Hash, V> Extend<(K, V)> for IdMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, entries: I) {
        self.0.extend(entries);
    }
}

impl<K: Eq + Hash + Borrow<Q>, V, Q: Eq + Hash + ?Sized> std::ops::Index<&Q> for IdMap<K, V> {
    type Output = V;

    fn index(&self, key: &Q) -> &V {
        &self.0[key]
    }
}

/// A set of ids.
#[derive(Clone, Debug)]
pub(crate) struct IdSet<K>(HashSet<K, IdState>);

impl<K> Default for IdSet<K> {
    fn default() -> Self {
        Self(HashSet::default())
    }
}

impl<K: Eq + Hash> IdSet<K> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn contains(&self, key: &K) -> bool {
        self.0.contains(key)
    }

    /// Whether `key` was not in the set yet.
    pub(crate) fn insert(&mut self, key: K) -> bool {
        self.0.insert(key)
    }

    pub(crate) fn remove(&mut self, key: &K) -> bool {
        self.0.remove(key)
    }

    /// The ids by ascending value.
    pub(crate) fn sorted(&self) -> Vec<K>
    where
        K: Ord + Copy,
    {
        let mut ids = self.0.iter().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    }

    /// The ids in no order; see [`IdMap::unordered`].
    pub(crate) fn unordered(&self) -> impl Iterator<Item = &K> {
        self.0.iter()
    }
}

impl<K: Eq + Hash> FromIterator<K> for IdSet<K> {
    fn from_iter<I: IntoIterator<Item = K>>(ids: I) -> Self {
        Self(ids.into_iter().collect())
    }
}

impl<K: Eq + Hash> Extend<K> for IdSet<K> {
    fn extend<I: IntoIterator<Item = K>>(&mut self, ids: I) {
        self.0.extend(ids);
    }
}
