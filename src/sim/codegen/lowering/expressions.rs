//! Expression and assignment-target lowering into typed simulator IR.

use super::collection::aggregate_path_suffix;
use super::objects::object_query;
use super::*;
use crate::sim::ir::{
    IrArrayDimension, IrArrayQuery, IrArrayQueryKind, IrArrayQueryTarget, IrBinOp, IrChandleExpr,
    IrConst, IrContainerExpr, IrContainerKind, IrFileInput, IrFileInputTarget, IrFileReadTarget,
    IrInsideItem, IrObjectQuery, IrObjectStmt, IrObjectType, IrPlusArgTarget, IrPlusArgText,
    IrStreamSelector, IrStringExpr, IrStringInsideItem,
};

fn inside_array_index_vectors(dims: &[(i32, i32)]) -> Vec<Vec<i32>> {
    fn visit(
        dims: &[(i32, i32)],
        dimension: usize,
        current: &mut Vec<i32>,
        values: &mut Vec<Vec<i32>>,
    ) {
        if dimension == dims.len() {
            values.push(current.clone());
            return;
        }
        let (left, right) = dims[dimension];
        let step = if left <= right { 1 } else { -1 };
        let mut index = left;
        loop {
            current.push(index);
            visit(dims, dimension + 1, current, values);
            current.pop();
            if index == right {
                break;
            }
            index = index.saturating_add(step);
        }
    }

    let mut values = Vec::new();
    visit(dims, 0, &mut Vec::new(), &mut values);
    values
}

fn validate_plusarg_format(format: &str, scope_path: &str) -> Result<(), String> {
    let bytes = format.as_bytes();
    if bytes.contains(&0) {
        return Err(format!(
            "$value$plusargs format contains NUL in `{scope_path}`"
        ));
    }
    let mut conversion = false;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        index += 1;
        let Some(&specifier) = bytes.get(index) else {
            return Err(format!(
                "$value$plusargs format ends with `%` in `{scope_path}`"
            ));
        };
        let mut specifier = specifier;
        if specifier == b'%' {
            index += 1;
            continue;
        }
        if specifier == b'0' {
            index += 1;
            let Some(&next) = bytes.get(index) else {
                return Err(format!(
                    "$value$plusargs format ends with `%0` in `{scope_path}`"
                ));
            };
            specifier = next;
        }
        if conversion {
            return Err(format!(
                "$value$plusargs format has more than one conversion in `{scope_path}`"
            ));
        }
        if !matches!(
            specifier.to_ascii_lowercase(),
            b'd' | b'h' | b'x' | b'o' | b'b' | b'f' | b'e' | b'g' | b's'
        ) {
            return Err(format!(
                "$value$plusargs format has unsupported conversion `%{}` in `{scope_path}`",
                char::from(specifier)
            ));
        }
        conversion = true;
        index += 1;
    }
    if !conversion {
        return Err(format!(
            "$value$plusargs format requires one conversion in `{scope_path}`"
        ));
    }
    Ok(())
}

impl<'a> Codegen<'a> {
    /// Lower the selector attached to a streaming operand.  The array base is
    /// retained by the owning streaming node; only selector bounds become
    /// runtime expressions, so each bound is evaluated once by the C call.
    pub(super) fn lower_stream_selector(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStreamSelector, String> {
        let lower_integral = |this: &mut Self, bound: NodeId| -> Result<IrExpr, String> {
            let value = this.lower_expr(path, bound)?;
            if value.is_real() {
                return Err(format!(
                    "streaming `with` selector bound must be integral in `{path}`"
                ));
            }
            Ok(value)
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                Ok(IrStreamSelector::Index(lower_integral(self, *index)?))
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                Ok(IrStreamSelector::Range {
                    left: lower_integral(self, *left)?,
                    right: lower_integral(self, *right)?,
                })
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => Ok(IrStreamSelector::Indexed {
                base: lower_integral(self, *base_expr)?,
                width: lower_integral(self, *width_expr)?,
                negative: *neg,
            }),
            _ => Err(format!("unsupported streaming `with` selector in `{path}`")),
        }
    }

    /// Return constant logical indices selected by a `with` expression.  A
    /// `None` result means at least one bound is runtime-valued and must stay
    /// in the typed runtime selector path.
    pub(super) fn static_stream_selector_indices(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<Vec<i128>>, String> {
        let range = |left: i128, right: i128| -> Result<Vec<i128>, String> {
            let distance = left.abs_diff(right);
            let count = distance
                .checked_add(1)
                .ok_or_else(|| format!("streaming selector range overflows in `{path}`"))?;
            if count > u128::from(LLG_MAX_WIDTH) {
                return Err(format!(
                    "streaming selector in `{path}` exceeds the runtime maximum width"
                ));
            }
            let count = usize::try_from(count)
                .map_err(|_| format!("streaming selector is too large in `{path}`"))?;
            let descending = left > right;
            (0..count)
                .map(|offset| {
                    let offset = i128::try_from(offset)
                        .map_err(|_| format!("streaming selector overflows in `{path}`"))?;
                    if descending {
                        left.checked_sub(offset)
                            .ok_or_else(|| format!("streaming selector overflows in `{path}`"))
                    } else {
                        left.checked_add(offset)
                            .ok_or_else(|| format!("streaming selector overflows in `{path}`"))
                    }
                })
                .collect()
        };
        let constant = |this: &Self, bound: NodeId| this.eval_bound_i128(bound).ok();
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                Ok(constant(self, *index).map(|index| vec![index]))
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                let (Some(left), Some(right)) = (constant(self, *left), constant(self, *right))
                else {
                    return Ok(None);
                };
                range(left, right).map(Some)
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let (Some(base), Some(width)) =
                    (constant(self, *base_expr), constant(self, *width_expr))
                else {
                    return Ok(None);
                };
                if width <= 0 {
                    return Err(format!(
                        "streaming indexed selector width must be positive in `{path}`"
                    ));
                }
                let end = if *neg {
                    base.checked_sub(width - 1)
                } else {
                    base.checked_add(width - 1)
                }
                .ok_or_else(|| format!("streaming selector overflows in `{path}`"))?;
                range(base, end).map(Some)
            }
            _ => Err(format!("unsupported streaming `with` selector in `{path}`")),
        }
    }

