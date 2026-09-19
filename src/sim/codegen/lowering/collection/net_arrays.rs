//! Fixed net-array cells share canonical electrical bits with connected ports.
use super::*;

type ArrayNetSelection = ((usize, u64), Vec<u32>);

impl Codegen<'_> {
    pub(super) fn array_net_selection(
        &self,
        node: NodeId,
    ) -> Result<Option<ArrayNetSelection>, String> {
        let Some(endpoint) = self.array_net_endpoint(node) else {
            return Ok(None);
        };
        let width = self.model.arrays[endpoint.0].elem_width;
        let whole = || (0..width).rev().collect::<Vec<_>>();
        let select = |bits: Vec<u32>, lower: i128, count: u32| -> Result<Vec<u32>, String> {
            let lower =
                usize::try_from(lower).map_err(|_| "net-array selection is out of bounds")?;
            let upper = lower
                .checked_add(count as usize)
                .filter(|upper| *upper <= bits.len())
                .ok_or("net-array selection is out of bounds")?;
            Ok(bits[bits.len() - upper..bits.len() - lower].to_vec())
        };
        let bits = match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                if self.array_of(*base).is_some() =>
            {
                let array = self.array_of(*base).ok_or("net-array base disappeared")?;
                let mut bits = whole();
                let ranges = self
                    .query_descriptor(*base)
                    .and_then(|descriptor| match &descriptor.shape {
                        TypeShape::FixedArray { element, .. } => match &element.shape {
                            TypeShape::PackedAtom { ranges } => Some(ranges.clone()),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap_or_default();
                for (dimension, index) in indices.iter().skip(array.dims.len()).enumerate() {
                    let range =
                        ranges
                            .get(dimension)
                            .copied()
                            .unwrap_or(crate::core::db::PackedRange {
                                left: i128::try_from(bits.len())
                                    .map_err(|_| "net-array width overflow")?
                                    - 1,
                                right: 0,
                            });
                    let extent = range.left.abs_diff(range.right) + 1;
                    let stride = u32::try_from(bits.len() as u128 / extent)
                        .map_err(|_| "net-array stride overflow")?;
                    let label = self.eval_bound_i128(*index)?;
                    if label < range.left.min(range.right) || label > range.left.max(range.right) {
                        return Err("net-array selection is out of bounds".into());
                    }
                    let lower = i128::try_from(label.abs_diff(range.right))
                        .map_err(|_| "net-array index overflow")?
                        * i128::from(stride);
                    bits = select(bits, lower, stride)?;
                }
                bits
            }
            NodeKind::Expr(ExprKind::BitSelect { base, .. }) if self.array_of(*base).is_some() => {
                whole()
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let (_, bits) = self
                    .array_net_selection(*base)?
                    .ok_or("net-array selection has no base")?;
                let lower = self.packed_relative_bound(*base, self.eval_bound_i128(*index)?)?;
                let width = self
                    .query_descriptor(node)
                    .and_then(|descriptor| descriptor.info.width)
                    .unwrap_or(1);
                select(bits, lower * i128::from(width), width)?
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let (_, bits) = self
                    .array_net_selection(*base)?
                    .ok_or("net-array selection has no base")?;
                let left = self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?;
                let right = self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?;
                let width = u32::try_from(left.abs_diff(right) + 1)
                    .map_err(|_| "net-array selection width overflow")?;
                select(bits, left.min(right), width)?
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                let (_, bits) = self
                    .array_net_selection(*base)?
                    .ok_or("net-array selection has no base")?;
                let lower = self.packed_relative_bound(*base, self.eval_bound_i128(*base_expr)?)?;
                let width = u32::try_from(self.eval_bound_i128(*width_expr)?)
                    .map_err(|_| "net-array selection width overflow")?;
                let negative = *neg ^ self.packed_range_ascending(*base);
                select(
                    bits,
                    if negative {
                        lower - i128::from(width) + 1
                    } else {
                        lower
                    },
                    width,
                )?
            }
            _ => return Err("net-array connection has an unsupported selection shape".into()),
        };
        Ok(Some((endpoint, bits)))
    }

    pub(super) fn publish_array_net_cells(
        &mut self,
        endpoints: HashMap<(usize, u64), Vec<Option<AliasBit>>>,
        nodes: &[NodeId],
    ) -> Result<(), String> {
        let mut endpoints = endpoints.into_iter().collect::<Vec<_>>();
        endpoints.sort_by_key(|(key, _)| *key);
        for ((array, element), peers) in endpoints {
            let owner = self
                .array_globals
                .iter()
                .find_map(|(owner, info)| (info.ir == array).then_some(*owner))
                .ok_or("net-array owner is missing")?;
            let kind = self
                .db
                .array_meta(owner)
                .and_then(|meta| meta.net_type())
                .and_then(Self::ir_net_kind)
                .ok_or("net-array kind cannot be resolved")?;
            let mut bindings = Vec::with_capacity(peers.len());
            for (physical, peer) in peers.into_iter().enumerate() {
                let physical =
                    u32::try_from(physical).map_err(|_| "net-array bit index overflow")?;
                if let Some(peer) = peer {
                    let signal = self
                        .signal_of(peer.net)
                        .ok_or("net-array peer has no storage")?;
                    let binding = self.model.signals[signal.ir]
                        .net_alias
                        .iter()
                        .find(|binding| binding.signal_bit == peer.bit)
                        .ok_or("net-array peer has no electrical bit")?;
                    if self.model.net_groups[binding.group].kind != kind {
                        return Err(
                            "net-array port connects incompatible net resolution kinds".into()
                        );
                    }
                    bindings.push(IrNetAliasBinding {
                        signal_bit: physical,
                        ..binding.clone()
                    });
                } else {
                    let group = self.model.net_groups.len();
                    self.model.net_groups.push(crate::sim::ir::IrNetGroup {
                        c_name: format!("g_array_net_{array}_{element}_{physical}"),
                        width: 1,
                        signed: false,
                        kind,
                        n_drivers: 1,
                        driver_strengths: vec![(6, 6)],
                        propagation_delay: None,
                    });
                    bindings.push(IrNetAliasBinding {
                        group,
                        slot: 0,
                        signal_bit: physical,
                        group_bit: 0,
                    });
                }
            }
            let index = self.model.signals.len();
            let storage = &self.model.arrays[array];
            let info = SignalInfo {
                global: format!("_llg_array_cell_{array}_{element}"),
                width: storage.elem_width,
                signed: storage.signed,
                two_state: false,
                real: false,
                shortreal: false,
                net_driver: None,
                ir: index,
            };
            let mut signal = IrSignal::new(
                info.global.clone(),
                None,
                IrType::Packed {
                    width: info.width,
                    signed: info.signed,
                    two_state: false,
                },
                None,
            )
            .map_err(|error| error.to_string())?;
            signal.net_alias = bindings;
            self.model.signals.push(signal);
            self.signals.push(info);
            self.model.arrays[array].net_elements.push((element, index));
            let mut groups = self.model.signals[index]
                .net_alias
                .iter()
                .map(|binding| binding.group)
                .collect::<Vec<_>>();
            groups.sort_unstable();
            groups.dedup();
            for source in nodes {
                let (target, strengths) = match self.kind(*source) {
                    NodeKind::ContAssign {
                        strength0,
                        strength1,
                        ..
                    } => {
                        let Some(target) = self.node(*source).children.first().copied() else {
                            continue;
                        };
                        (
                            target,
                            continuous_assignment_strengths_for_width(
                                *strength0,
                                *strength1,
                                &self.model.arrays[array].hdl_name,
                                self.model.arrays[array].elem_width,
                            )?,
                        )
                    }
                    NodeKind::Port {
                        direction: DbDirection::Output,
                        high_expr: Some(target),
                        strength0,
                        strength1,
                        low,
                        ..
                    } => (
                        *target,
                        self.effective_port_driver_strengths(
                            *source, *strength0, *strength1, *low,
                        )?,
                    ),
                    _ => continue,
                };
                if self.array_net_endpoint(target) == Some((array, element)) {
                    for group in &groups {
                        self.add_structural_driver(*group, *source, strengths)?;
                    }
                }
            }
        }
        Ok(())
    }
}
