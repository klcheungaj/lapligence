//! Selections.

use super::*;
use crate::sim::ir::{IrPackedSelect, IrTaggedMemberGuard, IrTaggedSelectStep};

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
                if let Some(info) = self.static_proc_local_signal(t) {
                    return Some(info);
                }
            }
            let (target, base_index) = self.hier_path_signal_target(parts, refs)?;
            if base_index + 1 == parts.len() {
                return self
                    .clocking_var_source_info(target)
                    .or_else(|| self.signal_of(target))
                    .or_else(|| self.static_proc_local_signal(target));
            }
        }
        None
    }

    pub(super) fn static_proc_local_signal(&self, node: NodeId) -> Option<&SignalInfo> {
        let instance = self.owning_inst(node)?;
        if let Some(signal) = self
            .static_formals
            .get(&(instance, node))
            .or_else(|| self.static_task_locals.get(&(instance, node)))
        {
            return Some(signal);
        }
        if self.db.variable_lifetime(node) != VariableLifetime::Static {
            return None;
        }
        self.proc_local_instances
            .get(&(instance, node))?
            .static_signal
            .as_ref()
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
                || self.static_proc_local_signal(target).is_some()
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
            if let Some(target) = self.sig_global_for_ir(info.ir) {
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

    /// Resolve a packed-member path rooted at a captured parameter. Unlike
    /// `packed_member_info`, this path has no signal storage; expression
    /// lowering reads the owned parameter value and projects the member as a
    /// constant instead.
    pub(super) fn packed_parameter_member_info(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, PackedMember)> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return None;
        };
        let (base_index, target) = refs.iter().enumerate().find_map(|(index, target)| {
            let target = (*target)?;
            (matches!(self.kind(target), NodeKind::Param { .. })
                && self.param_vals.contains_key(&target)
                && parts
                    .get(index)
                    .is_some_and(|part| part == &self.node(target).name))
            .then_some((index, target))
        })?;
        let member = self.packed_member_layout(target, parts.get(base_index + 1..)?)?;
        Some((target, member))
    }

    /// Lower the packed member path of a hierarchical reference into relative
    /// projections. Tagged-union checks are attached to the exact member step
    /// so reads and lvalues can share the same receiver and selector captures.
    pub(super) fn tagged_member_projection(
        &self,
        node: NodeId,
    ) -> Result<Option<(SignalInfo, Vec<IrTaggedSelectStep>)>, String> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return Ok(None);
        };
        let Some((target, base_index)) = self.hier_path_signal_target(parts, refs) else {
            return Ok(None);
        };
        let Some(info) = self.signal_of(target).cloned() else {
            return Ok(None);
        };
        let Some(mut descriptor) = self.query_descriptor(target).cloned() else {
            return Ok(None);
        };
        let mut steps = Vec::new();
        let mut checked = false;
        let mut tagged_storage = false;
        for name in &parts[base_index + 1..] {
            let TypeShape::Aggregate(layout) = &descriptor.shape else {
                return Ok(None);
            };
            tagged_storage |= layout.kind == AggregateKind::TaggedUnion;
            if !Self::packed_storage_layout(layout.kind, tagged_storage) {
                return Ok(None);
            }
            let index = layout
                .members
                .iter()
                .position(|member| member.name == *name)
                .ok_or_else(|| format!("packed member `{name}` is missing from its layout"))?;
            let member = &layout.members[index];
            let width = Self::fixed_descriptor_width(&member.descriptor)
                .ok_or_else(|| format!("packed member `{name}` has no fixed width"))?;
            let guard = if layout.kind == AggregateKind::TaggedUnion {
                let tag_width = layout
                    .tag_bits()
                    .ok_or_else(|| "tagged union tag width overflows".to_owned())?;
                if tag_width > 0 {
                    checked = true;
                    Some(IrTaggedMemberGuard {
                        member_index: u32::try_from(index)
                            .map_err(|_| "tagged union member index overflows".to_owned())?,
                        tag_width,
                        member_name: name.clone(),
                    })
                } else {
                    None
                }
            } else {
                None
            };
            let offset = Self::fixed_descriptor_path(
                &descriptor,
                &[AggregatePathPart::Member(name.clone())],
            )
            .map(|(_, offset)| offset)
            .ok_or_else(|| "packed member displacement overflows".to_owned())?;
            steps.push(IrTaggedSelectStep {
                selection: IrPackedSelect {
                    base: lhs_integer_expr(i128::from(offset)),
                    width,
                },
                two_state: member.two_state,
                guard,
            });
            descriptor = member.descriptor.clone();
        }
        if !checked {
            return Ok(None);
        }
        Ok(Some((info, steps)))
    }

    /// Guard a packed tagged-union member read with the active tag. Tagged
    /// payload bits share ordinary packed storage, but a member from another
    /// active arm has no value; expose X for that access instead of silently
    /// reading stale payload bits. Nested tagged layouts use the accumulated
    /// packed displacement for each tag slice.
    pub(super) fn guard_tagged_member_read(
        &self,
        node: NodeId,
        info: &SignalInfo,
        value: IrExpr,
    ) -> Result<IrExpr, String> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return Ok(value);
        };
        if let Some((root, steps)) = self.tagged_member_projection(node)? {
            return Ok(IrExpr::new(
                IrExprKind::TaggedSelect {
                    base: Box::new(self.signal_read_expr(&root)?),
                    steps,
                    location: self.source_location(node),
                },
                value.width,
                value.signed,
                None,
            ));
        }
        let Some((target, base_index)) = self.hier_path_signal_target(parts, refs) else {
            return Ok(value);
        };
        let Some(mut descriptor) = self.query_descriptor(target).cloned() else {
            return Ok(value);
        };
        let mut offset = 0u32;
        let mut guards = Vec::new();
        for name in &parts[base_index + 1..] {
            let TypeShape::Aggregate(layout) = &descriptor.shape else {
                break;
            };
            let index = layout
                .members
                .iter()
                .position(|member| member.name == *name)
                .ok_or_else(|| format!("tagged member `{name}` is missing from its layout"))?;
            let member = &layout.members[index];
            if layout.kind == AggregateKind::TaggedUnion
                && layout.tag_bits().is_some_and(|tag_width| tag_width > 0)
            {
                let total_width = Self::fixed_descriptor_width(&descriptor).ok_or_else(|| {
                    "tagged union member read has no fixed packed width".to_owned()
                })?;
                let tag_width = layout
                    .tag_bits()
                    .ok_or_else(|| "tagged union tag width overflows".to_owned())?;
                let tag_right = offset
                    .checked_add(total_width)
                    .and_then(|width| width.checked_sub(tag_width))
                    .ok_or_else(|| "tagged union tag offset underflows".to_owned())?;
                let tag_left = tag_right
                    .checked_add(tag_width)
                    .and_then(|width| width.checked_sub(1))
                    .ok_or_else(|| "tagged union tag offset overflows".to_owned())?;
                let tag = IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(self.signal_read_expr(info)?),
                        left: i64::from(tag_left),
                        right: i64::from(tag_right),
                    },
                    tag_width,
                    false,
                    None,
                );
                let expected = IrConst::packed(
                    vec![u64::try_from(index)
                        .map_err(|_| "tagged union member index overflows".to_owned())?],
                    vec![0],
                    vec![0],
                    tag_width,
                    false,
                    None,
                )
                .map_err(|error| error.to_string())?;
                guards.push(IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::CaseEq,
                        a: Box::new(tag),
                        b: Box::new(IrExpr::new(
                            IrExprKind::Const(expected),
                            tag_width,
                            false,
                            None,
                        )),
                    },
                    1,
                    false,
                    None,
                ));
            }
            let displacement = if matches!(
                layout.kind,
                AggregateKind::PackedUnion | AggregateKind::TaggedUnion
            ) {
                0
            } else {
                layout.members[index + 1..]
                    .iter()
                    .try_fold(0u32, |sum, following| {
                        sum.checked_add(Self::fixed_descriptor_width(&following.descriptor)?)
                    })
                    .ok_or_else(|| "tagged member packed displacement overflows".to_owned())?
            };
            offset = offset
                .checked_add(displacement)
                .ok_or_else(|| "tagged member packed offset overflows".to_owned())?;
            descriptor = member.descriptor.clone();
        }
        let Some(mut guard) = guards.first().cloned() else {
            return Ok(value);
        };
        for next in guards.into_iter().skip(1) {
            guard = IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::LogAnd,
                    a: Box::new(guard),
                    b: Box::new(next),
                },
                1,
                false,
                None,
            );
        }
        let fallback = const_x_expr(value.width);
        Ok(IrExpr::new(
            IrExprKind::Mux {
                sel: Box::new(guard),
                a: Box::new(value.clone()),
                b: Box::new(fallback),
            },
            value.width,
            value.signed,
            None,
        ))
    }

    /// Whether members of `kind` are bit ranges of one packed owner. Unpacked
    /// records and unions nested in a tagged union share that union's
    /// storage, using the flattened fixed-value member order.
    pub(super) fn packed_storage_layout(kind: AggregateKind, tagged_storage: bool) -> bool {
        match kind {
            AggregateKind::PackedStruct
            | AggregateKind::PackedUnion
            | AggregateKind::TaggedUnion => true,
            AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion => tagged_storage,
        }
    }

    pub(super) fn packed_member_layout(
        &self,
        target: NodeId,
        parts: &[String],
    ) -> Option<PackedMember> {
        let mut layout = self.db.aggregate_layout(target)?;
        let mut tagged_storage = layout.kind == AggregateKind::TaggedUnion;
        if !Self::packed_storage_layout(layout.kind, tagged_storage) {
            return None;
        }
        let mut descriptor = self.query_descriptor(target)?;
        let mut absolute_lsb = 0u32;
        let mut selected: Option<&AggregateMember> = None;
        for (part_index, member_name) in parts.iter().enumerate() {
            let index = layout
                .members
                .iter()
                .position(|member| member.name == *member_name)?;
            let member = &layout.members[index];
            let relative_lsb = if tagged_storage {
                Self::fixed_descriptor_path(
                    descriptor,
                    &[AggregatePathPart::Member(member_name.clone())],
                )?
                .1
            } else if matches!(
                layout.kind,
                AggregateKind::PackedUnion | AggregateKind::TaggedUnion
            ) {
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
            descriptor = &member.descriptor;
            match member.aggregate_layout() {
                Some(nested) if Self::packed_storage_layout(nested.kind, tagged_storage) => {
                    tagged_storage |= nested.kind == AggregateKind::TaggedUnion;
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
            width: member
                .ty
                .width
                .filter(|_| !tagged_storage)
                .or_else(|| Self::fixed_descriptor_width(&member.descriptor))?,
            signed: member.ty.signed,
            two_state: member.two_state,
            packed_ranges: member.packed_ranges.clone(),
        })
    }

    pub(super) fn unpacked_aggregate_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Var { .. } | NodeKind::Net { .. } => {
                self.unpacked_aggregates.contains_key(&node).then_some(node)
            }
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
                        found = sorted_node_ids(&self.unpacked_aggregates)
                            .into_iter()
                            .find_map(|target| {
                                (self.node(target).name == parts[base_index]
                                    && self.instance_path_of(target) == scope)
                                    .then_some((target, base_index))
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
        for target in sorted_node_ids(&self.unpacked_aggregates) {
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

    pub(super) fn packed_parameter_member_select_info(
        &self,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<(NodeId, PackedMember, u32, u32)>, String> {
        let Some((parameter, member)) = self.packed_parameter_member_info(base) else {
            return Ok(None);
        };
        if indices.is_empty()
            || member.packed_ranges.is_empty()
            || indices.len() > member.packed_ranges.len()
        {
            return Ok(None);
        }
        if indices
            .iter()
            .any(|index| self.eval_bound_i128(*index).is_err())
        {
            return Ok(None);
        }
        let (relative_lsb, width) =
            self.packed_selection_offset(&member.packed_ranges, indices, &member.name)?;
        let lsb = member
            .lsb
            .checked_add(relative_lsb)
            .ok_or_else(|| format!("packed-member `{}` offset overflows", member.name))?;
        Ok(Some((parameter, member, lsb, width)))
    }

    pub(super) fn packed_parameter_member_dynamic_select_info(
        &mut self,
        path: &str,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<(NodeId, PackedMember, IrExpr, u32)>, String> {
        if indices.len() != 1 {
            return Ok(None);
        }
        let Some((parameter, member)) = self.packed_parameter_member_info(base) else {
            return Ok(None);
        };
        let Some(range) = member.packed_ranges.first().copied() else {
            return Ok(None);
        };
        if self.eval_bound_i128(indices[0]).is_ok() {
            return Ok(None);
        }

        let stride = member.packed_ranges[1..]
            .iter()
            .try_fold(1u32, |stride, dimension| {
                let extent = dimension
                    .left
                    .abs_diff(dimension.right)
                    .checked_add(1)
                    .and_then(|extent| u32::try_from(extent).ok())?;
                stride.checked_mul(extent)
            })
            .ok_or_else(|| format!("packed member `{}` stride overflows", member.name))?;
        let first_extent = range
            .left
            .abs_diff(range.right)
            .checked_add(1)
            .and_then(|extent| u32::try_from(extent).ok())
            .ok_or_else(|| format!("packed member `{}` extent overflows", member.name))?;
        if first_extent.checked_mul(stride) != Some(member.width) {
            return Err(format!(
                "packed member `{}` dimensions disagree with its width",
                member.name
            ));
        }

        let mut index = self.lower_expr(path, indices[0])?;
        if index.is_real() || stride == 0 {
            return Err(
                "packed selection requires an integral index and nonzero stride".to_owned(),
            );
        }
        if index.fill.is_some()
            || matches!(&index.kind, IrExprKind::Fill(_))
            || matches!(&index.kind, IrExprKind::Const(value) if value.fill.is_some())
        {
            let width = index.width;
            index = IrExpr::new(
                IrExprKind::Concat { parts: vec![index] },
                width,
                false,
                None,
            );
        }

        let right = lhs_integer_expr(range.right);
        let multiply_bits = u32::BITS - (stride - 1).leading_zeros();
        let arithmetic_width = index
            .width
            .max(right.width)
            .checked_add(2)
            .and_then(|width| width.checked_add(multiply_bits))
            .filter(|width| *width <= LLG_MAX_WIDTH)
            .ok_or_else(|| {
                "packed selection index arithmetic exceeds the supported limit".to_owned()
            })?;
        let index = IrExpr::convert_to(index, arithmetic_width, true);
        let right = IrExpr::convert_to(right, arithmetic_width, true);
        let relative = if range.left < range.right {
            bin_expr(IrBinOp::Sub, right, index)
        } else {
            bin_expr(IrBinOp::Sub, index, right)
        };
        let scaled = bin_expr(
            IrBinOp::Mul,
            relative,
            IrExpr::convert_to(lhs_integer_expr(i128::from(stride)), arithmetic_width, true),
        );
        Ok(Some((parameter, member, scaled, stride)))
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
        self.packed_ranges_for_base(base)
            .and_then(|ranges| (ranges.len() == 1).then_some(ranges[0]))
    }

    /// Return the packed dimensions still visible after a constant packed
    /// projection.  The expression lowering and true-net alias lowering both
    /// use the same flattened storage order for multidimensional packed
    /// declarations and packed aggregate members.
    pub(super) fn packed_ranges_for_base(
        &self,
        base: NodeId,
    ) -> Option<Vec<crate::core::db::PackedRange>> {
        match self.kind(base) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let mut ranges = self.packed_ranges_for_base(*base)?;
                if indices.len() > ranges.len() {
                    return None;
                }
                ranges.drain(..indices.len());
                Some(ranges)
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => self
                .packed_member_info(base)
                .map(|(_, member)| member.packed_ranges)
                .or_else(|| {
                    self.hier_path_signal(base)
                        .and_then(|info| self.sig_global_for_ir(info.ir))
                        .and_then(|target| self.packed_ranges_for_base(target))
                }),
            NodeKind::Expr(ExprKind::Ref { target }) => {
                // A reference expression carries the formal's resolved type
                // metadata even when its declaration target is an argument
                // node, which is not itself a runtime signal. Keep that
                // metadata as the fallback after following the target so
                // packed ref-formal indices retain their declared direction.
                // A modport expression port is numbered by its expression's
                // self-determined type (SV 25.5.4), e.g. `[7:4]` for `r[7:4]`.
                if let Some(expression) = self.modport_expression_target(base) {
                    return self
                        .packed_ranges_for_base(expression)
                        .or_else(|| self.db.packed_dimensions(base).map(ToOwned::to_owned));
                }
                target
                    .and_then(|target| self.packed_ranges_for_base(target))
                    .or_else(|| self.db.packed_dimensions(base).map(ToOwned::to_owned))
            }
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                let target = self
                    .clocking_var_target(base)
                    .or_else(|| self.db.is_clocking_var(base).then_some(base))
                    .and_then(|target| self.db.clocking_var(target).map(|var| var.source))
                    .unwrap_or(base);
                self.db.packed_dimensions(target).map(ToOwned::to_owned)
            }
            // A computed value is numbered by its own type, e.g. `~v[3:2]`
            // keeps `[3:2]`. Instance-array terminals select from such
            // values (IEEE 1800-2009 23.3.3.5, 28.3.6).
            NodeKind::Expr(
                ExprKind::Operation { .. }
                | ExprKind::Conditional { .. }
                | ExprKind::Cast { .. }
                | ExprKind::PartSelect { .. }
                | ExprKind::IndexedPartSelect { .. },
            ) => self.db.packed_dimensions(base).map(ToOwned::to_owned),
            _ => None,
        }
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

    pub(super) fn source_size_cast_width(&self, expression: &str) -> Option<u32> {
        let token = expression.trim();
        let value = token.replace('_', "").parse::<u128>().ok().or_else(|| {
            sorted_node_ids(&self.param_vals)
                .into_iter()
                .find_map(|node| {
                    if self.node(node).name != token {
                        return None;
                    }
                    let value = &self.param_vals[&node];
                    let Val::Bits(value) = value else {
                        return None;
                    };
                    (!value.is_unknown()).then(|| value.to_u128()).flatten()
                })
        })?;
        u32::try_from(value).ok().filter(|width| *width != 0)
    }
}
