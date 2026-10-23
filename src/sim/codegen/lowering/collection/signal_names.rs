//! Model signals by dependency name.
//!
//! Dependency analysis maps a scalar dependency back to the first signal
//! whose storage name or alias-visible dependency name it carries. Scanning
//! every signal for that made each analysed write cost O(signals), so
//! analysing every process of a netlist was quadratic in its instance count.
//! The index keeps the first position per name. Signals are appended during
//! lowering, which only extends it; the few lowering steps that rename a
//! signal or give it alias bindings in place mark it stale.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::*;

/// First signal position per dependency-name hash. A hash only selects a
/// candidate: lookups confirm the name, and a mismatch (a hash collision)
/// falls back to scanning, so the result always equals the scan's.
#[derive(Default)]
pub(in super::super) struct SignalNameIndex {
    /// Signals indexed so far; later ones are appended on the next lookup.
    indexed: usize,
    /// A signal was renamed or aliased in place since the last lookup.
    stale: bool,
    first: HashMap<u64, usize>,
}

fn name_hash(name: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish()
}

impl Codegen<'_> {
    /// Record that a signal's storage name or alias bindings changed in
    /// place, so the next lookup rebuilds the index.
    pub(in super::super) fn invalidate_signal_names(&self) {
        self.signal_names.borrow_mut().stale = true;
    }

    /// Whether signal `index` carries dependency name `name`: its storage
    /// name, or the alias-visible cell an aliased signal is read through.
    fn signal_has_dependency_name(&self, index: usize, name: &str) -> bool {
        let signal = &self.model.signals[index];
        signal.c_name == name || (!signal.net_alias.is_empty() && alias_index(name) == Some(index))
    }

    /// The first signal whose storage or dependency name is `name`.
    pub(in super::super) fn signal_by_dependency_name(&self, name: &str) -> Option<usize> {
        let found = self.indexed_signal_by_dependency_name(name);
        #[cfg(debug_assertions)]
        if self.model.signals.len() <= CROSS_CHECK_SIGNALS {
            debug_assert_eq!(
                found,
                self.scan_signal_by_dependency_name(name),
                "signal name index disagrees with a scan for `{name}`"
            );
        }
        found
    }

    fn indexed_signal_by_dependency_name(&self, name: &str) -> Option<usize> {
        let mut index = self.signal_names.borrow_mut();
        let count = self.model.signals.len();
        if index.stale || index.indexed > count {
            *index = SignalNameIndex::default();
        }
        for position in index.indexed..count {
            let signal = &self.model.signals[position];
            index
                .first
                .entry(name_hash(&signal.c_name))
                .or_insert(position);
            if !signal.net_alias.is_empty() {
                let alias = format!("llg_net_alias_{position}.visible");
                index.first.entry(name_hash(&alias)).or_insert(position);
            }
        }
        index.indexed = count;
        let candidate = index.first.get(&name_hash(name)).copied();
        drop(index);
        match candidate {
            Some(position) if self.signal_has_dependency_name(position, name) => Some(position),
            Some(_) => self.scan_signal_by_dependency_name(name),
            None => None,
        }
    }

    fn scan_signal_by_dependency_name(&self, name: &str) -> Option<usize> {
        (0..self.model.signals.len()).find(|index| self.signal_has_dependency_name(*index, name))
    }
}

/// Above this many signals, debug builds skip the scan cross-check so large
/// generation tests keep their linear cost.
#[cfg(debug_assertions)]
const CROSS_CHECK_SIGNALS: usize = 4096;

/// The signal index named by an alias-visible dependency name, exactly as
/// `signal_dependency_name` spells it.
fn alias_index(name: &str) -> Option<usize> {
    let digits = name
        .strip_prefix("llg_net_alias_")?
        .strip_suffix(".visible")?;
    let index = digits.parse::<usize>().ok()?;
    (index.to_string() == digits).then_some(index)
}
