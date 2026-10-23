//! Model storage named by a frame lookup.
//!
//! A name that no frame scope binds resolves to model storage: the first
//! subprogram local with that C name, else the first emitted signal (or
//! alias-visible cell), else an object's change marker. Scanning the model
//! for every access made rendering a netlist cost accesses x signals, so
//! the model-wide renderers index the names once and give the index to
//! their frames. Frames without one keep the scan; both give the same
//! storage.

use std::collections::{HashMap, HashSet};

use crate::sim::ir::{IrModel, IrType};

use super::Binding;

/// The model storage a name resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModelStorage {
    /// Local `local` of subprogram `function`.
    Local { function: usize, local: usize },
    /// The alias-visible cell of signal `index`.
    Alias(usize),
    /// Signal `index`'s own storage.
    Signal(usize),
    /// A string or handle object's one-bit change marker.
    Marker,
}

/// The names [`ModelStorage`] resolution matches, indexed by first match.
pub(in crate::sim::emit_c) struct StorageNames<'m> {
    model: &'m IrModel,
    locals: HashMap<&'m str, (usize, usize)>,
    signals: HashMap<&'m str, usize>,
    markers: HashSet<&'m str>,
}

impl<'m> StorageNames<'m> {
    pub(in crate::sim::emit_c) fn new(model: &'m IrModel) -> Self {
        let mut locals = HashMap::new();
        for (function, definition) in model.funcs.iter().enumerate() {
            for (local, storage) in definition.locals.iter().enumerate() {
                locals.entry(storage.c_name()).or_insert((function, local));
            }
        }
        let mut signals = HashMap::new();
        for (index, signal) in model.signals.iter().enumerate() {
            if emitted(signal) {
                signals.entry(signal.c_name.as_str()).or_insert(index);
            }
        }
        let markers = model
            .objects
            .iter()
            .filter(|object| object.ty.has_change_marker())
            .map(|object| object.c_name.as_str())
            .collect();
        Self {
            model,
            locals,
            signals,
            markers,
        }
    }

    /// Whether this index was built for `model`.
    pub(super) fn indexes(&self, model: &IrModel) -> bool {
        std::ptr::eq(self.model, model)
    }

    /// [`Self::find`]; debug builds confirm it against the scan on models
    /// small enough for that to stay cheap.
    pub(super) fn find_checked(&self, name: &str) -> Option<ModelStorage> {
        let found = self.find(name);
        #[cfg(debug_assertions)]
        if self.model.signals.len() <= CROSS_CHECK_SIGNALS {
            debug_assert_eq!(
                found,
                scan(self.model, name),
                "storage name index disagrees with a scan for `{name}`"
            );
        }
        found
    }

    fn find(&self, name: &str) -> Option<ModelStorage> {
        if let Some(&(function, local)) = self.locals.get(name) {
            return Some(ModelStorage::Local { function, local });
        }
        let alias = alias_index(name).filter(|index| {
            self.model
                .signals
                .get(*index)
                .is_some_and(|signal| !signal.net_alias.is_empty())
        });
        let signal = self.signals.get(name).copied();
        // The scan meets the lower index first; at one index it tests the
        // alias name before the storage name.
        match (alias, signal) {
            (Some(alias), Some(signal)) if signal < alias => Some(ModelStorage::Signal(signal)),
            (Some(alias), _) => Some(ModelStorage::Alias(alias)),
            (None, Some(signal)) => Some(ModelStorage::Signal(signal)),
            (None, None) => is_marker(name, |object| self.markers.contains(object))
                .then_some(ModelStorage::Marker),
        }
    }
}

/// Above this many signals, debug builds skip the scan cross-check so large
/// generation tests keep their linear cost.
#[cfg(debug_assertions)]
const CROSS_CHECK_SIGNALS: usize = 4096;

/// Resolve `name` by scanning `model`, for frames without an index.
pub(super) fn scan(model: &IrModel, name: &str) -> Option<ModelStorage> {
    for (function, definition) in model.funcs.iter().enumerate() {
        if let Some(local) = definition
            .locals
            .iter()
            .position(|local| local.c_name() == name)
        {
            return Some(ModelStorage::Local { function, local });
        }
    }
    let alias = alias_index(name);
    for (index, signal) in model.signals.iter().enumerate() {
        if !signal.net_alias.is_empty() && alias == Some(index) {
            return Some(ModelStorage::Alias(index));
        }
        if signal.c_name == name && emitted(signal) {
            return Some(ModelStorage::Signal(index));
        }
    }
    is_marker(name, |object| {
        model
            .objects
            .iter()
            .any(|candidate| candidate.c_name == object && candidate.ty.has_change_marker())
    })
    .then_some(ModelStorage::Marker)
}

impl ModelStorage {
    pub(super) fn binding(self, model: &IrModel, name: &str) -> Binding {
        match self {
            Self::Local { function, local } => {
                let local = &model.funcs[function].locals[local];
                Binding {
                    address: format!("&{name}"),
                    width: if local.real { 0 } else { local.width() },
                    signed: local.signed(),
                    two_state: local.two_state,
                    shortreal: local.shortreal,
                    automatic: false,
                }
            }
            Self::Alias(index) => {
                let signal = &model.signals[index];
                Binding {
                    address: format!("&llg_net_alias_{index}.visible"),
                    width: signal.ty.width(),
                    signed: signal.ty.signed(),
                    two_state: signal.ty.two_state(),
                    shortreal: false,
                    automatic: false,
                }
            }
            Self::Signal(index) => {
                let signal = &model.signals[index];
                Binding {
                    address: format!("&{name}"),
                    width: signal.ty.width(),
                    signed: signal.ty.signed(),
                    two_state: matches!(
                        signal.ty,
                        IrType::Packed {
                            two_state: true,
                            ..
                        }
                    ),
                    shortreal: matches!(signal.ty, IrType::Real { shortreal: true }),
                    automatic: false,
                }
            }
            Self::Marker => Binding {
                address: format!("&{name}"),
                width: 1,
                signed: false,
                two_state: true,
                shortreal: false,
                automatic: false,
            },
        }
    }
}

/// Signals with storage a lookup may bind: emitted ones, and omitted net
/// members, which read through their group's resolved cell.
fn emitted(signal: &crate::sim::ir::IrSignal) -> bool {
    !signal.omit || signal.net_driver.is_some()
}

/// The one-bit change marker of a string or handle object, which a
/// whole-handle event control waits on (SIM-007).
fn is_marker(name: &str, has_marker: impl Fn(&str) -> bool) -> bool {
    name.strip_suffix("_llg_dep").is_some_and(has_marker)
}

/// The signal index of an alias-visible cell name, spelled exactly as
/// `llg_net_alias_{index}.visible`.
fn alias_index(name: &str) -> Option<usize> {
    let digits = name
        .strip_prefix("llg_net_alias_")?
        .strip_suffix(".visible")?;
    let index = digits.parse::<usize>().ok()?;
    (index.to_string() == digits).then_some(index)
}
