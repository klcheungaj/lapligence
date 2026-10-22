//! Module path delays (IEEE 1364-2001 14.2-14.4, IEEE 1800-2009 30.4-30.6).
//!
//! One [`IrModulePathSet`] holds the simple module paths of one module
//! instance. Each destination is a resolved net group: the runtime delays
//! every bit of that net by its paths, so every reader of the net sees the
//! delayed value (decision A7-D2). One hidden process per set records when
//! its source bits last changed ([`IrStmt::ModulePathStamp`]).
//!
//! [`IrStmt::ModulePathStamp`]: super::IrStmt::ModulePathStamp
//!
//! State-dependent and edge-sensitive paths (30.4.3, 30.4.4) carry their
//! condition (an expression of the set, evaluated by the runtime when a
//! destination transition is scheduled) and their edge. Pulse limits
//! (30.7) come from a `PATHPULSE$` table on the path, else from the model's
//! global percentages of the delays ([`IrPathPulseOptions`]).

use super::IrExpr;

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

/// The source edge an edge-sensitive path waits for (30.4.3). The edge is
/// detected on the least significant bit of the input terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrPathEdge {
    /// Any transition of the source starts the path.
    None,
    Posedge,
    Negedge,
    /// `edge`: a posedge or a negedge (IEEE 1800-2009 only).
    Any,
}

impl IrPathEdge {
    /// The runtime code: 0 none, 1 posedge, 2 negedge, 3 either.
    pub fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Posedge => 1,
            Self::Negedge => 2,
            Self::Any => 3,
        }
    }
}

/// The condition of a state-dependent path (30.4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrPathCondition {
    None,
    /// `if (expression)`: index into [`IrModulePathSet::conditions`].
    If(usize),
    /// `ifnone`: the path applies when no other path of its group is active.
    IfNone,
}

/// Pulse limits of one path in scheduler ticks per transition, in
/// [`PATH_TRANSITIONS`] order: `reject` then `error` (30.7.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrPathLimits {
    pub(in crate::sim) reject: [u64; PATH_TRANSITIONS],
    pub(in crate::sim) error: [u64; PATH_TRANSITIONS],
}

impl IrPathLimits {
    /// One reject and one error limit for every transition (`PATHPULSE$`).
    pub fn uniform(reject: u64, error: u64) -> Self {
        Self {
            reject: [reject; PATH_TRANSITIONS],
            error: [error; PATH_TRANSITIONS],
        }
    }

    pub fn reject(&self) -> &[u64; PATH_TRANSITIONS] {
        &self.reject
    }

    pub fn error(&self) -> &[u64; PATH_TRANSITIONS] {
        &self.error
    }
}

/// Pulse handling style of a path destination (30.7.4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrPulseStyle {
    OnEvent,
    OnDetect,
}

/// Global pulse controls (30.7.2, 30.7.4): reject and error limits as
/// percentages of each path delay, and style and showcancelled settings that
/// take precedence over specify-block declarations when given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrPathPulseOptions {
    pub reject_percent: u8,
    pub error_percent: u8,
    pub style: Option<IrPulseStyle>,
    pub show_cancelled: Option<bool>,
}

impl Default for IrPathPulseOptions {
    fn default() -> Self {
        Self {
            reject_percent: 100,
            error_percent: 100,
            style: None,
            show_cancelled: None,
        }
    }
}

/// Per-destination-bit style flags (`IrModulePathDestination::styles`).
pub const PATH_STYLE_ON_DETECT: u8 = 1;
pub const PATH_STYLE_SHOW_CANCELLED: u8 = 2;

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
    pub(in crate::sim) edge: IrPathEdge,
    pub(in crate::sim) condition: IrPathCondition,
    /// Nonzero for paths whose terminals equal those of an `ifnone` path:
    /// the `ifnone` path yields to an active path of the same group.
    pub(in crate::sim) group: u32,
    /// `PATHPULSE$` limits; `None` takes the global percentages.
    pub(in crate::sim) limits: Option<Box<IrPathLimits>>,
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
            edge: IrPathEdge::None,
            condition: IrPathCondition::None,
            group: 0,
            limits: None,
        }
    }

    /// The path waits for `edge` on its source and applies under `condition`.
    pub fn with_trigger(mut self, edge: IrPathEdge, condition: IrPathCondition) -> Self {
        self.edge = edge;
        self.condition = condition;
        self
    }

    pub fn with_group(mut self, group: u32) -> Self {
        self.group = group;
        self
    }

    pub fn with_limits(mut self, limits: IrPathLimits) -> Self {
        self.limits = Some(Box::new(limits));
        self
    }

    pub fn delays(&self) -> &[u64; PATH_TRANSITIONS] {
        &self.delays
    }

    pub fn edge(&self) -> IrPathEdge {
        self.edge
    }

    pub fn condition(&self) -> IrPathCondition {
        self.condition
    }

    pub fn group(&self) -> u32 {
        self.group
    }

    pub fn limits(&self) -> Option<&IrPathLimits> {
        self.limits.as_deref()
    }
}

/// The paths into one destination net group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrModulePathDestination {
    pub(in crate::sim) group: usize,
    pub(in crate::sim) paths: Vec<IrModulePath>,
    /// One byte of `PATH_STYLE_*` flags per destination bit, from
    /// `pulsestyle_*` and `showcancelled` declarations; `None` when none
    /// names the net.
    pub(in crate::sim) styles: Option<Vec<u8>>,
}

impl IrModulePathDestination {
    pub fn new(group: usize, paths: Vec<IrModulePath>) -> Self {
        Self {
            group,
            paths,
            styles: None,
        }
    }

    pub fn styles(&self) -> Option<&[u8]> {
        self.styles.as_deref()
    }

    pub fn group(&self) -> usize {
        self.group
    }

    pub fn paths(&self) -> &[IrModulePath] {
        &self.paths
    }
}

/// The module paths of one module instance.
#[derive(Clone, Debug, PartialEq)]
pub struct IrModulePathSet {
    /// Hierarchical name of the instance, for introspection.
    pub(in crate::sim) scope: String,
    /// Packed signals whose transitions the paths start from.
    pub(in crate::sim) sources: Vec<usize>,
    pub(in crate::sim) destinations: Vec<IrModulePathDestination>,
    /// State-dependent path conditions, read in the instance's scope. The
    /// runtime evaluates them all when it schedules a destination transition;
    /// the least significant bit decides, x and z count as true (30.4.4.1).
    pub(in crate::sim) conditions: Vec<IrExpr>,
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
            conditions: Vec::new(),
        }
    }

    pub fn with_conditions(mut self, conditions: Vec<IrExpr>) -> Self {
        self.conditions = conditions;
        self
    }

    pub fn conditions(&self) -> &[IrExpr] {
        &self.conditions
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
