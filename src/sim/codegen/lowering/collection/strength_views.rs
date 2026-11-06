//! Net strength views for `%v` (IEEE 1364-2001 17.1.1.5, IEEE 1800-2009
//! 21.2.1.5).
//!
//! A strength view is a two-state signal with eight bits per net bit that the
//! runtime republishes after every resolution of its net (`llg_net_t.strength`).
//! It exists only for nets that a `%v` argument reads or whose strength-only
//! changes something waits for, so ordinary nets keep their value-only
//! resolution. Monitors depend on the view itself, which is how a strength-only
//! change, such as St1 to Pu1, still reaches them.
//!
//! An event control on a net or a constant projection of nets, an implicit
//! event list, `wait` and `$monitor` also watch the views of the nets they read
//! (decision T1-D8). Only a net whose strength can change without its value
//! has a view for them ([`Codegen::note_strength_nets`]): one with nothing but
//! strong drivers shows one strength for each value, so it costs nothing.

use super::*;
use crate::sim::emit_c::LLG_MAX_WIDTH;
use crate::sim::ir::{IrDependency, IrDisplayArg, IrNetKind};

/// Bits of view storage per net bit.
const VIEW_BITS: u32 = 8;

/// The nets whose strength can change while their value holds.
#[derive(Default)]
pub(in super::super) struct StrengthNets {
    /// By net group; groups created later never vary.
    groups: Vec<bool>,
    /// The dependency name of each signal with a bit on such a net.
    signals: HashMap<String, usize>,
}

