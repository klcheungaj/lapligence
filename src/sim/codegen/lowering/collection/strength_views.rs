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
//!
//! A select with a dynamic index (`v[i]`, `v[i +: 2]`) of a net reads the
//! bytes of its net's view that the index selects when it is evaluated, so
//! `%v` shows and an event wakes on the strength of the bits selected now
//! (decision T1-D12). Bits past the net, or an unknown index, read StX, the
//! strength a value select there gives.
//!
//! A port link that converts (a width or type change between two nets)
//! drives its sink with one strength. When a reader names the sink's strength
//! (`%v`, or an event, implicit event list, `wait` or `$monitor` while the
//! source's strength can change alone), the lowering records the port
//! ([`Codegen::note_strength_link_reader`]) and the model is lowered again
//! with the port's overlapping bits joined into one net, so the sink shows
//! and wakes on the source's strength (decision T1-D13,
//! `nets/port_merges.rs`). A design without such a reader keeps every link.

use super::*;
use crate::sim::emit_c::LLG_MAX_WIDTH;
use crate::sim::ir::{IrDependency, IrDisplayArg, IrNetKind};

/// Bits of view storage per net bit.
const VIEW_BITS: u32 = 8;

/// The view byte of a strong unknown (StX): low nibble -6 + 7, high nibble
/// 6 + 7 (`llg_net_t.strength`).
const STRENGTH_X: u64 = 0xd1;

/// The nets whose strength can change while their value holds.
#[derive(Default)]
pub(in super::super) struct StrengthNets {
    /// By net group; groups created later never vary.
    groups: Vec<bool>,
    /// The dependency name of each signal with a bit on such a net.
    signals: HashMap<String, usize>,
}

