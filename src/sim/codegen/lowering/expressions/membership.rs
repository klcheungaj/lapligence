//! Membership.

use super::*;
use crate::sim::codegen::lowering::collection::native_values::LeafValue;
use crate::sim::codegen::lowering::collection::{ir_container_kind, lower_container_element};
use crate::sim::codegen::lowering::containers::pattern_key_expr;
use crate::sim::codegen::lowering::expressions::dispatch::after_statements;
use crate::sim::ir::{IrContainer, IrContainerElement, IrContainerStmt, IrInsideArray};

impl<'a> Codegen<'a> {
    /// `h inside {a, q, null}` over handles of one kind: set membership uses
    /// `==` on nonintegral operands (SV 11.4.13, decision R14-D1 for
    /// chandles) and handle equality compares identity, so the set is the
    /// `||` of identity tests, one per handle and one search per array. The
    /// selector is read once per item and the chain stops at the first match,
    /// so the selector and the handle items must be free of side effects;
    /// ranges have no handle meaning.
    pub(in super::super) fn lower_handle_inside(
        &mut self,
        path: &str,
        value: NodeId,
        items: &[NodeId],
    ) -> Result<IrExpr, String> {
        let unsupported = || {
            format!(
                "handle inside set in `{path}` must list handle variables, properties, elements, arrays or null without side effects"
            )
        };
        if !self.side_effect_free(value) {
            return Err(unsupported());
        }
        let selector = self
            .lower_handle_identity(path, value)?
            .ok_or_else(unsupported)?;
        let mut tests = Vec::new();
        for item in items {
            self.lower_handle_inside_item(path, &selector, *item, &mut tests)?;
        }
        tests
            .into_iter()
            .reduce(|previous, test| cmp_expr_ir(IrBinOp::LogOr, previous, test))
            .ok_or_else(|| format!("inside set is empty in `{path}`"))
    }

    fn lower_handle_inside_item(
        &mut self,
        path: &str,
        selector: &IrChandleExpr,
        node: NodeId,
        out: &mut Vec<IrExpr>,
    ) -> Result<(), String> {
        let test = match self.lower_inside_array(path, node)? {
            Some(array) => {
                self.require_set_members(path, node, &array, IrContainerElement::is_handle)?;
                IrObjectQuery::HandleInside {
                    handle: selector.clone(),
                    array,
                }
            }
            None => {
                let item = if self.side_effect_free(node) {
                    self.lower_handle_identity(path, node)?
                } else {
                    None
                };
                IrObjectQuery::ChandleEq(
                    selector.clone(),
                    item.ok_or_else(|| {
                        format!(
                            "handle inside set in `{path}` must list handle variables, properties, elements, arrays or null without side effects"
                        )
                    })?,
                )
            }
        };
        out.push(object_query(test, 1, false));
        Ok(())
    }

    /// Whether set membership compares `node` by identity.
    pub(in super::super) fn is_handle_set_operand(&self, path: &str, node: NodeId) -> bool {
        self.is_chandle_expr(path, node)
            || self.is_mailbox_expr(path, node)
            || self.is_process_expr(path, node)
            || self.event_target_of(node).is_some()
    }

