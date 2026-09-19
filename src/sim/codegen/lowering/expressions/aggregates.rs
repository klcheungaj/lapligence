//! Aggregates.

use super::*;

mod copies;

impl<'a> Codegen<'a> {
    /// Lower an assignment LHS: the pre-IR [`Self::analyze_lhs`] decisions
    /// converted to [`IrLhs`] (identical by construction during the seam
    /// transition; sub-expression codes ride along verbatim).
    pub(in super::super) fn lower_lhs(&mut self, path: &str, lhs: NodeId) -> Result<IrLhs, String> {
        if let Some(target) = self.assertion_local_lhs_target(lhs) {
            let binding = self
                .assertion_local_binding(target)?
                .ok_or_else(|| "local assertion variable is outside a sequence graph".to_owned())?;
            if let Some(direction) = self.db.assertion_formal_direction(target) {
                return Err(format!(
                    "assertion formal `{}` cannot be assigned through {direction:?} direction in `{path}`",
                    self.node(target).name
                ));
            }
            return Ok(IrLhs::WholeRef {
                addr: format!("llg_sequence_local_addr(data, {}u)", binding.slot),
                width: binding.width,
                signed: binding.signed,
                two_state: binding.two_state,
                shortreal: false,
            });
        }
        let mut clocking_targets = Vec::new();
        let all_clocking = self.clocking_lhs_targets(lhs, &mut clocking_targets);
        if !clocking_targets.is_empty() {
            if !all_clocking {
                return Err(format!(
                    "clocking output/inout concatenations cannot mix ordinary targets in `{path}`"
                ));
            }
            if let Some(target) = clocking_targets.iter().find(|target| {
                self.db
                    .clocking_var(**target)
                    .is_some_and(|var| matches!(var.direction, DbDirection::Input))
            }) {
                return Err(format!(
                    "clocking input member `{}` is read-only in `{path}`",
                    self.node(*target).name
                ));
            }
        }
        if let Some(target) = self.capture_target(lhs) {
            let binding = self
                .capture_binding(target)
                .expect("capture target must have a binding");
            return Ok(IrLhs::WholeRef {
                addr: format!("&{}", Codegen::capture_local_name(binding.storage)),
                width: binding.local.width,
                signed: binding.local.signed,
                two_state: binding.local.two_state,
                shortreal: false,
            });
        }
        if let Some(lhs) = self.virtual_interface_member_lhs(path, lhs)? {
            return Ok(lhs);
        }
        if let Some(lhs) = self.class_field_lhs(path, lhs)? {
            return Ok(lhs);
        }
        let lh = self.analyze_lhs(path, lhs)?;
        self.lhs_to_ir(lh)
    }

