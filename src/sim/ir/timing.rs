//! Module path delays (IEEE 1364-2001 14.2-14.4, IEEE 1800-2009 30.4-30.6).
//!
//! One [`IrModulePathSet`] holds the simple module paths of one module
//! instance. Each destination is a resolved net group: the runtime delays
//! every bit of that net by its paths, so every reader of the net sees the
//! delayed value (decision A7-D2). One hidden process per set records when
//! its source bits last changed ([`IrStmt::ModulePathStamp`]).
//!
//! [`IrStmt::ModulePathStamp`]: super::IrStmt::ModulePathStamp

/// Transitions a module path delay table covers, in the order of the
/// twelve-value form (IEEE 1800-2009 Syntax 30-6, Table 30-2): 0->1, 1->0,
/// 0->z, z->1, 1->z, z->0, 0->x, x->1, 1->x, x->0, x->z, z->x.
pub const PATH_TRANSITIONS: usize = 12;

const T01: usize = 0;
const T10: usize = 1;
const T0Z: usize = 2;
const TZ1: usize = 3;
const T1Z: usize = 4;
const TZ0: usize = 5;
const T0X: usize = 6;
const TX1: usize = 7;
const T1X: usize = 8;
const TX0: usize = 9;
const TXZ: usize = 10;
const TZX: usize = 11;

/// Expand one, two, three, six or twelve path delay values into the
/// twelve-transition table: Table 30-2 (V Table 47) for the 0/1/z
/// transitions, then, when the x transitions are not given, Table 30-3
/// (V Table 48): a transition to x takes the shortest delay of the
/// transitions from its source state to the known states, a transition
/// from x the longest delay of the transitions into its target state.
/// `None` for any other count.
pub fn expand_path_delays(values: &[u64]) -> Option<[u64; PATH_TRANSITIONS]> {
    let mut table = [0u64; PATH_TRANSITIONS];
    let known: [u64; 6] = match *values {
        [t] => [t; 6],
        // t01, t10, t0z, tz1, t1z, tz0.
        [rise, fall] => [rise, fall, rise, rise, fall, fall],
        [rise, fall, z] => [rise, fall, z, rise, z, fall],
        [t01, t10, t0z, tz1, t1z, tz0] => [t01, t10, t0z, tz1, t1z, tz0],
        [..] if values.len() == PATH_TRANSITIONS => {
            table.copy_from_slice(values);
            return Some(table);
        }
        _ => return None,
    };
    table[..6].copy_from_slice(&known);
    table[T0X] = table[T0Z].min(table[T01]);
    table[T1X] = table[T1Z].min(table[T10]);
    table[TZX] = table[TZ1].min(table[TZ0]);
    table[TX0] = table[TZ0].max(table[T10]);
    table[TX1] = table[TZ1].max(table[T01]);
    table[TXZ] = table[T1Z].max(table[T0Z]);
    Some(table)
}

/// The polarity operator of a path (30.4.7). Metadata only: it has no
/// simulation effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrPathPolarity {
    Unknown,
    Positive,
    Negative,
}

impl IrPathPolarity {
    /// The runtime metadata code: 0 unknown, 1 positive, 2 negative.
    pub fn code(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Positive => 1,
            Self::Negative => 2,
        }
    }
}

/// One source terminal to one destination terminal. A full connection
/// (`*>`) joins every bit of the source range to every bit of the
/// destination range; a parallel one (`=>`) joins bit `i` of each, from the
/// least significant storage bit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrModulePath {
    /// Index into [`IrModulePathSet::sources`].
    pub(in crate::sim) source: usize,
    pub(in crate::sim) source_lsb: u32,
    pub(in crate::sim) source_width: u32,
    pub(in crate::sim) dest_lsb: u32,
    pub(in crate::sim) dest_width: u32,
    pub(in crate::sim) full: bool,
    pub(in crate::sim) polarity: IrPathPolarity,
    /// Scheduler ticks per transition, in [`PATH_TRANSITIONS`] order.
    pub(in crate::sim) delays: [u64; PATH_TRANSITIONS],
    /// `file:line:col` of the path declaration, its stable source identity.
    pub(in crate::sim) origin: String,
}

impl IrModulePath {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: usize,
        source_lsb: u32,
        source_width: u32,
        dest_lsb: u32,
        dest_width: u32,
        full: bool,
        polarity: IrPathPolarity,
        delays: [u64; PATH_TRANSITIONS],
        origin: String,
    ) -> Self {
        Self {
            source,
            source_lsb,
            source_width,
            dest_lsb,
            dest_width,
            full,
            polarity,
            delays,
            origin,
        }
    }

    pub fn delays(&self) -> &[u64; PATH_TRANSITIONS] {
        &self.delays
    }
}

/// The paths into one destination net group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrModulePathDestination {
    pub(in crate::sim) group: usize,
    pub(in crate::sim) paths: Vec<IrModulePath>,
}

impl IrModulePathDestination {
    pub fn new(group: usize, paths: Vec<IrModulePath>) -> Self {
        Self { group, paths }
    }

    pub fn group(&self) -> usize {
        self.group
    }

    pub fn paths(&self) -> &[IrModulePath] {
        &self.paths
    }
}

/// The simple module paths of one module instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrModulePathSet {
    /// Hierarchical name of the instance, for introspection.
    pub(in crate::sim) scope: String,
    /// Packed signals whose transitions the paths start from.
    pub(in crate::sim) sources: Vec<usize>,
    pub(in crate::sim) destinations: Vec<IrModulePathDestination>,
}

impl IrModulePathSet {
    pub fn new(
        scope: String,
        sources: Vec<usize>,
        destinations: Vec<IrModulePathDestination>,
    ) -> Self {
        Self {
            scope,
            sources,
            destinations,
        }
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    pub fn sources(&self) -> &[usize] {
        &self.sources
    }

    pub fn destinations(&self) -> &[IrModulePathDestination] {
        &self.destinations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_forms_expand_by_tables_30_2_and_30_3() {
        assert_eq!(expand_path_delays(&[7]), Some([7; 12]));
        // Two values: rise for 0->1, 0->z, z->1; fall for 1->0, 1->z, z->0.
        assert_eq!(
            expand_path_delays(&[3, 5]),
            Some([3, 5, 3, 3, 5, 5, 3, 3, 5, 5, 5, 3])
        );
        // Three values: tz for 0->z and 1->z.
        assert_eq!(
            expand_path_delays(&[3, 5, 9]),
            Some([3, 5, 9, 3, 9, 5, 3, 3, 5, 5, 9, 3])
        );
        // The worked example under Table 30-3: (5, 12, 17, 10, 6, 22) gives
        // 0->x 5, 1->x 6, z->x 10, x->0 22, x->1 10, x->z 17.
        assert_eq!(
            expand_path_delays(&[5, 12, 17, 10, 6, 22]),
            Some([5, 12, 17, 10, 6, 22, 5, 10, 6, 22, 17, 10])
        );
        let twelve = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        assert_eq!(expand_path_delays(&twelve), Some(twelve));
        for count in [0, 4, 5, 7, 11, 13] {
            assert_eq!(expand_path_delays(&vec![1; count]), None, "{count} values");
        }
    }
}