    /// The identity of the object a handle expression or `null` names, or
    /// `None` when `node` is no handle.
    fn lower_handle_identity(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) {
            return Ok(Some(IrChandleExpr::Null));
        }
        Ok(if self.event_target_of(node).is_some() {
            Some(self.lower_event_identity(path, node)?)
        } else if self.is_process_expr(path, node) {
            Some(IrChandleExpr::Process(Box::new(
                self.lower_process(path, node)?,
            )))
        } else if self.is_mailbox_expr(path, node) {
            let element = self.mailbox_element_for_decl(node);
            Some(IrChandleExpr::Mailbox(Box::new(
                self.lower_mailbox_expr(path, node, element)?,
            )))
        } else if self.is_chandle_expr(path, node) {
            Some(self.lower_chandle(path, node)?)
        } else {
            None
        })
    }

    /// Lower an `inside` predicate whose items may need arrays that no
    /// expression can name (a call result, a slice): `lower` builds it, after
    /// the statements that fill those arrays.
    pub(in super::super) fn with_set_arrays(
        &mut self,
        lower: impl FnOnce(&mut Self) -> Result<IrExpr, String>,
    ) -> Result<IrExpr, String> {
        let outer = self.set_array_prelude.replace(Vec::new());
        let value = lower(self);
        let before = std::mem::replace(&mut self.set_array_prelude, outer).unwrap_or_default();
        let value = value?;
        Ok(if before.is_empty() {
            value
        } else {
            after_statements(before, value)
        })
    }

    /// The array that set item `node` names, when it is stored as a container
    /// or inside a value; a container-valued expression first fills a
    /// lexical temporary. `None` for a singular item and for an array of
    /// packed cells, which the caller lowers itself.
    fn lower_inside_array(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrInsideArray>, String> {
        let node = self.p30_unwrap_cast(node);
        if let Some(container) = self.container_of(node) {
            return Ok(Some(IrInsideArray::Container(container.ir)));
        }
        if let Some(array) = self.lower_element_array(path, node)? {
            return Ok(Some(array));
        }
        if self.set_array_prelude.is_none() {
            return Ok(None);
        }
        if self.is_fixed_view_slice(node) {
            let Some((view, start, count)) = self.fixed_view_slice(path, node)? else {
                return Ok(None);
            };
            let temporary = self.selection_temporary(view, node, count);
            return Ok(Some(self.set_array_temporary(
                temporary,
                IrStmt::Container(Box::new(IrContainerStmt::CopyRange {
                    dst: temporary,
                    dst_start: pattern_key_expr(0, 64, true, false),
                    src: view,
                    src_start: start,
                    count,
                })),
            )));
        }
        let temporary = match self.call_result_temporary(node)? {
            Some(temporary) => temporary,
            None => match self.container_valued_temporary(node)? {
                Some(temporary) => temporary,
                None => return Ok(None),
            },
        };
        let fill = self.lower_container_into(path, node, temporary, node)?;
        Ok(Some(self.set_array_temporary(temporary, fill)))
    }

    fn set_array_temporary(&mut self, temporary: usize, fill: IrStmt) -> IrInsideArray {
        if let Some(prelude) = self.set_array_prelude.as_mut() {
            prelude.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                temporary,
            ))));
            prelude.push(fill);
        }
        IrInsideArray::Container(temporary)
    }

    /// A lexical container of the type of `node`, an expression whose value
    /// is a dynamic array, queue or associative array.
    fn container_valued_temporary(&mut self, node: NodeId) -> Result<Option<usize>, String> {
        let Some(descriptor) = self.query_descriptor(node) else {
            return Ok(None);
        };
        let TypeShape::Container { element, array, .. } = &descriptor.shape else {
            return Ok(None);
        };
        let element = lower_container_element(element)?;
        let kind = ir_container_kind(array, &descriptor.name, "the element type")?;
        let ir = self.model.containers.len();
        self.model.containers.push(IrContainer {
            c_name: format!("S_llg_container_{ir}"),
            element,
            kind,
            initial_size: None,
            activation: true,
            class_field: None,
            receiver: None,
        });
        Ok(Some(ir))
    }

    /// The members of an array inside an activation record or a parameter,
    /// one value or container each.
    fn set_member_values(&mut self, path: &str, node: NodeId) -> Result<Vec<LeafValue>, String> {
        let mut setup = Vec::new();
        let reads = match self.set_array_prelude {
            Some(_) => self.record_leaf_reads(path, node, &mut setup)?,
            None => self.record_leaf_reads_in_place(path, node)?,
        };
        if let Some(prelude) = self.set_array_prelude.as_mut() {
            prelude.append(&mut setup);
        }
        let reads = reads.ok_or_else(|| {
            format!(
                "inside set item in `{path}` is an array that has no storage the set can traverse"
            )
        })?;
        Ok(reads.into_iter().map(|(_, value)| value).collect())
    }

    /// Report an array whose members the selector does not compare with.
    fn require_set_members(
        &self,
        path: &str,
        node: NodeId,
        array: &IrInsideArray,
        fits: impl Fn(&IrContainerElement) -> bool,
    ) -> Result<(), String> {
        let member = array
            .member(&self.model)
            .map_err(|error| error.to_string())?;
        if member.as_ref().is_some_and(fits) {
            return Ok(());
        }
        Err(format!(
            "inside set item `{}` in `{path}` is an array whose elements do not compare with the selector",
            self.node(node).name
        ))
    }

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
            return Ok(None);
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
                    ExprKind::Ref { .. }
                        | ExprKind::HierPath { .. }
                        | ExprKind::ArraySelect { .. }
                        | ExprKind::BitSelect { .. }
                )
        );
        if storage_array {
            // A record's column is one array, not a leaf.
            if self.record_column_array(node).is_some() {
                if let Some((_, cells)) = self.fixed_array_cells(path, node)? {
                    out.push(IrInsideItem::Cells(cells));
                    return Ok(());
                }
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
            if let Some((_, cells)) = self.fixed_array_cells(path, node)? {
                let storage = &self.model.arrays[cells.array];
                let selected = cells.shape(&self.model).map(|shape| shape.cells);
                // Small dense arrays keep one straight-line comparison per
                // cell; selections and larger or descriptor storage loop.
                if !cells.prefix.is_empty()
                    || storage.sparse()
                    || selected.is_none_or(|cells| {
                        cells > super::super::containers::FIXED_CELL_UNROLL_LIMIT
                    })
                {
                    out.push(IrInsideItem::Cells(cells));
                    return Ok(());
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
        // An element of container storage has no packed value as a whole.
        let mut array = self.lower_element_array(path, node)?;
        if array.is_none() {
            let record =
                matches!(self.native_path_of(node), Ok(Some((_, member))) if !member.is_empty());
            if !record {
                if let Some(item) = self.lower_fixed_array_value_item(path, node)? {
                    out.push(item);
                    return Ok(());
                }
            }
            array = self.lower_inside_array(path, node)?;
        }
        if let Some(array) = array {
            self.require_set_members(path, node, &array, |member| {
                member.is_packed() || member.is_real()
            })?;
            out.push(IrInsideItem::Array(array));
            return Ok(());
        }
        if self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(
                descriptor.shape,
                TypeShape::FixedArray { .. } | TypeShape::Container { .. }
            )
        }) {
            for leaf in self.set_member_values(path, node)? {
                out.push(match leaf {
                    LeafValue::Packed(value) | LeafValue::Real(value) => IrInsideItem::Value(value),
                    LeafValue::Container(container) => {
                        let array = IrInsideArray::Container(container);
                        self.require_set_members(path, node, &array, |member| {
                            member.is_packed() || member.is_real()
                        })?;
                        IrInsideItem::Array(array)
                    }
                    _ => {
                        return Err(format!(
                            "array-valued inside set item in `{path}` does not compare with the selector"
                        ))
                    }
                });
            }
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

        let leaves = match self.unpacked_aggregate_info(node) {
            Some((_, aggregate)) => aggregate.leaves,
            None => self
                .unpacked_path_for_expr(node)
                .and_then(|(target, prefix)| {
                    let aggregate = self.unpacked_aggregates.get(&target)?;
                    Some(
                        aggregate
                            .leaves
                            .iter()
                            .filter(|leaf| leaf.path.starts_with(&prefix))
                            .cloned()
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_default(),
        };
        if !leaves.is_empty() {
            for leaf in leaves {
                if let Some(container) = leaf.container {
                    let array = IrInsideArray::Container(container.ir);
                    self.require_set_members(path, node, &array, IrContainerElement::is_string)?;
                    out.push(IrStringInsideItem::Array(array));
                    continue;
                }
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
        if let Some(array) = self.lower_inside_array(path, node)? {
            self.require_set_members(path, node, &array, IrContainerElement::is_string)?;
            out.push(IrStringInsideItem::Array(array));
            return Ok(());
        }
        if self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(
                descriptor.shape,
                TypeShape::FixedArray { .. } | TypeShape::Container { .. }
            )
        }) {
            // A parameter's elements have no storage to traverse.
            if let Some(strings) = self.lower_parameter_strings(path, node)? {
                out.extend(strings.into_iter().map(IrStringInsideItem::Value));
                return Ok(());
            }
            for leaf in self.set_member_values(path, node)? {
                out.push(match leaf {
                    LeafValue::String(value) => IrStringInsideItem::Value(value),
                    LeafValue::Container(container) => {
                        let array = IrInsideArray::Container(container);
                        self.require_set_members(path, node, &array, IrContainerElement::is_string)?;
                        IrStringInsideItem::Array(array)
                    }
                    _ => {
                        return Err(format!(
                            "string inside set item in `{path}` is an array whose elements are not strings"
                        ))
                    }
                });
            }
            return Ok(());
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
