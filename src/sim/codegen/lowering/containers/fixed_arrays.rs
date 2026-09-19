//! Fixed arrays.

use super::*;

impl<'a> Codegen<'a> {
    // Fixed unpacked-array assignment (P30).

    /// A fixed-array view is represented by the complete coordinate list in
    /// logical (declared left-to-right) order.  Keeping the view as concrete
    /// coordinates lets the existing guarded ArrayRead/ArrayElem IR preserve
    /// direction, notifications, force precedence, and two-state conversion.
    pub(super) fn p30_fixed_array_assignment_candidate(&self, node: NodeId) -> bool {
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

    pub(super) fn p30_unwrap_cast(&self, node: NodeId) -> NodeId {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.p30_unwrap_cast(*operand),
            _ => node,
        }
    }

    fn p30_array_prefix_base(&self, node: NodeId) -> Option<(&ArrayInfo, usize)> {
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

    fn p30_array_view(
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

    fn p30_pattern_level(
        &self,
        path: &str,
        node: NodeId,
        bounds: (i32, i32),
    ) -> Result<Vec<NodeId>, String> {
        let NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            reordered,
            ..
        }) = self.kind(node)
        else {
            return Err(format!(
                "fixed unpacked-array assignment pattern in `{path}` is not an assignment pattern"
            ));
        };
        if *op != Operation::AssignmentPattern {
            return Err(format!(
                "fixed unpacked-array assignment pattern in `{path}` is not an assignment pattern"
            ));
        }
        let count = usize::try_from((i64::from(bounds.0) - i64::from(bounds.1)).unsigned_abs() + 1)
            .map_err(|_| format!("fixed unpacked-array pattern is too large in `{path}`"))?;
        let mut operands = operands.clone();
        if *reordered {
            operands.reverse();
        }
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
        let mut explicit = HashMap::<usize, NodeId>::new();
        let mut default = None;
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern { key, value, .. }) = self.kind(operand)
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
            let index = parse_pattern_i128(key).ok_or_else(|| {
                format!(
                    "fixed unpacked-array pattern key `{key}` is not a constant index in `{path}`"
                )
            })?;
            let index = i32::try_from(index).map_err(|_| {
                format!("fixed unpacked-array pattern index `{key}` is out of range in `{path}`")
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
                format!("fixed unpacked-array pattern index `{key}` is out of range in `{path}`")
            })?;
            if offset >= count || explicit.insert(offset, value).is_some() {
                return Err(format!(
                    "duplicate fixed unpacked-array pattern index `{key}` in `{path}`"
                ));
            }
        }
        (0..count)
            .map(|offset| {
                explicit.get(&offset).copied().or(default).ok_or_else(|| {
                    format!(
                        "fixed unpacked-array pattern does not cover offset {offset} in `{path}`"
                    )
                })
            })
            .collect()
    }

    pub(in super::super) fn p30_pattern_values(
        &self,
        path: &str,
        node: NodeId,
        dims: &[(i32, i32)],
    ) -> Result<Vec<NodeId>, String> {
        let Some((bounds, rest)) = dims.split_first() else {
            return Ok(vec![node]);
        };
        let values = match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == Operation::AssignmentPattern =>
            {
                self.p30_pattern_level(path, node, *bounds)?
            }
            _ => {
                let count = dims[1..]
                    .iter()
                    .map(|(left, right)| (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1)
                    .try_fold(1u64, |total, extent| total.checked_mul(extent))
                    .ok_or_else(|| {
                        format!("fixed unpacked-array pattern is too large in `{path}`")
                    })?;
                let count = usize::try_from(count).map_err(|_| {
                    format!("fixed unpacked-array pattern is too large in `{path}`")
                })?;
                let total = count
                    .checked_mul(
                        usize::try_from((i64::from(bounds.0) - i64::from(bounds.1)).unsigned_abs())
                            .map_err(|_| {
                                format!("fixed unpacked-array pattern is too large in `{path}`")
                            })?
                            .saturating_add(1),
                    )
                    .ok_or_else(|| {
                        format!("fixed unpacked-array pattern is too large in `{path}`")
                    })?;
                return Ok(std::iter::repeat_n(node, total).collect());
            }
        };
        if rest.is_empty() {
            return Ok(values);
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

    fn p30_lower_source_values(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        target_dims: &[(i32, i32)],
        captures: &mut Vec<IrStmt>,
        captured_indices: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<Vec<IrExpr>, String> {
        let fixed_source_dimensions = self
            .query_descriptor(rhs)
            .filter(|descriptor| Self::fixed_descriptor_width(descriptor).is_some())
            .and_then(|descriptor| match &descriptor.shape {
                TypeShape::FixedArray { dimensions, .. } => Some(dimensions.clone()),
                _ => None,
            });
        if let Some(dimensions) = fixed_source_dimensions.filter(|_| {
            matches!(self.kind(rhs), NodeKind::FuncCall { .. })
                || self.p30_fixed_array_assignment_candidate(rhs)
                || self.func.is_some()
        }) {
            let source = self.lower_expr(path, rhs)?;
            let count = dimensions
                .iter()
                .try_fold(1u32, |count, (left, right)| {
                    count.checked_mul(
                        u32::try_from(i64::from(*left).abs_diff(i64::from(*right)) + 1).ok()?,
                    )
                })
                .ok_or("fixed value shape exceeds supported width")?;
            if count == 0 || !source.width.is_multiple_of(count) {
                return Err("fixed value source shape disagrees with destination".into());
            }
            let element_width = source.width / count;
            let source = self.p30_capture_value(lhs, rhs, 0, source, captures);
            return Ok((0..count)
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
                .collect());
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, .. }) = self.kind(rhs) {
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
                let source = match self.lower_bitstream_source(path, *operand)? {
                    Some(value) => value,
                    None => self.lower_expr(path, *operand)?,
                };
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
        if matches!(
            self.kind(source_node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                ..
            })
        ) {
            let nodes = self.p30_pattern_values(path, source_node, target_dims)?;
            let mut values = Vec::with_capacity(nodes.len());
            for (ordinal, node) in nodes.into_iter().enumerate() {
                let value = self.lower_expr(path, node)?;
                values.push(self.p30_capture_value(lhs, rhs, ordinal, value, captures));
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
        if self.p30_container_source(rhs).is_some() {
            return Err(format!(
                "dynamic or queue array to fixed unpacked-array assignment in `{path}` requires a runtime-compatible fixed size"
            ));
        }
        Err(format!(
            "fixed unpacked-array assignment in `{path}` requires a compatible fixed array, slice, concatenation, or assignment pattern"
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

    pub(super) fn lower_p30_fixed_array_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
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
