//! Signal records by storage name.
//!
//! Collapsing a net into a resolved group renames its storage in every
//! signal record that names it: the emission list `signals` and the
//! by-name scope fallbacks. Searching both for each group cost groups x
//! records; the index finds the records of one storage name directly.

use super::*;

/// Positions of the signal records that name each storage global.
pub(super) struct SignalGlobalIndex {
    signals: HashMap<String, Vec<usize>>,
    scoped: HashMap<String, Vec<(String, String)>>,
}

impl Codegen<'_> {
    /// Index the current storage names of `signals` and `scope_sig_names`.
    pub(super) fn signal_global_index(&self) -> SignalGlobalIndex {
        let mut signals: HashMap<String, Vec<usize>> = HashMap::new();
        for (position, info) in self.signals.iter().enumerate() {
            signals
                .entry(info.global.clone())
                .or_default()
                .push(position);
        }
        let mut scoped: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (scope, names) in &self.scope_sig_names {
            for (name, info) in names {
                scoped
                    .entry(info.global.clone())
                    .or_default()
                    .push((scope.clone(), name.clone()));
            }
        }
        SignalGlobalIndex { signals, scoped }
    }

    /// Point every record still naming `old_global` at a group's resolved
    /// cell and driver slot. With `undriven_signals`, emission records that
    /// already drive a group keep their binding; scope fallbacks always
    /// follow. Records are matched against their current name, so a record
    /// renamed since the index was built is left alone.
    pub(super) fn retarget_signal_global(
        &mut self,
        index: &SignalGlobalIndex,
        old_global: &str,
        resolved: &str,
        driver: (&str, usize),
        undriven_signals: bool,
    ) {
        for position in index.signals.get(old_global).into_iter().flatten() {
            let info = &mut self.signals[*position];
            if info.global == old_global && !(undriven_signals && info.net_driver.is_some()) {
                info.global = resolved.to_owned();
                info.net_driver = Some((driver.0.to_owned(), driver.1));
            }
        }
        for (scope, name) in index.scoped.get(old_global).into_iter().flatten() {
            if let Some(info) = self
                .scope_sig_names
                .get_mut(scope)
                .and_then(|names| names.get_mut(name))
                .filter(|info| info.global == old_global)
            {
                info.global = resolved.to_owned();
                info.net_driver = Some((driver.0.to_owned(), driver.1));
            }
        }
    }
}
