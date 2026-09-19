//! Selections.

use super::*;

impl<'a> Codegen<'a> {
    /// Resolve a hierarchical reference read (`a.b.sig`, or the 2-part
    /// interface member `m.data`) to its signal, when the LAST path element
    /// resolves to a captured Net/Var (per-instance, via the db's refs).
    /// Longer or unresolvable paths return `None`.
    pub(super) fn hier_path_signal(&self, node: NodeId) -> Option<&SignalInfo> {
        if let Some(info) = self.sampled_signal_of(node) {
            return Some(info);
        }
        if let Some(target) = self.clocking_var_target(node) {
            if let Some(info) = self.clocking_var_source_info(target) {
                return Some(info);
            }
        }
        if let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) {
            if let Some(t) = refs.last().copied().flatten() {
                if let Some(info) = self.sampled_signal_of(t) {
                    return Some(info);
                }
                if let Some(info) = self.clocking_var_source_info(t) {
                    return Some(info);
                }
                if let Some(info) = self.signal_of(t) {
                    return Some(info);
                }
            }
            let (target, base_index) = self.hier_path_signal_target(parts, refs)?;
            if base_index + 1 == parts.len() {
                return self
                    .clocking_var_source_info(target)
                    .or_else(|| self.signal_of(target));
            }
        }
        None
    }

    /// Resolve a signal target when a semantic hierarchical path has no target
    /// identity. Scope/name lookup remains on the already-collected owned model
    /// and requires an exact scope prefix.
    fn hier_path_signal_target(
        &self,
        parts: &[String],
        refs: &[Option<NodeId>],
    ) -> Option<(NodeId, usize)> {
        if let Some((index, target)) = refs
            .iter()
            .enumerate()
            .find_map(|(index, target)| target.map(|target| (index, target)))
        {
            if self.sampled_signal_of(target).is_some()
                || self.clocking_var_source_info(target).is_some()
                || self.signal_of(target).is_some()
            {
                return Some((target, index));
            }
        }
        for base_index in (0..parts.len()).rev() {
            let mut scope = self.design_name.clone();
            if base_index != 0 {
                scope.push('.');
                scope.push_str(&parts[..base_index].join("."));
            }
            let Some(info) = self
                .scope_sig_names
                .get(&scope)
                .and_then(|names| names.get(&parts[base_index]))
            else {
                continue;
            };
            if let Some(target) = self
                .sig_globals
                .iter()
                .find_map(|(target, candidate)| (candidate.ir == info.ir).then_some(*target))
            {
                return Some((target, base_index));
            }
        }
        None
    }

    pub(super) fn packed_member_info(&self, node: NodeId) -> Option<(SignalInfo, PackedMember)> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return None;
        };
        let (target, base_index) = self.hier_path_signal_target(parts, refs)?;
        let info = self.signal_of(target)?.clone();
        Some((
            info,
            self.packed_member_layout(target, &parts[base_index + 1..])?,
        ))
    }

    pub(super) fn packed_member_layout(
        &self,
        target: NodeId,
        parts: &[String],
    ) -> Option<PackedMember> {
        let mut layout = self.db.aggregate_layout(target)?;
        if !matches!(
            layout.kind,
            AggregateKind::PackedStruct | AggregateKind::PackedUnion
        ) {
            return None;
        }
        let mut absolute_lsb = 0u32;
        let mut selected: Option<&AggregateMember> = None;
        for (part_index, member_name) in parts.iter().enumerate() {
            let index = layout
                .members
                .iter()
                .position(|member| member.name == *member_name)?;
            let member = &layout.members[index];
            let relative_lsb = if layout.kind == AggregateKind::PackedUnion {
                0
            } else {
                layout.members[index + 1..]
                    .iter()
                    .try_fold(0u32, |offset, following| {
                        offset.checked_add(following.ty.width?)
                    })?
            };
            absolute_lsb = absolute_lsb.checked_add(relative_lsb)?;
            selected = Some(member);
            match member.aggregate_layout() {
                Some(nested)
                    if matches!(
                        nested.kind,
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion
                    ) =>
                {
                    layout = nested;
                }
                _ if part_index + 1 == parts.len() => break,
                _ => return None,
            }
        }
        let member = selected?;
        Some(PackedMember {
            name: member.name.clone(),
            lsb: absolute_lsb,
            width: member.ty.width?,
            signed: member.ty.signed,
            two_state: member.two_state,
            packed_ranges: member.packed_ranges.clone(),
        })
    }

    pub(super) fn unpacked_aggregate_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Var { .. } => self.unpacked_aggregates.contains_key(&node).then_some(node),
            NodeKind::Expr(ExprKind::Ref { target }) => {
                target.filter(|target| self.unpacked_aggregates.contains_key(target))
            }
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) if parts.len() == 1 => refs
                .first()
                .copied()
                .flatten()
                .filter(|target| self.unpacked_aggregates.contains_key(target)),
            _ => None,
        }
    }

    pub(super) fn unpacked_aggregate_info(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, UnpackedAggregateInfo)> {
        let target = self.unpacked_aggregate_target(node)?;
        self.unpacked_aggregates
            .get(&target)
            .cloned()
            .map(|aggregate| (target, aggregate))
    }

    pub(super) fn unpacked_member_info(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, AggregateKind, AggregateMemberInfo)> {
        let (target, path) = self.unpacked_path_for_expr(node)?;
        let aggregate = self.unpacked_aggregates.get(&target)?;
        let member = aggregate
            .leaves
            .iter()
            .find(|member| member.path == path)
            .cloned()?;
        Some((target, aggregate.kind, member))
    }

    /// Resolve a hierarchical/member/constant-index selection to the
    /// declaration identity and canonical recursive path used by aggregate
    /// storage. Dynamic indices deliberately remain outside P28's fixed-value
    /// lowering boundary rather than being mistaken for a C address.
    pub(super) fn unpacked_path_for_expr(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, Vec<AggregatePathPart>)> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let (target, mut path) =
                    if let Some((target, members)) = self.db.array_select_path(node) {
                        (
                            target,
                            members
                                .iter()
                                .cloned()
                                .map(AggregatePathPart::Member)
                                .collect(),
                        )
                    } else {
                        match self.kind(*base) {
                            NodeKind::Array { .. } => self.unpacked_array_base_path(*base)?,
                            _ => self.unpacked_path_for_expr(*base)?,
                        }
                    };
                for index in indices {
                    let value = self.eval_bound_i128(*index).ok()?;
                    let value = i32::try_from(value).ok()?;
                    path.push(AggregatePathPart::Index(value));
                }
                Some((target, path))
            }
            NodeKind::Array { .. } => self.unpacked_array_base_path(node),
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let (target, base_index) = if let Some((index, target)) =
                    refs.iter().enumerate().find_map(|(index, target)| {
                        target
                            .filter(|target| self.unpacked_aggregates.contains_key(target))
                            .map(|target| (index, target))
                    }) {
                    (target, index)
                } else {
                    let mut found = None;
                    for base_index in (0..parts.len()).rev() {
                        let mut scope = self.design_name.clone();
                        if base_index != 0 {
                            scope.push('.');
                            scope.push_str(&parts[..base_index].join("."));
                        }
                        found = self.unpacked_aggregates.keys().find_map(|target| {
                            (self.node(*target).name == parts[base_index]
                                && self.instance_path_of(*target) == scope)
                                .then_some((*target, base_index))
                        });
                        if found.is_some() {
                            break;
                        }
                    }
                    found?
                };
                let path = parts
                    .iter()
                    .skip(base_index + 1)
                    .cloned()
                    .map(AggregatePathPart::Member)
                    .collect();
                Some((target, path))
            }
            _ => None,
        }
    }

    fn unpacked_array_base_path(&self, array: NodeId) -> Option<(NodeId, Vec<AggregatePathPart>)> {
        let name = self.node(array).name.as_str();
        let mut found = None;
        for target in self.unpacked_aggregates.keys().copied() {
            let layout = self.db.aggregate_layout(target)?;
            let Some(path) = aggregate_array_member_path(layout, name, &[]) else {
                continue;
            };
            if found.is_some() {
                // Identical member names in multiple aggregate declarations
                // cannot be safely resolved from the detached Array node.
                return None;
            }
            found = Some((target, path));
        }
        found
    }

    pub(super) fn aggregate_member_relative_bound(
        &self,
        member_name: &str,
        packed_ranges: &[crate::core::db::PackedRange],
        bound: i128,
    ) -> Result<u32, String> {
        let (left, right) = match packed_ranges {
            [range] => (range.left, range.right),
            [] => {
                return Err(format!(
                    "packed-member `{}` has no captured packed range",
                    member_name
                ));
            }
            _ => {
                return Err(format!(
                    "select on multidimensional packed member `{}` is not supported",
                    member_name
                ));
            }
        };
        if bound < left.min(right) || bound > left.max(right) {
            return Err(format!(
                "packed-member select bound {bound} is outside `{}` range [{left}:{right}]",
                member_name
            ));
        }
        let relative = if left >= right {
            bound.checked_sub(right)
        } else {
            right.checked_sub(bound)
        }
        .ok_or_else(|| format!("packed-member select bound {bound} overflows"))?;
        u32::try_from(relative)
            .map_err(|_| format!("packed-member select bound {bound} does not fit in u32"))
    }

    /// Resolve constant indices on the final packed member type.  The
    /// frontend keeps packed dimensions outermost-first, while the runtime
    /// stores the complete aggregate as one LSB-relative vector.  Keeping
    /// this calculation here lets expression and LHS lowering share the
    /// exact same mapping across nested structs and unions.
    pub(super) fn packed_member_select_info(
        &self,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<(SignalInfo, PackedMember, u32, u32)>, String> {
        let Some((info, member)) = self.packed_member_info(base) else {
            return Ok(None);
        };
        if indices.is_empty()
            || member.packed_ranges.is_empty()
            || indices.len() > member.packed_ranges.len()
        {
            return Ok(None);
        }
        let (relative_lsb, width) =
            self.packed_selection_offset(&member.packed_ranges, indices, &member.name)?;
        let lsb = member
            .lsb
            .checked_add(relative_lsb)
            .ok_or_else(|| format!("packed-member `{}` offset overflows", member.name))?;
        Ok(Some((info, member, lsb, width)))
    }

    /// Resolve a range select on the outermost packed dimension of a member.
    /// The remaining dimensions form the selected element width.
    pub(super) fn packed_member_range_info(
        &self,
        base: NodeId,
        left: i128,
        right: i128,
    ) -> Result<Option<(SignalInfo, PackedMember, u32, u32)>, String> {
        let Some((info, member)) = self.packed_member_info(base) else {
            return Ok(None);
        };
        let Some(range) = member.packed_ranges.first() else {
            return Ok(None);
        };
        let low = range.left.min(range.right);
        let high = range.left.max(range.right);
        if !((low..=high).contains(&left) && (low..=high).contains(&right)) {
            return Err(format!(
                "packed-member select bound is outside `{}` range [{left}:{right}]",
                member.name
            ));
        }
        let inner_width = member.packed_ranges[1..]
            .iter()
            .try_fold(1u128, |width, dimension| {
                dimension
                    .left
                    .abs_diff(dimension.right)
                    .checked_add(1)
                    .and_then(|extent| width.checked_mul(extent))
            })
            .ok_or_else(|| format!("packed-member `{}` width overflows", member.name))?;
        let left_slot = self.packed_range_slot(*range, left, &member.name)?;
        let right_slot = self.packed_range_slot(*range, right, &member.name)?;
        let first_slot = left_slot.min(right_slot);
        let extent = left_slot
            .abs_diff(right_slot)
            .checked_add(1)
            .ok_or_else(|| format!("packed-member `{}` select width overflows", member.name))?;
        let relative_lsb = first_slot
            .checked_mul(inner_width)
            .ok_or_else(|| format!("packed-member `{}` offset overflows", member.name))?;
        let width = extent
            .checked_mul(inner_width)
            .ok_or_else(|| format!("packed-member `{}` select width overflows", member.name))?;
        let lsb = u128::from(member.lsb)
            .checked_add(relative_lsb)
            .ok_or_else(|| format!("packed-member `{}` offset overflows", member.name))?;
        Ok(Some((
            info,
            member,
            u32::try_from(lsb)
                .map_err(|_| "packed-member select offset does not fit in u32".to_string())?,
            u32::try_from(width)
                .map_err(|_| "packed-member select width does not fit in u32".to_string())?,
        )))
    }

    pub(super) fn packed_range_slot(
        &self,
        range: crate::core::db::PackedRange,
        index: i128,
        member_name: &str,
    ) -> Result<u128, String> {
        let low = range.left.min(range.right);
        let high = range.left.max(range.right);
        if !(low..=high).contains(&index) {
            return Err(format!(
                "packed-member select index {index} is outside `{member_name}` range [{left}:{right}]",
                left = range.left,
                right = range.right
            ));
        }
        let slot = if range.left >= range.right {
            index - range.right
        } else {
            range.right - index
        };
        u128::try_from(slot)
            .map_err(|_| format!("packed-member `{member_name}` select offset is negative"))
    }

    fn packed_selection_offset(
        &self,
        dimensions: &[crate::core::db::PackedRange],
        indices: &[NodeId],
        label: &str,
    ) -> Result<(u32, u32), String> {
        let mut remaining = dimensions
            .iter()
            .try_fold(1u128, |width, range| {
                range
                    .left
                    .abs_diff(range.right)
                    .checked_add(1)
                    .and_then(|extent| width.checked_mul(extent))
            })
            .ok_or_else(|| format!("packed select width overflows for `{label}`"))?;
        let mut lsb = 0u128;
        for (range, index_node) in dimensions.iter().zip(indices) {
            let extent = range
                .left
                .abs_diff(range.right)
                .checked_add(1)
                .ok_or_else(|| format!("packed select dimension overflows for `{label}`"))?;
            let index = self.eval_bound_i128(*index_node)?;
            let slot = self.packed_range_slot(*range, index, label)?;
            remaining /= extent;
            lsb = lsb
                .checked_add(
                    slot.checked_mul(remaining)
                        .ok_or_else(|| format!("packed select offset overflows for `{label}`"))?,
                )
                .ok_or_else(|| format!("packed select offset overflows for `{label}`"))?;
        }
        Ok((
            u32::try_from(lsb)
                .map_err(|_| format!("packed select offset does not fit in u32 for `{label}`"))?,
            u32::try_from(remaining)
                .map_err(|_| format!("packed select width does not fit in u32 for `{label}`"))?,
        ))
    }

    /// Resolve a select on a multidimensional packed declaration to its
    /// corresponding slice in the flattened runtime vector.
    pub(super) fn packed_select_info(
        &self,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<(SignalInfo, u32, u32)>, String> {
        let target = match self.kind(base) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => Some(base),
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
            _ => None,
        };
        let Some(target) = target
            .and_then(|target| self.clocking_var_target(target).or(Some(target)))
            .or_else(|| self.clocking_var_target(base))
        else {
            return Ok(None);
        };
        let target = self
            .db
            .is_clocking_var(target)
            .then(|| self.db.clocking_var(target).map(|var| var.source))
            .flatten()
            .unwrap_or(target);
        let Some(dimensions) = self.db.packed_dimensions(target) else {
            return Ok(None);
        };
        if dimensions.len() < 2 || indices.is_empty() || indices.len() > dimensions.len() {
            return Ok(None);
        }
        let info = self
            .signal_of(target)
            .cloned()
            .ok_or_else(|| "packed select target is not a signal".to_string())?;
        let (lsb, width) = self.packed_selection_offset(dimensions, indices, "signal")?;
        Ok(Some((info, lsb, width)))
    }

    pub(super) fn packed_range_for_base(
        &self,
        base: NodeId,
    ) -> Option<crate::core::db::PackedRange> {
        let base = self
            .clocking_var_target(base)
            .or_else(|| self.db.is_clocking_var(base).then_some(base))
            .and_then(|target| self.db.clocking_var(target).map(|var| var.source))
            .unwrap_or(base);
        if let Some([range]) = self.db.packed_dimensions(base) {
            return Some(*range);
        }
        self.packed_member_info(base).and_then(|(_, member)| {
            match member.packed_ranges.as_slice() {
                [range] => Some(*range),
                _ => None,
            }
        })
    }

    pub(super) fn packed_range_ascending(&self, base: NodeId) -> bool {
        self.packed_range_for_base(base)
            .is_some_and(|range| range.left < range.right)
    }

    pub(super) fn packed_relative_bound(&self, base: NodeId, index: i128) -> Result<i128, String> {
        let Some(range) = self.packed_range_for_base(base) else {
            return Ok(index);
        };
        if range.left < range.right {
            range.right.checked_sub(index)
        } else {
            index.checked_sub(range.right)
        }
        .ok_or_else(|| "packed select offset overflows".into())
    }

    pub(super) fn lower_packed_index(
        &mut self,
        path: &str,
        base: NodeId,
        index: NodeId,
    ) -> Result<IrExpr, String> {
        let index = self.lower_expr(path, index)?;
        let Some(range) = self.packed_range_for_base(base) else {
            return Ok(index);
        };
        if range.left >= range.right && range.right == 0 {
            return Ok(index);
        }
        let ascending = range.left < range.right;
        let right = lhs_integer_expr(range.right);
        // Extend before subtracting so narrow or unsigned source indices do
        // not wrap into a valid bit position. Preserve X/Z in the arithmetic.
        let width = index
            .width
            .max(right.width)
            .checked_add(1)
            .ok_or_else(|| "packed index width overflows".to_string())?;
        let index = IrExpr::convert_to(index, width, true);
        let right = IrExpr::convert_to(right, width, true);
        Ok(if ascending {
            bin_expr(IrBinOp::Sub, right, index)
        } else {
            bin_expr(IrBinOp::Sub, index, right)
        })
    }

    /// Recover a parameterized function return range from admitted source when
    /// the semantic type projection is incomplete.
    fn declared_source_width(&self, declaration: NodeId, inst: NodeId) -> Option<u32> {
        let node = self.node(declaration);
        let file = node.file.as_deref()?;
        let source = self.db.source_text(file)?;
        let line = source.lines().nth(node.line.checked_sub(1)? as usize)?;
        let range = line.split_once('[')?.1.split_once(']')?.0;
        let (left, right) = range.split_once(':')?;
        let owning_module = |mut node: NodeId| loop {
            if matches!(self.kind(node), NodeKind::ModuleInst { .. }) {
                break Some(node);
            }
            node = self.node(node).parent()?;
        };
        let function_module = owning_module(inst)?;
        let term = |text: &str| {
            let text = text.trim().replace('_', "");
            text.parse::<u128>().ok().or_else(|| {
                self.param_vals.iter().find_map(|(node, value)| {
                    if self.node(*node).name != text
                        || owning_module(*node) != Some(function_module)
                    {
                        return None;
                    }
                    let Val::Bits(value) = value else {
                        return None;
                    };
                    (!value.is_unknown()).then(|| value.to_u128()).flatten()
                })
            })
        };
        let evaluate = |expression: &str| {
            for operator in ['+', '-'] {
                if let Some((left, right)) = expression.split_once(operator) {
                    let (left, right) = (term(left)?, term(right)?);
                    return if operator == '+' {
                        left.checked_add(right)
                    } else {
                        left.checked_sub(right)
                    };
                }
            }
            term(expression)
        };
        let left = evaluate(left)?;
        let right = evaluate(right)?;
        u32::try_from(left.abs_diff(right).checked_add(1)?).ok()
    }

    pub(super) fn effective_decl_width(
        &self,
        declaration: NodeId,
        inst: NodeId,
        captured: u32,
    ) -> u32 {
        self.declared_source_width(declaration, inst)
            .unwrap_or(captured)
    }

    pub(super) fn source_size_cast_width(&self, expression: &str) -> Option<u32> {
        let token = expression.trim();
        let value = token.replace('_', "").parse::<u128>().ok().or_else(|| {
            self.param_vals.iter().find_map(|(node, value)| {
                if self.node(*node).name != token {
                    return None;
                }
                let Val::Bits(value) = value else {
                    return None;
                };
                (!value.is_unknown()).then(|| value.to_u128()).flatten()
            })
        })?;
        u32::try_from(value).ok().filter(|width| *width != 0)
    }
}