    fn fixed_stream_parts(
        &self,
        path: &str,
        array: &ArrayInfo,
        selected: Option<&[i128]>,
    ) -> Result<Vec<IrExpr>, String> {
        if array.dims.is_empty() {
            return Err(format!(
                "fixed streaming array has no dimensions in `{path}`"
            ));
        }
        if array.real {
            return Err(format!(
                "real array streaming operand is not supported in `{path}`"
            ));
        }
        let indices = selected.map(|indices| indices.to_vec()).unwrap_or_else(|| {
            let (left, right) = array.dims[0];
            let step = if left <= right { 1 } else { -1 };
            let mut values = Vec::new();
            let mut index = i128::from(left);
            loop {
                values.push(index);
                if index == i128::from(right) {
                    break;
                }
                index += i128::from(step);
            }
            values
        });
        let rest = inside_array_index_vectors(&array.dims[1..]);
        let rest_count = u128::try_from(rest.len())
            .map_err(|_| format!("fixed streaming array is too large in `{path}`"))?;
        let total = u128::try_from(indices.len())
            .ok()
            .and_then(|count| count.checked_mul(rest_count))
            .ok_or_else(|| format!("fixed streaming array is too large in `{path}`"))?;
        let width = total
            .checked_mul(u128::from(array.elem_width))
            .ok_or_else(|| format!("fixed streaming array width overflows in `{path}`"))?;
        if width > u128::from(LLG_MAX_WIDTH) {
            return Err(format!(
                "fixed streaming array in `{path}` exceeds the runtime maximum width"
            ));
        }
        let mut parts = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
        for index in indices {
            let prefix = lhs_integer_expr(index);
            for suffix in &rest {
                let mut coordinates = Vec::with_capacity(1 + suffix.len());
                coordinates.push(prefix.clone());
                coordinates.extend(
                    suffix
                        .iter()
                        .map(|value| lhs_integer_expr(i128::from(*value))),
                );
                parts.push(IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: self.reference_array(array.ir),
                        indices: coordinates,
                        elem_sel: IrElemSel::Whole,
                    },
                    array.elem_width,
                    array.signed,
                    None,
                ));
            }
        }
        Ok(parts)
    }

    fn lower_stream_container_static(
        &self,
        path: &str,
        container: usize,
        indices: &[i128],
    ) -> Result<IrExpr, String> {
        let info = self
            .model
            .containers
            .get(container)
            .ok_or_else(|| format!("streaming container index is out of bounds in `{path}`"))?;
        let (width, signed, _) = info
            .element
            .packed()
            .ok_or_else(|| format!("streaming container requires a packed element in `{path}`"))?;
        let parts = indices
            .iter()
            .map(|index| {
                IrExpr::new(
                    IrExprKind::Container(Box::new(IrContainerExpr::Get {
                        container,
                        index: Box::new(lhs_integer_expr(*index)),
                    })),
                    width,
                    signed,
                    None,
                )
            })
            .collect::<Vec<_>>();
        Self::join_bitstream_parts(path, parts)
    }

    pub(super) fn lower_stream_operand(
        &mut self,
        path: &str,
        value_node: NodeId,
        with_node: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        if let Some(container) = self.container_of(value_node) {
            let element = self
                .model
                .containers
                .get(container.ir)
                .ok_or_else(|| format!("streaming container is out of bounds in `{path}`"))?;
            if !matches!(
                element.kind,
                IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
            ) {
                return Err(format!(
                    "associative arrays are not legal streaming operands in `{path}`"
                ));
            }
            let (_, _, _) = element.element.packed().ok_or_else(|| {
                format!("streaming container operand must have a packed element in `{path}`")
            })?;
            if let Some(with_node) = with_node {
                if let Some(indices) = self.static_stream_selector_indices(path, with_node)? {
                    return self.lower_stream_container_static(path, container.ir, &indices);
                }
                let selector = self.lower_stream_selector(path, with_node)?;
                return Ok(IrExpr::new(
                    IrExprKind::Container(Box::new(IrContainerExpr::Stream {
                        container: container.ir,
                        slice: 1,
                        direction: IrStreamDirection::LeftToRight,
                        selector: Some(selector),
                    })),
                    LLG_MAX_WIDTH,
                    false,
                    None,
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Stream {
                    container: container.ir,
                    slice: 1,
                    direction: IrStreamDirection::LeftToRight,
                    selector: None,
                })),
                LLG_MAX_WIDTH,
                false,
                None,
            ));
        }
        if let Some(array) = self.array_of(value_node).cloned() {
            let selected = match with_node {
                Some(with_node) => Some(
                    self.static_stream_selector_indices(path, with_node)?
                        .ok_or_else(|| {
                            format!(
                                "runtime `with` selector on a fixed array is not supported in `{path}`"
                    )
                        })?,
                ),
                None => None,
            };
            let parts = self.fixed_stream_parts(path, &array, selected.as_deref())?;
            return Self::join_bitstream_parts(path, parts);
        }
        if with_node.is_some() {
            return Err(format!(
                "streaming `with` selector requires a packed-element array in `{path}`"
            ));
        }
        if let Some(value) = self.lower_bitstream_source(path, value_node)? {
            return Ok(value);
        }
        self.lower_expr(path, value_node)
    }

    pub(super) fn lower_inside_items(
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

    pub(super) fn lower_inside_string_items(
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

    pub(super) fn query_descriptor(&self, node: NodeId) -> Option<&TypeDescriptor> {
        self.db
            .type_descriptor(node)
            .or_else(|| match self.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => self.db.type_descriptor(*target),
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                    .iter()
                    .rev()
                    .flatten()
                    .find_map(|target| self.db.type_descriptor(*target)),
                _ => None,
            })
    }

    fn query_dimensions_for(descriptor: &TypeDescriptor) -> Vec<IrArrayDimension> {
        match &descriptor.shape {
            TypeShape::PackedAtom { ranges } => {
                if ranges.is_empty() {
                    descriptor
                        .info
                        .width
                        .filter(|width| *width > 1)
                        .map(|width| {
                            vec![IrArrayDimension {
                                left: Some(i128::from(width) - 1),
                                right: Some(0),
                            }]
                        })
                        .unwrap_or_default()
                } else {
                    ranges
                        .iter()
                        .map(|range| IrArrayDimension {
                            left: Some(range.left),
                            right: Some(range.right),
                        })
                        .collect()
                }
            }
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) && descriptor.info.width.is_some_and(|width| width > 1) =>
            {
                vec![IrArrayDimension {
                    left: descriptor.info.width.map(|width| i128::from(width) - 1),
                    right: Some(0),
                }]
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let mut result = dimensions
                    .iter()
                    .map(|(left, right)| IrArrayDimension {
                        left: Some(i128::from(*left)),
                        right: Some(i128::from(*right)),
                    })
                    .collect::<Vec<_>>();
                result.extend(Self::query_dimensions_for(element));
                result
            }
            TypeShape::Container { element, .. } => {
                let mut result = vec![IrArrayDimension {
                    left: None,
                    right: None,
                }];
                result.extend(Self::query_dimensions_for(element));
                result
            }
            TypeShape::String => vec![IrArrayDimension {
                left: None,
                right: None,
            }],
            TypeShape::Real { .. } | TypeShape::Opaque { .. } | TypeShape::Aggregate(_) => {
                Vec::new()
            }
        }
    }

    fn query_unpacked_dimensions_for(descriptor: &TypeDescriptor) -> u32 {
        match &descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => u32::try_from(dimensions.len())
                .unwrap_or(u32::MAX)
                .saturating_add(Self::query_unpacked_dimensions_for(element)),
            TypeShape::Container { element, .. } => {
                1_u32.saturating_add(Self::query_unpacked_dimensions_for(element))
            }
            _ => 0,
        }
    }

    fn query_target(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(IrArrayQueryTarget, Vec<IrArrayDimension>), String> {
        let descriptor = self.query_descriptor(node).cloned().ok_or_else(|| {
            format!("array query argument has no owned type metadata in `{path}`")
        })?;
        let dimensions = Self::query_dimensions_for(&descriptor);
        if dimensions.is_empty() {
            return Err(format!(
                "array query argument in `{path}` has no queryable dimensions"
            ));
        }
        if let Some(container) = self.container_of(node) {
            return Ok((
                IrArrayQueryTarget::Container {
                    container: container.ir,
                    dimensions: dimensions.clone(),
                },
                dimensions,
            ));
        }
        if self
            .object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::String)
        {
            return Ok((
                IrArrayQueryTarget::String {
                    value: self.lower_string(path, node)?,
                    dimensions: dimensions.clone(),
                },
                dimensions,
            ));
        }
        Ok((
            IrArrayQueryTarget::Static {
                dimensions: dimensions.clone(),
            },
            dimensions,
        ))
    }

    fn query_integer(value: i128) -> IrExpr {
        IrExpr::resize_to(lhs_integer_expr(value), 32, true)
    }

    fn static_array_query(kind: IrArrayQueryKind, dimension: IrArrayDimension) -> Option<i128> {
        let (Some(left), Some(right)) = (dimension.left, dimension.right) else {
            return None;
        };
        match kind {
            IrArrayQueryKind::Left => Some(left),
            IrArrayQueryKind::Right => Some(right),
            IrArrayQueryKind::Low => Some(left.min(right)),
            IrArrayQueryKind::High => Some(left.max(right)),
            IrArrayQueryKind::Increment => Some(if left >= right { 1 } else { -1 }),
            IrArrayQueryKind::Size => left
                .checked_sub(right)
                .and_then(|extent| extent.unsigned_abs().checked_add(1))
                .and_then(|size| i128::try_from(size).ok()),
        }
    }

    fn lower_array_query(
        &mut self,
        path: &str,
        name: &str,
        args: &[NodeId],
    ) -> Result<IrExpr, String> {
        let [first, rest @ ..] = args else {
            return Err(format!("{name} requires one or two arguments in `{path}`"));
        };
        if rest.len() > 1 {
            return Err(format!("{name} requires one or two arguments in `{path}`"));
        }
        let (target, dimensions) = self.query_target(path, *first)?;
        let dimension_node = rest.first().copied();
        let known_dimension = dimension_node.and_then(|node| self.eval_bound_i128(node).ok());
        if let Some(index) = known_dimension {
            if index < 1
                || usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index > dimensions.len())
            {
                return Err(format!(
                    "{name} dimension {index} is outside the queryable range in `{path}`"
                ));
            }
        }
        let nested_runtime_dimension = dimensions
            .iter()
            .skip(1)
            .any(|dimension| dimension.left.is_none() || dimension.right.is_none());
        if nested_runtime_dimension && dimension_node.is_some() && known_dimension != Some(1) {
            return Err(format!(
                "{name} cannot select a nested runtime dimension in `{path}`"
            ));
        }
        let selected = known_dimension
            .and_then(|index| usize::try_from(index - 1).ok())
            .and_then(|index| dimensions.get(index).copied());
        if let Some(selected) = selected {
            if let Some(value) = Self::static_array_query(
                match name {
                    "$left" => IrArrayQueryKind::Left,
                    "$right" => IrArrayQueryKind::Right,
                    "$low" => IrArrayQueryKind::Low,
                    "$high" => IrArrayQueryKind::High,
                    "$increment" => IrArrayQueryKind::Increment,
                    "$size" => IrArrayQueryKind::Size,
                    _ => unreachable!(),
                },
                selected,
            ) {
                if selected.left.is_some() {
                    return Ok(Self::query_integer(value));
                }
            }
        }
        let kind = match name {
            "$left" => IrArrayQueryKind::Left,
            "$right" => IrArrayQueryKind::Right,
            "$low" => IrArrayQueryKind::Low,
            "$high" => IrArrayQueryKind::High,
            "$increment" => IrArrayQueryKind::Increment,
            "$size" => IrArrayQueryKind::Size,
            _ => unreachable!(),
        };
        let dimension = dimension_node
            .map(|node| self.lower_expr(path, node).map(Box::new))
            .transpose()?;
        let query = IrArrayQuery {
            kind,
            target,
            dimension,
        };
        let (width, signed) = query.result_type(&self.model);
        Ok(IrExpr::new(
            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::ArrayQuery(query))),
            width,
            signed,
            None,
        ))
    }

    /// Render context for the IR built so far (the enclosing function, when
    /// any, resolves formal reads).
    fn render_ctx(&self) -> RCtx<'_> {
        RCtx {
            model: &self.model,
            func: self.cur_fn_ir.map(|i| &self.model.funcs[i]),
            sampled: false,
            activation_label: None,
        }
    }

    /// Render a lowered expression to its C code.
    pub(super) fn render_ir_code(&self, ir: &IrExpr) -> Result<String, String> {
        let ctx = self.render_ctx();
        Ok(render_expr(&ctx, ir)?.code)
    }

    /// Lower an expression node decision-for-decision like the pre-IR
    /// emitter: same widths, signednesses, fills, and error strings.
    pub(super) fn lower_expr(&mut self, scope_path: &str, h: NodeId) -> Result<IrExpr, String> {
        if let Some(value) = self.lower_container_query(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_object_query(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.class_field_expr(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_enum_method(scope_path, h)? {
            return Ok(value);
        }
        if matches!(self.kind(h), NodeKind::MethodCall { .. }) && self.is_class_method_call(h) {
            let (name, callee) = match self.kind(h) {
                NodeKind::MethodCall { name, callee, .. } => (name.clone(), *callee),
                _ => return Err("malformed class method call".to_owned()),
            };
            let receiver = self.class_method_receiver(h)?;
            let mut value = self.lower_func_call_expr(scope_path, h, &name, callee)?;
            if let IrExprKind::CallFn(call) = &mut value.kind {
                call.receiver = receiver;
            }
            return Ok(value);
        }
        match self.kind(h) {
            NodeKind::Expr(ExprKind::Constant { .. }) => {
                if let Some(comparison) = self.recover_folded_real_parameter_comparison(h) {
                    return Ok(comparison);
                }
                let c = self.const_of_node(h)?;
                Ok(IrExpr::new(
                    IrExprKind::Const(c.clone()),
                    c.width,
                    c.signed,
                    c.fill,
                ))
            }
            NodeKind::EnumConst { value } => match value {
                Some(Val::Bits(v)) => {
                    let c = val_to_const(v)?;
                    Ok(IrExpr::new(
                        IrExprKind::Const(c.clone()),
                        c.width,
                        c.signed,
                        None,
                    ))
                }
                Some(Val::Real(v)) => Ok(real_literal_expr(*v)),
                Some(Val::Str(_)) => Err("string enum constant in expression".to_string()),
                None => Err("enum constant without value in expression".to_string()),
            },
            NodeKind::Expr(ExprKind::ScopeRef { target }) => {
                if let Some(info) = self.sampled_signal_of(*target) {
                    self.signal_read_expr(info)
                } else {
                    Err(format!(
                        "clocking block scope `{}` is not a value in `{scope_path}`",
                        self.node(*target).name
                    ))
                }
            }
            NodeKind::Expr(ExprKind::Ref { target }) => self.lower_ref_expr(scope_path, h, *target),
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(ai) = self.array_of(*base).cloned() {
                    if ai.real {
                        return Err(format!(
                            "select on a real array element in `{scope_path}` is not supported"
                        ));
                    }
                    if ai.dims.len() != 1 {
                        return Err(format!(
                            "array slice access (`{}[...]` on a {}-dimensional array) \
                             is not supported in `{scope_path}`",
                            self.node(*base).name,
                            ai.dims.len()
                        ));
                    }
                    let ie = self.lower_expr(scope_path, *index)?;
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: vec![ie],
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, &[*index])?
                {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, &[*index])? {
                    if width > 1 {
                        let right = i64::from(lsb);
                        return Ok(IrExpr::new(
                            IrExprKind::PartSel {
                                base: Box::new(self.signal_read_expr(&info)?),
                                left: right + i64::from(width) - 1,
                                right,
                            },
                            width,
                            false,
                            None,
                        ));
                    }
                }
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let ie = self.lower_member_select_index(scope_path, *base, *index)?;
                Ok(IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(base_value),
                        idx: Box::new(ie),
                    },
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(h) {
                    let member = member_info.member;
                    let signal = member_info.signal.ok_or_else(|| {
                        if member_info.object.is_some() {
                            format!(
                                "string aggregate member `{}` must be used in a string context",
                                member.name
                            )
                        } else {
                            format!("aggregate member `{}` has no scalar storage", member.name)
                        }
                    })?;
                    let value = if signal.real {
                        self.signal_read_expr(&signal)?
                    } else {
                        IrExpr::resize_to(
                            self.signal_read_expr(&signal)?,
                            member.ty.width.ok_or_else(|| {
                                format!("unpacked member `{}` has unresolved width", member.name)
                            })?,
                            member.ty.signed,
                        )
                    };
                    return Ok(if member.two_state && !signal.real {
                        IrExpr::to_two_state(value)
                    } else {
                        value
                    });
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, indices)?
                {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, indices)? {
                    let right = i64::from(lsb);
                    return Ok(IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: right + i64::from(width) - 1,
                            right,
                        },
                        width,
                        false,
                        None,
                    ));
                }
                let ai = self.array_of(*base).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve array base of select `{}` in `{scope_path}` (base kind: {:?})",
                        self.node(*base).name,
                        self.kind(*base)
                    )
                })?;
                let ndims = ai.dims.len();
                if indices.len() == ndims {
                    let ies = indices
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: ies,
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                if indices.len() == ndims + 1 {
                    if ai.real {
                        return Err(format!(
                            "select on a real array element in `{scope_path}` is not supported"
                        ));
                    }
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let (elem_sel, width) = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?;
                            let r =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?;
                            let (l, r, width) =
                                checked_select_bounds(l, r, "array-element part select")?;
                            (IrElemSel::Part(l, r), width)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect {
                            base_expr,
                            width_expr,
                            neg,
                            ..
                        }) => {
                            let width = self.indexed_part_select_width(*width_expr, scope_path)?;
                            (
                                IrElemSel::Indexed {
                                    base: Box::new(
                                        self.lower_packed_index(scope_path, *base, *base_expr)?,
                                    ),
                                    width,
                                    negative: *neg ^ self.packed_range_ascending(*base),
                                },
                                width,
                            )
                        }
                        _ => {
                            let ie = self.lower_packed_index(scope_path, *base, last)?;
                            (IrElemSel::Bit(Box::new(ie)), 1)
                        }
                    };
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: ies,
                            elem_sel,
                        },
                        width,
                        false,
                        None,
                    ));
                }
                Err(format!(
                    "array `{}` in `{scope_path}`: {}-level select on a \
                     {}-dimensional array is not supported",
                    self.node(*base).name,
                    indices.len(),
                    ndims
                ))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                if let Some((info, member, lsb, width)) = self.packed_member_range_info(
                    *base,
                    self.eval_bound_i128(*left)?,
                    self.eval_bound_i128(*right)?,
                )? {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                let mut l = self.eval_bound_i128(*left)?;
                let mut r = self.eval_bound_i128(*right)?;
                if let Some((_, member)) = self.packed_member_info(*base) {
                    l = i128::from(self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        l,
                    )?);
                    r = i128::from(self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        r,
                    )?);
                } else {
                    l = self.packed_relative_bound(*base, l)?;
                    r = self.packed_relative_bound(*base, r)?;
                }
                let (l, r, width) = checked_select_bounds(l, r, "part select")?;
                Ok(IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(base_value),
                        left: l,
                        right: r,
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let be = self.lower_packed_index(scope_path, *base, *base_expr)?;
                let we = self.lower_expr(scope_path, *width_expr)?;
                let width = self.indexed_part_select_width(*width_expr, scope_path)?;
                Ok(IrExpr::new(
                    IrExprKind::IdxPartSel {
                        base: Box::new(base_value),
                        base_idx: Box::new(be),
                        width_expr: Box::new(we),
                        neg: *neg ^ self.packed_range_ascending(*base),
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::Streaming {
                direction,
                slice_size,
                streams,
            }) => {
                if streams.is_empty() {
                    return Err(format!("empty streaming concatenation in `{scope_path}`"));
                }
                let mut parts = Vec::with_capacity(streams.len());
                for stream in streams {
                    let value =
                        self.lower_stream_operand(scope_path, stream.value, stream.with_expr)?;
                    if value.is_real() {
                        return Err(format!(
                            "streaming concatenation of real value in `{scope_path}` is not supported"
                        ));
                    }
                    parts.push(value);
                }
                let runtime_sized = parts.iter().any(|part| part.width == LLG_MAX_WIDTH);
                let width = if runtime_sized {
                    Some(LLG_MAX_WIDTH)
                } else {
                    parts.iter().try_fold(0u32, |width, part| {
                        width
                            .checked_add(part.width)
                            .filter(|width| *width <= LLG_MAX_WIDTH)
                    })
                }
                .ok_or_else(|| {
                    format!("streaming concatenation in `{scope_path}` has an oversized operand")
                })?;
                let value = if let [value] = parts.as_slice() {
                    value.clone()
                } else {
                    IrExpr::new(IrExprKind::Concat { parts }, width, false, None)
                };
                let slice = if *slice_size == 0 {
                    1
                } else {
                    (*slice_size).min(u64::from(width)) as u32
                };
                Ok(IrExpr::new(
                    IrExprKind::Stream {
                        value: Box::new(value),
                        slice,
                        direction: match direction {
                            DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                            DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
                        },
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                assignment,
                operands,
            }) if !*reordered
                && !*assignment
                && matches!(op, Operation::LogicalAnd | Operation::LogicalOr)
                && operands.len() == 2 =>
            {
                self.lower_logical_chain(scope_path, *op, operands)
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                assignment,
                operands,
            }) => self.lower_operation(scope_path, *op, *reordered, *assignment, operands),
            NodeKind::Expr(ExprKind::Cast {
                operand,
                ty,
                size_cast,
                size_cast_expr,
                cast_kind_known,
                two_state,
                propagated,
            }) => {
                if !cast_kind_known {
                    return Err(format!(
                        "cast kind cannot be determined without admitted source or semantic type metadata in `{scope_path}`"
                    ));
                }
                if matches!(ty.kind.as_str(), "real" | "shortreal") {
                    let v = self.lower_expr(scope_path, *operand)?;
                    return Ok(IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(v),
                            shortreal: ty.kind == "shortreal",
                        },
                        REAL_EXPR_WIDTH,
                        true,
                        None,
                    ));
                }
                let bitstream_source = self.lower_bitstream_source(scope_path, *operand)?;
                let source_value = match &bitstream_source {
                    Some(value) => value.clone(),
                    None => self.lower_expr(scope_path, *operand)?,
                };
                let target_width = size_cast_expr
                    .as_deref()
                    .and_then(|expression| self.source_size_cast_width(expression))
                    .or(ty.width);
                let (w, s) = match (target_width, ty.signed) {
                    (Some(w), s) => (w, if *size_cast { source_value.signed } else { s }),
                    (None, _) => {
                        return Err(format!(
                            "cast with unsized target type `{}` in `{scope_path}`",
                            ty.kind
                        ))
                    }
                };
                let v = source_value;
                if w > LLG_MAX_WIDTH {
                    return Err(format!(
                        "cast target in `{scope_path}` is {w} bits wide; the v1 \
                         runtime maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                // Context propagation extends using the target signedness
                // (§11.8.2); assignment and explicit casts use the source (§11.8.3).
                let v = if *propagated {
                    let source_width = v.width;
                    IrExpr::resize_to(v, source_width, s)
                } else {
                    v
                };
                if let Some(source) = bitstream_source {
                    if source.width != w {
                        return Err(format!(
                            "bit-stream cast source is {} bits but target is {} bits in `{scope_path}`",
                            source.width, w
                        ));
                    }
                    return Ok(IrExpr::new(
                        IrExprKind::BitStreamCast {
                            a: Box::new(source),
                            source_width: w,
                            target_two_state: *two_state || is_two_state_kind(&ty.kind),
                        },
                        w,
                        s,
                        None,
                    ));
                }
                // Value-preserving conversion (LRM 1800-2009 §6.24.1: the
                // cast yields the value a variable of the cast type holds
                // after the assignment — extension follows the SOURCE's
                // signedness, so int'(8'hFF) is 255, not -1).
                ir_to_explicit_cast_storage(v, w, s, *two_state || is_two_state_kind(&ty.kind))
            }
            NodeKind::SysCall { name } => self.lower_sys_func_expr(scope_path, name, h),
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                if *is_task {
                    return Err(format!(
                        "task call `{name}` used as an expression in `{scope_path}`"
                    ));
                }
                self.lower_func_call_expr(scope_path, h, name, *callee)
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if name == "triggered" => {
                let target = self.event_target_of(*receiver).ok_or_else(|| {
                    format!(
                        "sequence `.triggered` status is not supported for an unresolved receiver in `{scope_path}`"
                    )
                })?;
                let event = self.event_ref_of(&target, scope_path)?;
                Ok(IrExpr::new(
                    IrExprKind::EventTriggered(event),
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::MethodCall { name, .. } if name == "matched" => Err(format!(
                "sequence `.matched` status is not supported in `{scope_path}`"
            )),
            NodeKind::MethodCall { name, .. } if name == "triggered" => Err(format!(
                "sequence `.triggered` status is not supported in `{scope_path}`"
            )),
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(h) {
                    let member = member_info.member;
                    let signal = member_info.signal.ok_or_else(|| {
                        if member_info.object.is_some() {
                            format!(
                                "string aggregate member `{}` must be used in a string context",
                                member.name
                            )
                        } else {
                            format!("aggregate member `{}` has no scalar storage", member.name)
                        }
                    })?;
                    let member_value = if signal.real {
                        self.signal_read_expr(&signal)?
                    } else {
                        IrExpr::resize_to(
                            self.signal_read_expr(&signal)?,
                            member.ty.width.ok_or_else(|| {
                                format!("unpacked member `{}` has unresolved width", member.name)
                            })?,
                            member.ty.signed,
                        )
                    };
                    return Ok(if member.two_state && !member_value.is_real() {
                        IrExpr::to_two_state(member_value)
                    } else {
                        member_value
                    });
                }
                if let Some((info, member)) = self.packed_member_info(h) {
                    let base = self.signal_read_expr(&info)?;
                    let member_value = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(base),
                            left: i64::from(member.lsb + member.width - 1),
                            right: i64::from(member.lsb),
                        },
                        member.width,
                        false,
                        None,
                    );
                    let selected = IrExpr::resize_to(member_value, member.width, member.signed);
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                // Interface and ordinary hierarchical members both resolve
                // to their concrete owned storage identity.
                if let Some(info) = self.hier_path_signal(h) {
                    return self.signal_read_expr(info);
                }
                Err(format!(
                    "hierarchical reference `{}` is not supported (in `{scope_path}`): {:?}",
                    self.node(h).name,
                    self.kind(h)
                ))
            }
            other => Err(format!(
                "unsupported expression in `{scope_path}` (node kind {other:?})"
            )),
        }
    }

    fn recover_folded_real_parameter_comparison(&self, node: NodeId) -> Option<IrExpr> {
        let NodeKind::Expr(ExprKind::Constant {
            source: ConstantSource::Exact(source),
            size: 1,
            ..
        }) = self.kind(node)
        else {
            return None;
        };
        let (left, op, right) = [
            ("!=", IrBinOp::Neq),
            ("==", IrBinOp::Eq),
            ("<=", IrBinOp::Le),
            (">=", IrBinOp::Ge),
            ("<", IrBinOp::Lt),
            (">", IrBinOp::Gt),
        ]
        .into_iter()
        .find_map(|(token, op)| {
            source
                .split_once(token)
                .map(|(left, right)| (left.trim(), op, right.trim()))
        })?;

        let mut scope = self.node(node).parent;
        let mut lexical_scopes = Vec::new();
        let scope = loop {
            let candidate = scope?;
            if matches!(
                self.kind(candidate),
                NodeKind::ModuleInst { .. } | NodeKind::GenScope
            ) {
                break candidate;
            }
            lexical_scopes.push(candidate);
            scope = self.node(candidate).parent;
        };
        let real_parameter = |name: &str| {
            let shadowed = lexical_scopes.iter().any(|scope| {
                self.node(*scope).children.iter().any(|declaration| {
                    self.node(*declaration).name == name
                        && matches!(
                            self.kind(*declaration),
                            NodeKind::Var { .. }
                                | NodeKind::Array { .. }
                                | NodeKind::Param { .. }
                                | NodeKind::FuncArg { .. }
                        )
                })
            });
            if shadowed {
                return None;
            }
            self.node(scope).children.iter().find_map(|parameter| {
                (self.node(*parameter).name == name)
                    .then(|| self.param_vals.get(parameter))
                    .flatten()
                    .and_then(|value| match value {
                        Val::Real(value) => Some(*value),
                        Val::Bits(_) | Val::Str(_) => None,
                    })
            })
        };
        let reverse = |op| match op {
            IrBinOp::Lt => IrBinOp::Gt,
            IrBinOp::Le => IrBinOp::Ge,
            IrBinOp::Gt => IrBinOp::Lt,
            IrBinOp::Ge => IrBinOp::Le,
            other => other,
        };
        if let (Some(parameter), Some(literal)) =
            (real_parameter(left), parse_decimal_real_literal(right))
        {
            return Some(cmp_expr_ir(
                op,
                real_literal_expr(parameter),
                real_literal_expr(literal),
            ));
        }
        if let (Some(literal), Some(parameter)) =
            (parse_decimal_real_literal(left), real_parameter(right))
        {
            return Some(cmp_expr_ir(
                reverse(op),
                real_literal_expr(parameter),
                real_literal_expr(literal),
            ));
        }
        None
    }

    fn lower_ref_expr(
        &mut self,
        scope_path: &str,
        r: NodeId,
        target: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        if let Some(iterator) = self.container_iterator {
            if target == Some(iterator.node) {
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead("__llg_method_item".to_owned()),
                    iterator.item_width,
                    iterator.item_signed,
                    None,
                ));
            }
        }
        if let Some(captured) = self
            .capture_target(r)
            .or_else(|| target.filter(|target| self.capture_locals.contains_key(target)))
        {
            let binding = self
                .capture_binding(captured)
                .expect("capture target must have a binding");
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(Codegen::capture_local_name(binding.storage)),
                binding.local.width,
                binding.local.signed,
                None,
            ));
        }
        if self.lexical_proc_string_local(r).is_some() {
            return Err(format!(
                "string procedural local `{}` cannot be used as a packed expression in `{scope_path}`",
                self.node(r).name
            ));
        }
        if let Some((_, info)) = self.lexical_proc_local(r) {
            if let Some(signal) = &info.static_signal {
                return self.signal_read_expr(signal);
            }
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(info.c_name.clone()),
                info.width,
                info.signed,
                None,
            ));
        }
        if let Some(t) = target {
            let t = self.canonical_func_target(t).unwrap_or(t);
            if let Some(binding) = self.capture_binding(t) {
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead(Codegen::capture_local_name(binding.storage)),
                    binding.local.width,
                    binding.local.signed,
                    None,
                ));
            }
            if self.unpacked_aggregates.contains_key(&t) {
                return Err(format!(
                    "whole unpacked aggregate `{}` is not supported in scalar expression `{scope_path}`",
                    self.node(t).name
                ));
            }
            if let Some(info) = self.sampled_signal_of(t) {
                return self.signal_read_expr(info);
            }
            if let Some(info) = self.signal_of(t) {
                return self.signal_read_expr(info);
            }
            if !self.proc_local_is_shadowed(r) {
                if let Some(info) = self.proc_local_info(t) {
                    if let Some(signal) = &info.static_signal {
                        return self.signal_read_expr(signal);
                    }
                    return Ok(IrExpr::new(
                        IrExprKind::LocalRead(info.c_name.clone()),
                        info.width,
                        info.signed,
                        None,
                    ));
                }
            }
            // Function/task body reads: formals, locals and the return
            // variable (by arena node).
            if let Some(f) = &self.func {
                if let Some(ir) = f.arg_ir.get(&t) {
                    return Ok(ir.clone());
                }
                if let Some((cname, w, s, _, _shortreal)) = f.locals.get(&t) {
                    return Ok(IrExpr::new(
                        IrExprKind::LocalRead(cname.clone()),
                        *w,
                        *s,
                        None,
                    ));
                }
                if f.ret_node == Some(t) {
                    if let Some(rctx) = &f.ret {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(rctx.c_name.clone()),
                            rctx.width,
                            rctx.signed,
                            None,
                        ));
                    }
                }
            }
            if let Some(v) = self.param_vals.get(&t) {
                return match v {
                    Val::Bits(b) => {
                        let c = val_to_const(b)?;
                        Ok(IrExpr::new(
                            IrExprKind::Const(c.clone()),
                            c.width,
                            c.signed,
                            None,
                        ))
                    }
                    Val::Real(value) => Ok(real_literal_expr(*value)),
                    Val::Str(value) => match self.kind(t) {
                        NodeKind::Param { ty, .. } if ty.kind != "string" => match ty.width {
                            Some(width) => {
                                let c = string_to_const(value)?;
                                let expr = IrExpr::new(
                                    IrExprKind::Const(c.clone()),
                                    c.width,
                                    c.signed,
                                    None,
                                );
                                Ok(IrExpr::convert_to(expr, width, ty.signed))
                            }
                            None => Err(format!(
                                "string parameter `{}` used as a value is not supported",
                                self.node(t).name
                            )),
                        },
                        _ => Err(format!(
                            "string parameter `{}` used as a value is not supported",
                            self.node(t).name
                        )),
                    },
                };
            }
            if let NodeKind::EnumConst { value } = self.kind(t) {
                return enum_value_expr(value.as_ref(), &self.node(t).name);
            }
            return Err(format!(
                "cannot resolve bound expression reference `{}` in `{scope_path}`",
                self.node(r).name
            ));
        }
        // Unbound enum references can still arise in the flat definition
        // view. Any captured target identity must resolve above.
        let name = self.node(r).name.clone();
        if !name.is_empty() {
            // io_decls are not indexed, so formals resolve by name.
            if let Some(f) = &self.func {
                for (io, ir) in &f.arg_ir {
                    if self.node(*io).name == name {
                        return Ok(ir.clone());
                    }
                }
                for (node, (cname, w, s, _, _shortreal)) in &f.locals {
                    if self.node(*node).name == name {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(cname.clone()),
                            *w,
                            *s,
                            None,
                        ));
                    }
                }
                if let Some(rctx) = &f.ret {
                    if rctx
                        .node
                        .map(|n| self.node(n).name == name)
                        .unwrap_or(false)
                    {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(rctx.c_name.clone()),
                            rctx.width,
                            rctx.signed,
                            None,
                        ));
                    }
                }
            }
            if let Some(info) = self
                .scope_sig_names
                .get(scope_path)
                .and_then(|m| m.get(&name))
            {
                return self.signal_read_expr(info);
            }
            // Some unqualified module-local enum uses can lack a resolved
            // target. Resolve those only against the current
            // instance's matching flat module definition and only when the
            // enumerator name is unique there.
            let def_name = match self.kind(self.inst) {
                NodeKind::ModuleInst { def_name, .. } => strip_lib(def_name),
                _ => String::new(),
            };
            let mut matches = self
                .db
                .flat_modules()
                .iter()
                .filter(|module| match self.kind(**module) {
                    NodeKind::ModuleInst {
                        def_name: candidate,
                        ..
                    } => strip_lib(candidate) == def_name,
                    _ => false,
                })
                .flat_map(|module| self.node(*module).children.iter())
                .filter_map(|candidate| match self.kind(*candidate) {
                    NodeKind::EnumConst { value } if self.node(*candidate).name == name => {
                        Some((value.as_ref(), self.node(*candidate).name.as_str()))
                    }
                    _ => None,
                });
            if let Some((value, enum_name)) = matches.next() {
                if matches.next().is_none() {
                    return enum_value_expr(value, enum_name);
                }
            }
        }
        Err(format!(
            "cannot resolve expression reference `{name}` in `{scope_path}`"
        ))
    }

    /// A plain constant node (`ExprKind::Constant`); used where the old code
    /// called `read_const` directly on a handle.
    pub(super) fn const_of_node(&self, node: NodeId) -> Result<IrConst, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                value,
                size,
                const_type,
                source,
                time_scale,
                ..
            }) => {
                let mut c = if let Some(fill) = self.source_fill_literal(node) {
                    IrConst {
                        bits: vec![(fill == 1) as u64],
                        x: vec![(fill == 2) as u64],
                        z: vec![(fill == 3) as u64],
                        width: 1,
                        signed: false,
                        real: None,
                        fill: Some(fill),
                    }
                } else {
                    read_const_from(value, *size)?
                };
                if *const_type == ConstantType::Time && self.round_time_literals {
                    let raw = c.real_value().ok_or_else(|| {
                        format!(
                            "time literal at {}:{}:{} has no real value",
                            self.node(node).file.as_deref().unwrap_or("<unknown>"),
                            self.node(node).line,
                            self.node(node).col
                        )
                    })?;
                    c = IrConst::real(self.rounded_time_literal(node, raw, source, *time_scale)?);
                }
                let (signed, literal_width) = self.signed_based_literal_info(node);
                if signed {
                    if let Some(width) = literal_width {
                        if width < c.width {
                            c = read_const_from(value, width as i32)?;
                        }
                    }
                    c.signed = true;
                }
                Ok(c)
            }
            _ => Err("unsupported constant value format".to_string()),
        }
    }

    pub(super) fn indexed_part_select_width(
        &self,
        node: NodeId,
        scope_path: &str,
    ) -> Result<u32, String> {
        let value = self.eval_bound_i128(node).map_err(|_| {
            format!("indexed part-select width must be a constant in `{scope_path}`")
        })?;
        let width = u32::try_from(value)
            .map_err(|_| format!("indexed part-select width must be positive in `{scope_path}`"))?;
        if width == 0 {
            return Err(format!(
                "indexed part-select width must be positive in `{scope_path}`"
            ));
        }
        if width > LLG_MAX_WIDTH {
            return Err(format!(
                "indexed part-select width {width} exceeds maximum {LLG_MAX_WIDTH} in `{scope_path}`"
            ));
        }
        Ok(width)
    }

    /// Lower one operation, mirroring the pre-IR emitter's operand shapes,
    /// result widths/signedness and error strings arm-for-arm.
    fn lower_logical_chain(
        &mut self,
        scope_path: &str,
        operation: Operation,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        let mut pending = operands.iter().rev().copied().collect::<Vec<_>>();
        let mut values = Vec::new();
        while let Some(node) = pending.pop() {
            match self.kind(node) {
                NodeKind::Expr(ExprKind::Operation {
                    op,
                    reordered: false,
                    assignment: false,
                    operands,
                }) if *op == operation && operands.len() == 2 => {
                    pending.push(operands[1]);
                    pending.push(operands[0]);
                }
                _ => values.push(self.lower_expr(scope_path, node)?),
            }
        }
        let mut values = values.into_iter();
        let first = values
            .next()
            .ok_or_else(|| format!("logical operation has no operands in `{scope_path}`"))?;
        let ir_operation = if operation == Operation::LogicalAnd {
            IrBinOp::LogAnd
        } else {
            IrBinOp::LogOr
        };
        Ok(values.fold(first, |left, right| cmp_expr_ir(ir_operation, left, right)))
    }

    fn lower_operation(
        &mut self,
        scope_path: &str,
        otype: Operation,
        reordered: bool,
        assignment: bool,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        super::validate_operation_arity(otype, operands.len(), scope_path)?;
        if assignment
            || matches!(
                otype,
                Operation::PreIncrement
                    | Operation::PreDecrement
                    | Operation::PostIncrement
                    | Operation::PostDecrement
            )
        {
            return self.lower_mutation_expression(scope_path, otype, operands, assignment);
        }
        macro_rules! op {
            ($i:expr) => {
                self.lower_expr(scope_path, operands[$i])?
            };
        }
        let maxw = |a: &IrExpr, b: &IrExpr| a.width.max(b.width);

        match otype {
            Operation::Add => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Add, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Add, a, b, scope_path)
            }
            Operation::Subtract => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Sub, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Sub, a, b, scope_path)
            }
            Operation::Multiply => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Mul, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Mul, a, b, scope_path)
            }
            Operation::Divide | Operation::Modulo | Operation::Power => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    let rop = match otype {
                        Operation::Divide => IrRealBinOp::Div,
                        Operation::Modulo => IrRealBinOp::Mod,
                        _ => IrRealBinOp::Pow,
                    };
                    return Ok(real_bin_expr(rop, a, b));
                }
                let f = match otype {
                    Operation::Divide => IrBinOp::Div,
                    Operation::Modulo => IrBinOp::Mod,
                    _ => IrBinOp::Pow,
                };
                if matches!(f, IrBinOp::Div | IrBinOp::Mod) {
                    common_bin_expr_with_context(f, a, b, scope_path)
                } else {
                    let (width, signed) = (a.width, a.signed);
                    Ok(IrExpr::new(
                        IrExprKind::Bin {
                            op: f,
                            a: Box::new(a),
                            b: Box::new(b),
                        },
                        width,
                        signed,
                        None,
                    ))
                }
            }
            Operation::BitwiseAnd => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitAnd, a, b, scope_path)
            }
            Operation::BitwiseOr => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitOr, a, b, scope_path)
            }
            Operation::BitwiseXor => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitXor, a, b, scope_path)
            }
            Operation::BitwiseXnor => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitXNor, a, b, scope_path)
            }
            Operation::LogicalAnd => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogAnd, a, b))
            }
            Operation::LogicalOr => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogOr, a, b))
            }
            // This is the ordinary Boolean `->` expression.  Property
            // implication (`|->`/`|=>`) has separate operation tags and never
            // reaches this expression lowering path.
            Operation::Imply => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogImpl, a, b))
            }
            Operation::LogicalEquivalence => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogEquiv, a, b))
            }
            Operation::Equal => {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(value);
                }
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Eq, a, b, scope_path)
            }
            Operation::NotEqual => {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(value);
                }
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Neq, a, b, scope_path)
            }
            Operation::CaseEqual => {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(value);
                }
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                common_cmp_expr_ir(IrBinOp::CaseEq, a, b, scope_path)
            }
            Operation::CaseNotEqual => {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(value);
                }
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                common_cmp_expr_ir(IrBinOp::CaseNeq, a, b, scope_path)
            }
            Operation::WildEqual | Operation::WildNotEqual => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "wildcard equality on real value in `{scope_path}` is not supported"
                    ));
                }
                let width = maxw(&a, &b);
                let signed = a.signed && b.signed;
                let a = wildcard_operand_with_context(a, width, signed, scope_path)?;
                let b = wildcard_operand_with_context(b, width, signed, scope_path)?;
                let op = if otype == Operation::WildEqual {
                    IrBinOp::WildEq
                } else {
                    IrBinOp::WildNeq
                };
                Ok(cmp_expr_ir(op, a, b))
            }
            Operation::Less => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Lt, a, b, scope_path)
            }
            Operation::LessEqual => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Le, a, b, scope_path)
            }
            Operation::Greater => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Gt, a, b, scope_path)
            }
            Operation::GreaterEqual => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Ge, a, b, scope_path)
            }
            Operation::ShiftLeft
            | Operation::ShiftRight
            | Operation::ArithmeticShiftLeft
            | Operation::ArithmeticShiftRight => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "shift on real value in `{scope_path}` is not supported"
                    ));
                }
                let f = match otype {
                    Operation::ShiftLeft => IrBinOp::Shl,
                    Operation::ShiftRight => IrBinOp::Shr,
                    Operation::ArithmeticShiftLeft => IrBinOp::Ashl,
                    _ => IrBinOp::Ashr,
                };
                let (w, s) = (a.width, a.signed);
                Ok(IrExpr::new(
                    IrExprKind::Bin {
                        op: f,
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    w,
                    s,
                    None,
                ))
            }
            Operation::Conditional => {
                let sel = op!(0);
                let a = op!(1);
                let b = op!(2);
                let (w, s) = if a.is_real() || b.is_real() {
                    (REAL_EXPR_WIDTH, true)
                } else {
                    (maxw(&a, &b), a.signed && b.signed)
                };
                let (a, b) = if w == REAL_EXPR_WIDTH {
                    (a, b)
                } else {
                    (
                        checked_operand_with_context(a, w, s, scope_path, "conditional context")?,
                        checked_operand_with_context(b, w, s, scope_path, "conditional context")?,
                    )
                };
                Ok(IrExpr::new(
                    IrExprKind::Mux {
                        sel: Box::new(sel),
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    w,
                    s,
                    None,
                ))
            }
            Operation::UnaryMinus => {
                let a = op!(0);
                let w = a.width;
                if a.is_real() {
                    return Ok(real_un_expr(a));
                }
                // Unary minus of an unsized decimal literal (`-3`): a normalized
                // literal can be an unsigned 64-bit constant,
                // dropping the LRM signedness (unsized decimal literals are
                // signed, LRM 5.7.1).  Restore it so `$display("%d", -3)`
                // prints "-3" instead of the unsigned wrap.  Sized radix
                // literals (`-4'h3`) stay unsigned per the LRM.
                let signed_lit = matches!(
                    self.kind(operands[0]),
                    NodeKind::Expr(ExprKind::Constant {
                        value: ValueData::UInt(_),
                        ..
                    })
                ) && !a.signed;
                let s = a.signed;
                let neg = IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::Neg,
                        a: Box::new(a),
                    },
                    w,
                    s && !signed_lit,
                    None,
                );
                if signed_lit {
                    Ok(IrExpr::resize_to(neg, w, true))
                } else {
                    Ok(neg)
                }
            }
            Operation::UnaryPlus => {
                let a = op!(0);
                Ok(a)
            }
            Operation::LogicalNot => {
                let a = op!(0);
                Ok(IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::LogNot,
                        a: Box::new(a),
                    },
                    1,
                    false,
                    None,
                ))
            }
            Operation::BitwiseNot => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "bitwise negation of real value in `{scope_path}` is not supported"
                    ));
                }
                let w = a.width;
                let s = a.signed;
                Ok(IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::BitNeg,
                        a: Box::new(a),
                    },
                    w,
                    s,
                    None,
                ))
            }
            Operation::ReductionAnd => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedAnd, a))
            }
            Operation::ReductionNand => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNand, a))
            }
            Operation::ReductionOr => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedOr, a))
            }
            Operation::ReductionNor => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNor, a))
            }
            Operation::ReductionXor => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXor, a))
            }
            Operation::ReductionXnor => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXNor, a))
            }
            Operation::Inside => {
                let Some((value_node, item_nodes)) = operands.split_first() else {
                    return Err(format!("empty inside expression in `{scope_path}`"));
                };
                if item_nodes.is_empty() {
                    return Err(format!("inside set is empty in `{scope_path}`"));
                }
                if self.is_chandle_expr(scope_path, *value_node) {
                    return Err(format!(
                        "chandle-valued inside selector is not supported in `{scope_path}`"
                    ));
                }
                if self.is_string_expr(scope_path, *value_node) {
                    let value = self.lower_string(scope_path, *value_node)?;
                    let items = self.lower_inside_string_items(scope_path, item_nodes)?;
                    return Ok(object_query(
                        IrObjectQuery::StringInside { value, items },
                        1,
                        false,
                    ));
                }
                let value = self.lower_expr(scope_path, *value_node)?;
                let items = self.lower_inside_items(scope_path, item_nodes)?;
                Ok(IrExpr::new(
                    IrExprKind::Inside {
                        value: Box::new(value),
                        items,
                    },
                    1,
                    false,
                    None,
                ))
            }
            Operation::Concat => {
                let mut parts = Vec::new();
                for operand in operands {
                    parts.push(self.lower_expr(scope_path, *operand)?);
                }
                if reordered {
                    parts.reverse();
                }
                if parts.is_empty() {
                    return Err(format!("empty concatenation in `{scope_path}`"));
                }
                if parts.iter().any(|p| p.is_real()) {
                    return Err(format!(
                        "concatenation of real value in `{scope_path}` is not supported"
                    ));
                }
                let mut width = 0u32;
                for p in &parts {
                    width += p.width;
                }
                if width > LLG_MAX_WIDTH {
                    return Err(format!(
                        "concatenation in `{scope_path}` is {width} bits wide; \
                         the runtime maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Concat { parts },
                    width,
                    false,
                    None,
                ))
            }
            Operation::MultiConcat => {
                let count = {
                    let value = self.eval_bits(operands[0])?;
                    if value.is_unknown() {
                        return Err(format!("unknown replication count in `{scope_path}`"));
                    }
                    value
                        .to_u128()
                        .and_then(|count| u64::try_from(count).ok())
                        .ok_or_else(|| {
                            format!("replication count does not fit in u64 in `{scope_path}`")
                        })?
                };
                let mut pat_parts = Vec::new();
                for operand in operands.iter().skip(1) {
                    pat_parts.push(self.lower_expr(scope_path, *operand)?);
                }
                if pat_parts.is_empty() {
                    return Err(format!("empty replication in `{scope_path}`"));
                }
                if pat_parts.iter().any(|p| p.is_real()) {
                    return Err(format!(
                        "replication of real value in `{scope_path}` is not supported"
                    ));
                }
                let mut pwidth = pat_parts[0].width as u128;
                for p in &pat_parts[1..] {
                    pwidth += p.width as u128;
                }
                let total = pwidth * count as u128;
                if total > LLG_MAX_WIDTH as u128 {
                    return Err(format!(
                        "replication in `{scope_path}` is {total} bits wide; \
                         the runtime maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Replicate {
                        count,
                        parts: pat_parts,
                    },
                    total as u32,
                    false,
                    None,
                ))
            }
            Operation::Cast => Err(format!(
                "cast expressions are not supported in `{scope_path}` \
                 (the database does not capture the cast typespec)"
            )),
            Operation::MinTypMax => {
                let a = op!(0);
                Ok(a)
            }
            other => Err(format!(
                "unsupported operation op type {other:?} in `{scope_path}`"
            )),
        }
    }

    fn lower_mutation_expression(
        &mut self,
        scope_path: &str,
        op: Operation,
        operands: &[NodeId],
        _assignment: bool,
    ) -> Result<IrExpr, String> {
        let lhs_node = *operands.first().ok_or_else(|| {
            format!("assignment-like expression in `{scope_path}` has no left operand")
        })?;
        let lhs = self.lower_lhs(scope_path, lhs_node)?;
        if matches!(lhs, IrLhs::Stream { .. }) {
            return Err(format!(
                "assignment-like expression to a streaming target in `{scope_path}` is not supported"
            ));
        }
        // Lowering the LHS expression here is only for its final type. The
        // runtime read is reconstructed from the canonical descriptor, so a
        // dynamic index is never evaluated by both the read and the write.
        let current_type = self.lower_expr(scope_path, lhs_node)?;
        let post = matches!(op, Operation::PostIncrement | Operation::PostDecrement);
        let reads_current = !matches!(op, Operation::Assignment);
        let value = if op == Operation::Assignment {
            let rhs_node = *operands.get(1).ok_or_else(|| {
                format!("assignment expression in `{scope_path}` has no right operand")
            })?;
            let rhs = self.lower_expr(scope_path, rhs_node)?;
            apply_lhs_assignment_context(&self.model, &lhs, rhs)
        } else {
            let current = IrExpr::new(
                IrExprKind::LocalRead("_llg_mut_current".to_owned()),
                current_type.width,
                current_type.signed,
                None,
            );
            let rhs = if matches!(
                op,
                Operation::PreIncrement
                    | Operation::PostIncrement
                    | Operation::PreDecrement
                    | Operation::PostDecrement
            ) {
                if current.is_real() {
                    real_literal_expr(1.0)
                } else {
                    IrExpr::new(
                        IrExprKind::Const(IrConst {
                            bits: vec![1],
                            x: vec![0],
                            z: vec![0],
                            width: 32,
                            signed: true,
                            real: None,
                            fill: None,
                        }),
                        32,
                        true,
                        None,
                    )
                }
            } else {
                let rhs_node = *operands.get(1).ok_or_else(|| {
                    format!("compound assignment in `{scope_path}` has no right operand")
                })?;
                self.lower_expr(scope_path, rhs_node)?
            };
            let arithmetic = if matches!(op, Operation::PreDecrement | Operation::PostDecrement) {
                Operation::Subtract
            } else if matches!(op, Operation::PreIncrement | Operation::PostIncrement) {
                Operation::Add
            } else {
                op
            };
            super::lower_compound_expr_ir(scope_path, arithmetic, current, rhs)?
        };
        Ok(IrExpr::new(
            IrExprKind::Mutation(Box::new(crate::sim::ir::IrMutationExpr {
                lhs,
                value: Box::new(value),
                current_width: current_type.width,
                current_signed: current_type.signed,
                reads_current,
                post,
            })),
            current_type.width,
            current_type.signed,
            None,
        ))
    }

    fn lower_member_select_index(
        &mut self,
        scope_path: &str,
        base: NodeId,
        index: NodeId,
    ) -> Result<IrExpr, String> {
        if let Some((_, member)) = self.packed_member_info(base) {
            let canonical = matches!(member.packed_ranges.as_slice(), [range]
                if range.right == 0 && range.left == i128::from(member.width) - 1);
            if !canonical {
                let relative = self.aggregate_member_relative_bound(
                    &member.name,
                    &member.packed_ranges,
                    self.eval_bound_i128(index)?,
                )?;
                return Ok(lhs_integer_expr(i128::from(relative)));
            }
        }
        self.lower_packed_index(scope_path, base, index)
    }

    fn dynamic_cast_lhs_shape(&self, lhs: &IrLhs) -> Result<(u32, bool, bool, bool), String> {
        Ok(match lhs {
            IrLhs::Whole(index) => match self.model.signal(*index).ty {
                IrType::Real { shortreal } => (0, true, false, shortreal),
                IrType::Packed {
                    width,
                    signed,
                    two_state,
                } => (width, signed, two_state, false),
            },
            IrLhs::WholeRef {
                width,
                signed,
                two_state,
                shortreal,
                ..
            } => (*width, *signed, *two_state, *shortreal),
            IrLhs::Ref {
                width,
                signed,
                two_state,
                ..
            } => (*width, *signed, *two_state, false),
            IrLhs::Bit(_, _, two_state) => (1, false, *two_state, false),
            IrLhs::Part(_, left, right, two_state) => {
                (left.abs_diff(*right) as u32 + 1, false, *two_state, false)
            }
            IrLhs::IdxPart(_, _, _, width, _, two_state) => (*width, false, *two_state, false),
            IrLhs::ArrayElem { arr, elem_sel, .. } => {
                let array = self.model.array(*arr);
                match elem_sel {
                    IrElemSel::Whole if array.real => (0, true, false, array.shortreal),
                    IrElemSel::Whole => (array.elem_width, array.signed, array.two_state, false),
                    IrElemSel::Part(left, right) => (
                        left.abs_diff(*right) as u32 + 1,
                        false,
                        array.two_state,
                        false,
                    ),
                    IrElemSel::Bit(_) => (1, false, array.two_state, false),
                    IrElemSel::Indexed { width, .. } => (*width, false, array.two_state, false),
                }
            }
            IrLhs::Stream { .. } => {
                return Err("$cast destination cannot be a streaming assignment target".to_owned())
            }
        })
    }

    /// Flatten one fixed-size unpacked value in the declaration order required
    /// by a bit-stream cast. Dynamic containers, strings, real leaves, and
    /// unions remain outside this fixed-size lowering boundary.
    pub(super) fn lower_bitstream_source(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if let Some(array) = self.array_of(node).cloned() {
            if array.dims.is_empty() {
                return Err(format!(
                    "fixed array bit-stream source has no dimensions in `{path}`"
                ));
            }
            if array.real {
                return Err(format!(
                    "real array bit-stream source is not supported in `{path}`"
                ));
            }
            let mut parts = Vec::new();
            for indices in inside_array_index_vectors(&array.dims) {
                parts.push(IrExpr::new(
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
                ));
            }
            return Self::join_bitstream_parts(path, parts).map(Some);
        }
        if let Some((_, aggregate)) = self.unpacked_aggregate_info(node) {
            if aggregate.kind == AggregateKind::UnpackedUnion {
                return Err(format!(
                    "unpacked union bit-stream source is not supported in `{path}`"
                ));
            }
            let mut parts = Vec::with_capacity(aggregate.leaves.len());
            for leaf in aggregate.leaves {
                if leaf.object.is_some() {
                    return Err(format!(
                        "string/chandle aggregate bit-stream source is not supported in `{path}`"
                    ));
                }
                let value = self.aggregate_leaf_read(&leaf)?;
                if value.is_real() {
                    return Err(format!(
                        "real aggregate bit-stream source is not supported in `{path}`"
                    ));
                }
                parts.push(value);
            }
            return Self::join_bitstream_parts(path, parts).map(Some);
        }
        Ok(None)
    }

    fn join_bitstream_parts(path: &str, parts: Vec<IrExpr>) -> Result<IrExpr, String> {
        if parts.is_empty() {
            return Err(format!(
                "bit-stream source has no packed leaves in `{path}`"
            ));
        }
        if let [part] = parts.as_slice() {
            return Ok(part.clone());
        }
        let width = parts
            .iter()
            .try_fold(0u32, |total, part| {
                total
                    .checked_add(part.width)
                    .filter(|width| *width <= LLG_MAX_WIDTH)
            })
            .ok_or_else(|| {
                format!(
                    "bit-stream source in `{path}` exceeds the runtime maximum width of {LLG_MAX_WIDTH} bits"
                )
            })?;
        Ok(IrExpr::new(
            IrExprKind::Concat { parts },
            width,
            false,
            None,
        ))
    }

    /// Lower the two-argument `$cast` system subroutine.  The destination is
    /// retained as an IR LHS so the emitter can evaluate selectors once and
    /// leave it unchanged when enum membership validation fails.
    pub(super) fn lower_dynamic_cast(
        &mut self,
        path: &str,
        args: &[NodeId],
    ) -> Result<IrExpr, String> {
        let [destination, source] = args else {
            return Err(format!("$cast requires exactly two arguments in `{path}`"));
        };
        // Slang represents the output argument of a system task as the
        // assignment expression that binds its hidden output temporary.  The
        // first operand is the user's actual lvalue; the second is an owned
        // placeholder and must not be lowered as a destination.
        let destination = match self.kind(*destination) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if operands.len() == 2
                && matches!(self.kind(operands[1]), NodeKind::Expr(ExprKind::Other)) =>
            {
                operands[0]
            }
            _ => *destination,
        };
        let lhs = self.lower_lhs(path, destination)?;
        let (target_width, target_signed, target_two_state, target_shortreal) =
            self.dynamic_cast_lhs_shape(&lhs)?;
        let target_descriptor = self.query_descriptor(destination).cloned();
        if let Some(descriptor) = &target_descriptor {
            match &descriptor.shape {
                TypeShape::PackedAtom { .. } | TypeShape::Real { .. } => {}
                TypeShape::Aggregate(layout)
                    if matches!(
                        layout.kind,
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion
                    ) => {}
                TypeShape::Aggregate(_) => {
                    return Err(format!(
                        "$cast destination must be a singular value in `{path}`"
                    ));
                }
                TypeShape::FixedArray { .. }
                | TypeShape::Container { .. }
                | TypeShape::String
                | TypeShape::Opaque { .. } => {
                    return Err(format!(
                        "$cast destination type is not supported in `{path}`"
                    ));
                }
            }
        }
        let source_descriptor = self.query_descriptor(*source).cloned();
        if let Some(descriptor) = &source_descriptor {
            let unsupported = matches!(
                descriptor.shape,
                TypeShape::FixedArray { .. }
                    | TypeShape::Container { .. }
                    | TypeShape::String
                    | TypeShape::Opaque { .. }
            ) || matches!(
                &descriptor.shape,
                TypeShape::Aggregate(layout)
                    if !matches!(
                        layout.kind,
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion
                    )
            );
            if unsupported {
                return Err(format!("$cast source must be a singular value in `{path}`"));
            }
        }
        let rhs = self.lower_expr(path, *source)?;

        let mut valid_values = Vec::new();
        let target_is_enum = target_descriptor
            .as_ref()
            .is_some_and(|descriptor| descriptor.info.kind == "enum");
        if target_is_enum {
            if rhs.is_real() {
                return Err(format!("$cast enum source must be integral in `{path}`"));
            }
            let target_id = target_descriptor.as_ref().map(|descriptor| descriptor.id);
            let enum_nodes = self
                .db
                .node_ids()
                .filter(|node| {
                    matches!(
                        self.kind(*node),
                        NodeKind::EnumConst {
                            value: Some(Val::Bits(_))
                        }
                    ) && target_id.is_some_and(|id| {
                        self.query_descriptor(*node)
                            .is_some_and(|descriptor| descriptor.id == id)
                    })
                })
                .collect::<Vec<_>>();
            if enum_nodes.is_empty() {
                return Err(format!(
                    "$cast enum destination has no captured members in `{path}`"
                ));
            }
            for node in enum_nodes {
                let value = self.lower_expr(path, node)?;
                if value.is_real() {
                    return Err(format!(
                        "$cast enum member is not an integral value in `{path}`"
                    ));
                }
                valid_values.push(ir_to_storage(
                    value,
                    target_width,
                    target_signed,
                    target_two_state,
                )?);
            }
        }
        Ok(IrExpr::new(
            IrExprKind::DynamicCast(Box::new(crate::sim::ir::IrDynamicCast {
                lhs,
                rhs,
                target_width,
                target_signed,
                target_two_state,
                target_shortreal,
                valid_values,
            })),
            1,
            false,
            None,
        ))
    }

    fn packed_plusarg_text(constant: &IrConst, context: &str) -> Result<String, String> {
        if constant.real.is_some() || constant.width == 0 {
            return Err(format!(
                "{context} requires a literal or integral string argument"
            ));
        }
        let byte_count = constant.width.div_ceil(8) as usize;
        let mut bytes = Vec::with_capacity(byte_count);
        for byte in (0..byte_count).rev() {
            let bit = byte * 8;
            let limb = bit / 64;
            let shift = bit % 64;
            let mut value = constant.bits.get(limb).copied().unwrap_or(0) >> shift;
            if shift > 56 {
                value |= constant.bits.get(limb + 1).copied().unwrap_or(0) << (64 - shift);
            }
            bytes.push(value as u8);
        }
        if constant.x.iter().any(|bits| *bits != 0) || constant.z.iter().any(|bits| *bits != 0) {
            return Err(format!(
                "{context} contains unknown bits and cannot be used as a format"
            ));
        }
        let first = bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(bytes.len());
        String::from_utf8(bytes[first..].to_vec()).map_err(|_| {
            format!("{context} must contain valid UTF-8; arbitrary bytes are not supported")
        })
    }

    fn plusarg_string_expr(value: IrStringExpr, context: &str) -> Result<IrPlusArgText, String> {
        if let IrStringExpr::Literal(bytes) = value {
            let text = String::from_utf8(bytes).map_err(|_| {
                format!("{context} must contain valid UTF-8; arbitrary bytes are not supported")
            })?;
            if text.contains('\0') {
                return Err(format!("{context} contains NUL"));
            }
            Ok(IrPlusArgText::Literal(text))
        } else {
            Ok(IrPlusArgText::Dynamic(value))
        }
    }

    fn lower_plusarg_text(
        &mut self,
        scope_path: &str,
        node: NodeId,
        context: &str,
    ) -> Result<IrPlusArgText, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                value,
                ..
            }) => {
                let text = decoded_string_text(value, context)?;
                if text.contains('\0') {
                    return Err(format!("{context} contains NUL"));
                }
                Ok(IrPlusArgText::Literal(text))
            }
            _ if self.is_string_expr(scope_path, node) => {
                Self::plusarg_string_expr(self.lower_string(scope_path, node)?, context)
            }
            _ => {
                let expression = self.lower_expr(scope_path, node)?;
                if let IrExprKind::Const(constant) = &expression.kind {
                    return Ok(IrPlusArgText::Literal(Self::packed_plusarg_text(
                        constant, context,
                    )?));
                }
                if expression.is_real() {
                    return Err(format!("{context} requires a string or integral argument"));
                }
                Ok(IrPlusArgText::Dynamic(IrStringExpr::FromPacked(Box::new(
                    expression,
                ))))
            }
        }
    }

    fn lower_plusarg_target(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<IrPlusArgTarget, String> {
        if self.is_string_expr(scope_path, node) {
            self.ensure_string_actual_writable(scope_path, node)?;
            return Ok(IrPlusArgTarget::String {
                address: self.lower_string_actual_address(scope_path, node)?,
            });
        }
        if self.is_chandle_expr(scope_path, node) {
            return Err(format!(
                "$value$plusargs destination must be packed, real, or string storage in `{scope_path}`"
            ));
        }
        let lhs = self.lower_lhs(scope_path, node)?;
        if let IrLhs::Ref {
            const_ref: true, ..
        } = &lhs
        {
            return Err(format!(
                "$value$plusargs destination cannot be a const ref in `{scope_path}`"
            ));
        }
        let real = match &lhs {
            IrLhs::Whole(index) => match self.model.signal(*index).ty {
                IrType::Real { shortreal } => Some(shortreal),
                IrType::Packed { .. } => None,
            },
            IrLhs::WholeRef {
                width: 0,
                shortreal,
                ..
            } => Some(*shortreal),
            IrLhs::ArrayElem {
                arr,
                elem_sel: IrElemSel::Whole,
                ..
            } => {
                let arr = self.reference_array(*arr);
                self.model
                    .array(arr)
                    .real
                    .then_some(self.model.array(arr).shortreal)
            }
            _ => None,
        };
        if let Some(shortreal) = real {
            return Ok(IrPlusArgTarget::Real {
                lhs: Box::new(lhs),
                shortreal,
            });
        }
        let IrType::Packed {
            width,
            signed,
            two_state,
        } = self.reference_lhs_type(&lhs).ok_or_else(|| {
            format!("$value$plusargs destination is not writable storage in `{scope_path}`")
        })?
        else {
            return Err(format!(
                "$value$plusargs destination is not a supported packed lvalue in `{scope_path}`"
            ));
        };
        if width == 0 {
            return Err(format!(
                "$value$plusargs destination has no resolved type in `{scope_path}`"
            ));
        }
        Ok(IrPlusArgTarget::Packed {
            lhs: Box::new(lhs),
            width,
            signed,
            two_state,
        })
    }

    fn lower_file_input_target(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<IrFileInputTarget, String> {
        match self.lower_plusarg_target(scope_path, node)? {
            IrPlusArgTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            } => Ok(IrFileInputTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            }),
            IrPlusArgTarget::Real { lhs, shortreal } => {
                Ok(IrFileInputTarget::Real { lhs, shortreal })
            }
            IrPlusArgTarget::String { address } => Ok(IrFileInputTarget::String { address }),
        }
    }

    fn lower_file_read_target(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<IrFileReadTarget, String> {
        if let Some(array) = self.array_of(node).cloned() {
            let array = self.reference_array(array.ir);
            if self.model.array(array).real {
                return Err(format!(
                    "$fread destination array must contain packed elements in `{scope_path}`"
                ));
            }
            return Ok(IrFileReadTarget::Array { array });
        }
        match self.lower_plusarg_target(scope_path, node)? {
            IrPlusArgTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            } => Ok(IrFileReadTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            }),
            IrPlusArgTarget::Real { .. } | IrPlusArgTarget::String { .. } => Err(format!(
                "$fread destination must be a packed value or unpacked array in `{scope_path}`"
            )),
        }
    }

    fn file_input_actual(&self, node: NodeId) -> Result<NodeId, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if !operands.is_empty() => Ok(operands[0]),
            _ => Ok(node),
        }
    }

    pub(super) fn lower_plusarg_expr(
        &mut self,
        scope_path: &str,
        name: &str,
        call: NodeId,
    ) -> Result<IrExpr, String> {
        let args = self.node(call).children.clone();
        match name {
            "$test$plusargs" => {
                let [pattern] = args.as_slice() else {
                    return Err(format!(
                        "$test$plusargs requires exactly one argument in `{scope_path}`"
                    ));
                };
                let pattern =
                    self.lower_plusarg_text(scope_path, *pattern, "$test$plusargs pattern")?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::TestPlusArgs { pattern }),
                    32,
                    true,
                    None,
                ))
            }
            "$value$plusargs" => {
                let [format_node, destination] = args.as_slice() else {
                    return Err(format!(
                        "$value$plusargs requires exactly two arguments in `{scope_path}`"
                    ));
                };
                let format =
                    self.lower_plusarg_text(scope_path, *format_node, "$value$plusargs format")?;
                if let IrPlusArgText::Literal(format) = &format {
                    validate_plusarg_format(format, scope_path)?;
                }
                let destination = match self.kind(*destination) {
                    NodeKind::Expr(ExprKind::Operation {
                        op: Operation::Assignment,
                        operands,
                        ..
                    }) if !operands.is_empty() => operands[0],
                    _ => *destination,
                };
                let target = self.lower_plusarg_target(scope_path, destination)?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::ValuePlusArgs { format, target }),
                    32,
                    true,
                    None,
                ))
            }
            _ => Err(format!("unsupported plusarg system function {name}")),
        }
    }

    /// Lower the optional command argument of `$system` into an owned string
    /// expression. `None` preserves the standard's omitted-argument
    /// `system(NULL)` query, while an explicit empty argument remains an owned
    /// empty string. Slang has already checked the system-call arity; retaining
    /// the check here keeps malformed owned IR from reaching the emitter.
    pub(super) fn lower_system_command(
        &mut self,
        scope_path: &str,
        args: &[NodeId],
    ) -> Result<Option<IrStringExpr>, String> {
        match args {
            [] => Ok(None),
            [arg] => self.lower_string(scope_path, *arg).map(Some),
            _ => Err(format!(
                "$system accepts at most one string argument in `{scope_path}`"
            )),
        }
    }

    fn lower_sampled_func_expr(
        &mut self,
        scope_path: &str,
        name: &str,
        args: &[NodeId],
    ) -> Result<IrExpr, String> {
        use crate::sim::ir::{IrSampledCall, IrSampledFunc};

        if name == "$sampled" {
            let [argument] = args else {
                return Err(format!(
                    "$sampled requires exactly one argument in `{scope_path}`"
                ));
            };
            let argument = self.lower_expr(scope_path, *argument)?;
            if argument.is_real() || !super::assertions::sampled_compatible(&argument) {
                return Err(format!(
                    "$sampled argument must be a static packed expression in `{scope_path}`"
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::Sampled(IrSampledCall::new(
                    IrSampledFunc::Sampled,
                    argument.clone(),
                    None,
                    0,
                ))),
                argument.width,
                argument.signed,
                None,
            ));
        }

        let (kind, global, future) = match name {
            "$rose" => (IrSampledFunc::Rose, false, false),
            "$fell" => (IrSampledFunc::Fell, false, false),
            "$stable" => (IrSampledFunc::Stable, false, false),
            "$changed" => (IrSampledFunc::Changed, false, false),
            "$past" => (IrSampledFunc::Past, false, false),
            "$past_gclk" => (IrSampledFunc::Past, true, false),
            "$rose_gclk" => (IrSampledFunc::Rose, true, false),
            "$fell_gclk" => (IrSampledFunc::Fell, true, false),
            "$stable_gclk" => (IrSampledFunc::Stable, true, false),
            "$changed_gclk" => (IrSampledFunc::Changed, true, false),
            "$future_gclk" | "$rising_gclk" | "$falling_gclk" | "$steady_gclk"
            | "$changing_gclk" => (IrSampledFunc::Past, true, true),
            _ => return Err(format!("unsupported sampled-value function `{name}`")),
        };
        if future {
            return Err(format!(
                "future global sampled-value function `{name}` is not supported; future values are never read from live storage in `{scope_path}`"
            ));
        }

        let expected = if kind == IrSampledFunc::Past && !global {
            1..=4
        } else if global {
            1..=1
        } else {
            1..=2
        };
        if !expected.contains(&args.len()) {
            return Err(format!(
                "{name} has invalid argument count in `{scope_path}`"
            ));
        }
        let argument = self.lower_expr(scope_path, args[0])?;
        if argument.is_real() || !super::assertions::sampled_compatible(&argument) {
            return Err(format!(
                "{name} argument must be a static packed expression in `{scope_path}`"
            ));
        }

        let mut ticks = 0;
        let mut gate = None;
        let mut explicit_clock = None;
        if kind == IrSampledFunc::Past && !global {
            if let Some(node) = args.get(1) {
                if !matches!(self.kind(*node), NodeKind::Expr(ExprKind::Other)) {
                    let value = self.eval_bound_i128(*node).map_err(|_| {
                        format!("$past tick count must be a positive constant in `{scope_path}`")
                    })?;
                    ticks = u64::try_from(value).map_err(|_| {
                        format!("$past tick count is outside the supported range in `{scope_path}`")
                    })?;
                    if ticks == 0 {
                        return Err(format!(
                            "$past tick count must be positive in `{scope_path}`"
                        ));
                    }
                }
            }
            if ticks == 0 {
                ticks = 1;
            }
            if let Some(node) = args.get(2) {
                if !matches!(self.kind(*node), NodeKind::Expr(ExprKind::Other)) {
                    gate = Some(self.lower_boolean_expr(scope_path, *node)?);
                }
            }
            explicit_clock = args.get(3).copied();
        } else if !global {
            explicit_clock = args.get(1).copied();
        }
        if global && kind == IrSampledFunc::Past {
            ticks = 1;
        }

        let mut clock = if global {
            self.lower_global_sampled_clock(scope_path)?
        } else if let Some(node) = explicit_clock {
            self.lower_sampled_clock_event(scope_path, node)?
        } else {
            self.sampled_clock
                .or(self.lower_default_sampled_clock(scope_path)?)
                .ok_or_else(|| {
                    format!(
                        "{name} requires an explicit clocking event outside a clocked assertion in `{scope_path}`"
                    )
                })?
        };
        if let Some(event_gate) = clock.gate.take() {
            let event_gate = self.lower_boolean_expr(scope_path, event_gate)?;
            if !super::assertions::sampled_compatible(&event_gate) {
                return Err(format!(
                    "sampled clock gate must be a static packed expression in `{scope_path}`"
                ));
            }
            gate = Some(match gate {
                Some(gate) => IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::LogAnd,
                        a: Box::new(gate),
                        b: Box::new(event_gate),
                    },
                    1,
                    false,
                    None,
                ),
                None => event_gate,
            });
        }
        if let Some(gate) = &gate {
            if !super::assertions::sampled_compatible(gate) {
                return Err(format!(
                    "$past gate must be a static packed expression in `{scope_path}`"
                ));
            }
        }
        let domain = self.lower_sampled_domain(
            scope_path,
            SampledClock {
                signal: clock.signal,
                posedge: clock.posedge,
                gate: None,
            },
            argument.clone(),
            gate,
        )?;
        let (width, signed) = if kind == IrSampledFunc::Past {
            (argument.width, argument.signed)
        } else {
            (1, false)
        };
        Ok(IrExpr::new(
            IrExprKind::SysFunc(IrSysFunc::Sampled(IrSampledCall::new(
                kind,
                argument,
                Some(domain),
                ticks,
            ))),
            width,
            signed,
            None,
        ))
    }

    /// Lower system-function expressions ($system/$clog2/$time/$stime/$bits/
    /// $signed/$unsigned); timescale scaling happens here.
    pub(super) fn lower_sys_func_expr(
        &mut self,
        scope_path: &str,
        name: &str,
        call: NodeId,
    ) -> Result<IrExpr, String> {
        let args: Vec<NodeId> = self.node(call).children.clone();
        if matches!(
            name,
            "$sampled"
                | "$rose"
                | "$fell"
                | "$stable"
                | "$changed"
                | "$past"
                | "$past_gclk"
                | "$rose_gclk"
                | "$fell_gclk"
                | "$stable_gclk"
                | "$changed_gclk"
                | "$future_gclk"
                | "$rising_gclk"
                | "$falling_gclk"
                | "$steady_gclk"
                | "$changing_gclk"
        ) {
            return self.lower_sampled_func_expr(scope_path, name, &args);
        }
        if name == "$q_full" {
            let [q_id, status] = args.as_slice() else {
                return Err(format!(
                    "$q_full requires exactly two arguments in `{scope_path}`"
                ));
            };
            let q_id = self.lower_expr(scope_path, *q_id)?;
            if q_id.is_real() {
                return Err(format!(
                    "$q_full q_id must be a packed integer in `{scope_path}`"
                ));
            }
            let status = self.lower_stochastic_output(scope_path, *status, "$q_full status")?;
            return Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::QFull {
                    q_id: Box::new(q_id),
                    status: Box::new(status),
                }),
                32,
                true,
                None,
            ));
        }
        if let Some(kind) = Self::legacy_random_kind(name) {
            return self.lower_legacy_random_expr(scope_path, kind, &args);
        }
        if name == "index" {
            if let Some(iterator) = self.container_iterator {
                let [receiver] = args.as_slice() else {
                    return Err(format!(
                        "array-method iterator index in `{scope_path}` has an invalid argument list"
                    ));
                };
                let is_iterator = matches!(
                    self.kind(*receiver),
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target)
                    }) if *target == iterator.node
                );
                if !is_iterator {
                    return Err(format!(
                        "array-method iterator index in `{scope_path}` has an unresolved binding"
                    ));
                }
                if iterator.index_width == 0 {
                    return Err(format!(
                        "array-method iterator index in `{scope_path}` is not representable for this receiver"
                    ));
                }
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead("__llg_method_index".to_owned()),
                    iterator.index_width,
                    iterator.index_signed,
                    None,
                ));
            }
        }
        use crate::sim::ir::IrMathFunc;
        let math = match name {
            "$ln" => Some(IrMathFunc::Ln),
            "$log10" => Some(IrMathFunc::Log10),
            "$exp" => Some(IrMathFunc::Exp),
            "$sqrt" => Some(IrMathFunc::Sqrt),
            "$pow" => Some(IrMathFunc::Pow),
            "$floor" => Some(IrMathFunc::Floor),
            "$ceil" => Some(IrMathFunc::Ceil),
            "$sin" => Some(IrMathFunc::Sin),
            "$cos" => Some(IrMathFunc::Cos),
            "$tan" => Some(IrMathFunc::Tan),
            "$asin" => Some(IrMathFunc::Asin),
            "$acos" => Some(IrMathFunc::Acos),
            "$atan" => Some(IrMathFunc::Atan),
            "$atan2" => Some(IrMathFunc::Atan2),
            "$hypot" => Some(IrMathFunc::Hypot),
            "$sinh" => Some(IrMathFunc::Sinh),
            "$cosh" => Some(IrMathFunc::Cosh),
            "$tanh" => Some(IrMathFunc::Tanh),
            "$asinh" => Some(IrMathFunc::Asinh),
            "$acosh" => Some(IrMathFunc::Acosh),
            "$atanh" => Some(IrMathFunc::Atanh),
            _ => None,
        };
        if let Some(kind) = math {
            if args.len() != kind.arity() {
                return Err(format!(
                    "{name} requires {} arguments in `{scope_path}`",
                    kind.arity()
                ));
            }
            let args = args
                .into_iter()
                .map(|arg| self.lower_expr(scope_path, arg))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::Math { kind, args }),
                0,
                true,
                None,
            ));
        }
        match name {
            "$urandom" => {
                if args.len() > 1 {
                    return Err(format!(
                        "$urandom accepts zero or one argument in `{scope_path}`"
                    ));
                }
                let seed = args
                    .first()
                    .map(|arg| self.lower_expr(scope_path, *arg))
                    .transpose()?;
                let seed = seed
                    .map(|value| {
                        if value.is_real() {
                            Err(format!("$urandom seed must be integral in `{scope_path}`"))
                        } else {
                            Ok(IrExpr::convert_to(value, 32, false))
                        }
                    })
                    .transpose()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Urandom {
                        seed: seed.map(Box::new),
                    }),
                    32,
                    false,
                    None,
                ))
            }
            "$urandom_range" => {
                if !(1..=2).contains(&args.len()) {
                    return Err(format!(
                        "$urandom_range requires one or two arguments in `{scope_path}`"
                    ));
                }
                let max = self.lower_expr(scope_path, args[0])?;
                if max.is_real() {
                    return Err(format!(
                        "$urandom_range maximum must be integral in `{scope_path}`"
                    ));
                }
                let min = args
                    .get(1)
                    .map(|arg| self.lower_expr(scope_path, *arg))
                    .transpose()?;
                if min.as_ref().is_some_and(IrExpr::is_real) {
                    return Err(format!(
                        "$urandom_range minimum must be integral in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::UrandomRange {
                        max: Box::new(IrExpr::convert_to(max, 32, false)),
                        min: min.map(|value| Box::new(IrExpr::convert_to(value, 32, false))),
                    }),
                    32,
                    false,
                    None,
                ))
            }
            "$cast" => self.lower_dynamic_cast(scope_path, &args),
            "$test$plusargs" | "$value$plusargs" => self.lower_plusarg_expr(scope_path, name, call),
            "$system" => Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::System(
                    self.lower_system_command(scope_path, &args)?,
                )),
                32,
                true,
                None,
            )),
            "$fopen" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(format!(
                        "$fopen requires one or two string arguments in `{scope_path}`"
                    ));
                }
                let path = self.lower_string(scope_path, args[0])?;
                let mode = args
                    .get(1)
                    .map(|argument| self.lower_string(scope_path, *argument))
                    .transpose()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileOpen { path, mode }),
                    32,
                    true,
                    None,
                ))
            }
            "$ftell" | "$feof" => {
                let [argument] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one file descriptor in `{scope_path}`"
                    ));
                };
                let descriptor = self.lower_expr(scope_path, *argument)?;
                if descriptor.is_real() {
                    return Err(format!(
                        "{name} requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                let function = if name == "$ftell" {
                    IrSysFunc::FileTell(Box::new(descriptor))
                } else {
                    IrSysFunc::FileEof(Box::new(descriptor))
                };
                let (width, signed) = if name == "$ftell" {
                    (64, true)
                } else {
                    (32, true)
                };
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(function),
                    width,
                    signed,
                    None,
                ))
            }
            "$fseek" => {
                let [descriptor, offset, operation] = args.as_slice() else {
                    return Err(format!(
                        "$fseek requires descriptor, offset, and operation in `{scope_path}`"
                    ));
                };
                let descriptor = self.lower_expr(scope_path, *descriptor)?;
                let offset = self.lower_expr(scope_path, *offset)?;
                let operation = self.lower_expr(scope_path, *operation)?;
                if descriptor.is_real() || offset.is_real() || operation.is_real() {
                    return Err(format!(
                        "$fseek requires packed arguments in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileSeek {
                        descriptor: Box::new(descriptor),
                        offset: Box::new(offset),
                        operation: Box::new(operation),
                    }),
                    32,
                    true,
                    None,
                ))
            }
            "$ferror" => {
                if args.len() != 1 && args.len() != 2 {
                    return Err(format!(
                        "$ferror requires a descriptor and optional string output in `{scope_path}`"
                    ));
                }
                let descriptor = self.lower_expr(scope_path, args[0])?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$ferror requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                let message = args
                    .get(1)
                    .map(|argument| {
                        let argument = match self.kind(*argument) {
                            NodeKind::Expr(ExprKind::Operation {
                                op: Operation::Assignment,
                                operands,
                                ..
                            }) => operands.first().copied().ok_or_else(|| {
                                format!("$ferror output argument is malformed in `{scope_path}`")
                            })?,
                            _ => *argument,
                        };
                        self.ensure_string_actual_writable(scope_path, argument)?;
                        self.lower_string_actual_address(scope_path, argument)
                    })
                    .transpose()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileError {
                        descriptor: Box::new(descriptor),
                        message,
                    }),
                    32,
                    true,
                    None,
                ))
            }
            "$fgetc" => {
                let [descriptor] = args.as_slice() else {
                    return Err(format!(
                        "$fgetc requires exactly one file descriptor in `{scope_path}`"
                    ));
                };
                let descriptor = self.lower_expr(scope_path, *descriptor)?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$fgetc requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::Getc {
                        descriptor: Box::new(descriptor),
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$ungetc" => {
                let [character, descriptor] = args.as_slice() else {
                    return Err(format!(
                        "$ungetc requires a character and file descriptor in `{scope_path}`"
                    ));
                };
                let character = self.lower_expr(scope_path, *character)?;
                let descriptor = self.lower_expr(scope_path, *descriptor)?;
                if character.is_real() || descriptor.is_real() {
                    return Err(format!(
                        "$ungetc requires packed arguments in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::Ungetc {
                        character: Box::new(character),
                        descriptor: Box::new(descriptor),
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$fgets" => {
                let [destination, descriptor] = args.as_slice() else {
                    return Err(format!(
                        "$fgets requires a string destination and file descriptor in `{scope_path}`"
                    ));
                };
                let destination = self.file_input_actual(*destination)?;
                let target = if self.is_string_expr(scope_path, destination) {
                    self.ensure_string_actual_writable(scope_path, destination)?;
                    IrFileInputTarget::String {
                        address: self.lower_string_actual_address(scope_path, destination)?,
                    }
                } else {
                    self.lower_file_input_target(scope_path, destination)?
                };
                if matches!(target, IrFileInputTarget::Real { .. }) {
                    return Err(format!(
                        "$fgets destination must be packed or string storage in `{scope_path}`"
                    ));
                }
                let descriptor = self.lower_expr(scope_path, *descriptor)?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$fgets requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::Gets {
                        descriptor: Box::new(descriptor),
                        target,
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$fscanf" => {
                if args.len() < 2 {
                    return Err(format!(
                        "$fscanf requires a descriptor, format, and optional destinations in `{scope_path}`"
                    ));
                }
                let descriptor = self.lower_expr(scope_path, args[0])?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$fscanf requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                let format = self.lower_plusarg_text(scope_path, args[1], "$fscanf format")?;
                let targets = args[2..]
                    .iter()
                    .map(|argument| {
                        let actual = self.file_input_actual(*argument)?;
                        self.lower_file_input_target(scope_path, actual)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::ScanFile {
                        descriptor: Box::new(descriptor),
                        format,
                        targets,
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$sscanf" => {
                if args.len() < 2 {
                    return Err(format!(
                        "$sscanf requires a source string, format, and optional destinations in `{scope_path}`"
                    ));
                }
                let source = self.lower_string(scope_path, args[0])?;
                let format = self.lower_plusarg_text(scope_path, args[1], "$sscanf format")?;
                let targets = args[2..]
                    .iter()
                    .map(|argument| {
                        let actual = self.file_input_actual(*argument)?;
                        self.lower_file_input_target(scope_path, actual)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::ScanString {
                        source,
                        format,
                        targets,
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$fread" => {
                if !(2..=4).contains(&args.len()) {
                    return Err(format!(
                        "$fread requires destination, descriptor, and optional start/count in `{scope_path}`"
                    ));
                }
                let destination = self.file_input_actual(args[0])?;
                let target = self.lower_file_read_target(scope_path, destination)?;
                let descriptor = self.lower_expr(scope_path, args[1])?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$fread requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                let start = match args.get(2).copied() {
                    Some(node) if matches!(self.kind(node), NodeKind::Expr(ExprKind::Other)) => {
                        None
                    }
                    Some(node) => Some(self.lower_expr(scope_path, node)?),
                    None => None,
                };
                let count = args
                    .get(3)
                    .map(|node| self.lower_expr(scope_path, *node))
                    .transpose()?;
                if let Some(value) = start.as_ref().or(count.as_ref()) {
                    if value.is_real() {
                        return Err(format!(
                            "$fread start/count must be packed expressions in `{scope_path}`"
                        ));
                    }
                }
                if matches!(target, IrFileReadTarget::Packed { .. })
                    && (start.is_some() || count.is_some())
                {
                    return Err(format!(
                        "$fread start/count bounds require an unpacked array destination in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::Read {
                        descriptor: Box::new(descriptor),
                        target,
                        start: start.map(Box::new),
                        count: count.map(Box::new),
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$dimensions" | "$unpacked_dimensions" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one argument in `{scope_path}`"
                    ));
                };
                let descriptor = self.query_descriptor(*arg).ok_or_else(|| {
                    format!("{name} argument has no owned type metadata in `{scope_path}`")
                })?;
                let count = if name == "$dimensions" {
                    Self::query_dimensions_for(descriptor).len()
                } else {
                    usize::try_from(Self::query_unpacked_dimensions_for(descriptor))
                        .unwrap_or(usize::MAX)
                };
                Ok(Self::query_integer(i128::try_from(count).map_err(
                    |_| format!("{name} dimension count is too large in `{scope_path}`"),
                )?))
            }
            "$isunbounded" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "$isunbounded requires exactly one argument in `{scope_path}`"
                    ));
                };
                let target = match self.kind(*arg) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) => *target,
                    _ => *arg,
                };
                let is_unbounded = matches!(self.kind(target), NodeKind::Expr(ExprKind::Unbounded))
                    || matches!(
                        self.kind(target),
                        NodeKind::Param { ty, .. }
                            if ty.kind == "unbounded"
                                || ty.type_name.as_deref() == Some("$")
                    )
                    || self.query_descriptor(target).is_some_and(|descriptor| {
                        descriptor.info.kind == "unbounded"
                            || descriptor.name == "$"
                            || descriptor.name.contains("unbounded")
                    });
                if !is_unbounded && !matches!(self.kind(target), NodeKind::Param { .. }) {
                    return Err(format!(
                        "$isunbounded requires a parameter or unbounded literal in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Const(IrConst {
                        bits: vec![u64::from(is_unbounded)],
                        x: vec![0],
                        z: vec![0],
                        width: 1,
                        signed: false,
                        real: None,
                        fill: None,
                    }),
                    1,
                    false,
                    None,
                ))
            }
            "$left" | "$right" | "$low" | "$high" | "$increment" | "$size" => {
                self.lower_array_query(scope_path, name, &args)
            }
            "$realtime" => Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::Realtime {
                    precision_fs: self.design_precision_fs,
                    unit_fs: self.timescale_of_node(call).unit_fs,
                }),
                0,
                true,
                None,
            )),
            "$rtoi" | "$itor" | "$realtobits" | "$bitstoreal" | "$shortrealtobits"
            | "$bitstoshortreal" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one argument in `{scope_path}`"
                    ));
                };
                let arg = self.lower_expr(scope_path, *arg)?;
                match name {
                    "$rtoi" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::Rtoi(Box::new(arg))),
                        32,
                        true,
                        None,
                    )),
                    "$itor" => {
                        if arg.is_real() {
                            return Ok(arg);
                        }
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::Itor(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                    "$realtobits" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::RealToBits(Box::new(arg))),
                        64,
                        false,
                        None,
                    )),
                    "$bitstoreal" => {
                        if arg.is_real() || arg.width != 64 {
                            return Err(format!(
                                "$bitstoreal requires an exactly 64-bit packed argument in `{scope_path}`"
                            ));
                        }
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::BitsToReal(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                    "$shortrealtobits" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::ShortRealToBits(Box::new(arg))),
                        32,
                        false,
                        None,
                    )),
                    _ => {
                        if arg.is_real() || arg.width != 32 {
                            return Err(format!(
                                "$bitstoshortreal requires an exactly 32-bit packed argument in `{scope_path}`"
                            ));
                        }
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::BitsToShortReal(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                }
            }
            "$countones" | "$onehot" | "$onehot0" | "$isunknown" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one argument in `{scope_path}`"
                    ));
                };
                let arg = self.lower_expr(scope_path, *arg)?;
                if arg.is_real() {
                    return Err(format!(
                        "{name} requires a packed integral argument in `{scope_path}`"
                    ));
                }
                let kind = match name {
                    "$countones" => IrBitQuery::CountOnes,
                    "$onehot" => IrBitQuery::OneHot,
                    "$onehot0" => IrBitQuery::OneHot0,
                    _ => IrBitQuery::IsUnknown,
                };
                let (width, signed) = kind.result_type();
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::BitQuery {
                        kind,
                        arg: Box::new(arg),
                    }),
                    width,
                    signed,
                    None,
                ))
            }
            "$clog2" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("$clog2 without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "clog2 on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Clog2(Box::new(a))),
                    32,
                    false,
                    None,
                ))
            }
            "$time" | "$stime" => {
                // Both functions return the current time in the CALLING
                // module's unit; `$stime` is the 32-bit form. `llg_time()` is
                // in design-precision ticks (1 tick = design_precision_fs
                // fs), so now_fs = now * P.
                let unit_fs = self.timescale_of_node(call).unit_fs;
                let kind = if name == "$stime" {
                    IrTimeKind::STime
                } else {
                    IrTimeKind::Time
                };
                let width = kind.width();
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Time {
                        precision_fs: self.design_precision_fs,
                        unit_fs,
                        kind,
                    }),
                    width,
                    false,
                    None,
                ))
            }
            "$bits" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("$bits without argument in `{scope_path}`"))?;
                if let Some(descriptor) = self.query_descriptor(a).cloned() {
                    if let Some(width) = descriptor.fixed_size_bits() {
                        return Ok(Self::query_integer(i128::from(width)));
                    }
                    if let Some(container) = self.container_of(a) {
                        let element_width = match &self.model.containers[container.ir].element {
                            crate::sim::ir::IrContainerElement::Packed { width, .. } => *width,
                            _ => {
                                return Err(format!(
                                    "$bits on a non-packed container is not supported in `{scope_path}`"
                                ))
                            }
                        };
                        let size = IrExpr::new(
                            IrExprKind::Container(Box::new(IrContainerExpr::Size(container.ir))),
                            32,
                            true,
                            None,
                        );
                        return Ok(IrExpr::new(
                            IrExprKind::Bin {
                                op: IrBinOp::Mul,
                                a: Box::new(size),
                                b: Box::new(Self::query_integer(i128::from(element_width))),
                            },
                            32,
                            true,
                            None,
                        ));
                    }
                    if descriptor.shape == TypeShape::String
                        && self.object_of(scope_path, a).is_some_and(|index| {
                            self.model.objects[index].ty == IrObjectType::String
                        })
                    {
                        let len = IrExpr::new(
                            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringLen(
                                self.lower_string(scope_path, a)?,
                            ))),
                            32,
                            true,
                            None,
                        );
                        return Ok(IrExpr::new(
                            IrExprKind::Bin {
                                op: IrBinOp::Mul,
                                a: Box::new(len),
                                b: Box::new(Self::query_integer(8)),
                            },
                            32,
                            true,
                            None,
                        ));
                    }
                }
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "bits on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Bits(Box::new(a))),
                    32,
                    true,
                    None,
                ))
            }
            "$signed" | "$unsigned" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("{name} without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "{name} on real value in `{scope_path}` is not supported"
                    ));
                }
                let s = name == "$signed";
                let w = a.width;
                Ok(IrExpr::resize_to(a, w, s))
            }
            _ => Err(format!(
                "unsupported system function {name} in `{scope_path}`"
            )),
        }
    }

    /// Lower one output argument of an IEEE stochastic queue task. The C ABI
    /// receives a direct `sv4_t*`, so only whole packed storage is admitted;
    /// selected aliases and real values remain explicit unsupported lowering
    /// diagnostics rather than silently writing a temporary.
    pub(super) fn lower_stochastic_output(
        &mut self,
        path: &str,
        node: NodeId,
        label: &str,
    ) -> Result<IrLhs, String> {
        // Slang wraps output actuals in an assignment conversion whose first
        // operand is the caller's storage (the second is a frontend-only
        // converted placeholder). Match ordinary output-formal binding and
        // lower the actual itself as the direct runtime destination.
        let node = match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::Assignment =>
            {
                operands
                    .first()
                    .copied()
                    .ok_or_else(|| format!("{label} has a malformed output argument in `{path}`"))?
            }
            _ => node,
        };
        let lhs = self.lower_lhs(path, node).map_err(|error| {
            let children = self
                .node(node)
                .children
                .iter()
                .map(|child| format!("{child:?}={:?}", self.kind(*child)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{label} has an unsupported target ({:?}) children=[{children}] in `{path}`: {error}",
                self.kind(node)
            )
        })?;
        match &lhs {
            IrLhs::Whole(index) => {
                let signal = self.model.signal(*index);
                if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                    return Err(format!(
                        "{label} must name a whole packed integer variable, not a net, in `{path}`"
                    ));
                }
            }
            IrLhs::WholeRef { width, .. } if *width != 0 => {}
            IrLhs::WholeRef { .. } => {
                return Err(format!(
                    "{label} must name a whole packed integer variable in `{path}`"
                ));
            }
            _ => {
                return Err(format!(
                    "{label} must name a whole packed integer variable in `{path}`"
                ));
            }
        }
        Ok(lhs)
    }

    fn legacy_random_kind(name: &str) -> Option<crate::sim::ir::IrRandomFunc> {
        use crate::sim::ir::IrRandomFunc;
        Some(match name {
            "$random" => IrRandomFunc::Random,
            "$dist_uniform" => IrRandomFunc::Uniform,
            "$dist_normal" => IrRandomFunc::Normal,
            "$dist_exponential" => IrRandomFunc::Exponential,
            "$dist_poisson" => IrRandomFunc::Poisson,
            "$dist_chi_square" => IrRandomFunc::ChiSquare,
            "$dist_t" => IrRandomFunc::StudentT,
            "$dist_erlang" => IrRandomFunc::Erlang,
            _ => return None,
        })
    }

    fn lower_legacy_random_expr(
        &mut self,
        scope_path: &str,
        kind: crate::sim::ir::IrRandomFunc,
        args: &[NodeId],
    ) -> Result<IrExpr, String> {
        let (seed_node, parameter_nodes) = if kind == crate::sim::ir::IrRandomFunc::Random {
            match args {
                [] => (None, &[][..]),
                [seed] => (Some(*seed), &[][..]),
                _ => {
                    return Err(format!(
                        "$random accepts zero or one seed argument in `{scope_path}`"
                    ))
                }
            }
        } else {
            let Some((seed, parameters)) = args.split_first() else {
                return Err(format!(
                    "legacy random function requires a writable seed in `{scope_path}`"
                ));
            };
            (Some(*seed), parameters)
        };
        if parameter_nodes.len() != kind.arity() {
            return Err(format!(
                "legacy random function has {} parameter(s), got {} in `{scope_path}`",
                kind.arity(),
                parameter_nodes.len()
            ));
        }

        let seed = seed_node
            .map(|node| {
                // Slang inserts an implicit integral cast when adapting the
                // writable seed actual to the legacy system-function formal.
                // The cast is a value-view node, not storage, so peel it
                // before resolving the actual assignment target.
                let mut node = node;
                loop {
                    match self.kind(node) {
                        NodeKind::Expr(ExprKind::Cast { operand, .. }) => node = *operand,
                        NodeKind::Expr(ExprKind::Operation {
                            op: Operation::Assignment,
                            operands,
                            ..
                        }) if !operands.is_empty() => node = operands[0],
                        _ => break,
                    }
                }
                let lhs = self.lower_lhs(scope_path, node)?;
                let Some((width, _signed, _two_state, const_ref)) = self.ref_lhs_type(&lhs) else {
                    return Err(format!(
                        "legacy random seed must be an integral variable in `{scope_path}`"
                    ));
                };
                if width == 0 || const_ref {
                    return Err(format!(
                        "legacy random seed must be a writable integral variable in `{scope_path}`"
                    ));
                }
                Ok(Box::new(lhs))
            })
            .transpose()?;
        let mut parameters = Vec::with_capacity(parameter_nodes.len());
        for node in parameter_nodes {
            let parameter = self.lower_expr(scope_path, *node)?;
            if parameter.is_real() {
                return Err(format!(
                    "legacy random parameters must be integral in `{scope_path}`"
                ));
            }
            parameters.push(parameter);
        }
        Ok(IrExpr::new(
            IrExprKind::SysFunc(IrSysFunc::LegacyRandom {
                kind,
                seed,
                args: parameters,
            }),
            32,
            true,
            None,
        ))
    }

    /// Lower an assignment LHS: the pre-IR [`Self::analyze_lhs`] decisions
    /// converted to [`IrLhs`] (identical by construction during the seam
    /// transition; sub-expression codes ride along verbatim).
    pub(super) fn lower_lhs(&mut self, path: &str, lhs: NodeId) -> Result<IrLhs, String> {
        if self.clocking_var_target(lhs).is_some() {
            return Err(
                "clocking input members are read-only sampled values in `".to_owned() + path + "`",
            );
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
        if let Some(lhs) = self.class_field_lhs(path, lhs)? {
            return Ok(lhs);
        }
        let lh = self.analyze_lhs(path, lhs)?;
        self.lhs_to_ir(lh)
    }

    pub(super) fn lower_packed_aggregate_pattern(
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

    fn lower_packed_aggregate_pattern_value(
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

    pub(super) fn lower_unpacked_aggregate_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let lhs_aggregate = self.unpacked_aggregate_info(lhs);
        let rhs_aggregate = self.unpacked_aggregate_info(rhs);
        let rhs_is_pattern = matches!(
            self.kind(rhs),
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == Operation::AssignmentPattern
        );
        if lhs_aggregate.is_none() && rhs_aggregate.is_none() {
            return Ok(None);
        }
        let (lhs_target, lhs_aggregate) = lhs_aggregate.ok_or_else(|| {
            format!("unpacked aggregate used as a scalar assignment RHS in `{path}`")
        })?;
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
        if rhs_is_pattern {
            if op != Operation::Assignment {
                return Err(format!(
                    "compound assignment of unpacked aggregate pattern in `{path}` is not supported"
                ));
            }
            let layout = self.db.aggregate_layout(lhs_target).ok_or_else(|| {
                format!(
                    "unpacked aggregate `{}` in `{path}` has no captured layout",
                    self.node(lhs_target).name
                )
            })?;
            let mut values = Vec::new();
            self.aggregate_pattern_leaf_values(path, rhs, layout, &[], &mut values)?;
            let mut assignments = Vec::with_capacity(values.len());
            let mut captures = Vec::new();
            let mut captured = HashMap::<NodeId, (String, u32, bool)>::new();
            for (member_path, value_node) in values {
                let left = lhs_aggregate
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
                        IrObjectType::Chandle => IrObjectStmt::ChandleAssign(
                            index,
                            self.lower_chandle(path, value_node)?,
                        ),
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
                    let name = format!("_agg{}_{}", lhs_target.0, value_node.0);
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

    /// Compare two complete fixed unpacked aggregate values without
    /// flattening their storage into one packed expression.  Each leaf keeps
    /// its owned representation: packed leaves retain four-state comparison,
    /// real leaves use real comparison, and string leaves compare the owned
    /// runtime objects.  The aggregate result is the logical conjunction of
    /// leaf equalities; `!=`/case-`!=` invert that result after all leaves have
    /// participated, preserving unknown propagation for packed values.
    fn lower_unpacked_aggregate_comparison(
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
                        if self.model.objects[left_index].ty == IrObjectType::Chandle {
                            if self.model.objects[right_index].ty != IrObjectType::Chandle {
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

    fn aggregate_leaf_lhs(&self, leaf: &AggregateMemberInfo) -> Result<IrLhs, String> {
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

    fn aggregate_leaf_read(&self, leaf: &AggregateMemberInfo) -> Result<IrExpr, String> {
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

    pub(super) fn aggregate_pattern_leaf_values(
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

    fn aggregate_descriptor_pattern_values(
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
        match &descriptor.shape {
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
                    return self.aggregate_descriptor_default_values(
                        path,
                        pattern_node,
                        &next,
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
            if !super::collection::pattern_key_matches_descriptor(
                key_type,
                element,
                Self::descriptor_two_state(element),
                None,
            ) {
                return Err(format!(
                    "array assignment pattern key `{key}` has no matching index or type in `{path}`"
                ));
            }
            if type_values
                .iter()
                .any(|(previous, _)| super::collection::pattern_key_types_equal(previous, key_type))
            {
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
                        super::collection::pattern_key_matches_descriptor(
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
    pub(super) fn lhs_to_ir(&self, lh: Lhs) -> Result<IrLhs, String> {
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
            },
            Lhs::Canonical(lhs) => lhs,
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

/// Materialize an explicit cast's target width before the value enters any
/// enclosing assignment context. An unbased unsized fill is contextual only
/// until this boundary (IEEE 1800-2009 §6.24.1); retaining its marker would
/// incorrectly refill a wider destination instead of extending the cast value.
fn ir_to_explicit_cast_storage(
    value: IrExpr,
    width: u32,
    signed: bool,
    two_state: bool,
) -> Result<IrExpr, String> {
    let Some(fill) = value.fill else {
        return ir_to_storage(value, width, signed, two_state);
    };
    let limbs = (width as usize).div_ceil(64);
    let mut materialized = vec![u64::MAX; limbs];
    if let Some(last) = materialized.last_mut() {
        let tail = width % 64;
        if tail != 0 {
            *last = (1u64 << tail) - 1;
        }
    }
    let zeros = vec![0; limbs];
    let (bits, x, z) = match (fill, two_state) {
        (2 | 3, true) => (zeros.clone(), zeros.clone(), zeros),
        (0, _) => (zeros.clone(), zeros.clone(), zeros),
        (1, _) => (materialized, zeros.clone(), zeros),
        (2, _) => (zeros.clone(), materialized, zeros),
        (3, _) => (zeros.clone(), zeros, materialized),
        _ => return Err(format!("invalid explicit-cast fill value {fill}")),
    };
    let constant =
        IrConst::packed(bits, x, z, width, signed, None).map_err(|error| error.to_string())?;
    Ok(IrExpr::new(
        IrExprKind::Const(constant),
        width,
        signed,
        None,
    ))
}

fn parse_decimal_real_literal(token: &str) -> Option<f64> {
    let bytes = token.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'_'
            && (!index
                .checked_sub(1)
                .and_then(|previous| bytes.get(previous))
                .is_some_and(u8::is_ascii_digit)
                || !bytes.get(index + 1).is_some_and(u8::is_ascii_digit))
        {
            return None;
        }
    }
    let normalized = token.replace('_', "");
    let bytes = normalized.as_bytes();
    let mut index = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if index == integer_start {
        return None;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == exponent_start {
            return None;
        }
    }
    if index != bytes.len() {
        return None;
    }
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod cast_tests {
    use super::{ir_to_explicit_cast_storage, parse_decimal_real_literal};
    use crate::sim::codegen::lowering::ir_to_storage;
    use crate::sim::ir::{
        IrExpr, IrExprKind, IrLhs, IrModel, IrProcess, IrShape, IrSignal, IrStmt, IrType,
    };
    use crate::sim::opt::{self, OptConfig};

    #[test]
    fn explicit_cast_materializes_unsized_fill_at_target_width() {
        let fill = IrExpr::new(IrExprKind::Fill(1), 1, false, Some(1));
        let cast = ir_to_explicit_cast_storage(fill, 1, false, false).unwrap();

        assert_eq!(cast.width(), 1);
        assert_eq!(cast.fill(), None);
        assert!(matches!(
            cast.kind(),
            IrExprKind::Const(value)
                if value.bits() == [1]
                    && value.x_mask() == [0]
                    && value.z_mask() == [0]
        ));
    }

    #[test]
    fn folded_real_comparison_accepts_only_decimal_numeric_tokens() {
        assert_eq!(parse_decimal_real_literal("1.5"), Some(1.5));
        assert_eq!(parse_decimal_real_literal("-2_000e-3"), Some(-2.0));
        assert_eq!(parse_decimal_real_literal("42"), Some(42.0));
        for rejected in ["NaN", "inf", "+inf", "1.", ".5", "1__0", "1e", "8'h1"] {
            assert_eq!(parse_decimal_real_literal(rejected), None, "{rejected}");
        }
    }

    #[test]
    fn narrow_fill_cast_stays_materialized_when_assigned_wider() {
        let build = || {
            let fill = IrExpr::new(IrExprKind::Fill(1), 1, false, Some(1));
            let cast = ir_to_explicit_cast_storage(fill, 1, false, false).unwrap();
            let rhs = ir_to_storage(cast, 8, false, false).unwrap();
            let mut model = IrModel::new("cast".to_string(), 1).unwrap();
            model.signals.push(
                IrSignal::new(
                    "value".to_string(),
                    None,
                    IrType::packed(8, false).unwrap(),
                    None,
                )
                .unwrap(),
            );
            model.processes.push(IrProcess::new(
                "proc".to_string(),
                "cast.initial".to_string(),
                IrShape::RunOnce,
                Vec::new(),
                vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs,
                    nba: false,
                }],
            ));
            model
        };

        for config in [OptConfig::none(), OptConfig::default()] {
            let mut model = build();
            opt::run_ir(&mut model, &config);
            let IrStmt::Assign { rhs, .. } = &model.processes[0].body[0] else {
                panic!("optimizer replaced the cast assignment")
            };
            assert_eq!(rhs.fill(), None);
            if let IrExprKind::Const(value) = rhs.kind() {
                assert_eq!(value.width(), 8);
                assert_eq!(value.bits(), [1]);
                assert_eq!(value.fill(), None);
            }
        }
    }
}
