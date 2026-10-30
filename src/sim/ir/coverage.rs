//! Functional coverage (IEEE 1800-2009 19).
//!
//! Static metadata (`covergroups`, `coverage_types`) describes coverpoint
//! bins; [`IrCoverageStmt`] operations create, configure, sample and control
//! one instance's runtime record, and [`IrCoverageQuery`] computes coverage.
//! Runtime records are reached through a chandle (the record field of the
//! covergroup's synthetic class object); counters live in the runtime.
use super::*;

/// One covergroup declaration identity. Type coverage merges every instance
/// of every [`IrCovergroup`] naming the same type (19.11.3). The values are
/// the declaration's; procedural writes change the runtime copy (19.7.1).
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverageType {
    pub(in crate::sim) name: String,
    /// `type_option.weight` of the group (Table 19-3).
    pub(in crate::sim) weight: i64,
    /// `type_option.merge_instances` (19.11.3).
    pub(in crate::sim) merge: bool,
    /// `type_option.goal`.
    pub(in crate::sim) goal: i64,
    /// `type_option.comment`.
    pub(in crate::sim) comment: String,
    /// `type_option.strobe`.
    pub(in crate::sim) strobe: bool,
    /// The type options of each coverpoint, then each cross.
    pub(in crate::sim) items: Vec<IrCoverageTypeItem>,
}

/// The type options of one coverpoint or cross (Table 19-4).
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverageTypeItem {
    pub(in crate::sim) weight: i64,
    pub(in crate::sim) goal: i64,
    pub(in crate::sim) comment: String,
}

/// One elaborated covergroup declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCovergroup {
    pub(in crate::sim) name: String,
    /// The hierarchical path of the declaration's scope, the base of default
    /// instance names (decision S46-D1).
    pub(in crate::sim) path: String,
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
    /// Pairs of `bins` declarations that share a sequence, reported when
    /// option.detect_overlap is set (19.7).
    pub(in crate::sim) overlaps: Vec<(u32, u32)>,
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

/// The integral instance-specific options (Table 19-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverageOption {
    Weight,
    Goal,
    AtLeast,
    AutoBinMax,
    DetectOverlap,
    GetInstCoverage,
    PerInstance,
    CrossNumPrintMissing,
}

impl IrCoverageOption {
    pub(in crate::sim) fn code(self) -> u32 {
        match self {
            Self::Weight => 0,
            Self::Goal => 1,
            Self::AtLeast => 2,
            Self::AutoBinMax => 3,
            Self::DetectOverlap => 4,
            Self::GetInstCoverage => 5,
            Self::PerInstance => 6,
            Self::CrossNumPrintMissing => 7,
        }
    }

    /// The option's member type in `option` (19.10): `bit` or `int`.
    pub(in crate::sim) fn is_bit(self) -> bool {
        matches!(
            self,
            Self::DetectOverlap | Self::GetInstCoverage | Self::PerInstance
        )
    }
}

/// The integral type options (Table 19-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverageTypeOption {
    Weight,
    Goal,
    Strobe,
    MergeInstances,
}

impl IrCoverageTypeOption {
    pub(in crate::sim) fn code(self) -> u32 {
        match self {
            Self::Weight => 0,
            Self::Goal => 1,
            Self::Strobe => 2,
            Self::MergeInstances => 3,
        }
    }

    /// The option's member type in `type_option` (19.10).
    pub(in crate::sim) fn is_bit(self) -> bool {
        matches!(self, Self::Strobe | Self::MergeInstances)
    }
}

/// The string instance options (Table 19-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverageText {
    Name,
    Comment,
}

impl IrCoverageText {
    pub(in crate::sim) fn code(self) -> u32 {
        match self {
            Self::Name => 0,
            Self::Comment => 1,
        }
    }
}

