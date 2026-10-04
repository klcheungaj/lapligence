//! Fixed net-array views bind to the shared electrical range partition.
use super::net_collapse::{NetCollapsePlan, NetPoint};
use super::*;

#[cfg(test)]
mod tests;

type ArrayNetSelection = ((usize, u64), Vec<u32>);

/// One structural driver of a net-array cell and its drive strengths.
type CellSource = (NodeId, (u8, u8));

impl Codegen<'_> {
    /// Return the fixed-array cells covered by a constant array lvalue.
    ///
    /// A whole array and a selected row have no single `array_net_endpoint`,
    /// but they still name a deterministic set of electrical cells.  Keep
    /// this mapping in the owned lowering layer so continuous driver
    /// registration and alias-lvalue lowering use the same declared-order
    /// coordinates.
    pub(super) fn array_net_target_elements(
        &self,
        node: NodeId,
    ) -> Result<Option<Vec<u64>>, String> {
        let Some((array, selected)) = self.array_net_target_parts(node) else {
            return Ok(None);
        };
        if !array.is_net {
            return Ok(None);
        }
        let prefix = selected
            .iter()
            .take(array.dims.len())
            .map(|index| {
                self.eval_bound_i128(*index).map_err(|_| {
                    format!(
                        "continuous assignment to net array `{}` requires constant array indices",
                        array.global
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let slice = self.array_net_slice_bounds(node, &array, selected.len())?;
        let mut elements = Vec::new();
        for coordinates in super::port_array_index_vectors(&array.dims) {
            if coordinates
                .iter()
                .take(prefix.len())
                .zip(&prefix)
                .any(|(coordinate, selected)| i128::from(*coordinate) != *selected)
            {
                continue;
            }
            if let Some((low, high)) = slice {
                let coordinate = i128::from(coordinates[prefix.len()]);
                if coordinate < low || coordinate > high {
                    continue;
                }
            }
            let indices = coordinates
                .iter()
                .copied()
                .map(i128::from)
                .map(lhs_integer_expr)
                .collect::<Vec<_>>();
            let element = Self::array_constant_linear_index(&array, &indices)
                .ok_or("net-array target coordinate is out of bounds")?;
            elements.push(element);
        }
        Ok(Some(elements))
    }

    /// Return the inclusive index interval of an unpacked slice (SV 7.4.6)
    /// applied to the next dimension of a partially indexed net array. A
    /// part-select of a fully indexed element is a packed select instead.
    fn array_net_slice_bounds(
        &self,
        node: NodeId,
        array: &ArrayInfo,
        depth: usize,
    ) -> Result<Option<(i128, i128)>, String> {
        if depth >= array.dims.len() {
            return Ok(None);
        }
        let mut node = node;
        loop {
            match self.kind(node) {
                NodeKind::Expr(ExprKind::Cast { operand, .. }) => node = *operand,
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Assignment,
                    operands,
                    ..
                }) => match operands.first() {
                    Some(operand) => node = *operand,
                    None => return Ok(None),
                },
                _ => break,
            }
        }
        let constant = |index: NodeId| {
            self.eval_bound_i128(index).map_err(|_| {
                format!(
                    "continuous assignment to net array `{}` requires constant slice bounds",
                    array.global
                )
            })
        };
        let bounds = match self.kind(node) {
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                let (left, right) = (constant(*left)?, constant(*right)?);
                (left.min(right), left.max(right))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let start = constant(*base_expr)?;
                let span = constant(*width_expr)?
                    .checked_sub(1)
                    .filter(|span| *span >= 0)
                    .ok_or_else(|| {
                        format!("net array `{}` slice has a nonpositive width", array.global)
                    })?;
                let overflow = || format!("net array `{}` slice bounds overflow", array.global);
                if *neg {
                    (start.checked_sub(span).ok_or_else(overflow)?, start)
                } else {
                    (start, start.checked_add(span).ok_or_else(overflow)?)
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(bounds))
    }

    /// Collect the `(array, element)` net cells driven by one structural
    /// target. A positional assignment-pattern LHS is one driver site for
    /// every cell named by its leaves (SV 10.9.1), so each such cell needs a
    /// contribution slot for that source; other targets name one array.
    pub(super) fn continuous_net_array_cells(
        &self,
        target: NodeId,
        cells: &mut Vec<(usize, u64)>,
    ) -> Result<(), String> {
        let pattern = self.p30_unwrap_cast(target);
        if let NodeKind::Expr(ExprKind::Operation {
            op: Operation::AssignmentPattern,
            ..
        }) = self.kind(pattern)
        {
            let path = self.display_name(target);
            for operand in self
                .assignment_pattern_operands(&path, pattern)?
                .unwrap_or_default()
            {
                let leaf = self.p30_pattern_lvalue_operand(&path, operand)?;
                self.continuous_net_array_cells(leaf, cells)?;
            }
            return Ok(());
        }
        let Some((array, _)) = self.array_net_target_parts(target) else {
            return Ok(());
        };
        if let Some(elements) = self.array_net_target_elements(target)? {
            cells.extend(elements.into_iter().map(|element| (array.ir, element)));
        }
        Ok(())
    }

    pub(super) fn array_net_target_parts(&self, node: NodeId) -> Option<(ArrayInfo, Vec<NodeId>)> {
        match self.kind(node) {
            NodeKind::Array { .. } | NodeKind::Expr(ExprKind::Ref { .. }) => self
                .array_of(node)
                .cloned()
                .map(|array| (array, Vec::new())),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => self
                .array_of(node)
                .cloned()
                .map(|array| (array, Vec::new()))
                .or_else(|| {
                    refs.iter()
                        .flatten()
                        .find_map(|root| self.array_net_target_parts(*root))
                }),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some(array) = self.array_of(*base).cloned() {
                    Some((array, indices.clone()))
                } else {
                    let root = self
                        .db
                        .array_select_path(node)
                        .map_or(*base, |(root, _)| root);
                    let (array, mut prior) = self.array_net_target_parts(root)?;
                    prior.extend(indices.iter().copied());
                    Some((array, prior))
                }
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(array) = self.array_of(*base).cloned() {
                    Some((array, vec![*index]))
                } else {
                    self.array_net_target_parts(*base)
                }
            }
            NodeKind::Expr(
                ExprKind::PartSelect { base, .. } | ExprKind::IndexedPartSelect { base, .. },
            ) => self.array_net_target_parts(*base),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.array_net_target_parts(*operand),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) => operands
                .first()
                .and_then(|operand| self.array_net_target_parts(*operand)),
            _ => None,
        }
    }

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
        let fixed_selection = |root: NodeId, path: &[AggregatePathPart]| {
            let descriptor = self
                .query_descriptor(root)
                .ok_or("net-array member has no owned type")?;
            let (member, offset) = super::fixed_values::fixed_path_descriptor(descriptor, path)
                .ok_or("net-array member has no fixed layout")?;
            let width = Self::fixed_descriptor_width(&member)
                .ok_or("net-array member has no fixed width")?;
            if let NodeKind::Expr(ExprKind::ArraySelect { base, indices }) = self.kind(root) {
                if self
                    .array_of(*base)
                    .is_some_and(|array| indices.len() == array.dims.len())
                {
                    // A complete cell starts at bit zero. Materialize only the
                    // member's bits, rather than the whole record per member.
                    let end = offset
                        .checked_add(width)
                        .ok_or("net-array member width overflow")?;
                    if end
                        > Self::fixed_descriptor_width(descriptor)
                            .ok_or("net-array cell has no fixed width")?
                    {
                        return Err("net-array member is outside its cell".into());
                    }
                    return Ok((offset..end).rev().collect());
                }
            }
            let (_, bits) = self
                .array_net_selection(root)?
                .ok_or("net-array member has no base selection")?;
            select(bits, i128::from(offset), width)
        };
        let bits = match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let (position, root) = refs
                    .iter()
                    .enumerate()
                    .find_map(|(position, root)| {
                        root.filter(|root| self.array_net_endpoint(*root).is_some())
                            .map(|root| (position, root))
                    })
                    .ok_or("net-array member has no selected element")?;
                let path = parts[position + 1..]
                    .iter()
                    .cloned()
                    .map(AggregatePathPart::Member)
                    .collect::<Vec<_>>();
                fixed_selection(root, &path)?
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                if self.array_of(*base).is_none() =>
            {
                let (root, members) = self
                    .db
                    .array_select_path(node)
                    .map_or((*base, &[][..]), |(root, members)| (root, members));
                let mut path = members
                    .iter()
                    .cloned()
                    .map(AggregatePathPart::Member)
                    .collect::<Vec<_>>();
                for index in indices {
                    path.push(AggregatePathPart::Index(
                        i32::try_from(self.eval_bound_i128(*index)?)
                            .map_err(|_| "net-array member index is out of bounds")?,
                    ));
                }
                fixed_selection(root, &path)?
            }
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
        type_plan: &NetCollapsePlan,
        array_alias_bindings: HashMap<(usize, u64), Vec<IrNetAliasBinding>>,
        uwire_drivers: &mut HashMap<usize, std::collections::BTreeMap<usize, NodeId>>,
    ) -> Result<(), String> {
        let mut endpoints = endpoints.into_iter().collect::<Vec<_>>();
        endpoints.sort_by_key(|(key, _)| *key);
        // Index structural sources by the cells they drive once. Scanning every
        // source per cell made whole-array drivers quadratic in the cell count.
        let mut cell_sources: HashMap<(usize, u64), Vec<CellSource>> = HashMap::new();
        for source in nodes {
            let targets = match self.kind(*source) {
                NodeKind::ContAssign { .. } => {
                    let Some(target) = self.node(*source).children.first().copied() else {
                        continue;
                    };
                    vec![target]
                }
                NodeKind::Port {
                    direction: DbDirection::Output,
                    high_expr: Some(target),
                    ..
                } => vec![*target],
                // An input formal net array is driven by its port: either the
                // link from the actual or an `unconnected_drive` pull.
                NodeKind::Port {
                    direction: DbDirection::Input,
                    low: Some(target),
                    ..
                } => vec![*target],
                // Gate outputs, including pullup/pulldown, drive cells or
                // selected cell bits like any other structural source.
                NodeKind::Gate { terms, .. } => terms
                    .iter()
                    .filter(|term| {
                        matches!(term.direction, DbDirection::Output | DbDirection::Inout)
                    })
                    .map(|term| term.expr)
                    .collect(),
                _ => continue,
            };
            let mut cells = Vec::new();
            for target in targets {
                self.continuous_net_array_cells(target, &mut cells)?;
            }
            cells.sort_unstable();
            cells.dedup();
            for cell in cells {
                let (array, _) = cell;
                let strengths = match self.kind(*source) {
                    NodeKind::ContAssign {
                        strength0,
                        strength1,
                        ..
                    } => continuous_assignment_strengths_for_width(
                        *strength0,
                        *strength1,
                        &self.model.arrays[array].hdl_name,
                        self.model.arrays[array].elem_width,
                    )?,
                    NodeKind::Port {
                        strength0,
                        strength1,
                        low,
                        ..
                    } => {
                        self.effective_port_driver_strengths(*source, *strength0, *strength1, *low)?
                    }
                    NodeKind::Gate {
                        prim_type,
                        strength0,
                        strength1,
                        ..
                    } => gate_driver_strengths(
                        *prim_type,
                        *strength0,
                        *strength1,
                        &self.display_name(*source),
                    )?,
                    _ => unreachable!("structural source classified above"),
                };
                cell_sources
                    .entry(cell)
                    .or_default()
                    .push((*source, strengths));
            }
        }
        let mut owners = HashMap::new();
        for owner in sorted_node_ids(&self.array_globals) {
            owners.entry(self.array_globals[&owner].ir).or_insert(owner);
        }
        for (array, element) in cell_sources.keys() {
            if !endpoints
                .binary_search_by_key(&(*array, *element), |(key, _)| *key)
                .is_ok()
            {
                return Err(format!(
                    "structural driver reaches net-array cell {}[{element}] outside the electrical partition",
                    self.model.arrays[*array].hdl_name
                ));
            }
        }
        for ((array, element), peers) in endpoints {
            let owner = *owners.get(&array).ok_or("net-array owner is missing")?;
            let sources = cell_sources.remove(&(array, element)).unwrap_or_default();
            if type_plan.any_uwire() && !sources.is_empty() {
                let cell = (0..self.model.arrays[array].elem_width)
                    .map(|bit| AliasBit::Array {
                        owner,
                        element,
                        bit,
                    })
                    .collect::<Vec<_>>();
                self.note_uwire_drivers(type_plan, &cell, &sources, uwire_drivers);
            }
            let mut bindings = Vec::with_capacity(peers.len());
            for (physical, peer) in peers.into_iter().enumerate() {
                let physical =
                    u32::try_from(physical).map_err(|_| "net-array bit index overflow")?;
                let aliased = array_alias_bindings
                    .get(&(array, element))
                    .and_then(|bindings| {
                        bindings
                            .iter()
                            .find(|binding| binding.signal_bit == physical)
                    });
                if let Some(binding) = aliased {
                    let resolved_type = type_plan
                        .resolved(NetPoint::ArrayBit {
                            owner,
                            element,
                            bit: physical,
                        })
                        .ok_or("net-array alias has no type-collapse plan")?;
                    if Some(self.model.net_groups[binding.group].kind)
                        != Self::ir_net_kind(resolved_type.kind)
                    {
                        return Err("net-array publication and type-collapse plan disagree".into());
                    }
                    bindings.push(binding.clone());
                } else if let Some(peer) = peer {
                    let signal = self
                        .signal_of(peer.owner())
                        .ok_or("net-array peer has no storage")?;
                    let binding = self.model.signals[signal.ir]
                        .net_alias
                        .iter()
                        .find(|binding| binding.signal_bit == peer.bit())
                        .ok_or("net-array peer has no electrical bit")?;
                    let resolved_type = type_plan
                        .resolved(NetPoint::ArrayBit {
                            owner,
                            element,
                            bit: physical,
                        })
                        .ok_or("net-array port has no type-collapse plan")?;
                    if Some(self.model.net_groups[binding.group].kind)
                        != Self::ir_net_kind(resolved_type.kind)
                    {
                        return Err("net-array publication and type-collapse plan disagree".into());
                    }
                    bindings.push(IrNetAliasBinding {
                        signal_bit: physical,
                        ..binding.clone()
                    });
                } else {
                    return Err("net-array bit has no electrical partition".into());
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
            for (source, strengths) in sources {
                for group in &groups {
                    self.add_structural_driver(*group, source, strengths)?;
                }
            }
        }
        Ok(())
    }
}
