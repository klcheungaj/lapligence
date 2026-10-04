//! Net strength views for `%v` (IEEE 1364-2001 17.1.1.5, IEEE 1800-2009
//! 21.2.1.5).
//!
//! A strength view is a two-state signal with eight bits per net bit that the
//! runtime republishes after every resolution of its net (`llg_net_t.strength`).
//! It exists only for nets that a `%v` argument reads, so ordinary nets keep
//! their value-only resolution. Monitors depend on the view itself, which is
//! how a strength-only change, such as St1 to Pu1, still reaches them.

use super::*;
use crate::sim::emit_c::LLG_MAX_WIDTH;
use crate::sim::ir::IrDisplayArg;

/// Bits of view storage per net bit.
const VIEW_BITS: u32 = 8;

impl Codegen<'_> {
    /// The view signal of one net group, created on first use.
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
                    "`%v` strength view of a {}-bit net exceeds the packed width limit",
                    net.width
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
        if electrical.is_empty() {
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
                .ok_or("`%v` strength view exceeds the packed width limit")?;
        }
        let view = if parts.len() == 1 {
            parts.pop().expect("one strength view run")
        } else {
            IrExpr::new(IrExprKind::Concat { parts }, width, false, None)
        };
        Ok(Some(IrDisplayArg::Strength(view)))
    }

    /// Monitor dependencies on the strength views read by `%v` arguments.
    pub(in crate::sim::codegen::lowering) fn strength_view_dependencies(
        &self,
        args: &[IrDisplayArg],
    ) -> Vec<crate::sim::ir::IrDependency> {
        let mut reads = Vec::new();
        for arg in args {
            let IrDisplayArg::Strength(view) = arg else {
                continue;
            };
            let mut pending = vec![view];
            while let Some(expr) = pending.pop() {
                match &expr.kind {
                    &IrExprKind::SigRead(signal) => {
                        if let Some(info) = self.model.signals.get(signal) {
                            let read = crate::sim::ir::IrDependency::Scalar(info.c_name.clone());
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
        }
        reads
    }
}