/// A string option read (19.7, 19.7.1).
#[derive(Clone, Debug, PartialEq)]
pub enum IrCoverageTextQuery {
    Instance {
        record: IrChandleExpr,
        point: Option<u32>,
        which: IrCoverageText,
    },
    Type {
        coverage_type: usize,
        point: Option<u32>,
    },
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
    /// Set a string option of the record: the declaration's `option.name`
    /// or `option.comment`, a procedural write, or `set_inst_name` (19.8).
    Text {
        record: IrChandleExpr,
        point: Option<u32>,
        which: IrCoverageText,
        value: IrStringExpr,
    },
    /// A procedural write of a type option (19.7.1).
    TypeOption {
        coverage_type: usize,
        point: Option<u32>,
        option: IrCoverageTypeOption,
        value: IrExpr,
        site: String,
    },
    /// A procedural write of `type_option.comment`.
    TypeText {
        coverage_type: usize,
        point: Option<u32>,
        value: IrStringExpr,
    },
    /// `$set_coverage_db_name` (19.9).
    SetDbName { name: IrStringExpr },
    /// `$load_coverage_db` (19.9).
    LoadDb { name: IrStringExpr, site: String },
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
    /// Statement coverage (IEEE 1800-2009 40.2.2): the source statement
    /// [`IrAssertionCoverage::statements`]`[entity]` starts executing. It
    /// only counts: it reads and writes no design state.
    StatementHit { entity: u32 },
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
            && self
                .overlaps
                .iter()
                .all(|(a, b)| a < b && (*b as usize) < self.bins.len())
    }
}

impl IrCoverageStmt {
    /// The string operands, whose own expressions [`Self::expressions`]
    /// does not include.
    pub(in crate::sim) fn strings(&self) -> Vec<&IrStringExpr> {
        match self {
            Self::Text { value, .. } | Self::TypeText { value, .. } => vec![value],
            Self::SetDbName { name } | Self::LoadDb { name, .. } => vec![name],
            _ => Vec::new(),
        }
    }

    pub(in crate::sim) fn strings_mut(&mut self) -> Vec<&mut IrStringExpr> {
        match self {
            Self::Text { value, .. } | Self::TypeText { value, .. } => vec![value],
            Self::SetDbName { name } | Self::LoadDb { name, .. } => vec![name],
            _ => Vec::new(),
        }
    }

