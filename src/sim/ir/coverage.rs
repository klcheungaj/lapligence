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
    pub(in crate::sim) bins: IrCoverBins,
}

/// Bin layout of one coverpoint. Keys order values numerically: the value
/// masked to the coverpoint width, with the sign bit flipped when signed.
#[derive(Clone, Debug, PartialEq)]
pub enum IrCoverBins {
    /// `min(2^M, auto_bin_max)` automatic bins (19.5.2).
    Auto,
    /// One bin per enum member, in ascending key order.
    Enum(Vec<u64>),
    /// Explicit value bins: maximal disjoint key ranges in ascending order,
    /// each naming every bin that contains it, and which bins have a value.
    Explicit {
        segments: Vec<IrCoverSegment>,
        valued: Vec<bool>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrCoverSegment {
    pub(in crate::sim) lo: u64,
    pub(in crate::sim) hi: u64,
    pub(in crate::sim) bins: Vec<u32>,
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
    /// Options are final: resolve defaults and size the counters.
    Ready { record: IrChandleExpr },
    /// Add `object` to the objects the group's coverage event samples.
    Register { group: usize, object: IrChandleExpr },
    /// Count the coverpoint values of one sample of the record.
    Sample {
        record: IrChandleExpr,
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
            match &point.bins {
                IrCoverBins::Auto => {}
                IrCoverBins::Enum(keys) => {
                    if keys.is_empty()
                        || keys.windows(2).any(|pair| pair[0] >= pair[1])
                        || keys.iter().any(|key| *key > max)
                    {
                        return Err(format!("enum coverpoint `{}` keys are invalid", point.name));
                    }
                }
                IrCoverBins::Explicit { segments, valued } => {
                    let ordered = segments.windows(2).all(|pair| pair[0].hi < pair[1].lo);
                    let bounded = segments.iter().all(|segment| {
                        segment.lo <= segment.hi
                            && segment.hi <= max
                            && !segment.bins.is_empty()
                            && segment
                                .bins
                                .iter()
                                .all(|bin| (*bin as usize) < valued.len())
                    });
                    if valued.is_empty() || !ordered || !bounded {
                        return Err(format!(
                            "explicit coverpoint `{}` segments are invalid",
                            point.name
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

impl IrCoverageStmt {
    pub(in crate::sim) fn expressions(&self) -> Vec<&IrExpr> {
        match self {
            Self::Option { value, .. } => vec![value],
            Self::Sample { points, .. } => points
                .iter()
                .flat_map(|sample| sample.iff.iter().chain(std::iter::once(&sample.value)))
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
            Self::Option { value, .. } => vec![value],
            Self::Sample { points, .. } => points
                .iter_mut()
                .flat_map(|sample| {
                    sample
                        .iff
                        .iter_mut()
                        .chain(std::iter::once(&mut sample.value))
                })
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
