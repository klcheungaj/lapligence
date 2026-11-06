//! Whole-value transport for column-layout records (RTL-101).
//!
//! A column-layout record is never one packed payload. Its values move
//! column by column: scalar leaves through ordinary packed assignments and
//! member arrays through descriptor views. Sources that are not storage
//! (patterns, conditionals, calls) are first built into a lexical temporary
//! record, so every source is evaluated once before any destination write.
use super::super::super::collection::native_values::NativeEndpointLeaf;
use super::super::super::collection::{
    column_tagged_union, port_array_index_vectors, RecordColumn, RecordValue,
};
use super::*;
use crate::sim::ir::{IrFixedValue, IrMemoryView};

impl Codegen<'_> {
    /// The storage columns of a column-layout record or of a sub-record of
    /// one (`r`, `r.inner`). Member arrays and leaves resolve elsewhere.
    pub(in crate::sim::codegen::lowering) fn column_record_storage(
        &self,
        node: NodeId,
    ) -> Option<RecordValue> {
        if !self.record_columns {
            return None;
        }
        if let Some((root, prefix)) = self.activation_record_path(node) {
            let value = self.activation_records.get(&root)?;
            let descriptor = Self::descriptor_at_path(&value.descriptor, &prefix)?;
            if !Self::column_record_value_type(&descriptor) {
                return None;
            }
            let columns = value
                .columns
                .iter()
                .filter(|(path, _)| path.starts_with(&prefix))
                .map(|(path, column)| (path[prefix.len()..].to_vec(), column.clone()))
                .collect::<Vec<_>>();
            return (!columns.is_empty()).then_some(RecordValue {
                descriptor,
                columns,
            });
        }
        let selection = self.resolve_unpacked_aggregate(node)?;
        if !selection.storage.columns || !Self::column_record_value_type(&selection.descriptor) {
            return None;
        }
        let columns = selection
            .storage
            .leaves
            .iter()
            .filter(|leaf| leaf.path.starts_with(&selection.prefix))
            .map(|leaf| {
                let path = leaf.path[selection.prefix.len()..].to_vec();
                let column = match &leaf.array {
                    Some(array) if record_cell_leaf(leaf) => RecordColumn::Cell(array.ir),
                    Some(array) => RecordColumn::Array(array.ir),
                    None => RecordColumn::Leaf(Box::new(leaf.clone())),
                };
                (path, column)
            })
            .collect::<Vec<_>>();
        (!columns.is_empty()).then_some(RecordValue {
            descriptor: selection.descriptor,
            columns,
        })
    }

    /// Whether a column-layout value of this type moves as a whole: an
    /// unpacked structure or a column-layout tagged union.
    fn column_record_value_type(descriptor: &TypeDescriptor) -> bool {
        matches!(&descriptor.shape,
            TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct)
            || column_tagged_union(descriptor)
    }

    /// Whether an assignment or comparison operand involves a column-layout
    /// record type.
    pub(in crate::sim::codegen::lowering) fn column_record_type(&self, node: NodeId) -> bool {
        self.record_columns
            && self
                .query_descriptor(node)
                .is_some_and(|descriptor| self.record_column_layout(descriptor))
    }

    /// Allocate a lexical temporary with the column layout of `descriptor`.
    pub(in crate::sim::codegen::lowering) fn record_temporary(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        out: &mut Vec<IrStmt>,
    ) -> Result<RecordValue, String> {
        let value = self.allocate_record_columns(path, descriptor, true)?;
        out.extend(self.declare_record_columns(&value));
        Ok(value)
    }

    /// The scalar leaf transfer endpoint of a real, string or chandle
    /// column.
    fn record_leaf_endpoint(column: &RecordColumn) -> Option<NativeEndpointLeaf> {
        match column {
            RecordColumn::Native(value, leaf) => {
                Some(NativeEndpointLeaf::Value(*value, (**leaf).clone()))
            }
            RecordColumn::Leaf(leaf) => Some(NativeEndpointLeaf::Module(leaf.clone())),
            RecordColumn::Array(_) | RecordColumn::Cell(_) => None,
        }
    }

    pub(in crate::sim::codegen::lowering) fn record_column_read(
        &self,
        column: &RecordColumn,
    ) -> Result<IrExpr, String> {
        match column {
            RecordColumn::Leaf(leaf) => {
                let value = self.aggregate_leaf_read(leaf)?;
                Ok(if leaf.member.two_state && !value.is_real() {
                    IrExpr::to_two_state(value)
                } else {
                    value
                })
            }
            RecordColumn::Cell(cell) => {
                let array = self.reference_array(*cell);
                Ok(IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: array,
                        indices: vec![lhs_integer_expr(0)],
                        elem_sel: IrElemSel::Whole,
                    },
                    self.model.arrays[array].elem_width,
                    self.model.arrays[array].signed,
                    None,
                ))
            }
            RecordColumn::Array(_) => Err("record array column is not a packed value".into()),
            RecordColumn::Native(..) => {
                Err("record native member is read through its native value".into())
            }
        }
    }

    pub(in crate::sim::codegen::lowering) fn record_column_lhs(
        &self,
        column: &RecordColumn,
    ) -> Result<IrLhs, String> {
        match column {
            RecordColumn::Leaf(leaf) => self.aggregate_leaf_lhs(leaf),
            RecordColumn::Cell(cell) => self.reference_lhs(IrLhs::ArrayElem {
                arr: self.reference_array(*cell),
                indices: vec![lhs_integer_expr(0)],
                elem_sel: IrElemSel::Whole,
            }),
            RecordColumn::Array(_) => Err("record array column is not a packed target".into()),
            RecordColumn::Native(..) => {
                Err("record native member is written through its native value".into())
            }
        }
    }

    pub(in crate::sim::codegen::lowering) fn record_column_view(
        &self,
        array: usize,
    ) -> IrMemoryView {
        self.fixed_view_at(self.reference_array(array), &[])
    }

    fn record_shapes_match(
        &self,
        path: &str,
        left: &RecordValue,
        right: &RecordValue,
    ) -> Result<(), String> {
        let same = copies::equivalent_copy_shape(&left.descriptor, &right.descriptor)
            && left.columns.len() == right.columns.len()
            && left
                .columns
                .iter()
                .zip(&right.columns)
                .all(|((left, _), (right, _))| left.len() == right.len());
        if same {
            Ok(())
        } else {
            Err(format!(
                "assignment between incompatible unpacked aggregate types `{}` and `{}` in `{path}`",
                left.descriptor.name, right.descriptor.name
            ))
        }
    }

    /// Copy `src` into `dst` column by column.
    pub(in crate::sim::codegen::lowering) fn record_copy(
        &mut self,
        path: &str,
        dst: &RecordValue,
        src: &RecordValue,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        self.record_shapes_match(path, dst, src)?;
        // Whole native values of one type copy at once (SIM-003).
        let natives = match (self.record_native_group(dst), self.record_native_group(src)) {
            (Some(target), Some(source))
                if !nba
                    && self.model.native_values[target].ty
                        == self.model.native_values[source].ty =>
            {
                if target != source {
                    out.push(IrStmt::NativeValueCopy {
                        dst: target,
                        src: source,
                    });
                }
                true
            }
            _ => false,
        };
        for ((_, target), (_, source)) in dst.columns.iter().zip(&src.columns) {
            if natives && matches!(target, RecordColumn::Native(..)) {
                continue;
            }
            self.record_column_copy(path, target, source, nba, out)?;
        }
        Ok(())
    }

    fn record_column_copy(
        &mut self,
        path: &str,
        target: &RecordColumn,
        source: &RecordColumn,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        match (target, source) {
            (RecordColumn::Array(target), RecordColumn::Array(source))
            | (RecordColumn::Cell(target), RecordColumn::Cell(source)) => {
                if target != source {
                    out.push(IrStmt::FixedValueAssign {
                        dst: self.record_column_view(*target),
                        src: Box::new(IrFixedValue::Array(self.record_column_view(*source))),
                        nba,
                    });
                }
            }
            (RecordColumn::Leaf(left), RecordColumn::Leaf(right))
                if left.object.is_some() || right.object.is_some() =>
            {
                let (Some(left), Some(right)) = (left.object, right.object) else {
                    return Err(format!(
                        "record copy pairs an object member with a value member in `{path}`"
                    ));
                };
                let left = self.reference_object(left);
                let right = self.reference_object(right);
                let string = matches!(self.model.objects[left].ty, IrObjectType::String);
                if nba {
                    let value = if string {
                        NativeNbaValue::String(IrStringExpr::Read(right))
                    } else {
                        NativeNbaValue::Chandle(IrChandleExpr::Read(right))
                    };
                    out.push(self.object_leaf_nba(path, left, value)?);
                } else {
                    out.push(IrStmt::Object(Box::new(if string {
                        IrObjectStmt::StringAssign(left, IrStringExpr::Read(right))
                    } else {
                        IrObjectStmt::ChandleAssign(left, IrChandleExpr::Read(right))
                    })));
                }
            }
            (RecordColumn::Array(_), _) | (_, RecordColumn::Array(_)) => {
                return Err(format!(
                    "record copy pairs an array member with a scalar member in `{path}`"
                ));
            }
            (RecordColumn::Native(..), _) | (_, RecordColumn::Native(..)) => {
                let (Some(target), Some(source)) = (
                    Self::record_leaf_endpoint(target),
                    Self::record_leaf_endpoint(source),
                ) else {
                    return Err(format!(
                        "record copy pairs a native member with a packed member in `{path}`"
                    ));
                };
                out.push(self.native_leaf_copy(path, &target, &source, nba)?);
            }
            _ => {
                let lhs = self.record_column_lhs(target)?;
                let value = self.record_column_read(source)?;
                out.push(IrStmt::Assign {
                    rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                    lhs,
                    nba,
                });
            }
        }
        Ok(())
    }

    /// One-bit equality of two record values (SV 11.4.5): every column must
    /// compare equal; a known mismatch decides 0 regardless of X columns.
    pub(in crate::sim::codegen::lowering) fn record_equality(
        &mut self,
        path: &str,
        left: &RecordValue,
        right: &RecordValue,
        case: bool,
    ) -> Result<IrExpr, String> {
        self.record_shapes_match(path, left, right)?;
        let mut equality: Option<IrExpr> = None;
        for ((_, left), (_, right)) in left.columns.iter().zip(&right.columns) {
            let column = match (left, right) {
                (RecordColumn::Array(left), RecordColumn::Array(right))
                | (RecordColumn::Cell(left), RecordColumn::Cell(right)) => IrExpr::new(
                    IrExprKind::FixedValueCompare {
                        left: Box::new(IrFixedValue::Array(self.record_column_view(*left))),
                        right: Box::new(IrFixedValue::Array(self.record_column_view(*right))),
                        case,
                        negate: false,
                    },
                    1,
                    false,
                    None,
                ),
                (RecordColumn::Native(..), _)
                | (_, RecordColumn::Native(..))
                | (RecordColumn::Leaf(_), RecordColumn::Leaf(_)) => {
                    let (Some(left), Some(right)) = (
                        Self::record_leaf_endpoint(left),
                        Self::record_leaf_endpoint(right),
                    ) else {
                        return Err(format!(
                            "record equality pairs a native member with a packed member in `{path}`"
                        ));
                    };
                    self.native_leaf_equality(path, &left, &right, case)?
                }
                _ => {
                    let (left, right) = (
                        self.record_column_read(left)?,
                        self.record_column_read(right)?,
                    );
                    if case {
                        if left.is_real() || right.is_real() {
                            real_leaf_case_cmp_ir(IrBinOp::CaseEq, left, right, path)?
                        } else {
                            cmp_expr_ir(IrBinOp::CaseEq, left, right)
                        }
                    } else {
                        common_cmp_expr_ir(IrBinOp::Eq, left, right, path)?
                    }
                }
            };
            equality = Some(match equality {
                // Four-state AND: a known 0 dominates any X column.
                Some(previous) => cmp_expr_ir(IrBinOp::LogAnd, previous, column),
                None => column,
            });
        }
        equality.ok_or_else(|| format!("record equality has no columns in `{path}`"))
    }

    /// Lower `lhs = rhs` (or `<=`) when either side is a column-layout record.
    pub(in crate::sim::codegen::lowering) fn lower_column_record_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        if !self.record_columns {
            return Ok(None);
        }
        let Some(destination) = self.column_record_storage(lhs) else {
            if self.column_record_storage(rhs).is_some() || self.column_record_type(rhs) {
                return self.lower_record_image_assignment(path, lhs, rhs, nba, op);
            }
            return Ok(None);
        };
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment of unpacked aggregates in `{path}` is not supported"
            ));
        }
        let mut out = Vec::new();
        self.lower_record_value_into(path, &destination, rhs, nba, &mut out)?;
        let mut statement = IrStmt::Block(out);
        // A whole record member of a column-layout tagged union is checked
        // against its tag like a member array (see
        // `lower_p30_fixed_array_assignment`).
        let source = self.p30_unwrap_cast(rhs);
        if let Some(guard) = self.record_member_guard(source)? {
            let mut inactive = Vec::new();
            self.record_fill_all_uninitialized(path, &destination, nba, &mut inactive)?;
            statement = IrStmt::If {
                cond: Self::record_guard_check(guard, self.source_location(source))?,
                then_: vec![statement],
                els: Some(inactive),
                check: IrUniquePriorityCheck::None,
            };
        }
        if let Some(guard) = self.record_member_guard(lhs)? {
            statement = IrStmt::If {
                cond: Self::record_guard_check(guard, self.source_location(lhs))?,
                then_: vec![statement],
                els: None,
                check: IrUniquePriorityCheck::None,
            };
        }
        Ok(Some(statement))
    }

    /// Give every member of `target` its default-uninitialized value.
    fn record_fill_all_uninitialized(
        &mut self,
        path: &str,
        target: &RecordValue,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let TypeShape::Aggregate(layout) = &target.descriptor.shape else {
            return Err(format!("record value in `{path}` has no structure type"));
        };
        let layout = layout.clone();
        if layout.kind == AggregateKind::TaggedUnion {
            if let Some((_, RecordColumn::Cell(tag))) = target
                .columns
                .iter()
                .find(|(column_path, _)| column_path.is_empty())
            {
                let (width, two_state) = (
                    self.model.arrays[*tag].elem_width,
                    self.model.arrays[*tag].two_state,
                );
                out.push(IrStmt::FixedArrayFill {
                    array: self.reference_array(*tag),
                    value: IrExpr::new(
                        IrExprKind::Const(uniform_constant(width, false, two_state)),
                        width,
                        false,
                        None,
                    ),
                    nba,
                });
            }
        }
        for member in &layout.members {
            let key = AggregatePathPart::Member(member.name.clone());
            let columns = target
                .columns
                .iter()
                .filter(|(column_path, _)| column_path.first() == Some(&key))
                .cloned()
                .collect::<Vec<_>>();
            if columns.is_empty() {
                continue;
            }
            let member_value = RecordValue {
                descriptor: member.descriptor.clone(),
                columns,
            };
            self.record_fill_uninitialized(path, member, &member_value, nba, out)?;
        }
        Ok(())
    }

    /// Evaluate `node` into `dst`.
    fn lower_record_value_into(
        &mut self,
        path: &str,
        dst: &RecordValue,
        node: NodeId,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let node = self.p30_unwrap_cast(node);
        if let Some(source) = self.column_record_storage(node) {
            return self.record_copy(path, dst, &source, nba, out);
        }
        if let NodeKind::Expr(ExprKind::Operation {
            op: Operation::Conditional,
            operands,
            ..
        }) = self.kind(node)
        {
            let operands = operands.clone();
            return self.lower_record_conditional_into(path, dst, &operands, nba, out);
        }
        if let NodeKind::Expr(ExprKind::TaggedUnion { member, value }) = self.kind(node) {
            let (member, value) = (member.clone(), *value);
            let temporary = self.record_temporary(path, &dst.descriptor, out)?;
            self.lower_tagged_into(path, &temporary, &member, value, out)?;
            return self.record_copy(path, dst, &temporary, nba, out);
        }
        if self.assignment_pattern_operands(path, node)?.is_some() {
            let temporary = self.record_temporary(path, &dst.descriptor, out)?;
            self.lower_record_pattern_into(path, &temporary, node, out)?;
            return self.record_copy(path, dst, &temporary, nba, out);
        }
        if let Some(statements) = self.lower_record_call_into(path, dst, node, nba)? {
            out.extend(statements);
            return Ok(());
        }
        // Any other value of the type is its packed image: an element of an
        // array of records, a parameter, a bit-stream cast.
        let image = self.lower_expr(path, node)?;
        self.record_store_image(path, dst, image, nba, out)
    }

    /// The packed image of a record: the first member is the most
    /// significant, as in the leaf layout of the same type.
    pub(in crate::sim::codegen::lowering) fn record_image(
        &self,
        path: &str,
        value: &RecordValue,
    ) -> Result<IrExpr, String> {
        let mut parts = Vec::with_capacity(value.columns.len());
        for (member, column) in &value.columns {
            let part = match column {
                RecordColumn::Array(array) => self.column_image(*array).ok_or_else(|| {
                    format!(
                        "record member `{}` in `{path}` exceeds packed capacity",
                        aggregate_path_suffix(member)
                    )
                })?,
                _ => self.record_column_read(column)?,
            };
            if part.is_real() {
                return Err(format!(
                    "real record member `{}` has no packed image in `{path}`",
                    aggregate_path_suffix(member)
                ));
            }
            parts.push(part);
        }
        let width = parts.iter().map(|part| u64::from(part.width)).sum::<u64>();
        let width = u32::try_from(width)
            .ok()
            .filter(|width| *width <= LLG_MAX_WIDTH)
            .ok_or_else(|| format!("record value in `{path}` exceeds packed capacity"))?;
        Ok(IrExpr::new(
            IrExprKind::Concat { parts },
            width,
            false,
            None,
        ))
    }

    /// Store the packed image of a record into its columns. The image is
    /// captured first, so it may read the record it replaces.
    fn record_store_image(
        &mut self,
        path: &str,
        dst: &RecordValue,
        image: IrExpr,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let width = Self::record_payload_width(&dst.descriptor)
            .ok_or_else(|| format!("record value in `{path}` has no packed image"))?;
        if image.width != width {
            return Err(format!(
                "record value in `{path}` is {} bits wide for a record of {width} bits",
                image.width
            ));
        }
        let name = self.new_fn_name(path, "record_image");
        out.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed: false,
            two_state: false,
            init: Some(Box::new(IrExpr::resize_to(image, width, false))),
        });
        let mut left = i64::from(width);
        for (member, column) in &dst.columns {
            let (bits, cell_width, two_state) = match column {
                RecordColumn::Array(array) | RecordColumn::Cell(array) => {
                    let array = &self.model.arrays[self.reference_array(*array)];
                    (
                        array.total * u64::from(array.elem_width),
                        array.elem_width,
                        array.two_state,
                    )
                }
                RecordColumn::Leaf(leaf) if leaf.object.is_none() => {
                    let read = self.record_column_read(column)?;
                    (u64::from(read.width), read.width, false)
                }
                RecordColumn::Leaf(_) | RecordColumn::Native(..) => {
                    return Err(format!(
                        "record member `{}` has no packed image in `{path}`",
                        aggregate_path_suffix(member)
                    ));
                }
            };
            let right = left - bits as i64;
            let slice = IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(IrExpr::new(
                        IrExprKind::LocalRead(name.clone()),
                        width,
                        false,
                        None,
                    )),
                    left: left - 1,
                    right,
                },
                bits as u32,
                false,
                None,
            );
            let slice = if two_state {
                IrExpr::to_two_state(slice)
            } else {
                slice
            };
            left = right;
            if let RecordColumn::Array(array) = column {
                out.push(IrStmt::FixedValueAssign {
                    dst: self.record_column_view(*array),
                    src: Box::new(IrFixedValue::Packed {
                        value: Box::new(slice),
                        cell_width,
                        runtime_sized: false,
                    }),
                    nba,
                });
            } else {
                let lhs = self.record_column_lhs(column)?;
                out.push(IrStmt::Assign {
                    rhs: apply_lhs_assignment_context(&self.model, &lhs, slice),
                    lhs,
                    nba,
                });
            }
        }
        Ok(())
    }

    /// `lhs = rhs` where `lhs` holds a record as a packed image, not as
    /// columns: an element of an array of records.
    fn lower_record_image_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment of unpacked aggregates in `{path}` is not supported"
            ));
        }
        let descriptor = self
            .query_descriptor(lhs)
            .cloned()
            .ok_or_else(|| format!("record assignment target in `{path}` has no type"))?;
        let mut out = Vec::new();
        let source = self.record_value_of(path, &descriptor, rhs, &mut out)?;
        let image = self.record_image(path, &source)?;
        let target = self.analyze_lhs(path, lhs)?;
        let target = self.lhs_to_ir(target)?;
        out.push(IrStmt::Assign {
            rhs: apply_lhs_assignment_context(&self.model, &target, image),
            lhs: target,
            nba,
        });
        Ok(Some(IrStmt::Block(out)))
    }

    /// The statements of a declaration initializer of record `value`.
    pub(in crate::sim::codegen::lowering) fn lower_record_initializer(
        &mut self,
        path: &str,
        value: &RecordValue,
        initializer: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let mut out = Vec::new();
        self.lower_record_value_into(path, value, initializer, false, &mut out)?;
        Ok(out)
    }

    /// Evaluate `node` once into a fresh temporary of type `descriptor`.
    pub(in crate::sim::codegen::lowering) fn record_snapshot(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        node: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<RecordValue, String> {
        let temporary = self.record_temporary(path, descriptor, out)?;
        self.lower_record_value_into(path, &temporary, node, false, out)?;
        Ok(temporary)
    }

    /// The value of `node` as storage, or as a temporary built from it.
    pub(in crate::sim::codegen::lowering) fn record_value_of(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        node: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<RecordValue, String> {
        let unwrapped = self.p30_unwrap_cast(node);
        if let Some(source) = self.column_record_storage(unwrapped) {
            return Ok(source);
        }
        let temporary = self.record_temporary(path, descriptor, out)?;
        self.lower_record_value_into(path, &temporary, unwrapped, false, out)?;
        Ok(temporary)
    }

    /// `dst = sel ? left : right` (SV 11.4.11). A known selector evaluates
    /// one arm; an ambiguous selector evaluates both once and keeps each
    /// leaf that is equal (no X/Z) in both arms, giving the leaf's
    /// default-uninitialized value otherwise — the rule packed structure
    /// merges apply to records within packed capacity.
    fn lower_record_conditional_into(
        &mut self,
        path: &str,
        dst: &RecordValue,
        operands: &[NodeId],
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let [selector, left, right] = operands else {
            return Err(format!("record conditional in `{path}` has no arms"));
        };
        let selector = self.lower_boolean_expr(path, *selector)?;
        let name = self.new_fn_name(path, "record_select");
        let (width, signed) = (selector.width, selector.signed);
        out.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            init: Some(Box::new(selector)),
            two_state: false,
        });
        let captured = IrExpr::new(IrExprKind::LocalRead(name), width, signed, None);
        let known = |value: u64| -> Result<IrExpr, String> {
            Ok(cmp_expr_ir(
                IrBinOp::CaseEq,
                IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::RedOr,
                        a: Box::new(captured.clone()),
                    },
                    1,
                    false,
                    None,
                ),
                IrExpr::new(
                    IrExprKind::Const(
                        IrConst::packed(vec![value], vec![], vec![], 1, false, None)
                            .map_err(|error| error.to_string())?,
                    ),
                    1,
                    false,
                    None,
                ),
            ))
        };
        let mut when_true = Vec::new();
        self.lower_record_value_into(path, dst, *left, nba, &mut when_true)?;
        let mut when_false = Vec::new();
        self.lower_record_value_into(path, dst, *right, nba, &mut when_false)?;
        let mut ambiguous = Vec::new();
        let left_value = self.record_value_of(path, &dst.descriptor, *left, &mut ambiguous)?;
        let right_value = self.record_value_of(path, &dst.descriptor, *right, &mut ambiguous)?;
        self.record_merge(path, dst, &left_value, &right_value, nba, &mut ambiguous)?;
        out.push(IrStmt::If {
            cond: known(1)?,
            then_: when_true,
            els: Some(vec![IrStmt::If {
                cond: known(0)?,
                then_: when_false,
                els: Some(ambiguous),
                check: IrUniquePriorityCheck::None,
            }]),
            check: IrUniquePriorityCheck::None,
        });
        Ok(())
    }

    fn record_merge(
        &mut self,
        path: &str,
        dst: &RecordValue,
        left: &RecordValue,
        right: &RecordValue,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        self.record_shapes_match(path, dst, left)?;
        self.record_shapes_match(path, dst, right)?;
        let TypeShape::Aggregate(layout) = &dst.descriptor.shape else {
            return Err(format!("record merge in `{path}` has no structure type"));
        };
        if layout.kind == AggregateKind::TaggedUnion {
            // The tag cell merges as one unit: equal tags survive, others
            // become the tag's uninitialized value.
            let tag = |value: &RecordValue| {
                value
                    .columns
                    .iter()
                    .find(|(column_path, _)| column_path.is_empty())
                    .map(|(_, column)| column.clone())
                    .ok_or("tagged union value has no tag column")
            };
            let (target, from_left, from_right) = (tag(dst)?, tag(left)?, tag(right)?);
            let RecordColumn::Cell(target_cell) = target.clone() else {
                return Err(format!("tagged union tag in `{path}` is not a cell"));
            };
            let equal = common_cmp_expr_ir(
                IrBinOp::Eq,
                self.record_column_read(&from_left)?,
                self.record_column_read(&from_right)?,
                path,
            )?;
            let mut keep = Vec::new();
            self.record_column_copy(path, &target, &from_left, nba, &mut keep)?;
            let width = self.model.arrays[target_cell].elem_width;
            let two_state = self.model.arrays[target_cell].two_state;
            let reset = vec![IrStmt::FixedArrayFill {
                array: self.reference_array(target_cell),
                value: IrExpr::new(
                    IrExprKind::Const(uniform_constant(width, false, two_state)),
                    width,
                    false,
                    None,
                ),
                nba,
            }];
            out.push(IrStmt::If {
                cond: equal,
                then_: keep,
                els: Some(reset),
                check: IrUniquePriorityCheck::None,
            });
        }
        for member in &layout.members {
            let key = AggregatePathPart::Member(member.name.clone());
            let select = |value: &RecordValue| RecordValue {
                descriptor: member.descriptor.clone(),
                columns: value
                    .columns
                    .iter()
                    .filter(|(column_path, _)| column_path.first() == Some(&key))
                    .cloned()
                    .collect(),
            };
            let (target, from_left, from_right) = (select(dst), select(left), select(right));
            if layout.kind == AggregateKind::TaggedUnion {
                // A tagged union member merges as a whole.
                self.record_merge_columns(
                    path,
                    member,
                    &target,
                    &from_left,
                    &from_right,
                    nba,
                    out,
                )?;
                continue;
            }
            // A structure merges leaf by leaf (SV 11.4.11): each column on
            // its own, and a member array element by element.
            for index in 0..target.columns.len() {
                let column = |value: &RecordValue| RecordValue {
                    descriptor: value.descriptor.clone(),
                    columns: vec![value.columns[index].clone()],
                };
                let (target, from_left, from_right) =
                    (column(&target), column(&from_left), column(&from_right));
                if let (
                    (column_path, RecordColumn::Array(target_array)),
                    (_, RecordColumn::Array(left_array)),
                    (_, RecordColumn::Array(right_array)),
                ) = (
                    &target.columns[0],
                    &from_left.columns[0],
                    &from_right.columns[0],
                ) {
                    let leaf = Self::descriptor_at_path(&member.descriptor, &column_path[1..])
                        .ok_or_else(|| format!("record merge column is unresolved in `{path}`"))?;
                    let ambiguous = IrConst::packed(vec![], vec![1], vec![], 1, false, None)
                        .map_err(|error| error.to_string())?;
                    out.push(IrStmt::FixedValueAssign {
                        dst: self.record_column_view(*target_array),
                        src: Box::new(IrFixedValue::Conditional {
                            selector: Box::new(IrExpr::new(
                                IrExprKind::Const(ambiguous),
                                1,
                                false,
                                None,
                            )),
                            left: Box::new(IrFixedValue::Array(
                                self.record_column_view(*left_array),
                            )),
                            right: Box::new(IrFixedValue::Array(
                                self.record_column_view(*right_array),
                            )),
                            cell_leaves: super::super::fixed_cell_leaves(&leaf)?,
                        }),
                        nba,
                    });
                    continue;
                }
                self.record_merge_columns(
                    path,
                    member,
                    &target,
                    &from_left,
                    &from_right,
                    nba,
                    out,
                )?;
            }
        }
        Ok(())
    }

    /// Keep the columns of `target` from `left` when every one is equal in
    /// both arms, and reset them to their uninitialized value otherwise.
    #[allow(clippy::too_many_arguments)]
    fn record_merge_columns(
        &mut self,
        path: &str,
        member: &AggregateMember,
        target: &RecordValue,
        left: &RecordValue,
        right: &RecordValue,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let equal = self.record_equality(path, left, right, false)?;
        let mut keep = Vec::new();
        self.record_copy(path, target, left, nba, &mut keep)?;
        let mut reset = Vec::new();
        self.record_fill_uninitialized(path, member, target, nba, &mut reset)?;
        out.push(IrStmt::If {
            cond: equal,
            then_: keep,
            els: Some(reset),
            check: IrUniquePriorityCheck::None,
        });
        Ok(())
    }

    /// Give the columns of immediate member `member` its
    /// default-uninitialized value.
    fn record_fill_uninitialized(
        &mut self,
        path: &str,
        member: &AggregateMember,
        target: &RecordValue,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        for (column_path, column) in &target.columns {
            let leaf = Self::descriptor_at_path(&target.descriptor, &column_path[1..])
                .ok_or_else(|| format!("record merge column is unresolved in `{path}`"))?;
            let two_state = member.two_state || leaf.two_state;
            match column {
                RecordColumn::Cell(cell) => {
                    let width = self.model.arrays[*cell].elem_width;
                    let signed = self.model.arrays[*cell].signed;
                    let value = Self::fixed_descriptor_uninitialized(&leaf)
                        .unwrap_or_else(|| uniform_constant(width, signed, two_state));
                    out.push(IrStmt::FixedArrayFill {
                        array: self.reference_array(*cell),
                        value: IrExpr::new(IrExprKind::Const(value), width, signed, None),
                        nba,
                    });
                }
                RecordColumn::Array(array) => {
                    let TypeShape::FixedArray { element, .. } = &leaf.shape else {
                        return Err(format!("record merge column is not an array in `{path}`"));
                    };
                    let width = Self::fixed_descriptor_width(element)
                        .ok_or("record merge element has no width")?;
                    let signed = self.model.arrays[*array].signed;
                    let value =
                        Self::fixed_descriptor_uninitialized(element).unwrap_or_else(|| {
                            uniform_constant(width, signed, two_state || element.two_state)
                        });
                    out.push(IrStmt::FixedArrayFill {
                        array: self.reference_array(*array),
                        value: IrExpr::new(IrExprKind::Const(value), width, signed, None),
                        nba,
                    });
                }
                RecordColumn::Native(..) | RecordColumn::Leaf(_) => {
                    let target = Self::record_leaf_endpoint(column)
                        .ok_or("record native member has no endpoint")?;
                    out.push(self.native_leaf_reset(path, &target, nba)?);
                }
            }
        }
        Ok(())
    }

    /// Build an assignment pattern into the fresh temporary `dst`.
    fn lower_record_pattern_into(
        &mut self,
        path: &str,
        dst: &RecordValue,
        node: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let TypeShape::Aggregate(layout) = &dst.descriptor.shape else {
            return Err(format!("record pattern in `{path}` has no structure type"));
        };
        let layout = layout.clone();
        let pattern = self.unwrap_assignment_pattern_cast(node);
        let values = self.aggregate_pattern_values(path, pattern, &layout)?;
        let mut captured = HashMap::<NodeId, (String, u32, bool)>::new();
        for (index, value) in values {
            let member = layout.members.get(index).ok_or_else(|| {
                format!("record pattern member index {index} is out of bounds in `{path}`")
            })?;
            let key = AggregatePathPart::Member(member.name.clone());
            let target = RecordValue {
                descriptor: member.descriptor.clone(),
                columns: dst
                    .columns
                    .iter()
                    .filter(|(column_path, _)| column_path.first() == Some(&key))
                    .map(|(column_path, column)| (column_path[1..].to_vec(), column.clone()))
                    .collect(),
            };
            match &member.descriptor.shape {
                TypeShape::FixedArray { element, .. } => {
                    let Some((_, RecordColumn::Array(array))) = target.columns.first() else {
                        return Err(format!(
                            "record pattern array member has no column in `{path}`"
                        ));
                    };
                    let array = *array;
                    self.lower_record_array_item(
                        path,
                        array,
                        &member.descriptor,
                        element,
                        value,
                        out,
                    )?;
                }
                TypeShape::Aggregate(nested) if nested.kind == AggregateKind::UnpackedStruct => {
                    let item = self.p30_unwrap_cast(value);
                    if self.assignment_pattern_operands(path, item)?.is_some() {
                        self.lower_record_pattern_into(path, &target, item, out)?;
                    } else {
                        self.lower_record_value_into(path, &target, item, false, out)?;
                    }
                }
                _ => {
                    let Some((_, column)) = target.columns.first() else {
                        return Err(format!("record pattern member has no column in `{path}`"));
                    };
                    if let Some(leaf) = Self::record_leaf_endpoint(column) {
                        out.push(self.native_leaf_assign(path, &leaf, value, false)?);
                        continue;
                    }
                    let lhs = self.record_column_lhs(column)?;
                    let value = if let Some((name, width, signed)) = captured.get(&value) {
                        IrExpr::new(IrExprKind::LocalRead(name.clone()), *width, *signed, None)
                    } else {
                        let source = self.lower_expr(path, value)?;
                        let name = self.new_fn_name(path, "record_item");
                        let (width, signed) = (source.width, source.signed);
                        out.push(IrStmt::DeclLocal {
                            name: name.clone(),
                            width,
                            signed,
                            two_state: false,
                            init: Some(Box::new(source)),
                        });
                        captured.insert(value, (name.clone(), width, signed));
                        IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
                    };
                    out.push(IrStmt::Assign {
                        rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                        lhs,
                        nba: false,
                    });
                }
            }
        }
        Ok(())
    }

    /// One array-member item of a record pattern: a nested pattern, an
    /// equivalent array value, or a fill value for every element.
    fn lower_record_array_item(
        &mut self,
        path: &str,
        array: usize,
        descriptor: &TypeDescriptor,
        element: &TypeDescriptor,
        value: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let info = ArrayInfo {
            global: self.model.arrays[array].c_name.clone(),
            elem_width: self.model.arrays[array].elem_width,
            signed: self.model.arrays[array].signed,
            real: false,
            shortreal: false,
            is_net: false,
            dims: self.model.arrays[array].dims.clone(),
            init: None,
            ir: array,
        };
        if let Some(statements) =
            self.lower_descriptor_pattern_into(path, value, descriptor, &info)?
        {
            out.extend(statements);
            return Ok(());
        }
        let source = self.query_descriptor(value).cloned();
        if source
            .as_ref()
            .is_some_and(|source| copies::equivalent_copy_shape(source, descriptor))
        {
            let value = self.lower_fixed_value(path, value)?;
            out.push(IrStmt::FixedValueAssign {
                dst: self.record_column_view(array),
                src: Box::new(value),
                nba: false,
            });
            return Ok(());
        }
        let width =
            Self::fixed_descriptor_width(element).ok_or("record fill element has no width")?;
        let value = if matches!(element.shape, TypeShape::PackedAtom { .. }) {
            IrExpr::convert_to(self.lower_expr(path, value)?, width, element.info.signed)
        } else {
            self.record_fill_image(path, element, value, out)?
        };
        out.push(IrStmt::FixedArrayFill {
            array,
            value,
            nba: false,
        });
        Ok(())
    }

    /// The cell image of a fill of structured elements: `value` reaches each
    /// leaf of the element (SV 10.9.2), or is an element itself. Each source
    /// is evaluated once.
    fn record_fill_image(
        &mut self,
        path: &str,
        element: &TypeDescriptor,
        value: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<IrExpr, String> {
        let mut leaves = Vec::new();
        self.aggregate_descriptor_pattern_values(path, value, element, &[], &mut leaves)?;
        let mut parts = Vec::with_capacity(leaves.len());
        let mut sources = HashMap::<NodeId, IrExpr>::new();
        for (member, source) in leaves {
            let (leaf, offset) =
                super::super::super::collection::fixed_path_descriptor(element, &member)
                    .ok_or("record fill leaf path is invalid")?;
            let width =
                Self::fixed_descriptor_width(&leaf).ok_or("record fill leaf has no width")?;
            let read = match sources.get(&source) {
                Some(read) => read.clone(),
                None => {
                    let item = match self.column_record_storage(source) {
                        Some(record) => self.record_image(path, &record)?,
                        None => self.lower_expr(path, source)?,
                    };
                    let name = self.new_fn_name(path, "record_item");
                    let read = IrExpr::new(
                        IrExprKind::LocalRead(name.clone()),
                        item.width,
                        item.signed,
                        None,
                    );
                    out.push(IrStmt::DeclLocal {
                        name,
                        width: item.width,
                        signed: item.signed,
                        two_state: false,
                        init: Some(Box::new(item)),
                    });
                    sources.insert(source, read.clone());
                    read
                }
            };
            parts.push((offset, IrExpr::convert_to(read, width, leaf.info.signed)));
        }
        parts.sort_by_key(|(offset, _)| std::cmp::Reverse(*offset));
        Self::join_bitstream_parts(path, parts.into_iter().map(|(_, part)| part).collect())
    }

    /// `'{x, y, ...} = rec` with a column-layout record source: the source is
    /// snapshotted, every target and its selectors are frozen, then each
    /// target receives its member's columns (SV 10.10: the right-hand side is
    /// evaluated before the targets).
    pub(in crate::sim::codegen::lowering) fn lower_record_pattern_scatter(
        &mut self,
        path: &str,
        pattern: NodeId,
        rhs: NodeId,
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        if !self.record_columns
            || (self.column_record_storage(rhs).is_none() && !self.column_record_type(rhs))
        {
            return Ok(None);
        }
        let descriptor = self
            .query_descriptor(rhs)
            .cloned()
            .ok_or_else(|| format!("assignment-pattern source in `{path}` has no type"))?;
        let mut out = Vec::new();
        let snapshot = self.record_temporary(path, &descriptor, &mut out)?;
        self.lower_record_value_into(path, &snapshot, rhs, false, &mut out)?;
        let mut writes = Vec::new();
        let mut sequence = 0usize;
        self.record_scatter_targets(
            path,
            pattern,
            &snapshot,
            nba,
            &mut sequence,
            &mut out,
            &mut writes,
        )?;
        out.extend(writes);
        Ok(Some(IrStmt::Block(out)))
    }

    #[allow(clippy::too_many_arguments)]
    fn record_scatter_targets(
        &mut self,
        path: &str,
        pattern: NodeId,
        value: &RecordValue,
        nba: bool,
        sequence: &mut usize,
        captures: &mut Vec<IrStmt>,
        writes: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let TypeShape::Aggregate(layout) = &value.descriptor.shape else {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` does not match its record source"
            ));
        };
        let layout = layout.clone();
        let pattern = self.p30_unwrap_cast(pattern);
        let operands = self.assignment_pattern_operands(path, pattern)?.ok_or_else(|| {
            format!("positional assignment-pattern lvalue in `{path}` must contain only positional targets")
        })?;
        if operands.len() != layout.members.len() {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` has {} positional targets; expected {}",
                operands.len(),
                layout.members.len()
            ));
        }
        for (member, operand) in layout.members.iter().zip(operands) {
            let target = self.p30_pattern_lvalue_operand(path, operand)?;
            let key = AggregatePathPart::Member(member.name.clone());
            let member_value = RecordValue {
                descriptor: member.descriptor.clone(),
                columns: value
                    .columns
                    .iter()
                    .filter(|(column_path, _)| column_path.first() == Some(&key))
                    .map(|(column_path, column)| (column_path[1..].to_vec(), column.clone()))
                    .collect(),
            };
            let nested = self
                .assignment_pattern_operands(path, self.p30_unwrap_cast(target))?
                .is_some();
            if nba
                && (self.proc_local_target(target).is_some() || self.subroutine_auto_target(target))
            {
                return Err(format!(
                    "nonblocking assignment to an automatic assignment-pattern target in `{path}` is not supported"
                ));
            }
            match &member.descriptor.shape {
                TypeShape::Aggregate(nested_layout)
                    if nested_layout.kind == AggregateKind::UnpackedStruct && nested =>
                {
                    self.record_scatter_targets(
                        path,
                        target,
                        &member_value,
                        nba,
                        sequence,
                        captures,
                        writes,
                    )?;
                }
                TypeShape::Aggregate(nested_layout)
                    if nested_layout.kind == AggregateKind::UnpackedStruct =>
                {
                    let destination = self.column_record_storage(target).ok_or_else(|| {
                        format!("assignment-pattern lvalue in `{path}` needs column-layout record storage for a record member")
                    })?;
                    self.record_copy(path, &destination, &member_value, nba, writes)?;
                }
                TypeShape::FixedArray { .. } => {
                    if nested {
                        return Err(format!(
                            "assignment-pattern lvalue in `{path}` cannot split a record member array into elements"
                        ));
                    }
                    let Some((_, RecordColumn::Array(column))) = member_value.columns.first()
                    else {
                        return Err(format!("record member array has no column in `{path}`"));
                    };
                    // A net array has no memory view: one assignment drives
                    // it, as continuous pattern leaves expect (SV 10.9).
                    if let Some(array) = self.array_of(target).filter(|a| a.is_net).cloned() {
                        let value = self.column_image(*column).ok_or_else(|| {
                            format!("record member array in `{path}` exceeds packed capacity")
                        })?;
                        let arr = self.reference_array(array.ir);
                        let parts = port_array_index_vectors(&array.dims)
                            .into_iter()
                            .map(|cell| {
                                (
                                    IrLhs::ArrayElem {
                                        arr,
                                        indices: cell
                                            .into_iter()
                                            .map(|index| lhs_integer_expr(i128::from(index)))
                                            .collect(),
                                        elem_sel: IrElemSel::Whole,
                                    },
                                    array.elem_width,
                                )
                            })
                            .collect();
                        writes.push(IrStmt::Assign {
                            lhs: IrLhs::Stream {
                                parts,
                                width: value.width,
                                slice: 1,
                                direction: IrStreamDirection::LeftToRight,
                            },
                            rhs: value,
                            nba,
                        });
                        continue;
                    }
                    // A dense array takes no descriptor assignment.
                    if let Some(array) = self
                        .array_of(target)
                        .filter(|array| !self.model.arrays[array.ir].sparse())
                        .cloned()
                    {
                        let source = self.reference_array(*column);
                        let info = self.model.arrays[source].clone();
                        let cells = port_array_index_vectors(&array.dims);
                        let values = port_array_index_vectors(&info.dims);
                        if cells.len() != values.len() {
                            return Err(format!(
                                "assignment-pattern lvalue array shape mismatch in `{path}`"
                            ));
                        }
                        for (cell, value) in cells.into_iter().zip(values) {
                            let lhs = self.lhs_to_ir(Lhs::ArrayElem(ArrayElemLhs {
                                arr: array.clone(),
                                indices: cell
                                    .into_iter()
                                    .map(|index| lhs_integer_expr(i128::from(index)))
                                    .collect(),
                                elem_sel: ElemSel::Whole,
                            }))?;
                            let value = IrExpr::new(
                                IrExprKind::ArrayRead {
                                    arr: source,
                                    indices: value
                                        .into_iter()
                                        .map(|index| lhs_integer_expr(i128::from(index)))
                                        .collect(),
                                    elem_sel: IrElemSel::Whole,
                                },
                                info.elem_width,
                                info.signed,
                                None,
                            );
                            writes.push(IrStmt::Assign {
                                rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                                lhs,
                                nba,
                            });
                        }
                        continue;
                    }
                    let mut view = self.fixed_memory_view(path, target)?;
                    for selector in &mut view.selectors {
                        let name = self.new_fn_name(path, "pattern_selector");
                        let (width, signed) = (selector.value.width, selector.value.signed);
                        let value = std::mem::replace(
                            &mut selector.value,
                            IrExpr::new(IrExprKind::LocalRead(name.clone()), width, signed, None),
                        );
                        captures.push(IrStmt::DeclLocal {
                            name,
                            width,
                            signed,
                            two_state: false,
                            init: Some(Box::new(value)),
                        });
                    }
                    if view.total != self.model.arrays[*column].total {
                        return Err(format!(
                            "assignment-pattern lvalue array shape mismatch in `{path}`"
                        ));
                    }
                    writes.push(IrStmt::FixedValueAssign {
                        dst: view,
                        src: Box::new(IrFixedValue::Array(self.record_column_view(*column))),
                        nba,
                    });
                }
                _ => {
                    if nested {
                        return Err(format!(
                            "assignment-pattern lvalue nesting in `{path}` does not match its target type"
                        ));
                    }
                    let Some((_, column)) = member_value.columns.first() else {
                        return Err(format!("record member has no column in `{path}`"));
                    };
                    let lhs = self.lower_lhs(path, target)?;
                    let mut frozen = Vec::new();
                    let (lhs, _) =
                        self.freeze_call_lhs(lhs, "pattern_targets", sequence, &mut frozen)?;
                    captures.extend(frozen.into_iter().map(
                        |(name, width, signed, two_state, expr)| IrStmt::DeclLocal {
                            name,
                            width,
                            signed,
                            two_state,
                            init: Some(Box::new(expr)),
                        },
                    ));
                    let value = self.record_column_read(column)?;
                    writes.push(IrStmt::Assign {
                        rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                        lhs,
                        nba,
                    });
                }
            }
        }
        Ok(())
    }

    /// Calls returning a column-layout record (filled in by the call ABI).
    /// `tagged m value` into the fresh temporary `dst`: the tag names `m`,
    /// `m` receives the value and every other member its uninitialized value,
    /// as the payload padding of a packed tagged union does.
    fn lower_tagged_into(
        &mut self,
        path: &str,
        dst: &RecordValue,
        member_name: &str,
        value: Option<NodeId>,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let TypeShape::Aggregate(layout) = &dst.descriptor.shape else {
            return Err(format!("tagged expression in `{path}` has no union type"));
        };
        let layout = layout.clone();
        let index = layout
            .members
            .iter()
            .position(|member| member.name == member_name)
            .ok_or_else(|| format!("tagged union has no member `{member_name}` in `{path}`"))?;
        let Some((_, tag)) = dst
            .columns
            .iter()
            .find(|(column_path, _)| column_path.is_empty())
        else {
            return Err(format!("tagged union in `{path}` has no tag column"));
        };
        let tag_lhs = self.record_column_lhs(tag)?;
        let tag_width =
            packed_lhs_width(&self.model, &tag_lhs).ok_or("tagged union tag has no width")?;
        let tag_value = IrConst::packed(
            vec![u64::try_from(index).map_err(|_| "tagged union member index overflows")?],
            Vec::new(),
            Vec::new(),
            tag_width,
            false,
            None,
        )
        .map_err(|error| error.to_string())?;
        out.push(IrStmt::Assign {
            lhs: tag_lhs,
            rhs: IrExpr::new(IrExprKind::Const(tag_value), tag_width, false, None),
            nba: false,
        });
        for (position, member) in layout.members.iter().enumerate() {
            let key = AggregatePathPart::Member(member.name.clone());
            let columns = dst
                .columns
                .iter()
                .filter(|(column_path, _)| column_path.first() == Some(&key))
                .cloned()
                .collect::<Vec<_>>();
            if columns.is_empty() {
                continue;
            }
            if position != index {
                let target = RecordValue {
                    descriptor: member.descriptor.clone(),
                    columns,
                };
                self.record_fill_uninitialized(path, member, &target, false, out)?;
                continue;
            }
            let Some(value) = value else {
                return Err(format!(
                    "tagged member `{member_name}` has no value in `{path}`"
                ));
            };
            let relative = RecordValue {
                descriptor: member.descriptor.clone(),
                columns: columns
                    .iter()
                    .map(|(column_path, column)| (column_path[1..].to_vec(), column.clone()))
                    .collect(),
            };
            match &member.descriptor.shape {
                TypeShape::FixedArray { element, .. } => {
                    let Some((_, RecordColumn::Array(array))) = relative.columns.first() else {
                        return Err(format!(
                            "tagged member `{member_name}` has no column in `{path}`"
                        ));
                    };
                    let array = *array;
                    self.lower_record_array_item(
                        path,
                        array,
                        &member.descriptor,
                        element,
                        value,
                        out,
                    )?;
                }
                _ if Self::column_record_value_type(&member.descriptor) => {
                    self.lower_record_value_into(path, &relative, value, false, out)?;
                }
                _ => {
                    let Some((_, column)) = relative.columns.first() else {
                        return Err(format!(
                            "tagged member `{member_name}` has no column in `{path}`"
                        ));
                    };
                    let lhs = self.record_column_lhs(column)?;
                    let width =
                        packed_lhs_width(&self.model, &lhs).ok_or("tagged member has no width")?;
                    let source = ir_to_storage(
                        self.lower_expr(path, value)?,
                        width,
                        member.descriptor.info.signed,
                        member.descriptor.two_state,
                    )?;
                    out.push(IrStmt::Assign {
                        rhs: apply_lhs_assignment_context(&self.model, &lhs, source),
                        lhs,
                        nba: false,
                    });
                }
            }
        }
        Ok(())
    }

    /// `dst = f(...)` for a function returning a column-layout record: the
    /// call writes one trailing output per result column. A blocking store
    /// into descriptor columns binds them directly (outputs copy back after
    /// the callee returns); other destinations receive a temporary.
    fn lower_record_call_into(
        &mut self,
        path: &str,
        dst: &RecordValue,
        node: NodeId,
        nba: bool,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        let NodeKind::FuncCall { name, callee, .. } = self.kind(node) else {
            return Ok(None);
        };
        let (name, callee) = (name.clone(), *callee);
        let (function, _) = self.resolve_callee_env(self.inst, &name, false, callee)?;
        if !self.record_return(function) {
            return Ok(None);
        }
        let mut out = Vec::new();
        let native = self.record_formal_native(function);
        let direct = !nba
            && dst
                .columns
                .iter()
                .all(|(_, column)| !matches!(column, RecordColumn::Leaf(_)))
            && native.is_none_or(|native| {
                self.record_native_group(dst).is_some_and(|group| {
                    self.model.native_values[group].ty == self.model.native_values[native].ty
                })
            });
        let result = if direct {
            dst.clone()
        } else {
            let descriptor = self
                .activation_records
                .get(&function)
                .map(|value| value.descriptor.clone())
                .ok_or("record result has no columns")?;
            self.record_temporary(path, &descriptor, &mut out)?
        };
        self.record_call_result = true;
        let expression = self.lower_func_call_expr(path, node, &name, callee);
        self.record_call_result = false;
        let expression = Self::record_staged_call(expression?, &mut out)
            .ok_or("record call did not lower to a typed call")?;
        let mut args = expression.args;
        let outputs = self.model.funcs[expression.f]
            .formals
            .iter()
            .filter(|formal| formal.is_address())
            .count();
        let mut results = result
            .columns
            .iter()
            .filter_map(|(_, column)| match column {
                RecordColumn::Array(array) | RecordColumn::Cell(array) => {
                    Some(IrCallArg::FixedArray(self.reference_array(*array)))
                }
                RecordColumn::Leaf(_) | RecordColumn::Native(..) => None,
            })
            .collect::<Vec<_>>();
        if native.is_some() {
            let group = self
                .record_native_group(&result)
                .ok_or("record result has no native value")?;
            results.push(IrCallArg::NativeValue(group));
        }
        let first = outputs
            .checked_sub(results.len())
            .ok_or("record result has more columns than outputs")?;
        for (offset, argument) in results.into_iter().enumerate() {
            args.insert(first + offset, argument);
        }
        out.push(IrStmt::Call(Box::new(
            IrCall::new(expression.f, args, expression.depth, Vec::new(), Vec::new())
                .with_vpi_call(expression.vpi_call),
        )));
        if !direct {
            self.record_copy(path, dst, &result, nba, &mut out)?;
        }
        Ok(Some(out))
    }

    /// The function a record-returning call node invokes, if `node` is one.
    pub(in crate::sim::codegen::lowering) fn record_call_function(
        &self,
        node: NodeId,
    ) -> Result<Option<NodeId>, String> {
        let NodeKind::FuncCall { name, callee, .. } = self.kind(node) else {
            return Ok(None);
        };
        let (name, callee) = (name.clone(), *callee);
        let (function, _) = self.resolve_callee_env(self.inst, &name, false, callee)?;
        Ok(self.record_return(function).then_some(function))
    }

    /// The result of record call `node` in a fresh lexical temporary,
    /// declared and filled by the statements pushed to `out`.
    pub(in crate::sim::codegen::lowering) fn record_call_result_value(
        &mut self,
        path: &str,
        function: NodeId,
        node: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<RecordValue, String> {
        let descriptor = self
            .activation_records
            .get(&function)
            .map(|value| value.descriptor.clone())
            .ok_or("record result has no columns")?;
        let temporary = self.record_temporary(path, &descriptor, out)?;
        let call = self
            .lower_record_call_into(path, &temporary, node, false)?
            .ok_or("record call did not lower to a record result")?;
        out.extend(call);
        Ok(temporary)
    }

    /// `a == b` (or `!=`, `===`, `!==`) where an operand is a call returning
    /// a column-layout record. Each call runs once into its own lexical
    /// temporary, owned by the comparison's value scope, before the columns
    /// compare (SV 11.4.5).
    pub(in crate::sim::codegen::lowering) fn lower_record_call_comparison(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        case: bool,
        negate: bool,
    ) -> Result<Option<IrExpr>, String> {
        let mut statements = Vec::new();
        let operand = |this: &mut Self, node: NodeId, statements: &mut Vec<IrStmt>| {
            let node = this.p30_unwrap_cast(node);
            if let Some(value) = this.column_record_storage(node) {
                return Ok(value);
            }
            match this.record_call_function(node)? {
                Some(function) => this.record_call_result_value(path, function, node, statements),
                None => {
                    let descriptor = this.query_descriptor(node).cloned().ok_or_else(|| {
                        format!("record equality operand in `{path}` has no type")
                    })?;
                    this.record_value_of(path, &descriptor, node, statements)
                }
            }
        };
        let left = operand(self, lhs, &mut statements)?;
        let right = operand(self, rhs, &mut statements)?;
        let equality = self.record_equality(path, &left, &right, case)?;
        let value = if negate {
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
        };
        Ok(Some(Self::record_sequence(statements, value)))
    }

    /// A call operand with the statements staging its record arguments.
    pub(in crate::sim::codegen::lowering) fn record_call_operand(
        &mut self,
        path: &str,
        mut before: Vec<IrStmt>,
        call: IrExpr,
        after: Vec<IrStmt>,
    ) -> Result<IrExpr, String> {
        if after.is_empty() {
            return Ok(Self::record_sequence(before, call));
        }
        if call.is_real() || call.width == 0 {
            return Err(format!(
                "an output record argument in `{path}` must be record storage when the call's result is not packed"
            ));
        }
        let name = self.new_fn_name(path, "record_call");
        let (width, signed) = (call.width, call.signed);
        before.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            two_state: false,
            init: Some(Box::new(call)),
        });
        before.extend(after);
        Ok(Self::record_sequence(
            before,
            IrExpr::new(IrExprKind::LocalRead(name), width, signed, None),
        ))
    }

    /// The call of a lowered call operand, inside the statements staging its
    /// record arguments if it has any.
    pub(in crate::sim::codegen::lowering) fn record_staged_call_mut(
        expression: &mut IrExpr,
    ) -> Option<&mut crate::sim::ir::IrCallExpr> {
        match &mut expression.kind {
            IrExprKind::CallFn(call) => Some(call),
            IrExprKind::Sequence(sequence) => Self::record_staged_call_mut(&mut sequence.value),
            _ => None,
        }
    }

    /// One packed target for a whole record: its leaves, first member most
    /// significant, for a context that writes the record's image.
    pub(in crate::sim::codegen::lowering) fn record_image_lhs(
        &self,
        path: &str,
        value: &RecordValue,
    ) -> Result<IrLhs, String> {
        let mut parts = Vec::new();
        for (member, column) in &value.columns {
            match column {
                RecordColumn::Array(array) => {
                    let array = self.reference_array(*array);
                    let info = &self.model.arrays[array];
                    for coordinates in port_array_index_vectors(&info.dims) {
                        parts.push((
                            IrLhs::ArrayElem {
                                arr: array,
                                indices: coordinates
                                    .into_iter()
                                    .map(|index| lhs_integer_expr(i128::from(index)))
                                    .collect(),
                                elem_sel: IrElemSel::Whole,
                            },
                            info.elem_width,
                        ));
                    }
                }
                RecordColumn::Native(..) => {
                    return Err(format!(
                        "record member `{}` has no packed image in `{path}`",
                        aggregate_path_suffix(member)
                    ));
                }
                _ => {
                    let read = self.record_column_read(column)?;
                    if read.is_real() {
                        return Err(format!(
                            "real record member `{}` has no packed image in `{path}`",
                            aggregate_path_suffix(member)
                        ));
                    }
                    parts.push((self.record_column_lhs(column)?, read.width));
                }
            }
        }
        let width = parts.iter().map(|(_, width)| *width).sum();
        Ok(IrLhs::Stream {
            parts,
            width,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        })
    }

    /// The call of a lowered call operand; the statements staging its record
    /// arguments go to `out`.
    pub(in crate::sim::codegen::lowering) fn record_staged_call(
        expression: IrExpr,
        out: &mut Vec<IrStmt>,
    ) -> Option<Box<crate::sim::ir::IrCallExpr>> {
        match expression.kind {
            IrExprKind::CallFn(call) => Some(call),
            IrExprKind::Sequence(sequence) => {
                let sequence = *sequence;
                out.extend(sequence.statements);
                match sequence.value.kind {
                    IrExprKind::CallFn(call) => Some(call),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(in crate::sim::codegen::lowering) fn record_sequence(
        statements: Vec<IrStmt>,
        value: IrExpr,
    ) -> IrExpr {
        if statements.is_empty() {
            return value;
        }
        let (width, signed) = (value.width, value.signed);
        IrExpr::new(
            IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                statements,
                value,
            })),
            width,
            signed,
            None,
        )
    }

    /// A member, or an element of a member array, of a call returning a
    /// column-layout record (`f(x).m`, `f(x).s.m`, `f(x).a[i]`). The call
    /// runs once into a lexical temporary owned by the expression's value
    /// scope; the selected column is read from it. Members inside a
    /// column-layout tagged union keep their tag check.
    pub(in crate::sim::codegen::lowering) fn lower_record_call_select(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let (member_node, indices) = match self.kind(node) {
            NodeKind::Expr(ExprKind::MemberSelect { .. }) => (node, Vec::new()),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                if matches!(
                    self.kind(*base),
                    NodeKind::Expr(ExprKind::MemberSelect { .. })
                ) =>
            {
                (*base, indices.clone())
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index })
                if matches!(
                    self.kind(*base),
                    NodeKind::Expr(ExprKind::MemberSelect { .. })
                ) =>
            {
                (*base, vec![*index])
            }
            _ => return Ok(None),
        };
        // Walk the member chain down to the call.
        let mut members = Vec::new();
        let mut current = member_node;
        let call = loop {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::MemberSelect { base, member }) => {
                    members.push(AggregatePathPart::Member(member.clone()));
                    current = *base;
                }
                _ => break current,
            }
        };
        members.reverse();
        let Some(function) = self.record_call_function(call)? else {
            return Ok(None);
        };
        let mut statements = Vec::new();
        let value = self.record_call_result_value(path, function, call, &mut statements)?;
        let column = value
            .columns
            .iter()
            .find(|(column_path, _)| *column_path == members)
            .map(|(_, column)| column.clone())
            .ok_or_else(|| {
                format!(
                    "selection `{}` of a column-layout record call result in `{path}` must name a scalar member or a member array element",
                    aggregate_path_suffix(&members)
                )
            })?;
        let read = match (&column, indices.is_empty()) {
            (RecordColumn::Cell(_), true) => self.record_column_read(&column)?,
            (RecordColumn::Native(..), true) => {
                let leaf = Self::record_leaf_endpoint(&column)
                    .ok_or("record native member has no endpoint")?;
                match self.native_leaf_value(&leaf)? {
                    crate::sim::ir::IrNativeLeafExpr::Packed(value)
                    | crate::sim::ir::IrNativeLeafExpr::Real(value) => value,
                    _ => {
                        return Err(format!(
                            "string or chandle member of a record call result in `{path}` is not a packed or real value"
                        ))
                    }
                }
            }
            (RecordColumn::Array(array), false) => {
                let array = *array;
                if indices.len() != self.model.arrays[array].dims.len() {
                    return Err(format!(
                        "element select of a record call result member array in `{path}` must select one element"
                    ));
                }
                let indices = indices
                    .iter()
                    .map(|index| self.lower_expr(path, *index))
                    .collect::<Result<Vec<_>, _>>()?;
                IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: self.reference_array(array),
                        indices,
                        elem_sel: IrElemSel::Whole,
                    },
                    self.model.arrays[array].elem_width,
                    self.model.arrays[array].signed,
                    None,
                )
            }
            _ => {
                return Err(format!(
                    "selection `{}` of a column-layout record call result in `{path}` must name a scalar member or a member array element",
                    aggregate_path_suffix(&members)
                ))
            }
        };
        let read = match self.record_value_guard(&value, &members)? {
            Some(guard) => {
                let (tag, guard) = (guard.tag_read, guard.guard);
                let (tag_width, width, signed) = (tag.width, read.width, read.signed);
                if read.is_real() {
                    return Err(format!(
                        "real member of a tagged union record call result in `{path}` is not supported"
                    ));
                }
                IrExpr::new(
                    IrExprKind::TaggedSelect {
                        base: Box::new(IrExpr::new(
                            IrExprKind::Concat {
                                parts: vec![tag, read],
                            },
                            tag_width + width,
                            false,
                            None,
                        )),
                        steps: vec![crate::sim::ir::IrTaggedSelectStep {
                            selection: crate::sim::ir::IrPackedSelect {
                                base: lhs_integer_expr(0),
                                width,
                            },
                            two_state: false,
                            guard: Some(guard),
                        }],
                        location: self.source_location(node),
                    },
                    width,
                    signed,
                    None,
                )
            }
            None => read,
        };
        Ok(Some(Self::record_sequence(statements, read)))
    }

    /// `source matches .v` for a value beyond packed capacity (SV 12.6.1):
    /// the whole value is copied into `v`'s storage when the pattern is
    /// tested, and the binding itself always matches.
    pub(in crate::sim::codegen::lowering) fn lower_record_binding(
        &mut self,
        path: &str,
        target: NodeId,
        value: &RecordValue,
    ) -> Result<IrExpr, String> {
        let storage = match self.record_declaration_value(target) {
            Some(storage) => storage,
            None if self.automatic_block_record(path, target)? => self
                .activation_records
                .get(&target)
                .cloned()
                .ok_or("pattern binding has no columns")?,
            None => {
                return Err(format!(
                    "pattern binding `{}` beyond packed capacity has no storage in `{path}`",
                    self.node(target).name
                ))
            }
        };
        self.declare_binding_columns(path, &storage)?;
        let mut statements = Vec::new();
        self.record_copy(path, &storage, value, false, &mut statements)?;
        Ok(Self::record_sequence(statements, binding_match()))
    }

    /// `tagged m .v` binding member array column `column` of a value beyond
    /// packed capacity: the column is copied into `v`'s array storage.
    pub(in crate::sim::codegen::lowering) fn lower_array_binding(
        &mut self,
        path: &str,
        target: NodeId,
        column: usize,
    ) -> Result<IrExpr, String> {
        let array = match self.array_globals.get(&target).map(|array| array.ir) {
            Some(array) => array,
            None => {
                let info = self.fixed_activation_array(target).map_err(|error| {
                    format!(
                        "pattern binding `{}` in `{path}`: {error}",
                        self.node(target).name
                    )
                })?;
                let array = info.ir;
                if self.db.variable_lifetime(target) == VariableLifetime::Automatic {
                    self.array_globals.insert(target, info);
                } else {
                    let mut info = info;
                    self.make_fixed_array_persistent(&mut info);
                    self.model.arrays[array].descriptor = true;
                    self.array_globals.insert(target, info);
                }
                array
            }
        };
        if self.model.arrays[array].total != self.model.arrays[column].total {
            return Err(format!(
                "pattern binding `{}` in `{path}` does not have its member's shape",
                self.node(target).name
            ));
        }
        if self.model.arrays[array].activation {
            self.pend_binding_declaration(path, IrStmt::FixedArrayDeclare(array))?;
        } else if !self.model.arrays[array].sparse() {
            return Err(format!(
                "pattern binding `{}` in `{path}` needs descriptor array storage",
                self.node(target).name
            ));
        }
        let statements = vec![IrStmt::FixedValueAssign {
            dst: self.record_column_view(array),
            src: Box::new(IrFixedValue::Array(self.record_column_view(column))),
            nba: false,
        }];
        Ok(Self::record_sequence(statements, binding_match()))
    }

    /// Declare the lexical columns of a binding target before the statement
    /// that tests the pattern; persistent storage needs no declaration.
    fn declare_binding_columns(&mut self, path: &str, value: &RecordValue) -> Result<(), String> {
        let lexical = value.columns.iter().any(|(_, column)| match column {
            RecordColumn::Array(array) | RecordColumn::Cell(array) => {
                self.model.arrays[*array].activation
            }
            RecordColumn::Native(native, _) => self.model.native_values[*native].activation,
            RecordColumn::Leaf(_) => false,
        });
        if lexical {
            for declaration in self.declare_record_columns(value) {
                self.pend_binding_declaration(path, declaration)?;
            }
        }
        Ok(())
    }

    fn pend_binding_declaration(&mut self, path: &str, declaration: IrStmt) -> Result<(), String> {
        let pending = self.record_binding_declarations.as_mut().ok_or_else(|| {
            format!(
                "a whole-value pattern binding beyond packed capacity in `{path}` must be part of a procedural statement"
            )
        })?;
        if !pending.contains(&declaration) {
            pending.push(declaration);
        }
        Ok(())
    }

    /// One descriptor operand per column of a record actual, then one
    /// native operand for its real, string and chandle members. Those
    /// members pass as the actual's own native value when it is a whole
    /// subroutine record of the same type; otherwise an input is built leaf
    /// by leaf at the call, and an output or inout goes through a caller
    /// temporary that `prelude` declares (and fills for an inout) before the
    /// call and copies back after it.
    pub(in crate::sim::codegen::lowering) fn record_call_columns(
        &mut self,
        path: &str,
        formal: NodeId,
        actual: NodeId,
        prelude: Option<(&mut Vec<IrStmt>, &mut Vec<IrStmt>)>,
    ) -> Result<Vec<IrCallArg>, String> {
        let shape = self
            .activation_records
            .get(&formal)
            .cloned()
            .ok_or("column-layout record formal has no columns")?;
        if self
            .record_ref_binding(formal, actual)
            .is_some_and(|binding| binding.cell)
        {
            return Ok(vec![self.record_ref_cell_argument(path, formal, actual)?]);
        }
        let actual_node = self.p30_unwrap_cast(actual);
        let mut prelude = prelude;
        let value = match (self.column_record_storage(actual_node), prelude.as_mut()) {
            (Some(value), _) => value,
            // A value that is no record storage crosses in a temporary: an
            // element of an array of records, a pattern, a parameter.
            (None, Some((before, after))) => {
                self.record_call_staged(path, formal, &shape, actual_node, before, after)?
            }
            (None, None) => {
                return Err(format!(
                    "column-layout record argument for `{}` in `{path}` must be a record variable, member or formal",
                    self.node(formal).name
                ))
            }
        };
        self.record_shapes_match(path, &shape, &value)?;
        let mut arguments = value
            .columns
            .iter()
            .filter_map(|(_, column)| match column {
                RecordColumn::Array(array) | RecordColumn::Cell(array) => {
                    Some(IrCallArg::FixedValue(Box::new(IrFixedValue::Array(
                        self.record_column_view(*array),
                    ))))
                }
                RecordColumn::Leaf(_) | RecordColumn::Native(..) => None,
            })
            .collect::<Vec<_>>();
        let Some(native) = self.record_formal_native(formal) else {
            return Ok(arguments);
        };
        if self.record_ref_binding(formal, actual).is_some() {
            arguments.push(IrCallArg::NativeRefBound);
            return Ok(arguments);
        }
        let ty = self.model.native_values[native].ty;
        if let Some(group) = self
            .record_native_group(&value)
            .filter(|group| self.model.native_values[*group].ty == ty)
        {
            arguments.push(IrCallArg::NativeValue(group));
            return Ok(arguments);
        }
        if self.is_ref_formal(formal) {
            return Err(format!(
                "ref argument for `{}` in `{path}` must be a record variable or formal",
                self.node(formal).name
            ));
        }
        let pairs = shape
            .columns
            .iter()
            .zip(&value.columns)
            .filter_map(|((_, formal), (_, actual))| match formal {
                RecordColumn::Native(_, leaf) => Some(
                    Self::record_leaf_endpoint(actual)
                        .map(|actual| ((**leaf).clone(), actual))
                        .ok_or("record native member pairs with a packed member"),
                ),
                _ => None,
            })
            .collect::<Result<Vec<_>, _>>()?;
        let output = matches!(
            self.kind(formal),
            NodeKind::FuncArg {
                direction: DbDirection::Output | DbDirection::Inout,
                ..
            }
        );
        if !output {
            let mut leaves = Vec::with_capacity(pairs.len());
            for (leaf, actual) in &pairs {
                leaves.push(crate::sim::ir::IrNativeLeafValue {
                    items: leaf.items.clone(),
                    value: self.native_leaf_value(actual)?,
                });
            }
            arguments.push(IrCallArg::NativeLeaves {
                ty,
                leaves,
                containers: Vec::new(),
            });
            return Ok(arguments);
        }
        let Some((before, after)) = prelude else {
            return Err(format!(
                "output record argument for `{}` in `{path}` with real, string or chandle members must be a subroutine record of the same type unless the call is a statement",
                self.node(formal).name
            ));
        };
        let temporary = self.model.native_values.len();
        self.model
            .native_values
            .push(crate::sim::ir::IrNativeValue {
                c_name: format!("S_llg_native_{temporary}"),
                ty,
                activation: true,
                companions: Vec::new(),
                class_field: None,
                receiver: None,
            });
        before.push(IrStmt::NativeValueDeclare(temporary));
        let inout = matches!(
            self.kind(formal),
            NodeKind::FuncArg {
                direction: DbDirection::Inout,
                ..
            }
        );
        for (leaf, actual) in pairs {
            let staged = NativeEndpointLeaf::Value(temporary, leaf);
            if inout {
                before.push(self.native_leaf_copy(path, &staged, &actual, false)?);
            }
            after.push(self.native_leaf_copy(path, &actual, &staged, false)?);
        }
        arguments.push(IrCallArg::NativeValue(temporary));
        Ok(arguments)
    }

    /// A temporary for a record argument that is no record storage: an
    /// input takes the value before the call, an output gives it back after.
    fn record_call_staged(
        &mut self,
        path: &str,
        formal: NodeId,
        shape: &RecordValue,
        actual: NodeId,
        before: &mut Vec<IrStmt>,
        after: &mut Vec<IrStmt>,
    ) -> Result<RecordValue, String> {
        let direction = match self.kind(formal) {
            NodeKind::FuncArg { direction, .. } => *direction,
            _ => DbDirection::Input,
        };
        if matches!(direction, DbDirection::Ref) {
            return Err(format!(
                "ref argument for `{}` in `{path}` must be a record variable or member",
                self.node(formal).name
            ));
        }
        let temporary = self.record_temporary(path, &shape.descriptor, before)?;
        if !matches!(direction, DbDirection::Output) {
            self.lower_record_value_into(path, &temporary, actual, false, before)?;
        }
        if !matches!(direction, DbDirection::Input) {
            let image = self.record_image(path, &temporary)?;
            let target = self.analyze_lhs(path, actual)?;
            let target = self.lhs_to_ir(target)?;
            after.push(IrStmt::Assign {
                rhs: apply_lhs_assignment_context(&self.model, &target, image),
                lhs: target,
                nba: false,
            });
        }
        Ok(temporary)
    }

    /// Declare an automatic column-layout record local and run its
    /// initializer; static locals keep persistent columns.
    pub(in crate::sim::codegen::lowering) fn lower_record_local(
        &mut self,
        path: &str,
        declaration: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let value = self
            .activation_records
            .get(&declaration)
            .cloned()
            .ok_or("column-layout record local has no columns")?;
        let initializer = self.db.var_initializer(declaration);
        let automatic = value.columns.iter().any(|(_, column)| {
            matches!(column, RecordColumn::Array(array) | RecordColumn::Cell(array)
                if self.model.arrays[*array].activation)
        });
        if !automatic {
            // Static storage initializes once, before any process, in the
            // static schedule; a body lowered again does not repeat it.
            if let Some(initializer) = initializer {
                if !self
                    .record_initializers
                    .iter()
                    .any(|(registered, _)| *registered == declaration)
                {
                    self.reserve_initializer_order(declaration);
                    self.record_initializers.push((declaration, initializer));
                }
            }
            return Ok(Vec::new());
        }
        let mut statements = self.declare_record_columns(&value);
        if let Some(initializer) = initializer {
            self.lower_record_value_into(path, &value, initializer, false, &mut statements)?;
        }
        Ok(statements)
    }
}

fn uniform_constant(width: u32, signed: bool, two_state: bool) -> IrConst {
    let words = width.div_ceil(64) as usize;
    let x = if two_state {
        Vec::new()
    } else {
        let mut x = vec![u64::MAX; words];
        if !width.is_multiple_of(64) {
            x[words - 1] = (1u64 << (width % 64)) - 1;
        }
        x
    };
    IrConst::packed(vec![0; words], x, Vec::new(), width, signed, None)
        .expect("checked record member width")
}

/// The truth of a binding, which matches any value.
fn binding_match() -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(
            IrConst::packed(vec![1], vec![0], vec![0], 1, false, None)
                .expect("one-bit binding truth"),
        ),
        1,
        false,
        None,
    )
}