    pub(in crate::sim) fn expressions(&self) -> Vec<&IrExpr> {
        match self {
            Self::Option { value, .. }
            | Self::Slot { value, .. }
            | Self::TypeOption { value, .. } => vec![value],
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
            | Self::SampleInstance { .. }
            | Self::Text { .. }
            | Self::TypeText { .. }
            | Self::SetDbName { .. }
            | Self::LoadDb { .. }
            | Self::StatementHit { .. } => Vec::new(),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self) -> Vec<&mut IrExpr> {
        match self {
            Self::Option { value, .. }
            | Self::Slot { value, .. }
            | Self::TypeOption { value, .. } => vec![value],
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
            | Self::SampleInstance { .. }
            | Self::Text { .. }
            | Self::TypeText { .. }
            | Self::SetDbName { .. }
            | Self::LoadDb { .. }
            | Self::StatementHit { .. } => Vec::new(),
        }
    }

    pub(in crate::sim) fn chandles(&self) -> Vec<&IrChandleExpr> {
        match self {
            Self::Option { record, .. }
            | Self::Text { record, .. }
            | Self::Slot { record, .. }
            | Self::Ready { record }
            | Self::Sample { record, .. }
            | Self::Control { record, .. } => vec![record],
            Self::Register { object, .. } | Self::Watch { object, .. } => vec![object],
            Self::SampleInstance { record, object, .. } => vec![record, object],
            Self::Create { .. }
            | Self::SampleRegistered { .. }
            | Self::TypeOption { .. }
            | Self::TypeText { .. }
            | Self::SetDbName { .. }
            | Self::LoadDb { .. }
            | Self::StatementHit { .. } => Vec::new(),
        }
    }

    pub(in crate::sim) fn chandles_mut(&mut self) -> Vec<&mut IrChandleExpr> {
        match self {
            Self::Option { record, .. }
            | Self::Text { record, .. }
            | Self::Slot { record, .. }
            | Self::Ready { record }
            | Self::Sample { record, .. }
            | Self::Control { record, .. } => vec![record],
            Self::Register { object, .. } | Self::Watch { object, .. } => vec![object],
            Self::SampleInstance { record, object, .. } => vec![record, object],
            Self::Create { .. }
            | Self::SampleRegistered { .. }
            | Self::TypeOption { .. }
            | Self::TypeText { .. }
            | Self::SetDbName { .. }
            | Self::LoadDb { .. }
            | Self::StatementHit { .. } => Vec::new(),
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

/// A coverage query (19.8, 19.9) or an integral option read (19.7).
#[derive(Clone, Debug, PartialEq)]
pub enum IrCoverageQuery {
    /// `get_inst_coverage()` of one instance (or one of its coverpoints).
    Instance {
        record: IrChandleExpr,
        point: Option<u32>,
    },
    /// `get_coverage()`: type coverage of a declaration identity.
    Type {
        coverage_type: usize,
        point: Option<u32>,
    },
    /// `$get_coverage()`.
    Total,
    /// `option.member` of an instance, coverpoint or cross.
    Option {
        record: IrChandleExpr,
        point: Option<u32>,
        option: IrCoverageOption,
    },
    /// `type_option.member` of a type, coverpoint or cross.
    TypeOption {
        coverage_type: usize,
        point: Option<u32>,
        option: IrCoverageTypeOption,
    },
}

impl IrCoverageQuery {
    pub(in crate::sim) fn chandle(&self) -> Option<&IrChandleExpr> {
        match self {
            Self::Instance { record, .. } | Self::Option { record, .. } => Some(record),
            Self::Type { .. } | Self::Total | Self::TypeOption { .. } => None,
        }
    }

    pub(in crate::sim) fn chandle_mut(&mut self) -> Option<&mut IrChandleExpr> {
        match self {
            Self::Instance { record, .. } | Self::Option { record, .. } => Some(record),
            Self::Type { .. } | Self::Total | Self::TypeOption { .. } => None,
        }
    }

    /// The coverage type a query names.
    pub(in crate::sim) fn coverage_type(&self) -> Option<usize> {
        match self {
            Self::Type { coverage_type, .. } | Self::TypeOption { coverage_type, .. } => {
                Some(*coverage_type)
            }
            _ => None,
        }
    }

    /// `(width, signed)` of the result: `real` (width 0) for coverage, the
    /// member type of 19.10 for an option.
    pub(in crate::sim) fn result_type(&self) -> (u32, bool) {
        match self {
            Self::Instance { .. } | Self::Type { .. } | Self::Total => (0, true),
            Self::Option { option, .. } if option.is_bit() => (1, false),
            Self::TypeOption { option, .. } if option.is_bit() => (1, false),
            Self::Option { .. } | Self::TypeOption { .. } => (32, true),
        }
    }
}

impl IrCoverageTextQuery {
    pub(in crate::sim) fn chandle(&self) -> Option<&IrChandleExpr> {
        match self {
            Self::Instance { record, .. } => Some(record),
            Self::Type { .. } => None,
        }
    }

    pub(in crate::sim) fn chandle_mut(&mut self) -> Option<&mut IrChandleExpr> {
        match self {
            Self::Instance { record, .. } => Some(record),
            Self::Type { .. } => None,
        }
    }
}

/// The assertion statement kind of an assertion coverage item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrAssertionItemKind {
    Assert,
    Assume,
    Cover,
}

impl IrAssertionItemKind {
    /// The runtime `LLG_ASSERTION_*` kind constant.
    pub(in crate::sim) fn runtime_name(self) -> &'static str {
        match self {
            Self::Assert => "LLG_ASSERTION_ASSERT",
            Self::Assume => "LLG_ASSERTION_ASSUME",
            Self::Cover => "LLG_ASSERTION_COVER",
        }
    }
}

/// How an assertion coverage item is evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrAssertionItemForm {
    /// A concurrent `assert`/`assume`/`cover property`.
    Concurrent,
    /// A concurrent `cover sequence`: every match is a success (16.15.3).
    CoverSequence,
    /// A simple immediate assertion (16.3).
    Immediate,
    /// A deferred immediate assertion (16.4).
    Deferred,
}

impl IrAssertionItemForm {
    /// The runtime `LLG_ACOV_*` form constant.
    pub(in crate::sim) fn runtime_name(self) -> &'static str {
        match self {
            Self::Concurrent => "LLG_ACOV_CONCURRENT",
            Self::CoverSequence => "LLG_ACOV_COVER_SEQUENCE",
            Self::Immediate => "LLG_ACOV_IMMEDIATE",
            Self::Deferred => "LLG_ACOV_DEFERRED",
        }
    }
}

