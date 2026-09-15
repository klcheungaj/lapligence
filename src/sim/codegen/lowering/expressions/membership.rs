//! Membership.

use super::*;

impl<'a> Codegen<'a> {

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
