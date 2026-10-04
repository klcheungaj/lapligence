//! Dense per-node references into tables of distinct values.
//!
//! Large designs repeat a few type records and native detail spellings across
//! most nodes. Storing one copy per distinct value plus a four-byte slot per
//! node keeps the owned database proportional to the distinct metadata rather
//! than to the node count, while lookups stay one bounds-checked index.

use super::NodeId;
use std::collections::HashMap;
use std::hash::Hash;

const ABSENT: u32 = u32::MAX;

/// Values shared by node slots; an absent slot has no value.
#[derive(Debug)]
pub(super) struct DenseShared<T> {
    values: Vec<T>,
    slots: Vec<u32>,
}

impl<T> Default for DenseShared<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            slots: Vec::new(),
        }
    }
}

impl<T> DenseShared<T> {
    pub(super) fn get(&self, id: NodeId) -> Option<&T> {
        let slot = *self.slots.get(id.index())?;
        if slot == ABSENT {
            return None;
        }
        self.values.get(slot as usize)
    }

    /// Number of node slots, present or absent.
    pub(super) fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Number of distinct shared values.
    #[cfg(test)]
    pub(super) fn value_count(&self) -> usize {
        self.values.len()
    }
}

/// Builds a [`DenseShared`] table, interning values by a caller-chosen key.
pub(super) struct DenseSharedBuilder<K, T> {
    index: HashMap<K, u32>,
    table: DenseShared<T>,
}

impl<K: Eq + Hash, T> DenseSharedBuilder<K, T> {
    /// A builder with `slot_count` initially absent node slots.
    pub(super) fn new(slot_count: usize) -> Self {
        Self {
            index: HashMap::new(),
            table: DenseShared {
                values: Vec::new(),
                slots: vec![ABSENT; slot_count],
            },
        }
    }

    /// Point `id` at the value interned under `key`, creating that value with
    /// `make` only on the key's first use. Equal keys must denote equal values.
    pub(super) fn assign(
        &mut self,
        id: NodeId,
        key: K,
        make: impl FnOnce() -> T,
    ) -> Result<(), String> {
        let slot = match self.index.get(&key) {
            Some(slot) => *slot,
            None => {
                let slot = u32::try_from(self.table.values.len())
                    .ok()
                    .filter(|slot| *slot != ABSENT)
                    .ok_or_else(|| "too many distinct shared node values".to_owned())?;
                self.table.values.push(make());
                self.index.insert(key, slot);
                slot
            }
        };
        let target = self
            .table
            .slots
            .get_mut(id.index())
            .ok_or_else(|| "shared node value is outside the node arena".to_owned())?;
        *target = slot;
        Ok(())
    }

    pub(super) fn finish(mut self) -> DenseShared<T> {
        self.table.values.shrink_to_fit();
        self.table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_keys_share_one_value_and_absent_slots_stay_empty() {
        let mut builder = DenseSharedBuilder::new(4);
        let mut created = 0;
        for (index, key) in [(0, "a"), (2, "b"), (3, "a")] {
            builder
                .assign(NodeId::from_index(index), key, || {
                    created += 1;
                    key.to_uppercase()
                })
                .unwrap();
        }
        let table = builder.finish();
        assert_eq!(created, 2);
        assert_eq!(table.slot_count(), 4);
        assert_eq!(
            table.get(NodeId::from_index(0)).map(String::as_str),
            Some("A")
        );
        assert_eq!(table.get(NodeId::from_index(1)), None);
        assert_eq!(
            table.get(NodeId::from_index(2)).map(String::as_str),
            Some("B")
        );
        assert_eq!(
            table.get(NodeId::from_index(3)).map(String::as_str),
            Some("A")
        );
        assert_eq!(table.get(NodeId::from_index(4)), None);
    }

    #[test]
    fn assignments_outside_the_arena_are_rejected() {
        let mut builder = DenseSharedBuilder::new(1);
        assert!(builder
            .assign(NodeId::from_index(1), 7u64, || 7u64)
            .is_err());
        assert_eq!(builder.finish().get(NodeId::from_index(0)), None);
    }
}