/// One assertion coverage item (IEEE 1800-2009 40.2.2): an elaborated
/// assertion statement instance with its own attempt and result counters.
#[derive(Clone, Debug, PartialEq)]
pub struct IrAssertionItem {
    /// The owned semantic node of the statement, as the assertion runtime
    /// and the immediate assertion statements name it.
    pub(in crate::sim) identity: u64,
    pub(in crate::sim) kind: IrAssertionItemKind,
    pub(in crate::sim) form: IrAssertionItemForm,
    /// The statement label; empty for an unnamed assertion.
    pub(in crate::sim) name: String,
    /// The hierarchical name of a labelled assertion; empty otherwise.
    pub(in crate::sim) full_name: String,
    /// The module instance that owns the statement, as an index of
    /// [`IrAssertionCoverage::instances`]; `None` for a statement outside
    /// every instance (a class or package subroutine), owned by `$root`.
    pub(in crate::sim) instance: Option<u32>,
    /// The statement's source file, line and column (39.3.2); `None` and 0
    /// when the frontend recorded none.
    pub(in crate::sim) file: Option<String>,
    pub(in crate::sim) line: u32,
    pub(in crate::sim) column: u32,
    /// Where the statement ends (37.45 `vpiEndLine`, `vpiEndColumn`); 0
    /// when unknown.
    pub(in crate::sim) end_line: u32,
    pub(in crate::sim) end_column: u32,
    /// The statement's entry in [`super::IrModel::vpi_code`], which holds
    /// its clocking event, disable condition, property, condition and
    /// action statements (37.46, 37.51); `None` without expression objects.
    pub(in crate::sim) code: Option<u32>,
}

/// One elaborated module, interface or program instance of the hierarchy
/// that `$coverage_*` scope arguments select (40.3.2.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrCoverageInstance {
    pub(in crate::sim) path: String,
    pub(in crate::sim) definition: String,
    /// The enclosing instance; `None` for a top-level instance.
    pub(in crate::sim) parent: Option<u32>,
}

/// The assertion coverage tables of a model (40.2.2, 40.3.2, 40.5.3).
/// Items are sorted by identity; parents precede their children.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrAssertionCoverage {
    pub(in crate::sim) items: Vec<IrAssertionItem>,
    pub(in crate::sim) instances: Vec<IrCoverageInstance>,
    /// The FSMs of the IEEE 1800-2009 40.4 pragmas, in instance order.
    pub(in crate::sim) fsms: Vec<IrFsm>,
    /// Statement coverage entities (40.2.2), indexed by
    /// [`IrCoverageStmt::StatementHit`]; empty when the model has no
    /// statement coverage.
    pub(in crate::sim) statements: Vec<IrStatementItem>,
    /// Toggle coverage signals (40.2.2); empty when the model has no toggle
    /// coverage.
    pub(in crate::sim) toggles: Vec<IrToggleSignal>,
    /// Statement and toggle coverage collect from time 0 (a `$coverage_*`
    /// call names the kind) rather than only after a start request.
    pub(in crate::sim) statement_started: bool,
    pub(in crate::sim) toggle_started: bool,
}

/// One statement coverage entity: a source statement of an elaborated
/// instance (or of `$root` for code outside every instance).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrStatementItem {
    /// The owned semantic node of the statement.
    pub(in crate::sim) identity: u64,
    /// Owning instance as an index of [`IrAssertionCoverage::instances`];
    /// `None` for `$root`.
    pub(in crate::sim) instance: Option<u32>,
    /// Source file, line and column; empty and 0 when unknown.
    pub(in crate::sim) file: String,
    pub(in crate::sim) line: u32,
    pub(in crate::sim) column: u32,
    /// The statement's entry in [`super::IrModel::vpi_code`]; `None`
    /// without expression objects.
    pub(in crate::sim) code: Option<u32>,
}

/// One toggle coverage signal: a packed net or variable of an instance
/// whose every bit is a coverable entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrToggleSignal {
    /// The signal storage (an index of [`super::IrModel::signals`]).
    pub(in crate::sim) signal: usize,
    pub(in crate::sim) full_name: String,
    pub(in crate::sim) instance: u32,
    pub(in crate::sim) width: u32,
    /// The VPI catalog row of the net or variable; `None` without a
    /// catalog.
    pub(in crate::sim) object: Option<usize>,
}