    pub(in super::super) fn lower_packed_aggregate_pattern(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        op: Operation,
    ) -> Result<Option<IrExpr>, String> {
        if !matches!(
            self.kind(rhs),
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == Operation::AssignmentPattern
        ) {
            return Ok(None);
        }
        let target = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            NodeKind::Var { .. } => lhs,
            _ => return Ok(None),
        };
        let Some(layout) = self.db.aggregate_layout(target) else {
            return Ok(None);
        };
        if !matches!(
            layout.kind,
            AggregateKind::PackedStruct | AggregateKind::PackedUnion
        ) {
            return Ok(None);
        }
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment of packed aggregate pattern in `{path}` is not supported"
            ));
        }
        Ok(Some(
            self.lower_packed_aggregate_pattern_value(path, rhs, layout)?,
        ))
    }

    pub(in super::super) fn lower_packed_aggregate_pattern_value(
        &mut self,
        path: &str,
        rhs: NodeId,
        layout: &crate::core::db::AggregateLayout,
    ) -> Result<IrExpr, String> {
        let values = self.aggregate_pattern_values(path, rhs, layout)?;
        let mut members = Vec::with_capacity(values.len());
        for (member_index, value_node) in values {
            let member = layout.members.get(member_index).ok_or_else(|| {
                format!("aggregate pattern member index {member_index} is out of bounds")
            })?;
            let width = member.ty.width.ok_or_else(|| {
                format!(
                    "packed member `{}` has unresolved width in `{path}`",
                    member.name
                )
            })?;
            let value = if let Some(nested) = member.aggregate_layout() {
                if matches!(
                    self.kind(value_node),
                    NodeKind::Expr(ExprKind::Operation { op, .. })
                        if *op == Operation::AssignmentPattern
                ) {
                    if !matches!(
                        nested.kind,
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion
                    ) {
                        return Err(format!(
                            "nested unpacked aggregate member `{}` in `{path}` is not supported",
                            member.name
                        ));
                    }
                    self.lower_packed_aggregate_pattern_value(path, value_node, nested)?
                } else {
                    self.lower_expr(path, value_node)?
                }
            } else {
                self.lower_expr(path, value_node)?
            };
            members.push(ir_to_storage(
                value,
                width,
                member.ty.signed,
                member.two_state,
            )?);
        }
        let value = if layout.kind == AggregateKind::PackedUnion {
            members
                .into_iter()
                .next()
                .ok_or_else(|| format!("packed union assignment pattern is empty in `{path}`"))?
        } else {
            let width = members
                .iter()
                .try_fold(0u32, |total, member| total.checked_add(member.width()));
            let width = width.ok_or_else(|| {
                format!("packed aggregate assignment pattern width overflows in `{path}`")
            })?;
            IrExpr::new(IrExprKind::Concat { parts: members }, width, false, None)
        };
        Ok(value)
    }

    pub(in super::super) fn lower_unpacked_aggregate_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        if op == Operation::Assignment
            && self.query_descriptor(lhs).is_some_and(|descriptor| {
                matches!(
                    descriptor.shape,
                    TypeShape::FixedArray { .. } | TypeShape::Aggregate(_)
                )
            })
        {
            if let Some(target) = self.fixed_activation_lhs(path, lhs)? {
                let value = self.lower_expr(path, rhs)?;
                return Ok(Some(IrStmt::Assign {
                    lhs: target,
                    rhs: value,
                    nba,
                }));
            }
        }
        let lhs_aggregate = self.unpacked_aggregate_info(lhs);
        let rhs_aggregate = self.unpacked_aggregate_info(rhs);
        let lhs_sub = self.resolve_unpacked_aggregate(lhs);
        let rhs_sub = self.resolve_unpacked_aggregate(rhs);
        let rhs_is_pattern = matches!(
            self.kind(rhs),
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == Operation::AssignmentPattern
        );
        if lhs_aggregate.is_none() && rhs_aggregate.is_none() && lhs_sub.is_none() {
            // Not an aggregate destination: packed patterns and every other
            // assignment shape keep their existing lowering.
            return Ok(None);
        }
        if let Some(selection) = &lhs_sub {
            if rhs_sub.is_none()
                && !rhs_is_pattern
                && (matches!(self.kind(rhs), NodeKind::FuncCall { .. })
                    || self.fixed_activation_read(path, rhs)?.is_some())
            {
                let value = self.lower_expr(path, rhs)?;
                return self
                    .assign_packed_subaggregate(path, selection, value, nba, op)
                    .map(Some);
            }
            if matches!(&selection.descriptor.shape, TypeShape::Aggregate(layout)
                if matches!(layout.kind, AggregateKind::PackedStruct | AggregateKind::PackedUnion))
                && rhs_sub.is_none()
            {
                let value = match &selection.descriptor.shape {
                    TypeShape::Aggregate(layout) if rhs_is_pattern => {
                        self.lower_packed_aggregate_pattern_value(path, rhs, layout)?
                    }
                    _ => self.lower_expr(path, rhs)?,
                };
                return self
                    .assign_packed_subaggregate(path, selection, value, nba, op)
                    .map(Some);
            }
        }
        if rhs_is_pattern {
            return Ok(Some(
                self.lower_unpacked_pattern_assignment(path, lhs, rhs, nba, op)?,
            ));
        }
        if lhs_aggregate.is_none() || (rhs_aggregate.is_none() && rhs_sub.is_some()) {
            // Either side may denote a selected sub-value. The enclosing root
            // locates storage, but the selected descriptor determines its type.
            let lhs_selection = lhs_sub.ok_or_else(|| {
                format!("unpacked aggregate used as a scalar assignment RHS in `{path}`")
            })?;
            let rhs_selection = rhs_sub.ok_or_else(|| {
                format!("unpacked aggregate used as a scalar assignment LHS in `{path}`")
            })?;
            return Ok(Some(self.lower_unpacked_subaggregate_copy(
                path,
                &lhs_selection,
                &rhs_selection,
                nba,
                op,
            )?));
        }
        let (lhs_target, lhs_aggregate) = lhs_aggregate.expect("whole aggregate destination");
        let bitstream_cast_operand = match self.kind(rhs) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => Some(*operand),
            _ => None,
        };
        let bitstream_cast_target = bitstream_cast_operand.is_some()
            && self.query_descriptor(rhs).is_some_and(|descriptor| {
                matches!(
                    descriptor.shape,
                    TypeShape::Aggregate(ref layout)
                        if matches!(
                            layout.kind,
                            AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion
                        )
                )
            });
        if bitstream_cast_target {
            if op != Operation::Assignment {
                return Err(format!(
                    "compound assignment of unpacked aggregate bit-stream cast in `{path}` is not supported"
                ));
            }
            if lhs_aggregate.kind == AggregateKind::UnpackedUnion {
                return Err(format!(
                    "unpacked union bit-stream destination is not supported in `{path}`"
                ));
            }
            let operand = bitstream_cast_operand.expect("bit-stream cast has an operand");
            let source = match self.lower_bitstream_source(path, operand)? {
                Some(value) => value,
                None => self.lower_expr(path, operand)?,
            };
            if source.is_real() {
                return Err(format!(
                    "real bit-stream source cannot initialize an unpacked aggregate in `{path}`"
                ));
            }
            let total_width = lhs_aggregate
                .leaves
                .iter()
                .try_fold(0u32, |width, leaf| {
                    leaf.object
                        .is_none()
                        .then_some(())
                        .and_then(|_| width.checked_add(leaf.member.ty.width?))
                })
                .ok_or_else(|| {
                    format!("unpacked aggregate bit-stream width is unresolved in `{path}`")
                })?;
            if source.width != total_width {
                return Err(format!(
                    "bit-stream cast source is {} bits but unpacked aggregate destination requires {} in `{path}`",
                    source.width, total_width
                ));
            }
            let source_name = format!("_bitstream_agg_{}_{}", lhs_target.0, rhs.0);
            let source_width = source.width;
            let source_signed = source.signed;
            let captured = IrExpr::new(
                IrExprKind::LocalRead(source_name.clone()),
                source_width,
                source_signed,
                None,
            );
            // Keep one explicit capture so a source with side effects is
            // evaluated exactly once before any member write.
            let mut captures = vec![IrStmt::DeclLocal {
                name: source_name.clone(),
                width: source_width,
                signed: source_signed,
                init: Some(Box::new(source)),
                two_state: false,
            }];
            let mut offsets = Vec::with_capacity(lhs_aggregate.leaves.len());
            let mut right = 0u32;
            for leaf in lhs_aggregate.leaves.iter().rev() {
                let width = leaf.member.ty.width.ok_or_else(|| {
                    format!(
                        "unpacked member `{}` has unresolved width",
                        leaf.member.name
                    )
                })?;
                offsets.push((leaf, right, width));
                right = right.checked_add(width).ok_or_else(|| {
                    format!("unpacked aggregate bit-stream offset overflows in `{path}`")
                })?;
            }
            offsets.reverse();
            for (leaf, right, width) in offsets {
                let value = IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(captured.clone()),
                        left: i64::from(right + width - 1),
                        right: i64::from(right),
                    },
                    width,
                    false,
                    None,
                );
                let lhs = self.aggregate_leaf_lhs(leaf)?;
                let rhs = apply_lhs_assignment_context(&self.model, &lhs, value);
                captures.push(IrStmt::Assign { lhs, rhs, nba });
            }
            return Ok(Some(IrStmt::Block(captures)));
        }
        if lhs_aggregate.kind == AggregateKind::UnpackedStruct
            && matches!(self.kind(rhs), NodeKind::Expr(ExprKind::Constant { .. }))
            && lhs_aggregate
                .leaves
                .iter()
                .all(|leaf| leaf.path.len() == 1 && leaf.signal.is_some())
        {
            // The frontend folds constant unpacked-structure assignment patterns
            // to one integral payload. Recover the positional member values
            // from the standard first-member-most-significant layout.
            let total_width = lhs_aggregate
                .leaves
                .iter()
                .try_fold(0u32, |width, member| {
                    width.checked_add(member.member.ty.width?)
                })
                .ok_or_else(|| format!("unpacked struct pattern width overflow in `{path}`"))?;
            let packed = IrExpr::convert_to(self.lower_expr(path, rhs)?, total_width, false);
            let mut right = 0u32;
            let mut values = Vec::with_capacity(lhs_aggregate.leaves.len());
            for member in lhs_aggregate.leaves.iter().rev() {
                let width = member.member.ty.width.ok_or_else(|| {
                    format!(
                        "unpacked member `{}` has unresolved width",
                        member.member.name
                    )
                })?;
                values.push((member, right, width));
                right = right.checked_add(width).ok_or_else(|| {
                    format!("unpacked struct pattern offset overflow in `{path}`")
                })?;
            }
            values.reverse();
            let mut assignments = Vec::with_capacity(values.len());
            for (left, right, width) in values {
                let value = IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(packed.clone()),
                        left: i64::from(right + width - 1),
                        right: i64::from(right),
                    },
                    width,
                    false,
                    None,
                );
                let lhs = self.aggregate_leaf_lhs(left)?;
                let value = apply_lhs_assignment_context(&self.model, &lhs, value);
                assignments.push(IrStmt::Assign {
                    lhs,
                    rhs: value,
                    nba,
                });
            }
            return Ok(Some(IrStmt::Block(assignments)));
        }
        let (rhs_target, rhs_aggregate) = rhs_aggregate.ok_or_else(|| {
            format!("unpacked aggregate used as a scalar assignment LHS in `{path}`")
        })?;
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment of unpacked aggregates in `{path}` is not supported"
            ));
        }
        let same_type_identity = match (
            lhs_aggregate.type_identity.as_deref(),
            rhs_aggregate.type_identity.as_deref(),
        ) {
            (Some(left), Some(right)) => left == right,
            (None, None) => lhs_target == rhs_target,
            _ => false,
        };
        let compatible = same_type_identity
            && lhs_aggregate.kind == rhs_aggregate.kind
            && lhs_aggregate.members.len() == rhs_aggregate.members.len()
            && lhs_aggregate
                .members
                .iter()
                .zip(&rhs_aggregate.members)
                .all(|(left, right)| {
                    left.member.name == right.member.name
                        && left.member.descriptor == right.member.descriptor
                });
        if !compatible {
            return Err(format!(
                "assignment between incompatible unpacked aggregate types `{}` and `{}` in `{path}`",
                self.node(lhs_target).name,
                self.node(rhs_target).name
            ));
        }
        if lhs_aggregate.kind == AggregateKind::UnpackedUnion {
            let lhs = lhs_aggregate.leaves.first().ok_or_else(|| {
                format!(
                    "unpacked union `{}` has no members",
                    self.node(lhs_target).name
                )
            })?;
            let rhs = rhs_aggregate.leaves.first().ok_or_else(|| {
                format!(
                    "unpacked union `{}` has no members",
                    self.node(rhs_target).name
                )
            })?;
            if let (Some(lhs_signal), Some(rhs_signal)) = (&lhs.signal, &rhs.signal) {
                return Ok(Some(IrStmt::Assign {
                    lhs: self.reference_lhs(IrLhs::Whole(lhs_signal.ir))?,
                    rhs: self.signal_read_expr(rhs_signal)?,
                    nba,
                }));
            }
            return Err(format!(
                "unpacked union `{}` has no packed storage in `{path}`",
                self.node(lhs_target).name
            ));
        }
        let mut assignments = Vec::with_capacity(lhs_aggregate.leaves.len());
        for left in &lhs_aggregate.leaves {
            let right = rhs_aggregate
                .leaves
                .iter()
                .find(|right| right.path == left.path)
                .ok_or_else(|| {
                    format!(
                        "aggregate member path `{}` is missing from assignment source in `{path}`",
                        aggregate_path_suffix(&left.path)
                    )
                })?;
            if let (Some(lhs_object), Some(rhs_object)) = (left.object, right.object) {
                if nba {
                    return Err(format!(
                        "nonblocking assignment to object aggregate member `{}` is not supported in `{path}`",
                        aggregate_path_suffix(&left.path)
                    ));
                }
                let lhs_object = self.reference_object(lhs_object);
                let rhs_object = self.reference_object(rhs_object);
                let operation = match self.model.objects[lhs_object].ty {
                    IrObjectType::String => {
                        IrObjectStmt::StringAssign(lhs_object, IrStringExpr::Read(rhs_object))
                    }
                    IrObjectType::Chandle => {
                        IrObjectStmt::ChandleAssign(lhs_object, IrChandleExpr::Read(rhs_object))
                    }
                    IrObjectType::Semaphore => {
                        IrObjectStmt::ChandleAssign(lhs_object, IrChandleExpr::Read(rhs_object))
                    }
                    IrObjectType::Process => {
                        return Err(format!(
                            "process aggregate member assignment is not supported in `{path}`"
                        ));
                    }
                };
                assignments.push(IrStmt::Object(operation));
                continue;
            }
            let lhs = self.aggregate_leaf_lhs(left)?;
            let mut value = self.aggregate_leaf_read(right)?;
            if right.member.two_state && !value.is_real() {
                value = IrExpr::to_two_state(value);
            }
            let value = apply_lhs_assignment_context(&self.model, &lhs, value);
            assignments.push(IrStmt::Assign {
                lhs,
                rhs: value,
                nba,
            });
        }
        Ok(Some(IrStmt::Block(assignments)))
    }

    /// Lower an unpacked aggregate assignment pattern.  A whole aggregate
    /// destination resolves pattern member names against its captured layout;
    /// a nested destination (an array element or nested struct member)
    /// resolves against the recursive descriptor at that path so one walker
    /// serves both.  Source expressions are staged before any destination
    /// write so overlapping or side-effecting values evaluate exactly once.
    fn assign_packed_subaggregate(
        &mut self,
        path: &str,
        selection: &copies::AggregateSelection,
        value: IrExpr,
        nba: bool,
        op: Operation,
    ) -> Result<IrStmt, String> {
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment to an aggregate member in `{path}` requires a packed lvalue"
            ));
        }
        let width = Self::fixed_descriptor_width(&selection.descriptor)
            .ok_or_else(|| format!("packed member width is unresolved in `{path}`"))?;
        let name = self.new_fn_name(path, "packed_member_value");
        let value = ir_to_storage(value, width, selection.descriptor.info.signed, false)?;
        let mut body = vec![IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed: value.signed,
            two_state: false,
            init: Some(Box::new(value)),
        }];
        let mut remaining = width;
        for leaf in selection
            .storage
            .leaves
            .iter()
            .filter(|leaf| leaf.path.starts_with(&selection.prefix))
        {
            let leaf_width = leaf
                .member
                .ty
                .width
                .ok_or_else(|| format!("packed leaf has no width in `{path}`"))?;
            remaining = remaining
                .checked_sub(leaf_width)
                .ok_or_else(|| format!("packed member layout exceeds its width in `{path}`"))?;
            let value = IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(IrExpr::new(
                        IrExprKind::LocalRead(name.clone()),
                        width,
                        false,
                        None,
                    )),
                    left: i64::from(remaining + leaf_width - 1),
                    right: i64::from(remaining),
                },
                leaf_width,
                false,
                None,
            );
            body.push(IrStmt::Assign {
                lhs: self.aggregate_leaf_lhs(leaf)?,
                rhs: value,
                nba,
            });
        }
        if remaining != 0 {
            return Err(format!("packed member layout is incomplete in `{path}`"));
        }
        Ok(IrStmt::Block(body))
    }

    fn lower_unpacked_pattern_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
        op: Operation,
    ) -> Result<IrStmt, String> {
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment of unpacked aggregate pattern in `{path}` is not supported"
            ));
        }
        let Some((target, aggregate)) = self.unpacked_aggregate_info(lhs) else {
            return self.lower_unpacked_subaggregate_pattern(path, lhs, rhs, nba);
        };
        let layout = self.db.aggregate_layout(target).ok_or_else(|| {
            format!(
                "unpacked aggregate `{}` in `{path}` has no captured layout",
                self.node(target).name
            )
        })?;
        let mut values = Vec::new();
        self.aggregate_pattern_leaf_values(path, rhs, layout, &[], &mut values)?;
        self.assign_unpacked_pattern_values(path, target, &aggregate, values, nba)
    }

    /// Resolve a pattern whose destination is a sub-value of an unpacked
    /// aggregate (for example `s.items[0]`).  The declaration-relative path
    /// keeps the destination prefix separate from the pattern leaf paths.
    fn lower_unpacked_subaggregate_pattern(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let (target, prefix) = self.unpacked_path_for_expr(lhs).ok_or_else(|| {
            format!("unpacked aggregate pattern destination in `{path}` has no captured storage")
        })?;
        let aggregate = self
            .unpacked_aggregates
            .get(&target)
            .cloned()
            .ok_or_else(|| {
                format!("unpacked aggregate pattern destination in `{path}` has no captured leaves")
            })?;
        let root = self.query_descriptor(target).cloned().ok_or_else(|| {
            format!(
                "unpacked aggregate `{}` in `{path}` has no recursive type descriptor",
                self.node(target).name
            )
        })?;
        let descriptor = Self::descriptor_at_path(&root, &prefix).ok_or_else(|| {
            format!(
                "unpacked aggregate pattern destination in `{path}` has no matching recursive type"
            )
        })?;
        let mut values = Vec::new();
        self.aggregate_descriptor_pattern_values(path, rhs, &descriptor, &prefix, &mut values)?;
        self.assign_unpacked_pattern_values(path, target, &aggregate, values, nba)
    }

    fn assign_unpacked_pattern_values(
        &mut self,
        path: &str,
        target: NodeId,
        aggregate: &UnpackedAggregateInfo,
        values: Vec<(Vec<AggregatePathPart>, NodeId)>,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let mut assignments = Vec::with_capacity(values.len());
        let mut captures = Vec::new();
        let mut captured = HashMap::<NodeId, (String, u32, bool)>::new();
        for (member_path, value_node) in values {
            let left = aggregate
                .leaves
                .iter()
                .find(|leaf| leaf.path == member_path)
                .ok_or_else(|| {
                    format!(
                        "aggregate pattern path `{}` has no destination in `{path}`",
                        aggregate_path_suffix(&member_path)
                    )
                })?;
            if let Some(index) = left.object {
                if nba {
                    return Err(format!(
                        "nonblocking assignment to object aggregate member `{}` is not supported in `{path}`",
                        aggregate_path_suffix(&member_path)
                    ));
                }
                let operation = match self.model.objects[index].ty {
                    IrObjectType::String => {
                        IrObjectStmt::StringAssign(index, self.lower_string(path, value_node)?)
                    }
                    IrObjectType::Chandle => {
                        IrObjectStmt::ChandleAssign(index, self.lower_chandle(path, value_node)?)
                    }
                    IrObjectType::Semaphore => {
                        IrObjectStmt::ChandleAssign(index, self.lower_chandle(path, value_node)?)
                    }
                    IrObjectType::Process => {
                        return Err(format!(
                            "process aggregate member assignment is not supported in `{path}`"
                        ));
                    }
                };
                assignments.push(IrStmt::Object(operation));
                continue;
            }
            let lhs = self.aggregate_leaf_lhs(left)?;
            let value = if let Some((name, width, signed)) = captured.get(&value_node) {
                IrExpr::new(IrExprKind::LocalRead(name.clone()), *width, *signed, None)
            } else {
                let source = self.lower_expr(path, value_node)?;
                let name = format!("_agg{}_{}", target.0, value_node.0);
                let (width, signed) = (source.width, source.signed);
                captures.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(source)),
                });
                captured.insert(value_node, (name.clone(), width, signed));
                IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
            };
            let value = apply_lhs_assignment_context(&self.model, &lhs, value);
            assignments.push(IrStmt::Assign {
                lhs,
                rhs: value,
                nba,
            });
        }
        captures.extend(assignments);
        Ok(IrStmt::Block(captures))
    }

    /// Walk a recursive type descriptor by an aggregate path.  Atom and
    /// handle leaves have no further shape, so a path that descends past them
    /// is not a pattern destination.
    fn descriptor_at_path(
        descriptor: &TypeDescriptor,
        path: &[AggregatePathPart],
    ) -> Option<TypeDescriptor> {
        let mut current = descriptor.clone();
        for part in path {
            match (&current.shape, part) {
                (TypeShape::Aggregate(layout), AggregatePathPart::Member(name)) => {
                    let member = layout.members.iter().find(|member| member.name == *name)?;
                    current = member.descriptor.clone();
                }
                (
                    TypeShape::FixedArray {
                        dimensions,
                        element,
                    },
                    AggregatePathPart::Index(_),
                ) => {
                    if dimensions.len() <= 1 {
                        current = element.as_ref().clone();
                    } else {
                        current = TypeDescriptor {
                            two_state: current.two_state,
                            id: current.id,
                            name: current.name.clone(),
                            info: current.info.clone(),
                            shape: TypeShape::FixedArray {
                                dimensions: dimensions[1..].to_vec(),
                                element: element.clone(),
                            },
                        };
                    }
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Compare two complete fixed unpacked aggregate values without
    /// flattening their storage into one packed expression.  Each leaf keeps
    /// its owned representation: packed leaves retain four-state comparison,
    /// real leaves use real comparison, and string leaves compare the owned
    /// runtime objects.  The aggregate result is the logical conjunction of
    /// leaf equalities; `!=`/case-`!=` invert that result after all leaves have
    /// participated, preserving unknown propagation for packed values.
    pub(super) fn lower_unpacked_aggregate_comparison(
        &self,
        path: &str,
        op: Operation,
        operands: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        if !matches!(
            op,
            Operation::Equal | Operation::NotEqual | Operation::CaseEqual | Operation::CaseNotEqual
        ) {
            return Ok(None);
        }
        let [lhs, rhs] = operands else {
            return Ok(None);
        };
        let left = self.unpacked_aggregate_info(*lhs);
        let right = self.unpacked_aggregate_info(*rhs);
        if left.is_none() && right.is_none() {
            return Ok(None);
        }
        let (left_target, left_aggregate) = left
            .ok_or_else(|| format!("aggregate equality has a non-aggregate operand in `{path}`"))?;
        let (right_target, right_aggregate) = right
            .ok_or_else(|| format!("aggregate equality has a non-aggregate operand in `{path}`"))?;
        let compatible = match (
            left_aggregate.type_identity.as_deref(),
            right_aggregate.type_identity.as_deref(),
        ) {
            (Some(left), Some(right)) => left == right,
            (None, None) => left_target == right_target,
            _ => false,
        } && left_aggregate.kind == right_aggregate.kind
            && left_aggregate.members.len() == right_aggregate.members.len()
            && left_aggregate
                .members
                .iter()
                .zip(&right_aggregate.members)
                .all(|(left, right)| {
                    left.member.name == right.member.name
                        && left.member.descriptor == right.member.descriptor
                });
        if !compatible {
            return Err(format!(
                "aggregate equality compares incompatible types `{}` and `{}` in `{path}`",
                self.node(left_target).name,
                self.node(right_target).name
            ));
        }

        let compare = |op: IrBinOp, left: IrExpr, right: IrExpr| {
            if op == IrBinOp::CaseEq || op == IrBinOp::CaseNeq {
                if left.is_real() || right.is_real() {
                    return Err(format!(
                        "case equality on real aggregate member in `{path}` is not supported"
                    ));
                }
                Ok(cmp_expr_ir(op, left, right))
            } else {
                common_cmp_expr_ir(op, left, right, path)
            }
        };
        let equality = if left_aggregate.kind == AggregateKind::UnpackedUnion {
            let left = left_aggregate.leaves.first().ok_or_else(|| {
                format!(
                    "unpacked union `{}` has no members",
                    self.node(left_target).name
                )
            })?;
            let right = right_aggregate.leaves.first().ok_or_else(|| {
                format!(
                    "unpacked union `{}` has no members",
                    self.node(right_target).name
                )
            })?;
            let left = left.signal.as_ref().ok_or_else(|| {
                format!(
                    "unpacked union `{}` has no packed storage",
                    self.node(left_target).name
                )
            })?;
            let right = right.signal.as_ref().ok_or_else(|| {
                format!(
                    "unpacked union `{}` has no packed storage",
                    self.node(right_target).name
                )
            })?;
            compare(
                if matches!(op, Operation::CaseEqual | Operation::CaseNotEqual) {
                    IrBinOp::CaseEq
                } else {
                    IrBinOp::Eq
                },
                self.signal_read_expr(left)?,
                self.signal_read_expr(right)?,
            )?
        } else {
            let mut equality = None;
            for left in &left_aggregate.leaves {
                let right = right_aggregate
                    .leaves
                    .iter()
                    .find(|right| right.path == left.path)
                    .ok_or_else(|| {
                        format!(
                            "aggregate equality path `{}` is missing in `{path}`",
                            aggregate_path_suffix(&left.path)
                        )
                    })?;
                let member_equal = match (left.object, right.object) {
                    (Some(left), Some(right)) => {
                        let left_index = self.reference_object(left);
                        let right_index = self.reference_object(right);
                        if matches!(
                            self.model.objects[left_index].ty,
                            IrObjectType::Chandle | IrObjectType::Semaphore
                        ) {
                            if !matches!(
                                self.model.objects[right_index].ty,
                                IrObjectType::Chandle | IrObjectType::Semaphore
                            ) {
                                return Err(format!(
                                    "aggregate equality has mismatched object members in `{path}`"
                                ));
                            }
                            object_query(
                                IrObjectQuery::ChandleEq(
                                    IrChandleExpr::Read(left_index),
                                    IrChandleExpr::Read(right_index),
                                ),
                                1,
                                false,
                            )
                        } else {
                            let compare = IrExpr::new(
                                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringCompare(
                                    IrStringExpr::Read(left_index),
                                    IrStringExpr::Read(right_index),
                                    false,
                                ))),
                                32,
                                true,
                                None,
                            );
                            let zero = IrExpr::new(
                                IrExprKind::Const(
                                    IrConst::packed(vec![0], vec![], vec![], 32, true, None)
                                        .map_err(|error| error.to_string())?,
                                ),
                                32,
                                true,
                                None,
                            );
                            cmp_expr_ir(IrBinOp::Eq, compare, zero)
                        }
                    }
                    (None, None) => compare(
                        if matches!(op, Operation::CaseEqual | Operation::CaseNotEqual) {
                            IrBinOp::CaseEq
                        } else {
                            IrBinOp::Eq
                        },
                        self.aggregate_leaf_read(left)?,
                        self.aggregate_leaf_read(right)?,
                    )?,
                    _ => {
                        return Err(format!(
                            "aggregate equality has mismatched object/scalar member `{}` in `{path}`",
                            aggregate_path_suffix(&left.path)
                        ));
                    }
                };
                equality = Some(match equality {
                    Some(previous) => cmp_expr_ir(IrBinOp::LogAnd, previous, member_equal),
                    None => member_equal,
                });
            }
            equality.ok_or_else(|| format!("aggregate equality has no value leaves in `{path}"))?
        };
        Ok(Some(
            if matches!(op, Operation::NotEqual | Operation::CaseNotEqual) {
                IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::LogNot,
                        a: Box::new(equality),
                    },
                    1,
                    false,
                    None,
                )
            } else {
                equality
            },
        ))
    }

    pub(in super::super) fn aggregate_leaf_lhs(
        &self,
        leaf: &AggregateMemberInfo,
    ) -> Result<IrLhs, String> {
        let signal = leaf.signal.as_ref().ok_or_else(|| {
            format!(
                "aggregate member `{}` is not a packed or real assignment target",
                aggregate_path_suffix(&leaf.path)
            )
        })?;
        if signal.real {
            self.reference_lhs(IrLhs::Whole(signal.ir))
        } else {
            let width = leaf.member.ty.width.ok_or_else(|| {
                format!(
                    "aggregate member `{}` has unresolved width",
                    aggregate_path_suffix(&leaf.path)
                )
            })?;
            self.reference_lhs(IrLhs::Part(
                signal.ir,
                i64::from(width - 1),
                0,
                leaf.member.two_state,
            ))
        }
    }

    pub(in super::super) fn aggregate_leaf_read(
        &self,
        leaf: &AggregateMemberInfo,
    ) -> Result<IrExpr, String> {
        let signal = leaf.signal.as_ref().ok_or_else(|| {
            format!(
                "aggregate member `{}` is not a packed or real expression",
                aggregate_path_suffix(&leaf.path)
            )
        })?;
        let value = self.signal_read_expr(signal)?;
        Ok(if signal.real {
            value
        } else {
            IrExpr::resize_to(
                value,
                leaf.member.ty.width.ok_or_else(|| {
                    format!(
                        "aggregate member `{}` has unresolved width",
                        aggregate_path_suffix(&leaf.path)
                    )
                })?,
                leaf.member.ty.signed,
            )
        })
    }

    pub(in super::super) fn aggregate_pattern_leaf_values(
        &self,
        path: &str,
        node: NodeId,
        layout: &crate::core::db::AggregateLayout,
        prefix: &[AggregatePathPart],
        out: &mut Vec<(Vec<AggregatePathPart>, NodeId)>,
    ) -> Result<(), String> {
        let values = self.aggregate_pattern_values(path, node, layout)?;
        for (index, value) in values {
            let member = layout.members.get(index).ok_or_else(|| {
                format!("aggregate pattern member index {index} is out of bounds in `{path}`")
            })?;
            let mut member_path = prefix.to_vec();
            member_path.push(AggregatePathPart::Member(member.name.clone()));
            self.aggregate_descriptor_pattern_values(
                path,
                value,
                &member.descriptor,
                &member_path,
                out,
            )?;
        }
        Ok(())
    }

    pub(in super::super) fn aggregate_descriptor_pattern_values(
        &self,
        path: &str,
        node: NodeId,
        descriptor: &TypeDescriptor,
        prefix: &[AggregatePathPart],
        out: &mut Vec<(Vec<AggregatePathPart>, NodeId)>,
    ) -> Result<(), String> {
        // Typed aggregate casts wrap the assignment-pattern operation in the
        // owned Slang graph.  The cast supplies the destination type; it does
        // not turn the nested pattern into a scalar default for every leaf.
        // Peel only casts whose eventual operand is an assignment pattern so
        // ordinary scalar casts remain value expressions.
        let pattern_node = self.unwrap_assignment_pattern_cast(node);
        if !matches!(
            self.kind(pattern_node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                ..
            })
        ) && matches!(
            descriptor.shape,
            TypeShape::Aggregate(_) | TypeShape::FixedArray { .. }
        ) && self
            .query_descriptor(node)
            .is_some_and(|source| source.id == descriptor.id)
        {
            out.push((prefix.to_vec(), node));
            return Ok(());
        }
        match &descriptor.shape {
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) =>
            {
                out.push((prefix.to_vec(), node));
                Ok(())
            }
            TypeShape::Aggregate(layout) => {
                if matches!(
                    self.kind(pattern_node),
                    NodeKind::Expr(ExprKind::Operation { op, .. })
                        if *op == Operation::AssignmentPattern
                ) {
                    self.aggregate_pattern_leaf_values(path, pattern_node, layout, prefix, out)
                } else {
                    self.aggregate_descriptor_default_values(
                        path,
                        pattern_node,
                        descriptor,
                        prefix,
                        out,
                    )
                }
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let (left, right) = dimensions.first().copied().ok_or_else(|| {
                    format!("fixed array pattern has no captured bounds in `{path}`")
                })?;
                let next = if dimensions.len() == 1 {
                    element.as_ref().clone()
                } else {
                    TypeDescriptor {
                        two_state: descriptor.two_state,
                        id: descriptor.id,
                        name: descriptor.name.clone(),
                        info: descriptor.info.clone(),
                        shape: TypeShape::FixedArray {
                            dimensions: dimensions[1..].to_vec(),
                            element: element.clone(),
                        },
                    }
                };
                if !matches!(
                    self.kind(pattern_node),
                    NodeKind::Expr(ExprKind::Operation { op, .. })
                        if *op == Operation::AssignmentPattern
                ) {
                    // A scalar default applied to an array member recurses
                    // through every element, so the full array descriptor
                    // (not the element) drives the fan-out.
                    return self.aggregate_descriptor_default_values(
                        path,
                        pattern_node,
                        descriptor,
                        prefix,
                        out,
                    );
                }
                let values =
                    self.fixed_pattern_operands(path, pattern_node, (left, right), &next)?;
                for (offset, value) in values.into_iter().enumerate() {
                    let index = if left >= right {
                        left - i32::try_from(offset).map_err(|_| {
                            format!("fixed array pattern index overflow in `{path}`")
                        })?
                    } else {
                        left + i32::try_from(offset).map_err(|_| {
                            format!("fixed array pattern index overflow in `{path}`")
                        })?
                    };
                    let mut element_path = prefix.to_vec();
                    element_path.push(AggregatePathPart::Index(index));
                    self.aggregate_descriptor_pattern_values(
                        path,
                        value,
                        &next,
                        &element_path,
                        out,
                    )?;
                }
                Ok(())
            }
            _ => {
                out.push((prefix.to_vec(), node));
                Ok(())
            }
        }
    }

    fn unwrap_assignment_pattern_cast(&self, node: NodeId) -> NodeId {
        let mut operand = node;
        while let NodeKind::Expr(ExprKind::Cast { operand: next, .. }) = self.kind(operand) {
            operand = *next;
        }
        if matches!(
            self.kind(operand),
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == Operation::AssignmentPattern
        ) {
            operand
        } else {
            node
        }
    }

    /// Expand a scalar/default value through every recursive leaf while
    /// retaining the original source node for single-evaluation lowering.
    fn aggregate_descriptor_default_values(
        &self,
        path: &str,
        node: NodeId,
        descriptor: &TypeDescriptor,
        prefix: &[AggregatePathPart],
        out: &mut Vec<(Vec<AggregatePathPart>, NodeId)>,
    ) -> Result<(), String> {
        match &descriptor.shape {
            TypeShape::Aggregate(layout) => {
                if matches!(
                    layout.kind,
                    AggregateKind::PackedUnion | AggregateKind::UnpackedUnion
                ) {
                    return Err(format!(
                        "untagged union assignment pattern in `{path}` must select exactly one member"
                    ));
                }
                for member in &layout.members {
                    let mut member_path = prefix.to_vec();
                    member_path.push(AggregatePathPart::Member(member.name.clone()));
                    self.aggregate_descriptor_default_values(
                        path,
                        node,
                        &member.descriptor,
                        &member_path,
                        out,
                    )?;
                }
                Ok(())
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let (left, right) = dimensions.first().copied().ok_or_else(|| {
                    format!("fixed array pattern has no captured bounds in `{path}`")
                })?;
                let next = if dimensions.len() == 1 {
                    element.as_ref().clone()
                } else {
                    TypeDescriptor {
                        two_state: descriptor.two_state,
                        id: descriptor.id,
                        name: descriptor.name.clone(),
                        info: descriptor.info.clone(),
                        shape: TypeShape::FixedArray {
                            dimensions: dimensions[1..].to_vec(),
                            element: element.clone(),
                        },
                    }
                };
                let count = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
                let count = usize::try_from(count)
                    .map_err(|_| format!("fixed array pattern is too large in `{path}`"))?;
                for offset in 0..count {
                    let offset = i32::try_from(offset)
                        .map_err(|_| format!("fixed array pattern index overflow in `{path}`"))?;
                    let index = if left >= right {
                        left - offset
                    } else {
                        left + offset
                    };
                    let mut element_path = prefix.to_vec();
                    element_path.push(AggregatePathPart::Index(index));
                    self.aggregate_descriptor_default_values(
                        path,
                        node,
                        &next,
                        &element_path,
                        out,
                    )?;
                }
                Ok(())
            }
            _ => {
                out.push((prefix.to_vec(), node));
                Ok(())
            }
        }
    }

    fn fixed_pattern_operands(
        &self,
        path: &str,
        node: NodeId,
        bounds: (i32, i32),
        element: &TypeDescriptor,
    ) -> Result<Vec<NodeId>, String> {
        let (left, right) = bounds;
        let count = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
        let count = usize::try_from(count)
            .map_err(|_| format!("fixed array pattern is too large in `{path}`"))?;
        let NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            reordered,
            ..
        }) = self.kind(node)
        else {
            return Err(format!(
                "array initializer in `{path}` is not an assignment pattern"
            ));
        };
        if *op != Operation::AssignmentPattern {
            return Err(format!(
                "array initializer in `{path}` is not an assignment pattern"
            ));
        }
        let mut values = operands.clone();
        if *reordered {
            values.reverse();
        }
        let tagged = values.iter().any(|value| {
            matches!(
                self.kind(*value),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        });
        if !tagged {
            if values.len() != count {
                return Err(format!(
                    "array assignment pattern in `{path}` has {} positional values; expected {count}",
                    values.len()
                ));
            }
            return Ok(values);
        }
        if values.iter().any(|value| {
            !matches!(
                self.kind(*value),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        }) {
            return Err(format!(
                "mixed positional and keyed array assignment pattern in `{path}` is not supported"
            ));
        }

        let mut explicit = vec![None; count];
        let mut type_values = Vec::<(crate::core::db::AssignmentPatternKeyType, NodeId)>::new();
        let mut default = None;
        for operand in values {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key,
                key_type,
                value,
            }) = self.kind(operand)
            else {
                continue;
            };
            let key = key.as_deref().ok_or_else(|| {
                format!("array assignment pattern key is unavailable in `{path}`")
            })?;
            let value = value.ok_or_else(|| {
                format!("array assignment pattern key `{key}` has no value in `{path}`")
            })?;
            if key == "default" {
                if default.replace(value).is_some() {
                    return Err(format!(
                        "duplicate default key in array assignment pattern in `{path}`"
                    ));
                }
                continue;
            }
            if let Some(index) = Self::parse_pattern_index(key) {
                let offset = if left >= right {
                    i64::from(left) - i64::from(index)
                } else {
                    i64::from(index) - i64::from(left)
                };
                let Some(offset) = usize::try_from(offset)
                    .ok()
                    .filter(|offset| *offset < count)
                else {
                    return Err(format!(
                        "array assignment pattern index `{key}` is out of bounds in `{path}`"
                    ));
                };
                if explicit[offset].replace(value).is_some() {
                    return Err(format!(
                        "duplicate array assignment pattern index `{key}` in `{path}`"
                    ));
                }
                continue;
            }
            let Some(key_type) = key_type else {
                return Err(format!(
                    "array assignment pattern key `{key}` has no matching index or type in `{path}`"
                ));
            };
            if !super::super::collection::pattern_key_matches_descriptor(
                key_type,
                element,
                Self::descriptor_two_state(element),
                None,
            ) {
                return Err(format!(
                    "array assignment pattern key `{key}` has no matching index or type in `{path}`"
                ));
            }
            if type_values.iter().any(|(previous, _)| {
                super::super::collection::pattern_key_types_equal(previous, key_type)
            }) {
                return Err(format!(
                    "duplicate array assignment pattern type key `{key}` in `{path}`"
                ));
            }
            type_values.push((key_type.clone(), value));
        }

        let mut resolved = Vec::with_capacity(count);
        for (offset, explicit_value) in explicit.iter().copied().enumerate().take(count) {
            let value = explicit_value
                .or_else(|| {
                    type_values.iter().rev().find_map(|(key_type, value)| {
                        super::super::collection::pattern_key_matches_descriptor(
                            key_type,
                            element,
                            Self::descriptor_two_state(element),
                            None,
                        )
                        .then_some(*value)
                    })
                })
                .or(default);
            let Some(value) = value else {
                let offset = i32::try_from(offset).unwrap_or(i32::MAX);
                let index = if left >= right {
                    left - offset
                } else {
                    left + offset
                };
                return Err(format!(
                    "array assignment pattern in `{path}` does not cover index `{index}`"
                ));
            };
            resolved.push(value);
        }
        Ok(resolved)
    }

    fn parse_pattern_index(key: &str) -> Option<i32> {
        let key = key.trim();
        let key = key
            .strip_prefix('[')
            .and_then(|key| key.strip_suffix(']'))
            .unwrap_or(key)
            .trim();
        key.parse::<i32>().ok()
    }

    fn descriptor_two_state(descriptor: &TypeDescriptor) -> bool {
        matches!(
            descriptor.info.kind.as_str(),
            "bit" | "byte" | "shortint" | "int" | "longint" | "time"
        )
    }

    /// Convert a pre-IR [`Lhs`] to its [`IrLhs`] form using the registered
    /// model indices.
    pub(in super::super) fn lhs_to_ir(&self, lh: Lhs) -> Result<IrLhs, String> {
        Ok(match lh {
            Lhs::Whole(info) => self.reference_lhs(IrLhs::Whole(info.ir))?,
            Lhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                shortreal,
            } => IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                shortreal,
            },
            Lhs::Ref {
                addr,
                width,
                signed,
                two_state,
                const_ref,
            } => IrLhs::Ref {
                addr,
                width,
                signed,
                two_state,
                const_ref,
                bit: None,
            },
            Lhs::Canonical(lhs) => self.reference_lhs(lhs)?,
            Lhs::Bit(info, index, two_state) => {
                self.reference_lhs(IrLhs::Bit(info.ir, index, two_state))?
            }
            Lhs::Part(info, left, right, two_state) => {
                let (left, right, _) =
                    checked_select_bounds(left, right, "assignment part select")?;
                self.reference_lhs(IrLhs::Part(info.ir, left, right, two_state))?
            }
            Lhs::IdxPart(info, base, width_expr, width, neg, two_state) => self.reference_lhs(
                IrLhs::IdxPart(info.ir, base, width_expr, width, neg, two_state),
            )?,
            Lhs::ArrayElem(ae) => self.reference_lhs(IrLhs::ArrayElem {
                arr: self.reference_array(ae.arr.ir),
                indices: ae.indices,
                elem_sel: match ae.elem_sel {
                    ElemSel::Whole => IrElemSel::Whole,
                    ElemSel::Part(l, r) => {
                        let (left, right, _) =
                            checked_select_bounds(l, r, "array-element assignment part select")?;
                        IrElemSel::Part(left, right)
                    }
                    ElemSel::Bit(index) => IrElemSel::Bit(Box::new(index)),
                    ElemSel::Indexed(base, width, negative) => IrElemSel::Indexed {
                        base: Box::new(base),
                        width,
                        negative,
                    },
                },
            })?,
            Lhs::Stream {
                parts,
                slice,
                direction,
            } => {
                let mut ir_parts = Vec::with_capacity(parts.len());
                let mut width = 0u32;
                for part in parts {
                    let part = self.lhs_to_ir(part)?;
                    let part_width = packed_lhs_width(&self.model, &part).ok_or_else(|| {
                        "streaming assignment target has no packed width".to_string()
                    })?;
                    width = width.checked_add(part_width).ok_or_else(|| {
                        "streaming assignment target width exceeds the supported range".to_string()
                    })?;
                    if width > LLG_MAX_WIDTH {
                        return Err(format!(
                            "streaming assignment target is {width} bits wide; the runtime \
                             maximum supported width is {LLG_MAX_WIDTH}"
                        ));
                    }
                    ir_parts.push((part, part_width));
                }
                if width == 0 {
                    return Err("empty streaming assignment target".to_string());
                }
                let slice = slice
                    .unwrap_or(1)
                    .min(u128::from(width))
                    .try_into()
                    .expect("packed stream width fits u32");
                IrLhs::Stream {
                    parts: ir_parts,
                    width,
                    slice,
                    direction,
                }
            }
        })
    }
}
