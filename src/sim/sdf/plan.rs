//! The owned annotation plan: every SDF record matched to its targets, in
//! file order, ready to be applied as one unit.
//!
//! Order matters: "annotation of an SDF construct can be changed by
//! annotation of a subsequent construct" (IEEE 1800-2009 32.5), so ops are
//! applied first to last. Values stay as written (exact decimals with their
//! min:typ:max members, delay lists with their length), because selecting a
//! data set, scaling and expanding a list to twelve transitions (Table
//! 32-4) take the annotation call's arguments and happen when the plan is
//! applied. Delay lists are stored once and shared by the ops of every
//! instance a record matched, so a wildcard cell entry costs one small op
//! per instance.

use super::number::{fmt_optional, SdfValue};
use super::syntax::{SdfDelvalList, SdfTimescale, SdfVersion};
use super::targets::{escaped, SdfTargetCheck, SdfTargets};
use super::SdfPos;
use std::fmt;

/// A record that could not be annotated, or a discouraged form. The plan
/// still applies; these are reported (IEEE 1800-2009 32.3: "It shall issue
/// a warning for any data it is unable to annotate").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfWarning {
    pub pos: SdfPos,
    pub message: String,
}

impl SdfWarning {
    pub fn new(pos: SdfPos, message: impl Into<String>) -> Self {
        Self {
            pos,
            message: message.into(),
        }
    }
}

impl fmt::Display for SdfWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.pos, self.message)
    }
}

/// ABSOLUTE replaces values, INCREMENT adds to them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfMode {
    Absolute,
    Increment,
}

/// Which limit of a declared timing check: the first or second limit
/// argument (`$setuphold` setup/hold, `$recrem` recovery/removal,
/// `$nochange` start/end offset, `$width` limit/threshold, `$fullskew`
/// limits). [`SdfLimitSlot::Both`] marks a model row that holds both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfLimitSlot {
    First,
    Second,
    Both,
}

/// A delay record's values: an index into [`SdfPlan::delay_list`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdfDelayOp {
    pub mode: SdfMode,
    pub list: u32,
}

/// PATHPULSE/PATHPULSEPERCENT limits; an absent limit is left unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdfPulseOp {
    pub percent: bool,
    pub reject: Option<SdfValue>,
    pub error: Option<SdfValue>,
}

/// A port of a scope, or the selected declared indices of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdfPortTarget {
    pub scope: u32,
    pub port: u32,
    pub bits: Option<(i64, i64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SdfOpKind {
    /// IOPATH or DEVICE delays for input terminal `input` to output
    /// terminal `output` of declared path `path` of `scope`.
    PathDelay {
        scope: u32,
        path: u32,
        input: u32,
        output: u32,
        delay: SdfDelayOp,
    },
    PathPulse {
        scope: u32,
        path: u32,
        input: u32,
        output: u32,
        pulse: SdfPulseOp,
    },
    /// PORT: the delay from every source of the net to the load port.
    PortDelay {
        load: SdfPortTarget,
        delay: SdfDelayOp,
    },
    /// INTERCONNECT; `source` is `None` when the source port was not found,
    /// which makes the delay one from all sources (32.4.4).
    Interconnect {
        source: Option<SdfPortTarget>,
        load: SdfPortTarget,
        delay: SdfDelayOp,
    },
    /// NETDELAY (SDF 2.1) to an input or inout port.
    NetDelay {
        load: SdfPortTarget,
        delay: SdfDelayOp,
    },
    /// DEVICE on a primitive instance.
    PrimitiveDelay { scope: u32, delay: SdfDelayOp },
    /// A new limit for declared check `check` of `scope`. Timing check
    /// limits always replace (OVI SDF 3.0 3-8).
    CheckLimit {
        scope: u32,
        check: u32,
        slot: SdfLimitSlot,
        value: SdfValue,
    },
}

/// One operation and the position of the record it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct SdfOp {
    pub pos: SdfPos,
    pub kind: SdfOpKind,
}