/// The widest FSM state expression: the runtime monitor keeps a state value
/// in one 64-bit word.
pub(in crate::sim) const FSM_MAX_STATE_BITS: u32 = 64;

/// One part of an FSM state expression: `width` bits of signal storage
/// starting at bit `lsb`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrFsmPart {
    pub(in crate::sim) signal: usize,
    pub(in crate::sim) lsb: u32,
    pub(in crate::sim) width: u32,
}

/// One state of an FSM: the parameter that names it and its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrFsmState {
    pub(in crate::sim) name: String,
    pub(in crate::sim) value: u64,
}

/// The object a VPI application reaches through vpiFsmStateExpression
/// (40.5.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrFsmExpression {
    /// A whole signal, by its hierarchical name.
    Signal(String),
    /// A part-select `[left:right]` of a signal, as the pragma writes it.
    Select(String, i64, i64),
    /// A concatenation of signals, which has no VPI object in llg.
    Concatenation,
}

/// One FSM of one instance (IEEE 1800-2009 40.4, 40.5.3): the state
/// expression as signal parts (most significant first, `width` bits in all)
/// and the states in declaration order with distinct values below
/// `2^width`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrFsm {
    pub(in crate::sim) name: String,
    pub(in crate::sim) full_name: String,
    pub(in crate::sim) instance: u32,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) parts: Vec<IrFsmPart>,
    pub(in crate::sim) states: Vec<IrFsmState>,
    pub(in crate::sim) expression: IrFsmExpression,
    pub(in crate::sim) file: String,
    pub(in crate::sim) line: u32,
    pub(in crate::sim) column: u32,
}

impl IrAssertionCoverage {
    /// The item index of the assertion statement `identity`.
    pub(in crate::sim) fn item_index(&self, identity: u64) -> Option<usize> {
        self.items
            .binary_search_by_key(&identity, |item| item.identity)
            .ok()
    }