impl Codegen<'_> {
    /// Record the groups whose strength can change without their value, once
    /// every driver slot and switch is known and before the first process.
    ///
    /// With strong drivers alone each value has one strength. Any other drive
    /// strength can differ from the next driver's, or from the strong value of
    /// a force; a `tri0` or `tri1` net has its pull as a second strength; a
    /// switch reduces strength or hands a net another net's. A reader of a group
    /// whose view would pass the packed width limit rejects
    /// ([`Self::net_strength_view`]); the group costs nothing while none reads it.
    pub(in super::super) fn note_strength_nets(&mut self) {
        let mut groups: Vec<bool> = self
            .model
            .net_groups
            .iter()
            .map(|group| {
                matches!(group.kind, IrNetKind::Tri0 | IrNetKind::Tri1)
                    || group.driver_strengths.iter().any(|slot| *slot != (6, 6))
            })
            .collect();
        for group in self.pending_switch_groups() {
            groups[group] = true;
        }
        let mut signals = HashMap::new();
        if groups.contains(&true) {
            for (index, signal) in self.model.signals.iter().enumerate() {
                let bound = signal.net_alias.iter().any(|binding| groups[binding.group]);
                if bound || signal.net_driver.is_some_and(|(group, _)| groups[group]) {
                    signals.insert(self.signal_dependency_name(index), index);
                }
            }
        }
        self.strength_nets = StrengthNets { groups, signals };
    }

    fn strength_varies(&self, group: usize) -> bool {
        self.strength_nets.groups.get(group) == Some(&true)
    }

    /// The bits `lsb..lsb + width` of `signal` as ascending
    /// `(group, group bit, count)` runs on nets whose strength can vary.
    fn signal_strength_runs(&self, signal: usize, lsb: u32, width: u32) -> Vec<(usize, u32, u32)> {
        let info = self.model.signal(signal);
        if info.net_alias.is_empty() {
            return info
                .net_driver
                .into_iter()
                .filter(|(group, _)| self.strength_varies(*group))
                .map(|(group, _)| (group, lsb, width))
                .collect();
        }
        let mut bits = info
            .net_alias
            .iter()
            .filter(|binding| {
                binding.signal_bit >= lsb
                    && binding.signal_bit - lsb < width
                    && self.strength_varies(binding.group)
            })
            .map(|binding| (binding.group, binding.group_bit))
            .collect::<Vec<_>>();
        bits.sort_unstable();
        let mut runs: Vec<(usize, u32, u32)> = Vec::new();
        for (group, bit) in bits {
            match runs.last_mut() {
                Some((last, low, count)) if *last == group && *low + *count == bit => *count += 1,
                _ => runs.push((group, bit, 1)),
            }
        }
        runs
    }

    /// Add to `reads` the strength views of the nets it reads, so that a wait
    /// on them wakes on a strength-only change of the bits it reads. A model
    /// without such a net keeps `reads` as it is.
    pub(in super::super) fn add_strength_reads(
        &mut self,
        reads: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        if self.strength_nets.signals.is_empty() {
            return Ok(());
        }
        let mut views = Vec::new();
        for read in reads.iter() {
            let (storage, range) = match read {
                IrDependency::PackedRange {
                    storage,
                    lsb,
                    width,
                } => (&**storage, Some((*lsb, *width))),
                other => (other, None),
            };
            let signals = match storage {
                IrDependency::Scalar(name) => self
                    .strength_nets
                    .signals
                    .get(name)
                    .copied()
                    .into_iter()
                    .collect(),
                IrDependency::ArrayElement { array, index } => self.model.arrays[*array]
                    .net_elements
                    .iter()
                    .filter(|(element, _)| *element == *index)
                    .map(|(_, signal)| *signal)
                    .collect(),
                IrDependency::ArrayContents(array) => self.model.arrays[*array]
                    .net_elements
                    .iter()
                    .map(|(_, signal)| *signal)
                    .collect(),
                _ => Vec::new(),
            };
            for signal in signals {
                let (lsb, width) = range.unwrap_or((0, self.model.signal(signal).ty.width()));
                views.extend(self.signal_strength_runs(signal, lsb, width));
            }
        }
        for (group, low, count) in views {
            let view = self.net_strength_view(group)?;
            let name = self.model.signals[view].c_name.clone();
            let read = if low == 0 && count == self.model.net_groups[group].width {
                IrDependency::Scalar(name)
            } else {
                IrDependency::PackedRange {
                    storage: Box::new(IrDependency::Scalar(name)),
                    lsb: low * VIEW_BITS,
                    width: count * VIEW_BITS,
                }
            };
            if !reads.contains(&read) {
                reads.push(read);
            }
        }
        Ok(())
    }

    /// The view of `node` when it is a net or a constant projection of nets
    /// and the strength of one of its bits can change without its value: an
    /// event on it, or a `$monitor` argument naming it, also compares this.
    pub(in super::super) fn varying_strength_view(
        &mut self,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if self.strength_nets.signals.is_empty() {
            return Ok(None);
        }
        let varies = |cg: &Self, electrical: &[(usize, u32)]| {
            electrical
                .iter()
                .any(|(group, _)| cg.strength_varies(*group))
        };
        self.net_strength_expr(node, varies)
    }

    /// The hierarchical source name of a net group, or its C name when no
    /// declared signal names it.
    fn net_group_name(&self, group: usize) -> String {
        self.model
            .signals
            .iter()
            .find(|signal| {
                signal.net_driver.is_some_and(|(owner, _)| owner == group)
                    || signal.net_alias.iter().any(|bit| bit.group == group)
            })
            .and_then(|signal| signal.hdl_name.as_deref())
            .unwrap_or(&self.model.net_groups[group].c_name)
            .replace('\u{1f}', ".")
    }

    /// The view signal of one net group, created on first use. A net too wide
    /// for a view rejects, so no reader of its strength loses a wake.
    fn net_strength_view(&mut self, group: usize) -> Result<usize, String> {
        let net = &self.model.net_groups[group];
        if let Some(view) = net.strength_view {
            return Ok(view);
        }
        let width = net
            .width
            .checked_mul(VIEW_BITS)
            .filter(|width| *width <= LLG_MAX_WIDTH)
            .ok_or_else(|| {
                format!(
                    "strength view of the {}-bit net `{}` takes {VIEW_BITS} bits per net bit, past the packed width limit of {LLG_MAX_WIDTH} bits",
                    net.width,
                    self.net_group_name(group)
                )
            })?;
        let c_name = format!("{}__strength", net.c_name);
        let view = self.model.signals.len();
        self.model.signals.push(IrSignal {
            fixed_default: None,
            c_name,
            hdl_name: None,
            ty: IrType::Packed {
                width,
                signed: false,
                two_state: true,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        self.model.net_groups[group].strength_view = Some(view);
        Ok(view)
    }

    fn view_slice(&mut self, group: usize, group_bit: u32, bits: u32) -> Result<IrExpr, String> {
        let view = self.net_strength_view(group)?;
        let whole = self.model.net_groups[group].width;
        let read = IrExpr::new(IrExprKind::SigRead(view), whole * VIEW_BITS, false, None);
        if group_bit == 0 && bits == whole {
            return Ok(read);
        }
        let right = i64::from(group_bit) * i64::from(VIEW_BITS);
        Ok(IrExpr::new(
            IrExprKind::PartSel {
                base: Box::new(read),
                left: right + i64::from(bits * VIEW_BITS) - 1,
                right,
            },
            bits * VIEW_BITS,
            false,
            None,
        ))
    }

    /// The resolved group bit that owns one structural net bit.
    fn electrical_bit(&self, bit: AliasBit) -> Option<(usize, u32)> {
        let (signal, signal_bit) = match bit {
            AliasBit::Net { net, bit } => (self.sig_globals.get(&net)?.ir, bit),
            AliasBit::Array {
                owner,
                element,
                bit,
            } => {
                let array = self.array_globals.get(&owner)?.ir;
                let signal = self.model.arrays[array]
                    .net_elements
                    .iter()
                    .find_map(|(index, signal)| (*index == element).then_some(*signal))?;
                (signal, bit)
            }
        };
        let info = self.model.signals.get(signal)?;
        if let Some(binding) = info
            .net_alias
            .iter()
            .find(|binding| binding.signal_bit == signal_bit)
        {
            return Some((binding.group, binding.group_bit));
        }
        let (group, _) = info.net_driver?;
        (info.net_alias.is_empty() && self.model.net_groups.get(group)?.width == info.ty.width())
            .then_some((group, signal_bit))
    }

    /// Replace a `%v` value with its net's strength view when the source
    /// expression is a net or a constant projection of nets: whole nets,
    /// selects, net-array cells, alias views and concatenations. Every bit
    /// must be a resolved net bit; variables and other expressions keep the
    /// strong value-only formatting, which is exact for them.
    pub(in crate::sim::codegen::lowering) fn strength_display_arg(
        &mut self,
        node: NodeId,
    ) -> Result<Option<IrDisplayArg>, String> {
        Ok(self
            .net_strength_expr(node, |_, _| true)?
            .map(IrDisplayArg::Strength))
    }

    /// The strength view of a net or constant projection of nets whose
    /// resolved bits `accept` admits.
    fn net_strength_expr(
        &mut self,
        node: NodeId,
        accept: impl Fn(&Self, &[(usize, u32)]) -> bool,
    ) -> Result<Option<IrExpr>, String> {
        let Ok(bits) = self.alias_expression_bits(node, node) else {
            return Ok(None);
        };
        // Undriven net-array cells have no electrical group until a strength
        // consumer needs one.
        let mut cells = bits
            .iter()
            .filter_map(|bit| match *bit {
                AliasBit::Array { owner, element, .. } => {
                    Some((self.array_globals.get(&owner)?.ir, element))
                }
                AliasBit::Net { .. } => None,
            })
            .collect::<Vec<_>>();
        cells.dedup();
        for (array, element) in cells {
            self.materialize_undriven_net_cell(array, element)?;
        }
        let Some(electrical) = bits
            .into_iter()
            .map(|bit| self.electrical_bit(bit))
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(None);
        };
        if electrical.is_empty() || !accept(self, &electrical) {
            return Ok(None);
        }
        // Bits arrive most significant first; coalesce descending runs.
        let mut runs: Vec<(usize, u32, u32)> = Vec::new();
        for (group, group_bit) in electrical {
            match runs.last_mut() {
                Some((last_group, low, count))
                    if *last_group == group && group_bit.checked_add(1) == Some(*low) =>
                {
                    *low = group_bit;
                    *count += 1;
                }
                _ => runs.push((group, group_bit, 1)),
            }
        }
        let mut parts = Vec::with_capacity(runs.len());
        let mut width = 0u32;
        for (group, low, count) in runs {
            parts.push(self.view_slice(group, low, count)?);
            width = width
                .checked_add(count * VIEW_BITS)
                .filter(|width| *width <= LLG_MAX_WIDTH)
                .ok_or_else(|| {
                    format!(
                        "strength view of `{}` takes {VIEW_BITS} bits per net bit, past the packed width limit of {LLG_MAX_WIDTH} bits",
                        self.net_group_name(group)
                    )
                })?;
        }
        let view = if parts.len() == 1 {
            parts.pop().expect("one strength view run")
        } else {
            IrExpr::new(IrExprKind::Concat { parts }, width, false, None)
        };
        Ok(Some(view))
    }

    /// The dependencies of a strength view expression.
    pub(in super::super) fn strength_view_reads(&self, view: &IrExpr) -> Vec<IrDependency> {
        let mut reads = Vec::new();
        let mut pending = vec![view];
        while let Some(expr) = pending.pop() {
            match &expr.kind {
                &IrExprKind::SigRead(signal) => {
                    if let Some(info) = self.model.signals.get(signal) {
                        let read = IrDependency::Scalar(info.c_name.clone());
                        if !reads.contains(&read) {
                            reads.push(read);
                        }
                    }
                }
                IrExprKind::PartSel { base, .. } => pending.push(base),
                IrExprKind::Concat { parts } => pending.extend(parts),
                _ => {}
            }
        }
        reads
    }

    /// Monitor dependencies on the strength views read by `%v` arguments.
    pub(in crate::sim::codegen::lowering) fn strength_view_dependencies(
        &self,
        args: &[IrDisplayArg],
    ) -> Vec<IrDependency> {
        let mut reads = Vec::new();
        for arg in args {
            if let IrDisplayArg::Strength(view) = arg {
                for read in self.strength_view_reads(view) {
                    if !reads.contains(&read) {
                        reads.push(read);
                    }
                }
            }
        }
        reads
    }
}