/// A resolved file.
#[derive(Clone, Debug, PartialEq)]
pub struct SdfPlan {
    pub version: SdfVersion,
    pub timescale: SdfTimescale,
    /// The scope the file was resolved from.
    pub root: u32,
    pub ops: Vec<SdfOp>,
    pub(super) delay_lists: Vec<SdfDelvalList>,
    pub warnings: Vec<SdfWarning>,
}

impl SdfPlan {
    pub fn delay_list(&self, op: SdfDelayOp) -> &SdfDelvalList {
        &self.delay_lists[op.list as usize]
    }

    /// The version, timescale and scope, then one line per op and warning,
    /// naming targets by hierarchy and declaration: a deterministic dump for
    /// review and tests.
    pub fn render(&self, targets: &SdfTargets) -> String {
        let mut out = format!(
            "SDF {} timescale {} from {}\n",
            self.version,
            self.timescale,
            targets.path_of(self.root)
        );
        let port = |target: &SdfPortTarget| {
            let scope = targets.scope(target.scope);
            let mut text = format!(
                "{}.{}",
                targets.path_of(target.scope),
                escaped(&scope.ports[target.port as usize].name)
            );
            if let Some((msb, lsb)) = target.bits {
                if msb == lsb {
                    text.push_str(&format!("[{msb}]"));
                } else {
                    text.push_str(&format!("[{msb}:{lsb}]"));
                }
            }
            text
        };
        let path = |scope: u32, path: u32, input: u32, output: u32| {
            let declaration = &targets.scope(scope).paths[path as usize];
            format!(
                "{} path {path} ({} => {})",
                targets.path_of(scope),
                declaration.inputs[input as usize],
                declaration.outputs[output as usize]
            )
        };
        let mode = |mode: SdfMode| match mode {
            SdfMode::Absolute => "absolute",
            SdfMode::Increment => "increment",
        };
        for op in &self.ops {
            let line = match &op.kind {
                SdfOpKind::PathDelay {
                    scope,
                    path: index,
                    input,
                    output,
                    delay,
                } => format!(
                    "path delay {} {} = {}",
                    mode(delay.mode),
                    path(*scope, *index, *input, *output),
                    self.delay_list(*delay)
                ),
                SdfOpKind::PathPulse {
                    scope,
                    path: index,
                    input,
                    output,
                    pulse,
                } => format!(
                    "path pulse{} {} = {} {}",
                    if pulse.percent { " percent" } else { "" },
                    path(*scope, *index, *input, *output),
                    fmt_optional(pulse.reject.as_ref()),
                    fmt_optional(pulse.error.as_ref())
                ),
                SdfOpKind::PortDelay { load, delay } => format!(
                    "port delay {} {} = {}",
                    mode(delay.mode),
                    port(load),
                    self.delay_list(*delay)
                ),
                SdfOpKind::Interconnect {
                    source,
                    load,
                    delay,
                } => format!(
                    "interconnect {} {} -> {} = {}",
                    mode(delay.mode),
                    source.as_ref().map_or_else(|| "*".to_owned(), &port),
                    port(load),
                    self.delay_list(*delay)
                ),
                SdfOpKind::NetDelay { load, delay } => format!(
                    "net delay {} {} = {}",
                    mode(delay.mode),
                    port(load),
                    self.delay_list(*delay)
                ),
                SdfOpKind::PrimitiveDelay { scope, delay } => format!(
                    "primitive delay {} {} = {}",
                    mode(delay.mode),
                    targets.path_of(*scope),
                    self.delay_list(*delay)
                ),
                SdfOpKind::CheckLimit {
                    scope,
                    check,
                    slot,
                    value,
                } => {
                    let declaration: &SdfTargetCheck =
                        &targets.scope(*scope).checks[*check as usize];
                    format!(
                        "check limit {} check {check} {} {} = ({value})",
                        targets.path_of(*scope),
                        declaration.kind.name(),
                        match slot {
                            SdfLimitSlot::First => "first",
                            SdfLimitSlot::Second => "second",
                            SdfLimitSlot::Both => "both",
                        }
                    )
                }
            };
            out.push_str(&format!("{}: {line}\n", op.pos));
        }
        for warning in &self.warnings {
            out.push_str(&format!("warning {warning}\n"));
        }
        out
    }
}