    /// Items strictly ordered by identity, instance indices in range and
    /// every parent before its child, so the runtime can binary-search
    /// items and walk parents without cycles.
    pub(in crate::sim) fn validate(&self) -> Result<(), String> {
        if self
            .items
            .windows(2)
            .any(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err("assertion items are not ordered by identity".to_owned());
        }
        if self.items.len() > i32::MAX as usize || self.instances.len() > i32::MAX as usize {
            return Err("assertion coverage table is too large".to_owned());
        }
        for item in &self.items {
            if item
                .instance
                .is_some_and(|instance| instance as usize >= self.instances.len())
            {
                return Err("assertion item names an unknown instance".to_owned());
            }
        }
        for (index, instance) in self.instances.iter().enumerate() {
            if instance
                .parent
                .is_some_and(|parent| parent as usize >= index)
            {
                return Err("coverage instance parent does not precede it".to_owned());
            }
        }
        if self.fsms.len() > i32::MAX as usize
            || self.statements.len() > i32::MAX as usize
            || self.toggles.len() > i32::MAX as usize
        {
            return Err("FSM coverage table is too large".to_owned());
        }
        let instance_known = |instance: Option<u32>| {
            instance.is_none_or(|index| (index as usize) < self.instances.len())
        };
        if self
            .statements
            .iter()
            .any(|item| !instance_known(item.instance))
            || self
                .toggles
                .iter()
                .any(|toggle| !instance_known(Some(toggle.instance)) || toggle.width == 0)
        {
            return Err("a statement or toggle entity names an unknown instance".to_owned());
        }
        for fsm in &self.fsms {
            if fsm.instance as usize >= self.instances.len() {
                return Err(format!("FSM `{}` names an unknown instance", fsm.full_name));
            }
            let total = fsm
                .parts
                .iter()
                .try_fold(0u32, |total, part| total.checked_add(part.width));
            if fsm.width == 0
                || fsm.width > FSM_MAX_STATE_BITS
                || total != Some(fsm.width)
                || fsm.parts.iter().any(|part| part.width == 0)
            {
                return Err(format!(
                    "FSM `{}` state parts do not make up its {}-bit state",
                    fsm.full_name, fsm.width
                ));
            }
            if fsm.states.is_empty() || fsm.states.len() > i32::MAX as usize {
                return Err(format!("FSM `{}` has no states", fsm.full_name));
            }
            let mut values = std::collections::HashSet::new();
            for state in &fsm.states {
                if (fsm.width < 64 && state.value >> fsm.width != 0) || !values.insert(state.value)
                {
                    return Err(format!(
                        "FSM `{}` state `{}` is out of range or repeated",
                        fsm.full_name, state.name
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn items(&self) -> &[IrAssertionItem] {
        &self.items
    }

    pub fn instances(&self) -> &[IrCoverageInstance] {
        &self.instances
    }

    pub fn fsms(&self) -> &[IrFsm] {
        &self.fsms
    }
}

/// A built-in coverage access function of IEEE 1800-2009 40.3.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCoverageApiFunction {
    /// `$coverage_control(control, type, scope_def, scope)`.
    Control,
    /// `$coverage_get_max(type, scope_def, scope)`.
    GetMax,
    /// `$coverage_get(type, scope_def, scope)`.
    Get,
    /// `$coverage_merge(type, name)`.
    Merge,
    /// `$coverage_save(type, name)`.
    Save,
}

impl IrCoverageApiFunction {
    /// The runtime `LLG_COVERAGE_*` function constant.
    pub(in crate::sim) fn runtime_name(self) -> &'static str {
        match self {
            Self::Control => "LLG_COVERAGE_CONTROL",
            Self::GetMax => "LLG_COVERAGE_GET_MAX",
            Self::Get => "LLG_COVERAGE_GET",
            Self::Merge => "LLG_COVERAGE_MERGE",
            Self::Save => "LLG_COVERAGE_SAVE",
        }
    }

    /// The number of integral arguments before the scope or name.
    pub(in crate::sim) fn integral_arguments(self) -> usize {
        match self {
            Self::Control => 3,
            Self::GetMax | Self::Get => 2,
            Self::Merge | Self::Save => 1,
        }
    }
}

/// The `modules_or_instance` argument of 40.3.2.1-40.3.2.3.
#[derive(Clone, Debug, PartialEq)]
pub enum IrCoverageScope {
    /// An instance of [`IrAssertionCoverage::instances`], named by a
    /// hierarchical reference.
    Instance(u32),
    /// `$root`.
    Root,
    /// A string naming a module definition (Table 40-2).
    Definition(IrStringExpr),
}

/// One call of a 40.3.2 coverage function; the result is a signed `int`
/// status or count.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCoverageApiCall {
    pub(in crate::sim) function: IrCoverageApiFunction,
    /// The integral arguments in source order (control, type, scope_def).
    pub(in crate::sim) args: Vec<IrExpr>,
    /// The hierarchy argument of control, get_max and get.
    pub(in crate::sim) scope: Option<IrCoverageScope>,
    /// The database name of merge and save.
    pub(in crate::sim) name: Option<IrStringExpr>,
    /// `file:line:column` of the call, for run-time errors.
    pub(in crate::sim) site: String,
}

impl IrCoverageApiCall {
    /// The string operands: a definition-name scope and a database name.
    pub(in crate::sim) fn strings(&self) -> impl Iterator<Item = &IrStringExpr> {
        let scope = match &self.scope {
            Some(IrCoverageScope::Definition(text)) => Some(text),
            _ => None,
        };
        scope.into_iter().chain(self.name.as_ref())
    }

    /// Visit every expression operand, string operands included.
    pub(in crate::sim) fn expressions(&self, f: &mut dyn FnMut(&IrExpr)) {
        for arg in &self.args {
            f(arg);
        }
        for text in self.strings() {
            text.expressions(&mut |child| f(child));
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, f: &mut dyn FnMut(&mut IrExpr)) {
        for arg in &mut self.args {
            f(arg);
        }
        let Self { scope, name, .. } = self;
        if let Some(IrCoverageScope::Definition(text)) = scope {
            text.expressions_mut(&mut |child| f(child));
        }
        if let Some(text) = name {
            text.expressions_mut(&mut |child| f(child));
        }
    }
}
