//! Fixed arrays.

use super::*;

#[cfg(test)]
mod tests;

/// One target cell's source in a flattened fixed-array assignment pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum P30PatternSource {
    /// A leaf expression; a scalar `default:` value repeats for every cell.
    Leaf(NodeId),
    /// Declaration-order element `index` of an array-valued item covering
    /// the remaining target dimensions `dims` (SV 10.9.1 positional items).
    Element {
        node: NodeId,
        index: usize,
        dims: Vec<(i32, i32)>,
    },
}

impl<'a> Codegen<'a> {
    // Fixed unpacked-array assignment (P30).

    /// Lower a positional assignment-pattern lvalue (SV 10.9).  Slang
    /// represents each lvalue position as an assignment operation whose RHS
    /// is an `EmptyArgument`; the pattern itself therefore needs a small
    /// target-side walk before the ordinary lvalue lowerer can be used.
    ///
    /// The source values are lowered through the existing fixed-array source
    /// path. Procedural destinations are frozen as a group before any scatter
    /// write, so an earlier output cannot change a later output's selector.
    /// Continuous destinations retain their constant topology for net mapping.
    pub(in super::super) fn lower_p30_pattern_lvalue_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        kind: PatternAssignmentKind,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let blocking = kind != PatternAssignmentKind::Nonblocking;
        let pattern = self.p30_unwrap_cast(lhs);
        let NodeKind::Expr(ExprKind::Operation { op: pattern_op, .. }) = self.kind(pattern) else {
            return Ok(None);
        };
        if !matches!(
            pattern_op,
            Operation::AssignmentPattern | Operation::MultiAssignmentPattern
        ) {
            return Ok(None);
        }
        if *pattern_op == Operation::MultiAssignmentPattern {
            return Err(format!(
                "replicated assignment-pattern lvalue in `{path}` is not supported"
            ));
        }
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment of a positional assignment-pattern lvalue in `{path}` is not supported"
            ));
        }

        let target_descriptor = self.query_descriptor(lhs).cloned().ok_or_else(|| {
            format!("positional assignment-pattern lvalue in `{path}` has no owned target type")
        })?;
        let source_descriptor = self.query_descriptor(rhs).cloned().ok_or_else(|| {
            format!("positional assignment-pattern RHS in `{path}` has no owned source type")
        })?;
        self.p30_require_pattern_shape(
            path,
            &target_descriptor,
            &source_descriptor,
            "assignment-pattern lvalue",
        )?;

        let mut targets = Vec::new();
        self.p30_collect_pattern_lvalue_targets(path, pattern, &target_descriptor, &mut targets)?;
        // Oversized sources scatter descriptor rows for every assignment
        // kind; continuous scatters keep their constant row topology.
        if let Some(statement) =
            self.lower_descriptor_pattern_scatter(path, rhs, &targets, !blocking)?
        {
            return Ok(Some(statement));
        }
        let mut lowered_targets = Vec::with_capacity(targets.len());
        let mut target_widths = Vec::with_capacity(targets.len());
        for (target, descriptor) in targets {
            if !blocking
                && (self.proc_local_target(target).is_some() || self.subroutine_auto_target(target))
            {
                return Err(format!(
                    "nonblocking assignment to an automatic assignment-pattern target in `{path}` is not supported"
                ));
            }
            let lowered = self.lower_lhs(path, target)?;
            if !blocking && matches!(lowered, IrLhs::Ref { .. }) {
                return Err(format!(
                    "nonblocking assignment through a reference formal in `{path}` is not supported"
                ));
            }
            let width = packed_lhs_width(&self.model, &lowered).ok_or_else(|| {
                format!("assignment-pattern lvalue target in `{path}` is not a packed value target")
            })?;
            let expected = Self::fixed_descriptor_width(&descriptor).ok_or_else(|| {
                format!("assignment-pattern lvalue target in `{path}` has unresolved element width")
            })?;
            if width != expected {
                return Err(format!(
                    "assignment-pattern lvalue target in `{path}` is {width} bits wide; expected {expected}"
                ));
            }
            target_widths.push((expected, descriptor.info.signed));
            lowered_targets.push(lowered);
        }

        let mut captures = Vec::new();
        let mut captured_indices = HashMap::new();
        let source_values = if let TypeShape::FixedArray { dimensions, .. } =
            &source_descriptor.shape
        {
            self.p30_lower_source_values(
                path,
                lhs,
                rhs,
                dimensions,
                &mut captures,
                &mut captured_indices,
            )?
        } else {
            let value = self.lower_expr(path, rhs)?;
            let expected = Self::fixed_descriptor_width(&source_descriptor).ok_or_else(|| {
                format!("assignment-pattern lvalue RHS in `{path}` has unresolved width")
            })?;
            if value.width != expected || value.is_real() {
                return Err(format!(
                    "assignment-pattern lvalue RHS in `{path}` has width {}; expected {expected}",
                    value.width
                ));
            }
            vec![self.p30_capture_value(lhs, rhs, 0, value, &mut captures)]
        };
        let values = Self::p30_split_pattern_source_values(path, source_values, &target_widths)?;
        if values.len() != lowered_targets.len() {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` has {} targets but RHS supplies {} values",
                lowered_targets.len(),
                values.len()
            ));
        }
        if kind != PatternAssignmentKind::Continuous {
            let tag = self.new_fn_name(path, "pattern_targets");
            lowered_targets =
                self.capture_pattern_lvalue_targets(lowered_targets, &tag, &mut captures)?;
        }
        for (target, value) in lowered_targets.into_iter().zip(values) {
            let value = apply_lhs_assignment_context(&self.model, &target, value);
            captures.push(IrStmt::Assign {
                lhs: target,
                rhs: value,
                nba: !blocking,
            });
        }
        Ok(Some(IrStmt::Block(captures)))
    }

    fn capture_pattern_lvalue_targets(
        &self,
        targets: Vec<IrLhs>,
        tag: &str,
        statements: &mut Vec<IrStmt>,
    ) -> Result<Vec<IrLhs>, String> {
        let mut captures = Vec::new();
        let mut sequence = 0;
        let mut frozen = Vec::new();
        frozen
            .try_reserve_exact(targets.len())
            .map_err(|_| "cannot allocate assignment-pattern target captures".to_owned())?;
        for target in targets {
            let (target, _) = self.freeze_call_lhs(target, tag, &mut sequence, &mut captures)?;
            frozen.push(target);
        }
        statements.extend(
            captures
                .into_iter()
                .map(|(name, width, signed, two_state, expr)| IrStmt::DeclLocal {
                    name,
                    width,
                    signed,
                    two_state,
                    init: Some(Box::new(expr)),
                }),
        );
        Ok(frozen)
    }

    /// Lower the ordered writable leaves of a positional assignment-pattern
    /// lvalue. Call and port output bindings use the same target order when
    /// copying a packed formal value back to the caller.
    pub(in super::super) fn positional_pattern_lvalue_parts(
        &mut self,
        path: &str,
        lhs: NodeId,
    ) -> Result<Option<Vec<Lhs>>, String> {
        let pattern = self.p30_unwrap_cast(lhs);
        let NodeKind::Expr(ExprKind::Operation { op, .. }) = self.kind(pattern) else {
            return Ok(None);
        };
        if !matches!(
            op,
            Operation::AssignmentPattern | Operation::MultiAssignmentPattern
        ) {
            return Ok(None);
        }
        if *op == Operation::MultiAssignmentPattern {
            return Err(format!(
                "replicated assignment-pattern lvalue in `{path}` is not supported"
            ));
        }

        let descriptor = self.query_descriptor(lhs).cloned().ok_or_else(|| {
            format!("positional assignment-pattern lvalue in `{path}` has no owned target type")
        })?;
        let mut targets = Vec::new();
        self.p30_collect_pattern_lvalue_targets(path, pattern, &descriptor, &mut targets)?;
        let mut parts = Vec::with_capacity(targets.len());
        for (target, _) in targets {
            parts.push(self.analyze_lhs(path, target)?);
        }
        Ok(Some(parts))
    }

    /// Return the ordered leaf target nodes of a positional assignment-pattern
    /// lvalue, matching the order of the leaf assignments that
    /// [`Self::lower_p30_pattern_lvalue_assignment`] emits.
    pub(in super::super) fn positional_pattern_lvalue_targets(
        &self,
        path: &str,
        lhs: NodeId,
    ) -> Result<Vec<NodeId>, String> {
        let pattern = self.p30_unwrap_cast(lhs);
        let descriptor = self.query_descriptor(lhs).cloned().ok_or_else(|| {
            format!("positional assignment-pattern lvalue in `{path}` has no owned target type")
        })?;
        let mut targets = Vec::new();
        self.p30_collect_pattern_lvalue_targets(path, pattern, &descriptor, &mut targets)?;
        Ok(targets.into_iter().map(|(target, _)| target).collect())
    }

    fn p30_collect_pattern_lvalue_targets(
        &self,
        path: &str,
        node: NodeId,
        descriptor: &TypeDescriptor,
        out: &mut Vec<(NodeId, TypeDescriptor)>,
    ) -> Result<(), String> {
        let pattern = self.p30_unwrap_cast(node);
        let Some(count) = Self::p30_pattern_component_count(descriptor) else {
            out.push((node, descriptor.clone()));
            return Ok(());
        };
        let Some(operands) = self.assignment_pattern_operands(path, pattern)? else {
            return Err(format!(
                "positional assignment-pattern lvalue in `{path}` must contain only positional targets"
            ));
        };
        let NodeKind::Expr(ExprKind::Operation { op, .. }) = self.kind(pattern) else {
            unreachable!("assignment-pattern operands checked above");
        };
        if *op != Operation::AssignmentPattern {
            return Err(format!(
                "replicated assignment-pattern lvalue in `{path}` is not supported"
            ));
        }
        if operands.len() != count {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` has {} positional targets; expected {count}",
                operands.len()
            ));
        }
        for (offset, operand) in operands.into_iter().enumerate() {
            let next = Self::p30_pattern_component_descriptor(descriptor, offset)
                .ok_or_else(|| format!("assignment-pattern lvalue shape is invalid in `{path}`"))?;
            let target = self.p30_pattern_lvalue_operand(path, operand)?;
            let target_pattern = self.p30_unwrap_cast(target);
            let nested_pattern = matches!(
                self.kind(target_pattern),
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::AssignmentPattern | Operation::MultiAssignmentPattern,
                    ..
                })
            );
            if nested_pattern && Self::p30_pattern_component_count(&next).is_some() {
                self.p30_collect_pattern_lvalue_targets(path, target, &next, out)?;
            } else {
                if nested_pattern {
                    return Err(format!(
                        "assignment-pattern lvalue nesting in `{path}` does not match its target type"
                    ));
                }
                out.push((target, next.clone()));
            }
        }
        Ok(())
    }

    pub(in super::super) fn p30_pattern_lvalue_operand(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<NodeId, String> {
        let NodeKind::Expr(ExprKind::Operation {
            op: Operation::Assignment,
            assignment: true,
            operands,
            ..
        }) = self.kind(node)
        else {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` contains a keyed/default or malformed target"
            ));
        };
        let [target, empty] = operands.as_slice() else {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` contains a malformed target"
            ));
        };
        let mut empty = *empty;
        while let NodeKind::Expr(ExprKind::Cast { operand, .. }) = self.kind(empty) {
            empty = *operand;
        }
        if !matches!(self.kind(empty), NodeKind::Expr(ExprKind::Other))
            || self.db.semantic_detail(empty) != Some("EmptyArgument")
        {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` contains a keyed/default or malformed target"
            ));
        }
        Ok(*target)
    }

    fn p30_require_pattern_shape(
        &self,
        path: &str,
        target: &TypeDescriptor,
        source: &TypeDescriptor,
        label: &str,
    ) -> Result<(), String> {
        match (&target.shape, &source.shape) {
            (
                TypeShape::FixedArray {
                    dimensions: target_dims,
                    element: target_element,
                },
                TypeShape::FixedArray {
                    dimensions: source_dims,
                    element: source_element,
                },
            ) => {
                if target_dims.len() != source_dims.len()
                    || target_dims.iter().zip(source_dims).any(|(left, right)| {
                        (i64::from(left.0) - i64::from(left.1)).unsigned_abs()
                            != (i64::from(right.0) - i64::from(right.1)).unsigned_abs()
                    })
                {
                    return Err(format!(
                        "{label} in `{path}` has incompatible fixed-array shape"
                    ));
                }
                self.p30_require_pattern_shape(path, target_element, source_element, label)
            }
            (TypeShape::Aggregate(target_layout), TypeShape::Aggregate(source_layout))
                if matches!(
                    target_layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::UnpackedStruct
                ) && target_layout.kind == source_layout.kind =>
            {
                if target_layout.members.len() != source_layout.members.len() {
                    return Err(format!(
                        "{label} in `{path}` has incompatible structure shape"
                    ));
                }
                for (target_member, source_member) in
                    target_layout.members.iter().zip(&source_layout.members)
                {
                    self.p30_require_pattern_shape(
                        path,
                        &target_member.descriptor,
                        &source_member.descriptor,
                        label,
                    )?;
                }
                Ok(())
            }
            (TypeShape::Aggregate(target_layout), TypeShape::Aggregate(source_layout))
                if target_layout.kind == AggregateKind::PackedUnion
                    && source_layout.kind == AggregateKind::PackedUnion =>
            {
                let same_type = match (
                    target_layout.type_identity.as_deref(),
                    source_layout.type_identity.as_deref(),
                ) {
                    (Some(target), Some(source)) => target == source,
                    _ => {
                        target_layout.type_id.is_some()
                            && target_layout.type_id == source_layout.type_id
                    }
                };
                if !same_type {
                    return Err(format!(
                        "{label} in `{path}` has incompatible packed-union type identity"
                    ));
                }
                let target_width = Self::fixed_descriptor_width(target)
                    .ok_or_else(|| format!("{label} in `{path}` has unresolved target width"))?;
                let source_width = Self::fixed_descriptor_width(source)
                    .ok_or_else(|| format!("{label} in `{path}` has unresolved source width"))?;
                if target_width != source_width {
                    return Err(format!(
                        "{label} in `{path}` has incompatible packed-union widths ({target_width} versus {source_width})"
                    ));
                }
                Ok(())
            }
            (TypeShape::PackedAtom { .. }, TypeShape::PackedAtom { .. }) => {
                let target_width = Self::fixed_descriptor_width(target)
                    .ok_or_else(|| format!("{label} in `{path}` has unresolved target width"))?;
                let source_width = Self::fixed_descriptor_width(source)
                    .ok_or_else(|| format!("{label} in `{path}` has unresolved source width"))?;
                let target_ranges = match &target.shape {
                    TypeShape::PackedAtom { ranges } => ranges,
                    _ => unreachable!(),
                };
                let source_ranges = match &source.shape {
                    TypeShape::PackedAtom { ranges } => ranges,
                    _ => unreachable!(),
                };
                if target_width != source_width
                    || target_ranges.len() != source_ranges.len()
                    || target_ranges
                        .iter()
                        .zip(source_ranges)
                        .any(|(left, right)| {
                            left.left.abs_diff(left.right) != right.left.abs_diff(right.right)
                        })
                {
                    return Err(format!(
                        "{label} in `{path}` has incompatible element widths ({target_width} versus {source_width})"
                    ));
                }
                Ok(())
            }
            _ => Err(format!("{label} in `{path}` has incompatible value shape")),
        }
    }

    fn p30_pattern_component_count(descriptor: &TypeDescriptor) -> Option<usize> {
        let extent = match &descriptor.shape {
            TypeShape::FixedArray { dimensions, .. } => {
                let (left, right) = dimensions.first()?;
                u128::from(i64::from(*left).abs_diff(i64::from(*right))) + 1
            }
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::UnpackedStruct
                ) =>
            {
                return Some(layout.members.len());
            }
            TypeShape::PackedAtom { ranges } => {
                let range = ranges.first()?;
                range.left.abs_diff(range.right).checked_add(1)?
            }
            _ => return None,
        };
        usize::try_from(extent).ok()
    }

    fn p30_pattern_component_descriptor(
        descriptor: &TypeDescriptor,
        offset: usize,
    ) -> Option<TypeDescriptor> {
        match &descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let (left, right) = dimensions.first()?;
                let count =
                    usize::try_from(u128::from(i64::from(*left).abs_diff(i64::from(*right)) + 1))
                        .ok()?;
                (offset < count).then(|| {
                    if dimensions.len() == 1 {
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
                    }
                })
            }
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::UnpackedStruct
                ) =>
            {
                layout
                    .members
                    .get(offset)
                    .map(|member| member.descriptor.clone())
            }
            TypeShape::PackedAtom { ranges } => {
                let range = ranges.first()?;
                let count = u32::try_from(range.left.abs_diff(range.right).checked_add(1)?).ok()?;
                let width = descriptor.info.width?;
                if offset >= usize::try_from(count).ok()?
                    || count == 0
                    || width == 0
                    || width % count != 0
                    || width / count == 0
                {
                    return None;
                }
                let width = width / count;
                let mut component = descriptor.clone();
                component.info.width = Some(width);
                component.shape = TypeShape::PackedAtom {
                    ranges: ranges[1..].to_vec(),
                };
                Some(component)
            }
            _ => None,
        }
    }

    fn p30_split_pattern_source_values(
        path: &str,
        source_values: Vec<IrExpr>,
        target_widths: &[(u32, bool)],
    ) -> Result<Vec<IrExpr>, String> {
        let mut values = Vec::with_capacity(target_widths.len());
        let mut target = 0usize;
        for source in source_values {
            if source.is_real() || source.width == 0 {
                return Err(format!(
                    "assignment-pattern source in `{path}` must be a nonempty packed value"
                ));
            }
            let mut cursor = source.width;
            while cursor > 0 {
                let Some((width, signed)) = target_widths.get(target).copied() else {
                    return Err(format!(
                        "assignment-pattern lvalue in `{path}` has fewer RHS positions than targets"
                    ));
                };
                if width == 0 || width > cursor {
                    return Err(format!(
                        "assignment-pattern source in `{path}` does not match target position {target}"
                    ));
                }
                cursor -= width;
                values.push(IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(source.clone()),
                        left: i64::from(cursor + width - 1),
                        right: i64::from(cursor),
                    },
                    width,
                    signed,
                    None,
                ));
                target += 1;
            }
        }
        if target != target_widths.len() {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` has {} targets but RHS supplies only {target} values",
                target_widths.len()
            ));
        }
        Ok(values)
    }

    /// A fixed-array view is represented by the complete coordinate list in
    /// logical (declared left-to-right) order.  Keeping the view as concrete
    /// coordinates lets the existing guarded ArrayRead/ArrayElem IR preserve
    /// direction, notifications, force precedence, and two-state conversion.
    pub(super) fn p30_fixed_array_assignment_candidate(&self, node: NodeId) -> bool {
        if self.array_of(node).is_some() {
            return true;
        }
        match self.kind(node) {
            NodeKind::Array { .. } | NodeKind::Expr(ExprKind::Ref { .. }) => {
                self.array_of(node).is_some()
            }
            NodeKind::Expr(ExprKind::PartSelect { base, .. }) => {
                self.p30_array_prefix_base(*base).is_some()
            }
            NodeKind::Expr(ExprKind::BitSelect { base, .. }) => {
                self.p30_array_prefix_base(*base).is_some()
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => self
                .p30_array_prefix_base(*base)
                .is_some_and(|(array, consumed)| {
                    consumed.saturating_add(indices.len()) < array.dims.len()
                }),
            _ => false,
        }
    }

    fn p30_container_source(&self, node: NodeId) -> Option<ContainerInfo> {
        if let Some(container) = self.container_of(node) {
            return Some(container);
        }
        let operand = match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => Some(*operand),
            _ => None,
        }?;
        self.p30_container_source(operand)
    }

    pub(in super::super) fn p30_unwrap_cast(&self, node: NodeId) -> NodeId {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.p30_unwrap_cast(*operand),
            _ => node,
        }
    }

    pub(in super::super) fn p30_array_prefix_base(
        &self,
        node: NodeId,
    ) -> Option<(&ArrayInfo, usize)> {
        if let Some(array) = self.array_of(node) {
            return Some((array, 0));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.p30_array_prefix_base(*operand),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let (array, consumed) = self.p30_array_prefix_base(*base)?;
                let consumed = consumed.checked_add(indices.len())?;
                (consumed < array.dims.len()).then_some((array, consumed))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, .. }) => {
                let (array, consumed) = self.p30_array_prefix_base(*base)?;
                let consumed = consumed.checked_add(1)?;
                (consumed < array.dims.len()).then_some((array, consumed))
            }
            _ => None,
        }
    }

    fn p30_index_expr(
        &mut self,
        path: &str,
        node: NodeId,
        captures: &mut Vec<IrStmt>,
        captured: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<IrExpr, String> {
        if let Some((name, width, signed)) = captured.get(&node) {
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(name.clone()),
                *width,
                *signed,
                None,
            ));
        }
        let value = self.lower_expr(path, node)?;
        if value.is_real() {
            return Err(format!(
                "fixed unpacked-array index in `{path}` must be an integral expression"
            ));
        }
        let name = format!("_p30_idx_{}_{}", node.0, captured.len());
        let width = value.width;
        let signed = value.signed;
        captures.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            init: Some(Box::new(value)),
            two_state: false,
        });
        captured.insert(node, (name.clone(), width, signed));
        Ok(IrExpr::new(
            IrExprKind::LocalRead(name),
            width,
            signed,
            None,
        ))
    }

    fn p30_const_index(value: i32) -> IrExpr {
        pattern_key_expr(i128::from(value), 32, true, false)
    }

    fn p30_append_coordinates(
        dims: &[(i32, i32)],
        prefix: &[IrExpr],
        out: &mut Vec<Vec<IrExpr>>,
    ) -> Result<(), String> {
        if dims.is_empty() {
            out.push(prefix.to_vec());
            return Ok(());
        }
        let (left, right) = dims[0];
        let count = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
        for offset in 0..count {
            let offset = i32::try_from(offset)
                .map_err(|_| "fixed unpacked-array coordinate count overflows".to_string())?;
            let index = if left >= right {
                left.checked_sub(offset)
            } else {
                left.checked_add(offset)
            }
            .ok_or_else(|| "fixed unpacked-array coordinate overflows".to_string())?;
            let mut next = prefix.to_vec();
            next.push(Self::p30_const_index(index));
            Self::p30_append_coordinates(&dims[1..], &next, out)?;
        }
        Ok(())
    }

    fn p30_view_from_prefix(array: ArrayInfo, prefix: &[IrExpr]) -> Result<P30ArrayView, String> {
        if prefix.len() > array.dims.len() {
            return Err("fixed unpacked-array index rank exceeds the declared rank".to_string());
        }
        let mut coordinates = Vec::new();
        Self::p30_append_coordinates(&array.dims[prefix.len()..], prefix, &mut coordinates)?;
        Ok(P30ArrayView { array, coordinates })
    }

    fn p30_slice_view_with_prefix(
        &self,
        path: &str,
        array: ArrayInfo,
        prefix: &[IrExpr],
        left_node: NodeId,
        right_node: NodeId,
    ) -> Result<P30ArrayView, String> {
        let left = self.eval_bound_i128(left_node).map_err(|_| {
            format!("fixed unpacked-array slice bounds in `{path}` must be constant integers")
        })?;
        let right = self.eval_bound_i128(right_node).map_err(|_| {
            format!("fixed unpacked-array slice bounds in `{path}` must be constant integers")
        })?;
        let left = i32::try_from(left).map_err(|_| {
            format!("fixed unpacked-array slice left bound is out of range in `{path}`")
        })?;
        let right = i32::try_from(right).map_err(|_| {
            format!("fixed unpacked-array slice right bound is out of range in `{path}`")
        })?;
        let Some((decl_left, decl_right)) = array.dims.get(prefix.len()).copied() else {
            return Err(format!(
                "fixed unpacked-array slice has no dimension in `{path}`"
            ));
        };
        if left < decl_left.min(decl_right)
            || left > decl_left.max(decl_right)
            || right < decl_left.min(decl_right)
            || right > decl_left.max(decl_right)
        {
            return Err(format!(
                "fixed unpacked-array slice [{left}:{right}] is outside declared bounds [{decl_left}:{decl_right}] in `{path}`"
            ));
        }
        let count = (i64::from(left) - i64::from(right)).unsigned_abs();
        let step = if left >= right { -1 } else { 1 };
        let mut coordinates = Vec::new();
        for offset in 0..count {
            let offset = i32::try_from(offset)
                .map_err(|_| format!("fixed unpacked-array slice is too large in `{path}`"))?;
            let index = left
                .checked_add(step * offset)
                .ok_or_else(|| format!("fixed unpacked-array slice overflows in `{path}`"))?;
            let mut coordinate_prefix = prefix.to_vec();
            coordinate_prefix.push(Self::p30_const_index(index));
            Self::p30_append_coordinates(
                &array.dims[prefix.len() + 1..],
                &coordinate_prefix,
                &mut coordinates,
            )?;
        }
        // A range includes both endpoints.  The loop above intentionally uses
        // the absolute difference, so add the endpoint when the range is not
        // a singleton.
        let endpoint = if count == 0 { left } else { right };
        let mut coordinate_prefix = prefix.to_vec();
        coordinate_prefix.push(Self::p30_const_index(endpoint));
        Self::p30_append_coordinates(
            &array.dims[prefix.len() + 1..],
            &coordinate_prefix,
            &mut coordinates,
        )?;
        Ok(P30ArrayView { array, coordinates })
    }

    fn p30_array_prefix(
        &mut self,
        path: &str,
        node: NodeId,
        captures: &mut Vec<IrStmt>,
        captured_indices: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<Option<(ArrayInfo, Vec<IrExpr>)>, String> {
        if let Some(array) = self.array_of(node).cloned() {
            return Ok(Some((array, Vec::new())));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.p30_array_prefix(path, *operand, captures, captured_indices)
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let Some((array, mut prefix)) =
                    self.p30_array_prefix(path, *base, captures, captured_indices)?
                else {
                    return Ok(None);
                };
                if prefix.len().saturating_add(indices.len()) >= array.dims.len() {
                    return Ok(None);
                }
                for index in indices {
                    prefix.push(self.p30_index_expr(path, *index, captures, captured_indices)?);
                }
                Ok(Some((array, prefix)))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let Some((array, mut prefix)) =
                    self.p30_array_prefix(path, *base, captures, captured_indices)?
                else {
                    return Ok(None);
                };
                if prefix.len().saturating_add(1) >= array.dims.len() {
                    return Ok(None);
                }
                prefix.push(self.p30_index_expr(path, *index, captures, captured_indices)?);
                Ok(Some((array, prefix)))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn p30_array_view(
        &mut self,
        path: &str,
        node: NodeId,
        captures: &mut Vec<IrStmt>,
        captured_indices: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<Option<P30ArrayView>, String> {
        let cast_operand = match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => Some(*operand),
            _ => None,
        };
        if let Some(operand) = cast_operand {
            return self.p30_array_view(path, operand, captures, captured_indices);
        }
        if let Some((array, prefix)) =
            self.p30_array_prefix(path, node, captures, captured_indices)?
        {
            return Ok(Some(Self::p30_view_from_prefix(array, &prefix)?));
        }
        enum Select {
            Slice {
                base: NodeId,
                left: NodeId,
                right: NodeId,
            },
            Partial {
                base: NodeId,
                indices: Vec<NodeId>,
            },
        }
        let select = match self.kind(node) {
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => Some(Select::Slice {
                base: *base,
                left: *left,
                right: *right,
            }),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => Some(Select::Partial {
                base: *base,
                indices: indices.clone(),
            }),
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => Some(Select::Partial {
                base: *base,
                indices: vec![*index],
            }),
            _ => None,
        };
        match select {
            Some(Select::Slice { base, left, right }) => {
                let Some((array, prefix)) =
                    self.p30_array_prefix(path, base, captures, captured_indices)?
                else {
                    return Ok(None);
                };
                Ok(Some(self.p30_slice_view_with_prefix(
                    path, array, &prefix, left, right,
                )?))
            }
            Some(Select::Partial { base, indices }) => {
                let Some((array, mut prefix)) =
                    self.p30_array_prefix(path, base, captures, captured_indices)?
                else {
                    return Ok(None);
                };
                if prefix.len().saturating_add(indices.len()) >= array.dims.len() {
                    return Ok(None);
                }
                for index in indices {
                    prefix.push(self.p30_index_expr(path, index, captures, captured_indices)?);
                }
                Ok(Some(Self::p30_view_from_prefix(array, &prefix)?))
            }
            None => Ok(None),
        }
    }

    pub(in super::super) fn p30_pattern_level(
        &self,
        path: &str,
        node: NodeId,
        bounds: (i32, i32),
    ) -> Result<Vec<NodeId>, String> {
        let Some(operands) = self.assignment_pattern_operands(path, node)? else {
            return Err(format!(
                "fixed unpacked-array assignment pattern in `{path}` is not an assignment pattern"
            ));
        };
        let count = usize::try_from((i64::from(bounds.0) - i64::from(bounds.1)).unsigned_abs() + 1)
            .map_err(|_| format!("fixed unpacked-array pattern is too large in `{path}`"))?;
        let tagged = operands.iter().any(|operand| {
            matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        });
        if !tagged {
            if operands.len() != count {
                return Err(format!(
                    "fixed unpacked-array assignment pattern in `{path}` has {} positional values; expected {count}",
                    operands.len()
                ));
            }
            return Ok(operands);
        }
        if operands.iter().any(|operand| {
            !matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        }) {
            return Err(format!(
                "mixed positional and keyed fixed unpacked-array assignment pattern in `{path}` is not supported"
            ));
        }
        let element = self.p30_pattern_element_descriptor(node);
        let mut explicit = HashMap::<usize, NodeId>::new();
        let mut type_values = Vec::<(AssignmentPatternKeyType, NodeId)>::new();
        let mut default = None;
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key,
                key_type,
                value,
                ..
            }) = self.kind(operand)
            else {
                unreachable!();
            };
            let key = key.as_deref().ok_or_else(|| {
                format!("fixed unpacked-array pattern key is unavailable in `{path}`")
            })?;
            let value = value.ok_or_else(|| {
                format!("fixed unpacked-array pattern key `{key}` has no value in `{path}`")
            })?;
            if key == "default" {
                if default.replace(value).is_some() {
                    return Err(format!(
                        "duplicate default key in fixed unpacked-array pattern in `{path}`"
                    ));
                }
                continue;
            }
            if let Some(index) = self.assignment_pattern_index_key(path, operand)? {
                let index = i32::try_from(index).map_err(|_| {
                    format!(
                        "fixed unpacked-array pattern index `{key}` is out of range in `{path}`"
                    )
                })?;
                let (left, right) = bounds;
                let Some(offset) = (if left >= right {
                    left.checked_sub(index)
                } else {
                    index.checked_sub(left)
                })
                .filter(|offset| *offset >= 0) else {
                    return Err(format!(
                        "fixed unpacked-array pattern index `{key}` is outside [{left}:{right}] in `{path}`"
                    ));
                };
                let offset = usize::try_from(offset).map_err(|_| {
                    format!(
                        "fixed unpacked-array pattern index `{key}` is out of range in `{path}`"
                    )
                })?;
                if offset >= count || explicit.insert(offset, value).is_some() {
                    return Err(format!(
                        "duplicate fixed unpacked-array pattern index `{key}` in `{path}`"
                    ));
                }
                continue;
            }
            let Some(key_type) = key_type.as_ref() else {
                return Err(format!(
                    "fixed unpacked-array pattern key `{key}` has no matching index or type in `{path}`"
                ));
            };
            type_values.push(((**key_type).clone(), value));
        }
        (0..count)
            .map(|offset| {
                explicit
                    .get(&offset)
                    .copied()
                    .or_else(|| {
                        element.as_ref().and_then(|element| {
                            type_values.iter().rev().find_map(|(key_type, value)| {
                                super::super::collection::pattern_key_matches_descriptor(
                                    key_type,
                                    element,
                                    element.two_state,
                                    None,
                                )
                                .then_some(*value)
                            })
                        })
                    })
                    .or(default)
                    .ok_or_else(|| {
                        format!(
                            "fixed unpacked-array pattern does not cover offset {offset} in `{path}`"
                        )
                    })
            })
            .collect()
    }

    fn p30_pattern_element_descriptor(&self, node: NodeId) -> Option<TypeDescriptor> {
        if let Some(element) = self.db.packed_pattern_element(node) {
            return Some(element.clone());
        }
        let descriptor = self.query_descriptor(node)?;
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return None;
        };
        if dimensions.len() == 1 {
            return Some(element.as_ref().clone());
        }
        Some(TypeDescriptor {
            id: descriptor.id,
            two_state: descriptor.two_state,
            name: descriptor.name.clone(),
            info: descriptor.info.clone(),
            shape: TypeShape::FixedArray {
                dimensions: dimensions[1..].to_vec(),
                element: element.clone(),
            },
        })
    }

    pub(in super::super) fn p30_pattern_values(
        &self,
        path: &str,
        node: NodeId,
        dims: &[(i32, i32)],
    ) -> Result<Vec<P30PatternSource>, String> {
        let Some((bounds, rest)) = dims.split_first() else {
            return Ok(vec![P30PatternSource::Leaf(node)]);
        };
        let values = match self.assignment_pattern_operands(path, node)? {
            Some(_) => self.p30_pattern_level(path, node, *bounds)?,
            None => {
                let total = dims
                    .iter()
                    .map(|(left, right)| (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1)
                    .try_fold(1u64, |total, extent| total.checked_mul(extent))
                    .and_then(|total| usize::try_from(total).ok())
                    .ok_or_else(|| {
                        format!("fixed unpacked-array pattern is too large in `{path}`")
                    })?;
                // An unpacked-array item supplies one element per remaining
                // cell, left bound to left bound; only a scalar value (such
                // as a `default:` leaf) is replicated into every cell.
                let array_elements = self.query_descriptor(node).and_then(|descriptor| {
                    let TypeShape::FixedArray { dimensions, .. } = &descriptor.shape else {
                        return None;
                    };
                    dimensions
                        .iter()
                        .map(|(left, right)| i64::from(*left).abs_diff(i64::from(*right)) + 1)
                        .try_fold(1u64, |total, extent| total.checked_mul(extent))
                });
                return match array_elements {
                    None => Ok(std::iter::repeat_n(P30PatternSource::Leaf(node), total).collect()),
                    Some(count) if usize::try_from(count).ok() == Some(total) => Ok((0..total)
                        .map(|index| P30PatternSource::Element {
                            node,
                            index,
                            dims: dims.to_vec(),
                        })
                        .collect()),
                    Some(count) => Err(format!(
                        "fixed unpacked-array pattern item has {count} elements where {total} are required in `{path}`"
                    )),
                };
            }
        };
        if rest.is_empty() {
            return Ok(values.into_iter().map(P30PatternSource::Leaf).collect());
        }
        let mut flattened = Vec::new();
        for value in values {
            flattened.extend(self.p30_pattern_values(path, value, rest)?);
        }
        Ok(flattened)
    }

    fn p30_capture_value(
        &self,
        lhs: NodeId,
        rhs: NodeId,
        ordinal: usize,
        value: IrExpr,
        captures: &mut Vec<IrStmt>,
    ) -> IrExpr {
        let name = format!("_p30_value_{}_{}_{}", lhs.0, rhs.0, ordinal);
        let width = value.width;
        let signed = value.signed;
        captures.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            init: Some(Box::new(value)),
            two_state: false,
        });
        IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
    }

    /// Capture a bounded fixed integral value before projecting its cells.
    /// The caller retains specialized array conversions; this fallback admits
    /// other expressions by owned result shape, including module conditionals.
    fn p30_lower_fixed_source_value(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        captures: &mut Vec<IrStmt>,
    ) -> Result<Option<Vec<IrExpr>>, String> {
        let fixed_source =
            self.query_descriptor(rhs)
                .and_then(|descriptor| match &descriptor.shape {
                    TypeShape::FixedArray { dimensions, .. } => {
                        Self::fixed_descriptor_width(descriptor)
                            .map(|width| (dimensions.clone(), width))
                    }
                    _ => None,
                });
        if let Some((dimensions, expected_width)) = fixed_source {
            if dimensions.is_empty() {
                return Err(format!("fixed array source has no dimensions in `{path}`"));
            }
            let source = self.lower_expr(path, rhs)?;
            let count = dimensions
                .iter()
                .try_fold(1u32, |count, (left, right)| {
                    count.checked_mul(
                        u32::try_from(i64::from(*left).abs_diff(i64::from(*right)) + 1).ok()?,
                    )
                })
                .ok_or("fixed value shape exceeds supported width")?;
            if source.width != expected_width || !source.width.is_multiple_of(count) {
                return Err(format!(
                    "fixed array source payload disagrees with its declared shape in `{path}`"
                ));
            }
            let element_width = source.width / count;
            // Stage the complete expression before splitting it into cells.
            // This preserves branch evaluation counts, overlapping copies and
            // NBA issue-time values without repeating the selector or arms.
            let source = self.p30_capture_value(lhs, rhs, 0, source, captures);
            let values = (0..count)
                .map(|index| {
                    let right = source.width - (index + 1) * element_width;
                    IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(source.clone()),
                            left: i64::from(right + element_width - 1),
                            right: i64::from(right),
                        },
                        element_width,
                        false,
                        None,
                    )
                })
                .collect();
            return Ok(Some(values));
        }
        Ok(None)
    }

    pub(in super::super) fn p30_lower_source_values(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        target_dims: &[(i32, i32)],
        captures: &mut Vec<IrStmt>,
        captured_indices: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<Vec<IrExpr>, String> {
        if matches!(self.kind(rhs), NodeKind::Expr(ExprKind::Cast { .. })) {
            let target_is_fixed = self
                .query_descriptor(rhs)
                .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }));
            if target_is_fixed {
                let target_array = self
                    .p30_array_prefix_base(lhs)
                    .map(|(array, _)| array.clone())
                    .ok_or_else(|| {
                        format!("fixed bit-stream destination has no array storage in `{path}`")
                    })?;
                if target_array.real {
                    return Err(format!(
                        "real fixed-array bit-stream destination is not supported in `{path}`"
                    ));
                }
                // Preserve every conversion before scattering the snapshot.
                // The destination's state domain cannot stand in for an
                // intermediate cast (for example logic'(bit'(source))).
                let source = self.lower_expr(path, rhs)?;
                if source.is_real() {
                    return Err(format!(
                        "real bit-stream source cannot initialize a packed array in `{path}`"
                    ));
                }
                let count = target_dims
                    .iter()
                    .try_fold(1u64, |total, (left, right)| {
                        total.checked_mul(
                            (i64::from(*left) - i64::from(*right))
                                .unsigned_abs()
                                .checked_add(1)?,
                        )
                    })
                    .ok_or_else(|| {
                        format!("fixed bit-stream destination is too large in `{path}`")
                    })?;
                let expected = count
                    .checked_mul(u64::from(target_array.elem_width))
                    .and_then(|width| u32::try_from(width).ok())
                    .ok_or_else(|| {
                        format!("fixed bit-stream destination width overflows in `{path}`")
                    })?;
                if source.width != expected {
                    return Err(format!(
                        "bit-stream cast source is {} bits but fixed array destination requires {} in `{path}`",
                        source.width, expected
                    ));
                }
                let source = self.p30_capture_value(lhs, rhs, 0, source, captures);
                let mut values = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
                let mut cursor = source.width;
                for ordinal in 0..count {
                    let width = target_array.elem_width;
                    let right = cursor.checked_sub(width).ok_or_else(|| {
                        format!("fixed bit-stream source cursor underflow in `{path}`")
                    })?;
                    let left = cursor - 1;
                    let value = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(source.clone()),
                            left: i64::from(left),
                            right: i64::from(right),
                        },
                        width,
                        false,
                        None,
                    );
                    values.push(self.p30_capture_value(
                        lhs,
                        rhs,
                        usize::try_from(ordinal + 1).unwrap_or(usize::MAX),
                        value,
                        captures,
                    ));
                    cursor = right;
                }
                return Ok(values);
            }
        }
        let source_node = self.p30_unwrap_cast(rhs);
        let concat = match self.kind(source_node) {
            NodeKind::Expr(ExprKind::Operation {
                op,
                operands,
                reordered,
                ..
            }) if *op == Operation::Concat => Some((operands.clone(), *reordered)),
            _ => None,
        };
        if let Some((mut operands, reordered)) = concat {
            if reordered {
                operands.reverse();
            }
            let mut values = Vec::new();
            for operand in operands {
                values.extend(self.p30_lower_source_values(
                    path,
                    lhs,
                    operand,
                    target_dims,
                    captures,
                    captured_indices,
                )?);
            }
            return Ok(values);
        }
        if self
            .assignment_pattern_operands(path, source_node)?
            .is_some()
        {
            let sources = self.p30_pattern_values(path, source_node, target_dims)?;
            let mut values = Vec::with_capacity(sources.len());
            // A repeated array-valued item (for example a row `default:`) is
            // evaluated once and its captured elements fan out.
            let mut array_items: HashMap<NodeId, Vec<IrExpr>> = HashMap::new();
            for (ordinal, source) in sources.into_iter().enumerate() {
                match source {
                    P30PatternSource::Leaf(node) => {
                        let value = self.lower_expr(path, node)?;
                        values.push(self.p30_capture_value(lhs, rhs, ordinal, value, captures));
                    }
                    P30PatternSource::Element { node, index, dims } => {
                        if let std::collections::hash_map::Entry::Vacant(entry) =
                            array_items.entry(node)
                        {
                            entry.insert(self.p30_lower_source_values(
                                path,
                                lhs,
                                node,
                                &dims,
                                captures,
                                captured_indices,
                            )?);
                        }
                        let element = array_items[&node].get(index).cloned().ok_or_else(|| {
                            format!(
                                "fixed unpacked-array pattern item is missing element {index} in `{path}`"
                            )
                        })?;
                        values.push(element);
                    }
                }
            }
            return Ok(values);
        }
        if let Some(view) = self.p30_array_view(path, rhs, captures, captured_indices)? {
            let mut values = Vec::with_capacity(view.coordinates.len());
            for (ordinal, coordinates) in view.coordinates.into_iter().enumerate() {
                let value = IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: self.reference_array(view.array.ir),
                        indices: coordinates,
                        elem_sel: IrElemSel::Whole,
                    },
                    view.array.elem_width,
                    view.array.signed,
                    None,
                );
                values.push(self.p30_capture_value(lhs, rhs, ordinal, value, captures));
            }
            return Ok(values);
        }
        // Calls, conditional values, and activation expressions do not have
        // a storage view. Lower their complete fixed payload once, then split
        // it into declaration-order elements after the view-specific cases
        // above have had a chance to retain runtime row selectors.
        if matches!(self.kind(rhs), NodeKind::FuncCall { .. })
            || self.p30_fixed_array_assignment_candidate(rhs)
            || self.func.is_some()
        {
            if let Some(values) = self.p30_lower_fixed_source_value(path, lhs, rhs, captures)? {
                return Ok(values);
            }
        }
        if self.p30_container_source(rhs).is_some() {
            return Err(format!(
                "dynamic or queue array to fixed unpacked-array assignment in `{path}` requires a runtime-compatible fixed size"
            ));
        }
        // A conditional (or other fixed integral array expression) need not
        // look like a storage view. Its owned type is sufficient to select the
        // existing value lowerer, including R01's aggregate-aware ArrayMux.
        if let Some(values) = self.p30_lower_fixed_source_value(path, lhs, rhs, captures)? {
            return Ok(values);
        }
        Err(format!(
            "fixed unpacked-array assignment in `{path}` requires a compatible fixed-array expression, slice, concatenation, or assignment pattern"
        ))
    }

    fn p30_target_pattern_dims(&self, node: NodeId, array: &ArrayInfo) -> Vec<(i32, i32)> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::PartSelect {
                base, left, right, ..
            }) => {
                let mut dims = Vec::with_capacity(array.dims.len());
                let bounds = self
                    .eval_bound_i128(*left)
                    .ok()
                    .zip(self.eval_bound_i128(*right).ok())
                    .and_then(|(left, right)| {
                        Some((i32::try_from(left).ok()?, i32::try_from(right).ok()?))
                    })
                    .unwrap_or((0, 0));
                let consumed = self
                    .p30_array_prefix_base(*base)
                    .map(|(_, consumed)| consumed)
                    .unwrap_or(0);
                dims.push(bounds);
                dims.extend_from_slice(&array.dims[consumed.saturating_add(1)..]);
                dims
            }
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. })
                if indices.len() < array.dims.len() =>
            {
                array.dims[indices.len()..].to_vec()
            }
            NodeKind::Expr(ExprKind::BitSelect { .. }) if array.dims.len() > 1 => {
                array.dims[1..].to_vec()
            }
            _ => array.dims.clone(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_p30_container_to_fixed(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        target: P30ArrayView,
        source: ContainerInfo,
        mut captures: Vec<IrStmt>,
    ) -> Result<IrStmt, String> {
        let kind = self.model.containers[source.ir].kind.clone();
        if !matches!(
            kind,
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "associative array to fixed unpacked-array assignment in `{path}` is not supported"
            ));
        }
        let source_type = &self.model.containers[source.ir].element;
        let (source_width, source_signed) = (source_type.width(), source_type.signed());
        if source_width == 0 {
            return Err(format!(
                "real dynamic or queue array to fixed unpacked-array assignment in `{path}` is not supported"
            ));
        }
        let destination_count = u32::try_from(target.coordinates.len())
            .map_err(|_| format!("fixed unpacked-array assignment is too large in `{path}`"))?;
        let size = IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::Size(source.ir))),
            32,
            true,
            None,
        );
        let expected = pattern_key_expr(i128::from(destination_count), 32, true, false);
        let condition = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Eq,
                a: Box::new(size),
                b: Box::new(expected),
            },
            1,
            false,
            None,
        );
        let destination_two_state = self.model.arrays[target.array.ir].two_state;
        let target_array = target.array;
        let target_coordinates = target.coordinates;
        let mut then_body = Vec::with_capacity(target_coordinates.len() * 2);
        for (ordinal, coordinates) in target_coordinates.into_iter().enumerate() {
            let index = pattern_key_expr(
                i128::try_from(ordinal).unwrap_or(i128::MAX),
                32,
                true,
                false,
            );
            let value = IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Get {
                    container: source.ir,
                    index: Box::new(index),
                })),
                source_width,
                source_signed,
                None,
            );
            let value = self.p30_capture_value(lhs, rhs, ordinal, value, &mut then_body);
            let value = if target_array.real {
                IrExpr::new(
                    IrExprKind::CastToReal {
                        a: Box::new(value),
                        shortreal: target_array.shortreal,
                    },
                    0,
                    true,
                    None,
                )
            } else {
                ir_to_storage(
                    value,
                    target_array.elem_width,
                    target_array.signed,
                    destination_two_state,
                )?
            };
            then_body.push(IrStmt::Assign {
                lhs: IrLhs::ArrayElem {
                    arr: self.reference_array(target_array.ir),
                    indices: coordinates,
                    elem_sel: IrElemSel::Whole,
                },
                rhs: value,
                nba: !blocking,
            });
        }
        captures.push(IrStmt::If {
            cond: condition,
            then_: then_body,
            els: Some(vec![IrStmt::Severity {
                level: crate::sim::ir::IrSeverityLevel::Fatal,
                fmt: "\"fixed unpacked-array assignment size mismatch\"".to_owned(),
                args: Vec::new(),
                scope: path.to_owned(),
                location: self.source_location(lhs),
                fatal_finish_number: Some(0),
                runtime_failure: true,
            }]),
            check: IrUniquePriorityCheck::None,
        });
        Ok(IrStmt::Block(captures))
    }

    /// Lower a bit-stream cast from a packed-element dynamic array or queue
    /// into a fixed unpacked array.  A bit-stream cast matches total width,
    /// rather than requiring the source and destination element widths to be
    /// identical.  The runtime size check is emitted before any destination
    /// write, preserving the destination on a mismatch.
    #[allow(clippy::too_many_arguments)]
    fn lower_p30_bitstream_container_cast(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        target: P30ArrayView,
        source: ContainerInfo,
        mut captures: Vec<IrStmt>,
    ) -> Result<IrStmt, String> {
        let source_model = self
            .model
            .containers
            .get(source.ir)
            .cloned()
            .ok_or_else(|| format!("bit-stream source container is out of bounds in `{path}`"))?;
        if !matches!(
            source_model.kind,
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "associative arrays are not legal bit-stream sources in `{path}`"
            ));
        }
        let (source_width, source_signed, _) = source_model.element.packed().ok_or_else(|| {
            format!("bit-stream source container requires a packed element in `{path}`")
        })?;
        if source_width == 0 {
            return Err(format!(
                "bit-stream source container has an empty packed element in `{path}`"
            ));
        }
        let target_array = target.array;
        if target_array.real {
            return Err(format!(
                "real fixed-array bit-stream destination is not supported in `{path}`"
            ));
        }
        let target_count = u64::try_from(target.coordinates.len())
            .map_err(|_| format!("fixed bit-stream destination is too large in `{path}`"))?;
        let target_width = target_count
            .checked_mul(u64::from(target_array.elem_width))
            .and_then(|width| u32::try_from(width).ok())
            .ok_or_else(|| format!("fixed bit-stream destination width overflows in `{path}`"))?;
        if target_width == 0 || target_width % source_width != 0 {
            return Err(format!(
                "fixed bit-stream destination width {target_width} is not divisible by source element width {source_width} in `{path}`"
            ));
        }
        let source_count = usize::try_from(target_width / source_width)
            .map_err(|_| format!("bit-stream source element count overflows in `{path}`"))?;
        let size = IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::Size(source.ir))),
            32,
            true,
            None,
        );
        let expected = pattern_key_expr(
            i128::from(u64::try_from(source_count).unwrap_or(u64::MAX)),
            32,
            true,
            false,
        );
        let condition = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Eq,
                a: Box::new(size),
                b: Box::new(expected),
            },
            1,
            false,
            None,
        );
        let mut source_parts = Vec::with_capacity(source_count);
        for ordinal in 0..source_count {
            let index = pattern_key_expr(
                i128::try_from(ordinal).unwrap_or(i128::MAX),
                32,
                true,
                false,
            );
            source_parts.push(IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Get {
                    container: source.ir,
                    index: Box::new(index),
                })),
                source_width,
                source_signed,
                None,
            ));
        }
        let source_value = if let [value] = source_parts.as_slice() {
            value.clone()
        } else {
            IrExpr::new(
                IrExprKind::Concat {
                    parts: source_parts,
                },
                target_width,
                false,
                None,
            )
        };
        let source_value = self.p30_capture_value(lhs, rhs, 0, source_value, &mut captures);
        let destination_two_state = self.model.arrays[target_array.ir].two_state;
        let mut then_body = Vec::with_capacity(target.coordinates.len() * 2);
        let mut cursor = target_width;
        for (ordinal, coordinates) in target.coordinates.into_iter().enumerate() {
            let right = cursor
                .checked_sub(target_array.elem_width)
                .ok_or_else(|| format!("fixed bit-stream source cursor underflow in `{path}`"))?;
            let value = IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(source_value.clone()),
                    left: i64::from(cursor - 1),
                    right: i64::from(right),
                },
                target_array.elem_width,
                false,
                None,
            );
            let value =
                self.p30_capture_value(lhs, rhs, ordinal.saturating_add(1), value, &mut then_body);
            then_body.push(IrStmt::Assign {
                lhs: IrLhs::ArrayElem {
                    arr: self.reference_array(target_array.ir),
                    indices: coordinates,
                    elem_sel: IrElemSel::Whole,
                },
                rhs: ir_to_storage(
                    value,
                    target_array.elem_width,
                    target_array.signed,
                    destination_two_state,
                )?,
                nba: !blocking,
            });
            cursor = right;
        }
        captures.push(IrStmt::If {
            cond: condition,
            then_: then_body,
            els: Some(vec![IrStmt::Severity {
                level: crate::sim::ir::IrSeverityLevel::Fatal,
                fmt: "\"fixed bit-stream cast size mismatch\"".to_owned(),
                args: Vec::new(),
                scope: path.to_owned(),
                location: self.source_location(lhs),
                fatal_finish_number: Some(0),
                runtime_failure: true,
            }]),
            check: IrUniquePriorityCheck::None,
        });
        Ok(IrStmt::Block(captures))
    }

    pub(in super::super) fn lower_p30_fixed_array_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        if op == Operation::Assignment {
            if let Ok(dst) = self.fixed_memory_view(path, lhs) {
                if self.model.arrays[dst.array].sparse()
                    && self.descriptor_value_transport(path, &dst, rhs, blocking)?
                {
                    let src = self.lower_fixed_value(path, rhs)?;
                    return Ok(Some(IrStmt::FixedValueAssign {
                        dst,
                        src: Box::new(src),
                        nba: !blocking,
                    }));
                }
            }
        }
        if blocking && op == Operation::Assignment {
            if let Some(statement) = self.lower_nonflatten_call(path, rhs, lhs)? {
                return Ok(Some(statement));
            }
        }
        if let Some(dst) = self
            .array_of(lhs)
            .cloned()
            .filter(|dst| self.model.arrays[dst.ir].sparse())
        {
            if let NodeKind::Expr(ExprKind::Streaming {
                direction,
                slice_size,
                streams,
            }) = self.kind(self.p30_unwrap_cast(rhs))
            {
                if let [stream] = streams.as_slice() {
                    if stream.with_expr.is_none() {
                        if let Some(src) = self.array_of(self.p30_unwrap_cast(stream.value)) {
                            let slice = if *direction == DbStreamingDirection::LeftToRight {
                                0
                            } else {
                                u32::try_from((*slice_size).max(1))
                                    .map_err(|_| "fixed stream slice exceeds supported capacity")?
                            };
                            if self.model.arrays[src.ir].sparse()
                                && dst.elem_width == src.elem_width
                                && self.model.arrays[dst.ir].total
                                    == self.model.arrays[src.ir].total
                                && (slice == 0
                                    || dst.elem_width.is_multiple_of(slice)
                                    || slice.is_multiple_of(dst.elem_width))
                            {
                                return Ok(Some(IrStmt::FixedArrayCopy {
                                    dst: dst.ir,
                                    src: src.ir,
                                    nba: !blocking,
                                    slice,
                                }));
                            }
                        }
                    }
                }
                return Err(format!("non-flattened fixed stream in `{path}` requires one complete array and an element-aligned slice"));
            }
        }
        if let (Some(dst), Some(src)) =
            (self.array_of(lhs), self.array_of(self.p30_unwrap_cast(rhs)))
        {
            if self.model.arrays[dst.ir].sparse() && self.model.arrays[src.ir].sparse() {
                if op != Operation::Assignment {
                    return Err(format!(
                        "compound fixed-array copy in `{path}` is unsupported"
                    ));
                }
                let dst = dst.ir;
                let src = src.ir;
                let target = self
                    .query_descriptor(lhs)
                    .ok_or("missing fixed-array target descriptor")?;
                let source = self
                    .query_descriptor(rhs)
                    .ok_or("missing fixed-array source descriptor")?;
                self.p30_require_pattern_shape(path, target, source, "fixed-array copy")?;
                return Ok(Some(IrStmt::FixedArrayCopy {
                    dst,
                    src,
                    nba: !blocking,
                    slice: 0,
                }));
            }
        }
        if let Some(array) = self
            .array_of(lhs)
            .cloned()
            .filter(|array| self.model.arrays[array.ir].sparse())
        {
            if op != Operation::Assignment {
                return Err(format!(
                    "compound descriptor pattern assignment in `{path}` is unsupported"
                ));
            }
            if let Some(statement) =
                self.lower_descriptor_pattern(path, lhs, rhs, &array, !blocking)?
            {
                return Ok(Some(statement));
            }
            return Err(format!("fixed-array value in `{path}` requires a non-flattened whole copy, supported stream, call or default pattern"));
        }
        let mut captures = Vec::new();
        let mut captured_indices = HashMap::new();
        let Some(target) = self.p30_array_view(path, lhs, &mut captures, &mut captured_indices)?
        else {
            return Ok(None);
        };
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment to a fixed unpacked array in `{path}` is not supported"
            ));
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, .. }) = self.kind(rhs) {
            let target_is_fixed = self
                .query_descriptor(rhs)
                .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }));
            if target_is_fixed && !self.db.is_implicit_conversion(rhs) {
                if let Some(source) = self.container_of(*operand) {
                    return Ok(Some(self.lower_p30_bitstream_container_cast(
                        path,
                        lhs,
                        rhs,
                        blocking,
                        target.clone(),
                        source,
                        captures,
                    )?));
                }
            }
        }
        if let Some(source) = self.p30_container_source(rhs) {
            return Ok(Some(self.lower_p30_container_to_fixed(
                path, lhs, rhs, blocking, target, source, captures,
            )?));
        }
        let target_dims = self.p30_target_pattern_dims(lhs, &target.array);
        let values = self.p30_lower_source_values(
            path,
            lhs,
            rhs,
            &target_dims,
            &mut captures,
            &mut captured_indices,
        )?;
        if values.len() != target.coordinates.len() {
            return Err(format!(
                "fixed unpacked-array assignment in `{path}` has {} source elements; destination requires {}",
                values.len(),
                target.coordinates.len()
            ));
        }
        let target_array = target.array;
        let target_coordinates = target.coordinates;
        let destination_two_state = self.model.arrays[target_array.ir].two_state;
        for (value, coordinates) in values.into_iter().zip(target_coordinates) {
            let value = if target_array.real {
                if value.is_real() {
                    if target_array.shortreal {
                        IrExpr::new(
                            IrExprKind::CastToReal {
                                a: Box::new(value),
                                shortreal: true,
                            },
                            0,
                            true,
                            None,
                        )
                    } else {
                        value
                    }
                } else {
                    IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(value),
                            shortreal: target_array.shortreal,
                        },
                        0,
                        true,
                        None,
                    )
                }
            } else {
                ir_to_storage(
                    value,
                    target_array.elem_width,
                    target_array.signed,
                    destination_two_state,
                )?
            };
            let lhs = IrLhs::ArrayElem {
                arr: self.reference_array(target_array.ir),
                indices: coordinates,
                elem_sel: IrElemSel::Whole,
            };
            captures.push(IrStmt::Assign {
                lhs,
                rhs: value,
                nba: !blocking,
            });
        }
        Ok(Some(IrStmt::Block(captures)))
    }
}
