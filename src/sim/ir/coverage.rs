//! Functional coverage (IEEE 1800-2009 19).
//!
//! Static metadata (`covergroups`, `coverage_types`) describes coverpoint
//! bins; [`IrCoverageStmt`] operations create, configure, sample and control
//! one instance's runtime record, and [`IrCoverageQuery`] computes coverage.
//! Runtime records are reached through a chandle (the record field of the
//! covergroup's synthetic class object); counters live in the runtime.
use super::*;

/// One covergroup declaration identity. Type coverage merges every instance
/// of every [`IrCovergroup`] naming the same type (19.11.3).
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverageType {
    pub(in crate::sim) name: String,
    /// `type_option.weight` of the group (Table 19-3).
    pub(in crate::sim) weight: i64,
}

/// One elaborated covergroup declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCovergroup {
    pub(in crate::sim) name: String,
    pub(in crate::sim) coverage_type: usize,
    pub(in crate::sim) points: Vec<IrCoverpoint>,
    /// Argument values bins read, filled by [`IrCoverageStmt::Slot`] at
    /// `new` (19.5).
    pub(in crate::sim) slots: u32,
    /// Bin `iff` guards every sample evaluates ([`IrCoverageStmt::Sample`]).
    pub(in crate::sim) guards: u32,
    /// `type_option.strobe` (19.7.1): the sample method (no formals) that a
    /// Postponed-region flush calls for each instance its clocking event
    /// requested in the time slot.
    pub(in crate::sim) strobe: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverpoint {
    pub(in crate::sim) name: String,
    /// 1..=64 bits.
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    /// `type_option.weight` of the coverpoint (Table 19-3).
    pub(in crate::sim) type_weight: i64,
    /// The automatic bins of a coverpoint that declares no `bins` (19.5.2).
    pub(in crate::sim) automatic: IrCoverAutomatic,
    /// Declared bins in source order, `ignore_bins` and `illegal_bins`
    /// included.
    pub(in crate::sim) bins: Vec<IrCoverBin>,
    /// Value items the bins name by range.
    pub(in crate::sim) items: Vec<IrCoverItem>,
}