/// Converting port links and the strength readers of their sinks (module
/// docs).
#[derive(Default)]
pub(in super::super) struct StrengthLinks {
    /// Ports an earlier lowering found read for strength; this lowering
    /// joins their bits.
    pub(in super::super) requested: HashSet<NodeId>,
    /// The joined ports: their (formal bit, actual bit) pairs, least
    /// significant bits aligned.
    pub(in super::super) pairs: HashMap<NodeId, Vec<(AliasBit, AliasBit)>>,
    /// Joined ports whose sink is wider than the bits they join: the link
    /// stays for the sink's other bits and drives the joined low bits z.
    pub(in super::super) masked: HashMap<NodeId, u32>,
    /// Converting ports that keep their link: (sink net, source net).
    pub(in super::super) links: HashMap<NodeId, (NodeId, NodeId)>,
    /// The ports of [`Self::links`] by sink net.
    pub(in super::super) by_sink: HashMap<NodeId, Vec<NodeId>>,
    /// The ports of [`Self::links`] by a net group their sink has a bit in,
    /// built for the first reader, once every group exists.
    by_group: Option<HashMap<usize, Vec<NodeId>>>,
    /// The ports of [`Self::links`] by their sink's dependency name, built
    /// with [`Self::by_group`].
    by_name: HashMap<String, Vec<NodeId>>,
    /// Ports whose sink a strength reader of this lowering names.
    found: HashSet<NodeId>,
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
        self.note_strength_link_reads(reads);
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
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        self.note_strength_link_reader(node, false);
        if self.strength_nets.signals.is_empty() {
            return Ok(None);
        }
        let varies = |cg: &Self, electrical: &[(usize, u32)]| {
            electrical
                .iter()
                .any(|(group, _)| cg.strength_varies(*group))
        };
        self.net_strength_expr(path, node, varies)
    }

    /// The index of `node` when it is a select whose bits are not static
    /// (a dynamic bit or indexed part-select): its strength view then reads
    /// that index as well as the view.
    pub(in super::super) fn dynamic_strength_index(&self, node: NodeId) -> Option<NodeId> {
        let index = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => *index,
            NodeKind::Expr(ExprKind::IndexedPartSelect { base_expr, .. }) => *base_expr,
            _ => return None,
        };
        self.alias_expression_bits(node, node)
            .is_err()
            .then_some(index)
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
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrDisplayArg>, String> {
        self.note_strength_link_reader(node, true);
        Ok(self
            .net_strength_expr(path, node, |_, _| true)?
            .map(IrDisplayArg::Strength))
    }

    /// The strength view of a net, a constant projection of nets or a
    /// dynamic select of one, whose resolved bits `accept` admits.
    fn net_strength_expr(
        &mut self,
        path: &str,
        node: NodeId,
        accept: impl Fn(&Self, &[(usize, u32)]) -> bool,
    ) -> Result<Option<IrExpr>, String> {
        if self.dynamic_strength_index(node).is_some() {
            return self.dynamic_select_strength(path, node, accept);
        }
        self.static_strength_expr(node, accept)
    }

    /// The view bytes a dynamic select of a net reads: those of the bits the
    /// index selects, StX for bits past the net or for an unknown index
    /// (module docs). The lowered select gives the index as an offset from
    /// the net's least significant bit, so the view, eight bits per net bit
    /// in the same order, takes it scaled. A select whose base is not a
    /// static projection of nets, or that does not lower to a packed select
    /// of it (a net-array element), has no view.
    fn dynamic_select_strength(
        &mut self,
        path: &str,
        node: NodeId,
        accept: impl Fn(&Self, &[(usize, u32)]) -> bool,
    ) -> Result<Option<IrExpr>, String> {
        let base = match self.kind(node) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. } | ExprKind::IndexedPartSelect { base, .. },
            ) => *base,
            _ => return Ok(None),
        };
        let Some(view) = self.static_strength_expr(base, accept)? else {
            return Ok(None);
        };
        let bits = view.width / VIEW_BITS;
        let value = self.lower_expr(path, node)?;
        // `high` says whether the offset names the window's most significant
        // bit (`-:` on some ranges) instead of its least significant one.
        let (low, count, high) = match value.kind() {
            IrExprKind::BitSel { base, idx } if base.width == bits => ((**idx).clone(), 1, false),
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                neg,
                ..
            } if base.width == bits => ((**base_idx).clone(), value.width, *neg),
            _ => return Ok(None),
        };
        let too_wide = || {
            format!(
                "strength view of a {count}-bit dynamic select of a {bits}-bit net takes {VIEW_BITS} bits per net bit, past the packed width limit of {LLG_MAX_WIDTH} bits"
            )
        };
        let bytes = |count: u32| -> Result<IrExpr, String> {
            let width = count * VIEW_BITS;
            let mut words = vec![0u64; width.div_ceil(64) as usize];
            for byte in 0..count {
                let bit = byte * VIEW_BITS;
                words[(bit / 64) as usize] |= STRENGTH_X << (bit % 64);
            }
            let constant = IrConst::packed(words, Vec::new(), Vec::new(), width, false, None)
                .map_err(|error| error.to_string())?;
            Ok(IrExpr::new(IrExprKind::Const(constant), width, false, None))
        };
        let integer = |value: u64, width: u32, signed: bool| -> Result<IrExpr, String> {
            let constant =
                IrConst::packed(vec![value], Vec::new(), Vec::new(), width, signed, None)
                    .map_err(|error| error.to_string())?;
            Ok(IrExpr::new(
                IrExprKind::Const(constant),
                width,
                signed,
                None,
            ))
        };
        let binary = |op: IrBinOp, a: IrExpr, b: IrExpr, width: u32, signed: bool| {
            IrExpr::new(
                IrExprKind::Bin {
                    op,
                    a: Box::new(a),
                    b: Box::new(b),
                },
                width,
                signed,
                None,
            )
        };
        // One more bit than the offset, and at least 64, so that adding the
        // padding cannot wrap an offset past the net into range.
        let (width, signed) = (low.width.saturating_add(1).max(64), low.signed);
        let low = IrExpr::convert_to(low, width, signed);
        let shifted = |by: u32| -> Result<IrExpr, String> {
            Ok(if by == 0 {
                low.clone()
            } else {
                binary(
                    IrBinOp::Add,
                    low.clone(),
                    integer(u64::from(by), width, signed)?,
                    width,
                    signed,
                )
            })
        };
        // A wider select pads the view with `count` StX bytes on each side,
        // so a window that overlaps the net reads StX for its bits past it.
        let (padded, pad) = if count == 1 {
            (view, 0)
        } else {
            let total = bits
                .checked_add(2 * count)
                .and_then(|bits| bits.checked_mul(VIEW_BITS))
                .filter(|width| *width <= LLG_MAX_WIDTH)
                .ok_or_else(too_wide)?;
            let parts = vec![bytes(count)?, view, bytes(count)?];
            (
                IrExpr::new(IrExprKind::Concat { parts }, total, false, None),
                count,
            )
        };
        // With `low` the window's least significant bit, it overlaps the net
        // exactly when bit `low + count - 1` of a `bits + count - 1`-bit
        // vector exists; the value select's own index conversion decides
        // that, so an unknown or negative offset is out. An offset naming the
        // most significant bit is `low + count - 1` already.
        let (top, first) = if high {
            (0, pad + 1 - count)
        } else {
            (count - 1, pad)
        };
        let span = bits + count - 1;
        let inside = binary(
            IrBinOp::CaseEq,
            IrExpr::new(
                IrExprKind::BitSel {
                    base: Box::new(IrExpr::new(IrExprKind::Fill(1), span, false, Some(1))),
                    idx: Box::new(shifted(top)?),
                },
                1,
                false,
                None,
            ),
            integer(1, 1, false)?,
            1,
            false,
        );
        let start = binary(
            IrBinOp::Mul,
            shifted(first)?,
            integer(u64::from(VIEW_BITS), width, signed)?,
            width,
            signed,
        );
        let window = count * VIEW_BITS;
        let slice = IrExpr::new(
            IrExprKind::IdxPartSel {
                base: Box::new(padded),
                base_idx: Box::new(start),
                width_expr: Box::new(lhs_integer_expr(i128::from(window))),
                neg: false,
            },
            window,
            false,
            None,
        );
        Ok(Some(IrExpr::new(
            IrExprKind::Mux {
                sel: Box::new(inside),
                a: Box::new(slice),
                b: Box::new(bytes(count)?),
            },
            window,
            false,
            None,
        )))
    }

    /// The strength view of a net or constant projection of nets whose
    /// resolved bits `accept` admits.
    fn static_strength_expr(
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

    /// The view dependencies of a strength view expression; those of a
    /// dynamic select's index are the caller's.
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
                IrExprKind::PartSel { base, .. } | IrExprKind::IdxPartSel { base, .. } => {
                    pending.push(base)
                }
                IrExprKind::Concat { parts } => pending.extend(parts),
                // A dynamic select's index and range check read no view.
                IrExprKind::Mux { a, .. } => pending.push(a),
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

    /// Record the converting port links whose sink `node` (a net, a
    /// projection of nets or a dynamic select of one) has a bit on: every
    /// one for a `%v` reader (`display`), otherwise those whose source's
    /// strength can change alone (module docs).
    fn note_strength_link_reader(&mut self, node: NodeId, display: bool) {
        if self.strength_links.links.is_empty() {
            return;
        }
        let node = match self.kind(node) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. } | ExprKind::IndexedPartSelect { base, .. },
            ) if self.dynamic_strength_index(node).is_some() => *base,
            _ => node,
        };
        let Ok(bits) = self.alias_expression_bits(node, node) else {
            return;
        };
        self.index_strength_links();
        let mut ports = Vec::new();
        for bit in bits {
            let Some((group, _)) = self.electrical_bit(bit) else {
                continue;
            };
            let by_group = self.strength_links.by_group.as_ref();
            for port in by_group
                .and_then(|map| map.get(&group))
                .into_iter()
                .flatten()
            {
                if !ports.contains(port) {
                    ports.push(*port);
                }
            }
        }
        self.note_strength_links(ports, display);
    }

    /// Index the sinks of the converting links by group and dependency
    /// name, once every group exists (the first reader).
    fn index_strength_links(&mut self) {
        if self.strength_links.by_group.is_some() {
            return;
        }
        let mut by_group: HashMap<usize, Vec<NodeId>> = HashMap::new();
        let mut by_name: HashMap<String, Vec<NodeId>> = HashMap::new();
        for (sink, ports) in &self.strength_links.by_sink {
            let Some(info) = self.signal_of(*sink) else {
                continue;
            };
            by_name
                .entry(self.signal_dependency_name(info.ir))
                .or_default()
                .extend(ports);
            {
                let signal = self.model.signal(info.ir);
                let mut groups = signal
                    .net_driver
                    .iter()
                    .map(|(group, _)| *group)
                    .chain(signal.net_alias.iter().map(|binding| binding.group))
                    .collect::<Vec<_>>();
                groups.sort_unstable();
                groups.dedup();
                for group in groups {
                    by_group.entry(group).or_default().extend(ports);
                }
            }
        }
        self.strength_links.by_group = Some(by_group);
        self.strength_links.by_name = by_name;
    }

    /// [`Self::note_strength_link_reader`] for a process's read set: an
    /// implicit event list or a `wait` that reads a sink.
    fn note_strength_link_reads(&mut self, reads: &[IrDependency]) {
        if self.strength_links.links.is_empty() {
            return;
        }
        self.index_strength_links();
        let mut ports = Vec::new();
        for read in reads {
            let name = match read {
                IrDependency::PackedRange { storage, .. } => match &**storage {
                    IrDependency::Scalar(name) => name,
                    _ => continue,
                },
                IrDependency::Scalar(name) => name,
                _ => continue,
            };
            for port in self.strength_links.by_name.get(name).into_iter().flatten() {
                if !ports.contains(port) {
                    ports.push(*port);
                }
            }
        }
        self.note_strength_links(ports, false);
    }

    /// Record `ports` and, along each source that is itself the sink of
    /// such a link, the links it reads through. An event reader records
    /// only a chain whose first source's strength can change alone.
    fn note_strength_links(&mut self, ports: Vec<NodeId>, display: bool) {
        for port in ports {
            if self.strength_links.found.contains(&port) {
                continue;
            }
            let mut chain = Vec::new();
            let mut pending = vec![port];
            let mut varies = false;
            while let Some(port) = pending.pop() {
                if chain.contains(&port) {
                    continue;
                }
                chain.push(port);
                let Some(&(_, source)) = self.strength_links.links.get(&port) else {
                    continue;
                };
                match self.strength_links.by_sink.get(&source) {
                    Some(next) => pending.extend(next.iter().copied()),
                    None => varies |= self.net_strength_varies(source),
                }
            }
            if display || varies {
                self.strength_links.found.extend(chain);
            }
        }
    }

    /// Whether a bit of `net` is on a group whose strength can change alone.
    fn net_strength_varies(&self, net: NodeId) -> bool {
        let Some(info) = self.signal_of(net) else {
            return false;
        };
        let signal = self.model.signal(info.ir);
        signal
            .net_driver
            .iter()
            .map(|(group, _)| *group)
            .chain(signal.net_alias.iter().map(|binding| binding.group))
            .any(|group| self.strength_varies(group))
    }

    /// The converting ports a strength reader found in this lowering, for a
    /// further lowering that joins them.
    pub(in crate::sim::codegen::lowering) fn strength_link_retries(&self) -> Vec<NodeId> {
        let mut ports = self
            .strength_links
            .found
            .iter()
            .copied()
            .collect::<Vec<_>>();
        ports.sort_by_key(|port| port.index());
        ports
    }
}
