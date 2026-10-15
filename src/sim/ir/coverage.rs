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
    /// Crosses (19.6); coverage items are numbered points first, then
    /// crosses.
    pub(in crate::sim) crosses: Vec<IrCoverCross>,
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
    /// Transition bins (19.5.1), when the coverpoint declares any.
    pub(in crate::sim) transitions: Option<IrCoverTransitions>,
    /// A cross reads the bins each sample counts.
    pub(in crate::sim) crossed: bool,
}

/// One cross (19.6). Its coordinates along each crossed coverpoint are that
/// coverpoint's coverable bins, ranked; each bin's select expression is a
/// union of boxes, a box an intersection of select conditions.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverCross {
    pub(in crate::sim) name: String,
    /// The crossed coverpoints, by index.
    pub(in crate::sim) points: Vec<u32>,
    /// `type_option.weight` of the cross (Table 19-3).
    pub(in crate::sim) type_weight: i64,
    pub(in crate::sim) selects: Vec<IrCoverSelect>,
    pub(in crate::sim) bins: Vec<IrCoverCrossBin>,
    /// `intersect` values, resolved against the crossed coverpoint's type.
    pub(in crate::sim) items: Vec<IrCoverItem>,
}

/// `binsof(point[.bin]) [intersect { items }]`, possibly negated: a set of
/// coordinates along dimension `dim`.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverSelect {
    pub(in crate::sim) dim: u32,
    /// The value bins declaration of the coverpoint, by index.
    pub(in crate::sim) decl: Option<u32>,
    pub(in crate::sim) intersect: bool,
    pub(in crate::sim) first_item: u32,
    pub(in crate::sim) n_items: u32,
    pub(in crate::sim) negate: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverCrossBin {
    pub(in crate::sim) name: String,
    pub(in crate::sim) site: String,
    /// `Bins`, `Ignore` or `Illegal`.
    pub(in crate::sim) kind: IrCoverBinKind,
    pub(in crate::sim) guard: Option<u32>,
    /// The select expression in disjunctive form: select indices per box.
    pub(in crate::sim) boxes: Vec<Vec<u32>>,
}

impl IrCoverCross {
    /// Whether an `intersect` value reads a slot.
    pub(in crate::sim) fn dynamic(&self) -> bool {
        self.items.iter().any(|item| {
            !matches!(
                (item.lo, item.hi),
                (IrCoverBound::Key(_), IrCoverBound::Key(_))
            )
        })
    }
}

/// The transition bins of one coverpoint. Keys fall into classes: class `i`
/// holds the keys from `bounds[i]` up to the next bound; each step accepts a
/// set of classes. Each set is an automaton whose states are numbered from
/// its steps' `state`.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverTransitions {
    /// Ascending class lower bounds; the first is key 0.
    pub(in crate::sim) bounds: Vec<u64>,
    /// Words of one step's class bitset.
    pub(in crate::sim) words: u32,
    /// `words` per step.
    pub(in crate::sim) accept: Vec<u64>,
    pub(in crate::sim) steps: Vec<IrCoverTransStep>,
    pub(in crate::sim) sets: Vec<IrCoverTransSet>,
    /// Offsets into `start_sets`, one per class plus the end.
    pub(in crate::sim) start_index: Vec<u32>,
    /// Per class: the sets whose first step accepts it.
    pub(in crate::sim) start_sets: Vec<u32>,
    /// Automaton states of all sets.
    pub(in crate::sim) states: u32,
    /// The longest sequence of an array bin: the recent keys an instance
    /// keeps to name the sequence it counts.
    pub(in crate::sim) window: u32,
    pub(in crate::sim) bins: Vec<IrCoverTransBin>,
    /// The `default sequence` bin.
    pub(in crate::sim) default_bin: Option<u32>,
}

/// The repetition of a step; a step without repetition is `[* 1]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverRepeat {
    Consecutive,
    /// `[-> m:n]`, and `[= m:n]` that ends its set.
    GoTo,
    /// `[= m:n]` followed by another step.
    Nonconsecutive,
}

