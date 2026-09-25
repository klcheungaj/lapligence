//! Membership.

use super::*;

impl<'a> Codegen<'a> {
    fn inside_has_integral_leaves(descriptor: &TypeDescriptor) -> bool {
        match &descriptor.shape {
            TypeShape::FixedArray { element, .. } => Self::inside_has_integral_leaves(element),
            TypeShape::PackedAtom { .. } => true,
            TypeShape::Aggregate(layout) => matches!(
                layout.kind,
                AggregateKind::PackedStruct
                    | AggregateKind::PackedUnion
                    | AggregateKind::TaggedUnion
            ),
            _ => false,
        }
    }

    fn inside_has_stored_leaves(descriptor: &TypeDescriptor) -> bool {
        match &descriptor.shape {
            TypeShape::FixedArray { element, .. } => Self::inside_has_stored_leaves(element),
            TypeShape::Real { .. } => true,
            _ => Self::inside_has_integral_leaves(descriptor),
        }
    }

    fn lower_fixed_array_elements(
        descriptor: &TypeDescriptor,
        right: u32,
        elements: &mut Vec<IrInsideArrayElement>,
    ) -> Result<(), String> {
        match &descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let (left, array_right) = dimensions
                    .first()
                    .copied()
                    .ok_or("fixed-array inside item has no dimensions")?;
                let count = i64::from(left).abs_diff(i64::from(array_right)) + 1;
                let next = if dimensions.len() == 1 {
                    *element.clone()
                } else {
                    TypeDescriptor {
                        shape: TypeShape::FixedArray {
                            dimensions: dimensions[1..].to_vec(),
                            element: element.clone(),
                        },
                        ..descriptor.clone()
                    }
                };
                let stride = Self::fixed_descriptor_width(&next)
                    .ok_or("fixed-array inside item has an unsupported element width")?;
                for ordinal in 0..count {
                    let offset = u32::try_from(count - ordinal - 1)
                        .ok()
                        .and_then(|offset| offset.checked_mul(stride))
                        .ok_or("fixed-array inside item payload offset overflow")?;
                    let child_right = right
                        .checked_add(offset)
                        .ok_or("fixed-array inside item payload offset overflow")?;
                    Self::lower_fixed_array_elements(&next, child_right, elements)?;
                }
            }
            _ => {
                let width = Self::fixed_descriptor_width(descriptor)
                    .ok_or("fixed-array inside item has an unsupported scalar element")?;
                let left = right
                    .checked_add(
                        width
                            .checked_sub(1)
                            .ok_or("fixed-array element has zero width")?,
                    )
                    .ok_or("fixed-array inside item payload bound overflow")?;
                elements.push(IrInsideArrayElement {
                    left: i64::from(left),
                    right: i64::from(right),
                    width,
                    signed: descriptor.info.signed,
                });
            }
        }
        Ok(())
    }

    fn lower_fixed_array_value_item(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrInsideItem>, String> {
        let Some(descriptor) = self.query_descriptor(node).cloned() else {
            return Ok(None);
        };
        if !matches!(descriptor.shape, TypeShape::FixedArray { .. }) {
            return Ok(None);
        }
        if !Self::inside_has_integral_leaves(&descriptor) {
            return Err(format!(
                "fixed-array inside set item requires singular integral elements in `{path}`"
            ));
        }
        let width = Self::fixed_descriptor_width(&descriptor).ok_or_else(|| {
            format!("fixed-array inside set item has no supported width in `{path}`")
        })?;
        let value = self.lower_expr(path, node)?;
        if value.width != width {
            return Err(format!(
                "fixed-array inside set item width {} does not match its type width {width} in `{path}`",
                value.width
            ));
        }
        let mut elements = Vec::new();
        Self::lower_fixed_array_elements(&descriptor, 0, &mut elements)?;
        if elements.is_empty() {
            return Err(format!(
                "fixed-array inside set item has no scalar elements in `{path}`"
            ));
        }
        Ok(Some(IrInsideItem::FixedArray { value, elements }))
    }

    pub(in super::super) fn lower_inside_items(
        &mut self,
        path: &str,
        nodes: &[NodeId],
    ) -> Result<Vec<IrInsideItem>, String> {
        if nodes.is_empty() {
            return Err(format!("inside set is empty in `{path}`"));
        }
        let mut items = Vec::new();
        for node in nodes {
            self.lower_inside_item(path, *node, &mut items)?;
        }
        if items.is_empty() {
            return Err(format!("inside set is empty in `{path}`"));
        }
        Ok(items)
    }

    fn lower_inside_item(
        &mut self,
        path: &str,
        node: NodeId,
        out: &mut Vec<IrInsideItem>,
    ) -> Result<(), String> {
        if let NodeKind::Expr(ExprKind::Operation {
            op: Operation::List,
            operands,
            ..
        }) = self.kind(node)
        {
            let operands = operands.clone();
            if operands.is_empty() {
                return Err(format!("malformed inside set item in `{path}`"));
            }
            let is_nested = operands.iter().any(|operand| {
                matches!(
                    self.kind(*operand),
                    NodeKind::Expr(ExprKind::Operation {
                        op: Operation::List,
                        ..
                    })
                )
            });
            if operands.len() == 2 && !is_nested {
                let low = (!self.is_unbounded_inside_node(operands[0]))
                    .then(|| self.lower_expr(path, operands[0]))
                    .transpose()?;
                let high = (!self.is_unbounded_inside_node(operands[1]))
                    .then(|| self.lower_expr(path, operands[1]))
                    .transpose()?;
                if low.is_none() && high.is_none() {
                    return Err(format!("inside range has no bounded endpoint in `{path}`"));
                }
                match (low, high) {
                    (Some(low), Some(high)) => out.push(IrInsideItem::Range { low, high }),
                    (low, high) => out.push(IrInsideItem::OpenRange { low, high }),
                }
            } else {
                for operand in operands {
                    self.lower_inside_item(path, operand, out)?;
                }
            }
            return Ok(());
        }

        if self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(&descriptor.shape, TypeShape::Aggregate(layout)
                if matches!(layout.kind, AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion))
        }) {
            return Err(format!(
                "inside set item requires a singular value or an array of singular values in `{path}`"
            ));
        }
        let storage_array = self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(descriptor.shape, TypeShape::FixedArray { .. })
                && Self::inside_has_stored_leaves(descriptor)
        }) && matches!(
            self.kind(node),
            NodeKind::Array { .. }
                | NodeKind::Expr(
                    ExprKind::Ref { .. } | ExprKind::HierPath { .. } | ExprKind::ArraySelect { .. }
                )
        );
        if storage_array {
            if let Some((_, aggregate)) = self.unpacked_aggregate_info(node) {
                for leaf in aggregate.leaves {
                    let value = self.aggregate_leaf_read(&leaf).map_err(|error| {
                        format!(
                            "inside set aggregate member is not a scalar value in `{path}`: {error}"
                        )
                    })?;
                    out.push(IrInsideItem::Value(value));
                }
                return Ok(());
            }
            if let Some((target, prefix)) = self.unpacked_path_for_expr(node) {
                if let Some(aggregate) = self.unpacked_aggregates.get(&target) {
                    let leaves = aggregate
                        .leaves
                        .iter()
                        .filter(|leaf| leaf.path.starts_with(&prefix))
                        .cloned()
                        .collect::<Vec<_>>();
                    if !leaves.is_empty() {
                        for leaf in leaves {
                            let value = self.aggregate_leaf_read(&leaf).map_err(|error| {
                                format!(
                                    "inside set aggregate member is not a scalar value in `{path}`: {error}"
                                )
                            })?;
                            out.push(IrInsideItem::Value(value));
                        }
                        return Ok(());
                    }
                }
            }
            if let Some(array) = self.array_of(node).cloned() {
                if array.dims.is_empty() {
                    return Err(format!("inside set array has no dimensions in `{path}`"));
                }
                for indices in inside_array_index_vectors(&array.dims) {
                    out.push(IrInsideItem::Value(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(array.ir),
                            indices: indices
                                .into_iter()
                                .map(|index| lhs_integer_expr(i128::from(index)))
                                .collect(),
                            elem_sel: IrElemSel::Whole,
                        },
                        array.elem_width,
                        array.signed,
                        None,
                    )));
                }
                return Ok(());
            }
        }
        if let Some(item) = self.lower_fixed_array_value_item(path, node)? {
            out.push(item);
            return Ok(());
        }
        if let Some(container) = self.container_of(node) {
            out.push(IrInsideItem::Container {
                container: container.ir,
            });
            return Ok(());
        }
        if self.is_string_expr(path, node) {
            return Err(format!(
                "string-valued inside set item is incompatible with a packed selector in `{path}`"
            ));
        }
        if self.is_chandle_expr(path, node) {
            return Err(format!(
                "chandle-valued inside set item is not supported in `{path}`"
            ));
        }
        let value = self.lower_expr(path, node)?;
        out.push(IrInsideItem::Value(value));
        Ok(())
    }

    pub(in super::super) fn lower_inside_string_items(
        &mut self,
        path: &str,
        nodes: &[NodeId],
    ) -> Result<Vec<IrStringInsideItem>, String> {
        if nodes.is_empty() {
            return Err(format!("inside set is empty in `{path}`"));
        }
        let mut items = Vec::new();
        for node in nodes {
            self.lower_inside_string_item(path, *node, &mut items)?;
        }
        if items.is_empty() {
            return Err(format!("inside set is empty in `{path}`"));
        }
        Ok(items)
    }

    fn lower_inside_string_item(
        &mut self,
        path: &str,
        node: NodeId,
        out: &mut Vec<IrStringInsideItem>,
    ) -> Result<(), String> {
        if let NodeKind::Expr(ExprKind::Operation {
            op: Operation::List,
            operands,
            ..
        }) = self.kind(node)
        {
            let operands = operands.clone();
            if operands.is_empty() {
                return Err(format!("malformed string inside set item in `{path}`"));
            }
            let is_nested = operands.iter().any(|operand| {
                matches!(
                    self.kind(*operand),
                    NodeKind::Expr(ExprKind::Operation {
                        op: Operation::List,
                        ..
                    })
                )
            });
            if operands.len() == 2 && !is_nested {
                if self.is_unbounded_inside_node(operands[0])
                    || self.is_unbounded_inside_node(operands[1])
                {
                    return Err(format!(
                        "unbounded string inside range is not legal in `{path}`"
                    ));
                }
                out.push(IrStringInsideItem::Range {
                    low: self.lower_string(path, operands[0])?,
                    high: self.lower_string(path, operands[1])?,
                });
            } else {
                for operand in operands {
                    self.lower_inside_string_item(path, operand, out)?;
                }
            }
            return Ok(());
        }

        if let Some((_, aggregate)) = self.unpacked_aggregate_info(node) {
            for leaf in aggregate.leaves {
                let object = leaf.object.ok_or_else(|| {
                    format!(
                        "non-string aggregate member `{}` in string inside set in `{path}`",
                        aggregate_path_suffix(&leaf.path)
                    )
                })?;
                out.push(IrStringInsideItem::Value(IrStringExpr::Read(
                    self.reference_object(object),
                )));
            }
            return Ok(());
        }
        if let Some((target, prefix)) = self.unpacked_path_for_expr(node) {
            if let Some(aggregate) = self.unpacked_aggregates.get(&target) {
                let leaves = aggregate
                    .leaves
                    .iter()
                    .filter(|leaf| leaf.path.starts_with(&prefix))
                    .cloned()
                    .collect::<Vec<_>>();
                if !leaves.is_empty() {
                    for leaf in leaves {
                        let object = leaf.object.ok_or_else(|| {
                            format!(
                                "non-string aggregate member `{}` in string inside set in `{path}`",
                                aggregate_path_suffix(&leaf.path)
                            )
                        })?;
                        out.push(IrStringInsideItem::Value(IrStringExpr::Read(
                            self.reference_object(object),
                        )));
                    }
                    return Ok(());
                }
            }
        }
        if self.array_of(node).is_some() || self.container_of(node).is_some() {
            return Err(format!(
                "string-valued inside set array/container has unsupported storage in `{path}`"
            ));
        }
        out.push(IrStringInsideItem::Value(self.lower_string(path, node)?));
        Ok(())
    }

    fn is_unbounded_inside_node(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Unbounded) => true,
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.is_unbounded_inside_node(*operand)
            }
            _ => false,
        }
    }
}