/// Keys order values numerically: the value masked to the coverpoint width,
/// with the sign bit flipped when signed.
#[derive(Clone, Debug, PartialEq)]
pub enum IrCoverAutomatic {
    None,
    /// `min(2^M, auto_bin_max)` automatic bins.
    Auto,
    /// One bin per enum member, in ascending key order.
    Enum(Vec<u64>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverBinKind {
    Bins,
    Ignore,
    Illegal,
    /// `bins b = default`.
    Default,
    /// `illegal_bins b = default`.
    IllegalDefault,
}

impl IrCoverBinKind {
    pub(in crate::sim) fn code(self) -> u32 {
        match self {
            Self::Bins => 0,
            Self::Ignore => 1,
            Self::Illegal => 2,
            Self::Default => 3,
            Self::IllegalDefault => 4,
        }
    }
}

/// `bins b`, `bins b[]` and `bins b[N]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverBinShape {
    Scalar,
    PerValue,
    Count(IrCoverCount),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverCount {
    Const(u64),
    Slot(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverBin {
    pub(in crate::sim) name: String,
    /// Declaration location for run-time messages.
    pub(in crate::sim) site: String,
    pub(in crate::sim) kind: IrCoverBinKind,
    pub(in crate::sim) shape: IrCoverBinShape,
    /// Index of the bin's `iff` guard among the group's guards.
    pub(in crate::sim) guard: Option<u32>,
    pub(in crate::sim) first_item: u32,
    pub(in crate::sim) n_items: u32,
}

/// One bound of a value item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverBound {
    /// A key resolved at compile time.
    Key(u64),
    /// An argument value resolved by 19.5.6 when the instance is ready.
    Slot(u32),
    /// `$` as a lower bound.
    Min,
    /// `$` as an upper bound.
    Max,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverItem {
    pub(in crate::sim) lo: IrCoverBound,
    pub(in crate::sim) hi: IrCoverBound,
    /// Location of an item that reads a slot, for run-time warnings.
    pub(in crate::sim) site: Option<String>,
}

impl IrCoverpoint {
    /// Whether a bin reads a slot, so its layout is built per instance.
    pub(in crate::sim) fn dynamic(&self) -> bool {
        self.items.iter().any(|item| {
            !matches!(
                (item.lo, item.hi),
                (IrCoverBound::Key(_), IrCoverBound::Key(_))
            )
        }) || self
            .bins
            .iter()
            .any(|bin| matches!(bin.shape, IrCoverBinShape::Count(IrCoverCount::Slot(_))))
    }
}

/// Instance-specific options a covergroup body may set (Table 19-1). Only
/// options that change coverage values reach the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverageOption {
    Weight,
    Goal,
    AtLeast,
    AutoBinMax,
}

impl IrCoverageOption {
    pub(in crate::sim) fn code(self) -> u32 {
        match self {
            Self::Weight => 0,
            Self::Goal => 1,
            Self::AtLeast => 2,
            Self::AutoBinMax => 3,
        }
    }
}

/// One coverpoint sample: `value` counts unless `iff` is false or unknown.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverageSample {
    pub(in crate::sim) point: u32,
    pub(in crate::sim) iff: Option<IrExpr>,
    pub(in crate::sim) value: IrExpr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrCoverageStmt {
    /// Store a new runtime record of `group` into the chandle storage that
    /// native access `target` names.
    Create { group: usize, target: String },
    /// Apply one option to the record; `point` `None` is the group scope.
    Option {
        record: IrChandleExpr,
        point: Option<u32>,
        option: IrCoverageOption,
        value: IrExpr,
        site: String,
    },
    /// Store argument value `slot` that bins of the record read.
    Slot {
        record: IrChandleExpr,
        slot: u32,
        value: IrExpr,
    },
    /// Options and slots are final: resolve defaults, build the bin layouts
    /// and size the counters.
    Ready { record: IrChandleExpr },
    /// Add `object` to the objects the group's coverage event samples.
    Register { group: usize, object: IrChandleExpr },
    /// Count the coverpoint values of one sample of the record. `guards`
    /// are the group's bin `iff` guards, evaluated first.
    Sample {
        record: IrChandleExpr,
        guards: Vec<IrExpr>,
        points: Vec<IrCoverageSample>,
    },
    /// `start()` / `stop()`; `point` `None` is the whole instance.
    Control {
        record: IrChandleExpr,
        point: Option<u32>,
        start: bool,
    },
    /// Call sample method `function` (no formals) once per registered object
    /// of `group`, in registration order, with the object as receiver. With
    /// `strobe`, request one such pass in the Postponed region of the
    /// current time slot instead (19.7.1).
    SampleRegistered {
        group: usize,
        function: usize,
        depth: IrDepth,
        strobe: bool,
    },
    /// Start the per-instance sampling-event waiter of a new covergroup
    /// `object`: the captured branch `waiter` (a helper of the group's
    /// sample method whose only capture, slot 0, is the object) as a
    /// runtime-owned process that no process owns.
    Watch {
        object: IrChandleExpr,
        waiter: String,
        label: String,
    },
    /// One instance's clocking event occurred: call sample method
    /// `function` (no formals) on `object` unless `record` is stopped, or,
    /// with `strobe`, request one such call in the Postponed region of the
    /// current time slot (19.7.1).
    SampleInstance {
        group: usize,
        record: IrChandleExpr,
        object: IrChandleExpr,
        function: usize,
        depth: IrDepth,
        strobe: bool,
    },
}

impl IrCovergroup {
    /// Static table invariants the runtime relies on.
    pub(in crate::sim) fn validate(&self, types: usize) -> Result<(), String> {
        if self.coverage_type >= types {
            return Err("covergroup names an unknown coverage type".to_owned());
        }
        if u32::try_from(self.points.len()).is_err() {
            return Err("too many coverpoints".to_owned());
        }
        for point in &self.points {
            if !(1..=64).contains(&point.width) {
                return Err(format!("coverpoint `{}` is not 1 to 64 bits", point.name));
            }
            let max = if point.width == 64 {
                u64::MAX
            } else {
                (1u64 << point.width) - 1
            };
            if let IrCoverAutomatic::Enum(keys) = &point.automatic {
                if keys.is_empty()
                    || keys.windows(2).any(|pair| pair[0] >= pair[1])
                    || keys.iter().any(|key| *key > max)
                {
                    return Err(format!("enum coverpoint `{}` keys are invalid", point.name));
                }
            }
            let bound_valid = |bound: IrCoverBound| match bound {
                IrCoverBound::Key(key) => key <= max,
                IrCoverBound::Slot(slot) => slot < self.slots,
                IrCoverBound::Min | IrCoverBound::Max => true,
            };
            let items_valid = point.items.iter().all(|item| {
                bound_valid(item.lo)
                    && bound_valid(item.hi)
                    && match (item.lo, item.hi) {
                        (IrCoverBound::Key(lo), IrCoverBound::Key(hi)) => lo <= hi,
                        _ => item.site.is_some(),
                    }
            });
            let bins_valid = point.bins.iter().all(|bin| {
                (bin.first_item as usize)
                    .checked_add(bin.n_items as usize)
                    .is_some_and(|end| end <= point.items.len())
                    && bin.guard.is_none_or(|guard| guard < self.guards)
                    && match bin.shape {
                        IrCoverBinShape::Count(IrCoverCount::Const(count)) => count > 0,
                        IrCoverBinShape::Count(IrCoverCount::Slot(slot)) => slot < self.slots,
                        _ => true,
                    }
            });
            if !items_valid || !bins_valid {
                return Err(format!("coverpoint `{}` bins are invalid", point.name));
            }
        }
        Ok(())
    }
}

impl IrCoverageStmt {
    pub(in crate::sim) fn expressions(&self) -> Vec<&IrExpr> {
        match self {
            Self::Option { value, .. } | Self::Slot { value, .. } => vec![value],
            Self::Sample { guards, points, .. } => guards
                .iter()
                .chain(
                    points
                        .iter()
                        .flat_map(|sample| sample.iff.iter().chain(std::iter::once(&sample.value))),
                )
                .collect(),
            Self::Create { .. }
            | Self::Ready { .. }
            | Self::Register { .. }
            | Self::Control { .. }
            | Self::SampleRegistered { .. }
            | Self::Watch { .. }
            | Self::SampleInstance { .. } => Vec::new(),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self) -> Vec<&mut IrExpr> {
        match self {
            Self::Option { value, .. } | Self::Slot { value, .. } => vec![value],
            Self::Sample { guards, points, .. } => guards
                .iter_mut()
                .chain(points.iter_mut().flat_map(|sample| {
                    sample
                        .iff
                        .iter_mut()
                        .chain(std::iter::once(&mut sample.value))
                }))
                .collect(),
            Self::Create { .. }
            | Self::Ready { .. }
            | Self::Register { .. }
            | Self::Control { .. }
            | Self::SampleRegistered { .. }
            | Self::Watch { .. }
            | Self::SampleInstance { .. } => Vec::new(),
        }
    }

    pub(in crate::sim) fn chandles(&self) -> Vec<&IrChandleExpr> {
        match self {
            Self::Option { record, .. }
            | Self::Slot { record, .. }
            | Self::Ready { record }
            | Self::Sample { record, .. }
            | Self::Control { record, .. } => vec![record],
            Self::Register { object, .. } | Self::Watch { object, .. } => vec![object],
            Self::SampleInstance { record, object, .. } => vec![record, object],
            Self::Create { .. } | Self::SampleRegistered { .. } => Vec::new(),
        }
    }

    pub(in crate::sim) fn chandles_mut(&mut self) -> Vec<&mut IrChandleExpr> {
        match self {
            Self::Option { record, .. }
            | Self::Slot { record, .. }
            | Self::Ready { record }
            | Self::Sample { record, .. }
            | Self::Control { record, .. } => vec![record],
            Self::Register { object, .. } | Self::Watch { object, .. } => vec![object],
            Self::SampleInstance { record, object, .. } => vec![record, object],
            Self::Create { .. } | Self::SampleRegistered { .. } => Vec::new(),
        }
    }

    /// The sample method this statement calls (for every registered object,
    /// or for one instance).
    pub(in crate::sim) fn called_function(&self) -> Option<usize> {
        match self {
            Self::SampleRegistered { function, .. } | Self::SampleInstance { function, .. } => {
                Some(*function)
            }
            _ => None,
        }
    }

    /// Native access names this statement stores through.
    pub(in crate::sim) fn written_access(&self) -> Option<&str> {
        match self {
            Self::Create { target, .. } => Some(target),
            _ => None,
        }
    }
}

/// A real-valued coverage query (19.8, 19.9).
#[derive(Clone, Debug, PartialEq)]
pub enum IrCoverageQuery {
    /// `get_inst_coverage()` of one instance (or one of its coverpoints).
    Instance {
        record: IrChandleExpr,
        point: Option<u32>,
    },
    /// `get_coverage()`: type coverage of a declaration identity. `weight`
    /// is the queried scope's `type_option.weight`, which decides the
    /// zero-denominator result.
    Type {
        coverage_type: usize,
        point: Option<u32>,
        weight: i64,
    },
    /// `$get_coverage()`.
    Total,
}

impl IrCoverageQuery {
    pub(in crate::sim) fn chandle(&self) -> Option<&IrChandleExpr> {
        match self {
            Self::Instance { record, .. } => Some(record),
            Self::Type { .. } | Self::Total => None,
        }
    }

    pub(in crate::sim) fn chandle_mut(&mut self) -> Option<&mut IrChandleExpr> {
        match self {
            Self::Instance { record, .. } => Some(record),
            Self::Type { .. } | Self::Total => None,
        }
    }
}