impl IrCoverRepeat {
    pub(in crate::sim) fn code(self) -> u32 {
        match self {
            Self::Consecutive => 0,
            Self::GoTo => 1,
            Self::Nonconsecutive => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverTransStep {
    pub(in crate::sim) repeat: IrCoverRepeat,
    /// The final step of its set.
    pub(in crate::sim) last: bool,
    pub(in crate::sim) lo: u32,
    pub(in crate::sim) hi: u32,
    /// The state of repetition count 0.
    pub(in crate::sim) state: u32,
    pub(in crate::sim) set: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverTransSet {
    pub(in crate::sim) bin: u32,
    pub(in crate::sim) first_step: u32,
    pub(in crate::sim) steps: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverTransBin {
    pub(in crate::sim) name: String,
    pub(in crate::sim) site: String,
    /// `Default` and `IllegalDefault` are `default sequence` bins.
    pub(in crate::sim) kind: IrCoverBinKind,
    /// `b[]`: one bin per sequence.
    pub(in crate::sim) array: bool,
    pub(in crate::sim) guard: Option<u32>,
    /// Bins that count toward coverage (19.11.1): 0 or 1 for a scalar bin,
    /// the number of sequences of an array bin.
    pub(in crate::sim) coverable: u64,
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
    /// The coverpoint has transition bins: a false `iff` ends the
    /// sequences in progress (decision S45-D9).
    pub(in crate::sim) history: bool,
}

/// One cross sample, after every coverpoint of the group: counts the
/// products of the bins the crossed coverpoints counted, unless `iff` is
/// false.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverageCrossSample {
    pub(in crate::sim) cross: u32,
    pub(in crate::sim) iff: Option<IrExpr>,
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
        crosses: Vec<IrCoverageCrossSample>,
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
            if point.crossed && point.transitions.is_some() {
                return Err(format!(
                    "coverpoint `{}` is crossed and has transition bins",
                    point.name
                ));
            }
            if let Some(transitions) = &point.transitions {
                if !transitions.valid(max, self.guards) {
                    return Err(format!(
                        "coverpoint `{}` transition bins are invalid",
                        point.name
                    ));
                }
            }
        }
        for cross in &self.crosses {
            let points_valid = cross.points.len() >= 2
                && cross.points.iter().all(|point| {
                    self.points
                        .get(*point as usize)
                        .is_some_and(|point| point.crossed)
                });
            let selects_valid = cross.selects.iter().all(|select| {
                let point = cross
                    .points
                    .get(select.dim as usize)
                    .and_then(|point| self.points.get(*point as usize));
                point.is_some_and(|point| {
                    select
                        .decl
                        .is_none_or(|decl| (decl as usize) < point.bins.len())
                }) && (select.first_item as usize)
                    .checked_add(select.n_items as usize)
                    .is_some_and(|end| end <= cross.items.len())
            });
            let bins_valid = cross.bins.iter().all(|bin| {
                matches!(
                    bin.kind,
                    IrCoverBinKind::Bins | IrCoverBinKind::Ignore | IrCoverBinKind::Illegal
                ) && bin.guard.is_none_or(|guard| guard < self.guards)
                    && bin
                        .boxes
                        .iter()
                        .flatten()
                        .all(|select| (*select as usize) < cross.selects.len())
            });
            if !points_valid || !selects_valid || !bins_valid {
                return Err(format!("cross `{}` is invalid", cross.name));
            }
        }
        Ok(())
    }
}

impl IrCoverTransitions {
    /// The table invariants the runtime indexes by.
    fn valid(&self, max_key: u64, guards: u32) -> bool {
        let classes = self.bounds.len();
        let words = self.words as usize;
        let states = self.states as u64;
        self.bounds.first() == Some(&0)
            && self.bounds.windows(2).all(|pair| pair[0] < pair[1])
            && self.bounds.iter().all(|bound| *bound <= max_key)
            && words == classes.div_ceil(64)
            && self.accept.len() == self.steps.len() * words
            && self.start_index.len() == classes + 1
            && self.start_index.windows(2).all(|pair| pair[0] <= pair[1])
            && self.start_index.last().copied() == Some(self.start_sets.len() as u32)
            && self
                .start_sets
                .iter()
                .all(|set| (*set as usize) < self.sets.len())
            && self.steps.iter().all(|step| {
                let span = match step.repeat {
                    IrCoverRepeat::Nonconsecutive => u64::from(step.hi) + 1,
                    _ => u64::from(step.hi),
                };
                (1..=step.hi).contains(&step.lo)
                    && u64::from(step.state) + span <= states
                    && (step.set as usize) < self.sets.len()
                    && (step.repeat != IrCoverRepeat::Nonconsecutive || !step.last)
            })
            && self.sets.iter().enumerate().all(|(index, set)| {
                let end = set.first_step as usize + set.steps as usize;
                set.steps > 0
                    && (set.bin as usize) < self.bins.len()
                    && end <= self.steps.len()
                    && self.steps[set.first_step as usize..end]
                        .iter()
                        .enumerate()
                        .all(|(offset, step)| {
                            step.set as usize == index
                                && step.last == (offset + 1 == set.steps as usize)
                        })
                    && (!self.bins[set.bin as usize].array || set.steps <= self.window)
            })
            && self
                .bins
                .iter()
                .all(|bin| bin.guard.is_none_or(|guard| guard < guards))
            && self
                .default_bin
                .is_none_or(|bin| (bin as usize) < self.bins.len())
    }
}

impl IrCoverageStmt {
    pub(in crate::sim) fn expressions(&self) -> Vec<&IrExpr> {
        match self {
            Self::Option { value, .. } | Self::Slot { value, .. } => vec![value],
            Self::Sample {
                guards,
                points,
                crosses,
                ..
            } => guards
                .iter()
                .chain(
                    points
                        .iter()
                        .flat_map(|sample| sample.iff.iter().chain(std::iter::once(&sample.value))),
                )
                .chain(crosses.iter().filter_map(|cross| cross.iff.as_ref()))
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
            Self::Sample {
                guards,
                points,
                crosses,
                ..
            } => guards
                .iter_mut()
                .chain(points.iter_mut().flat_map(|sample| {
                    sample
                        .iff
                        .iter_mut()
                        .chain(std::iter::once(&mut sample.value))
                }))
                .chain(crosses.iter_mut().filter_map(|cross| cross.iff.as_mut()))
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
