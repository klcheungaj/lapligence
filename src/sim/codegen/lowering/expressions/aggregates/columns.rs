//! Whole-value transport for column-layout records (RTL-101).
//!
//! A column-layout record is never one packed payload. Its values move
//! column by column: scalar leaves through ordinary packed assignments and
//! member arrays through descriptor views. Sources that are not storage
//! (patterns, conditionals, calls) are first built into a lexical temporary
//! record, so every source is evaluated once before any destination write.
use super::*;
use crate::sim::ir::{IrFixedValue, IrMemoryView};

/// One stored column of a record value, in declaration order.
#[derive(Clone)]
pub(in crate::sim::codegen::lowering) enum RecordColumn {
    /// Module storage leaf: packed, real, string or chandle.
    Leaf(AggregateMemberInfo),
    /// Fixed-array column, persistent or lexical.
    Array(usize),
    /// Lexical packed temporary.
    Local {
        name: String,
        width: u32,
        signed: bool,
        two_state: bool,
    },
}

/// A record value as its columns, with paths relative to `descriptor`.
#[derive(Clone)]
pub(in crate::sim::codegen::lowering) struct RecordValue {
    pub(in crate::sim::codegen::lowering) descriptor: TypeDescriptor,
    pub(in crate::sim::codegen::lowering) columns: Vec<(Vec<AggregatePathPart>, RecordColumn)>,
}

/// The column shape of one record leaf, used to allocate temporaries.
enum ColumnShape {
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Array(TypeDescriptor),
}

/// Column shapes of a record type in the declaration order used by module
/// storage (`collect_aggregate_descriptor_leaves` with columns).
fn column_shapes(
    descriptor: &TypeDescriptor,
    prefix: &[AggregatePathPart],
    two_state: bool,
    out: &mut Vec<(Vec<AggregatePathPart>, ColumnShape)>,
) -> Result<(), String> {
    match &descriptor.shape {
        TypeShape::FixedArray { .. } => {
            out.push((prefix.to_vec(), ColumnShape::Array(descriptor.clone())));
            Ok(())
        }
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => {
            for member in &layout.members {
                let mut path = prefix.to_vec();
                path.push(AggregatePathPart::Member(member.name.clone()));
                column_shapes(&member.descriptor, &path, member.two_state, out)?;
            }
            Ok(())
        }
        TypeShape::PackedAtom { .. } | TypeShape::Aggregate(_) => {
            let width = Codegen::fixed_descriptor_width(descriptor).ok_or_else(|| {
                format!(
                    "record member `{}` has no packed width",
                    aggregate_path_suffix(prefix)
                )
            })?;
            out.push((
                prefix.to_vec(),
                ColumnShape::Packed {
                    width,
                    signed: descriptor.info.signed,
                    two_state,
                },
            ));
            Ok(())
        }
        _ => Err(format!(
            "record member `{}` is not integral; a temporary column-layout record value supports integral members only",
            aggregate_path_suffix(prefix)
        )),
    }
}

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
        let selection = self.resolve_unpacked_aggregate(node)?;
        if !selection.storage.columns
            || !matches!(&selection.descriptor.shape,
                TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct)
        {
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
                    Some(array) => RecordColumn::Array(array.ir),
                    None => RecordColumn::Leaf(leaf.clone()),
                };
                (path, column)
            })
            .collect::<Vec<_>>();
        (!columns.is_empty()).then_some(RecordValue {
            descriptor: selection.descriptor,
            columns,
        })
    }

    /// Whether an assignment or comparison operand involves a column-layout
    /// record type.
    pub(in crate::sim::codegen::lowering) fn column_record_type(&self, node: NodeId) -> bool {
        self.record_columns
            && self
                .query_descriptor(node)
                .is_some_and(Self::column_layout_descriptor)
    }

    /// Allocate a lexical temporary with the column layout of `descriptor`.
    fn record_temporary(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        out: &mut Vec<IrStmt>,
    ) -> Result<RecordValue, String> {
        let mut shapes = Vec::new();
        column_shapes(descriptor, &[], descriptor.two_state, &mut shapes)?;
        let mut columns = Vec::with_capacity(shapes.len());
        for (member_path, shape) in shapes {
            let column = match shape {
                ColumnShape::Packed {
                    width,
                    signed,
                    two_state,
                } => {
                    let name = self.new_fn_name(path, "record_leaf");
                    out.push(IrStmt::DeclLocal {
                        name: name.clone(),
                        width,
                        signed,
                        init: None,
                        two_state,
                    });
                    RecordColumn::Local {
                        name,
                        width,
                        signed,
                        two_state,
                    }
                }
                ColumnShape::Array(array) => {
                    let ir = self.record_temporary_array(path, &array)?;
                    out.push(IrStmt::FixedArrayDeclare(ir));
                    RecordColumn::Array(ir)
                }
            };
            columns.push((member_path, column));
        }
        Ok(RecordValue {
            descriptor: descriptor.clone(),
            columns,
        })
    }

    fn record_column_read(&self, column: &RecordColumn) -> Result<IrExpr, String> {
        match column {
            RecordColumn::Leaf(leaf) => {
                let value = self.aggregate_leaf_read(leaf)?;
                Ok(if leaf.member.two_state && !value.is_real() {
                    IrExpr::to_two_state(value)
                } else {
                    value
                })
            }
            RecordColumn::Local {
                name,
                width,
                signed,
                ..
            } => Ok(IrExpr::new(
                IrExprKind::LocalRead(name.clone()),
                *width,
                *signed,
                None,
            )),
            RecordColumn::Array(_) => Err("record array column is not a packed value".into()),
        }
    }

    fn record_column_lhs(&self, column: &RecordColumn) -> Result<IrLhs, String> {
        match column {
            RecordColumn::Leaf(leaf) => self.aggregate_leaf_lhs(leaf),
            RecordColumn::Local {
                name,
                width,
                signed,
                two_state,
            } => Ok(IrLhs::WholeRef {
                addr: format!("&{name}"),
                width: *width,
                signed: *signed,
                two_state: *two_state,
                shortreal: false,
            }),
            RecordColumn::Array(_) => Err("record array column is not a packed target".into()),
        }
    }

    fn record_column_view(&self, array: usize) -> IrMemoryView {
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
    fn record_copy(
        &mut self,
        path: &str,
        dst: &RecordValue,
        src: &RecordValue,
        nba: bool,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        self.record_shapes_match(path, dst, src)?;
        for ((_, target), (_, source)) in dst.columns.iter().zip(&src.columns) {
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
            (RecordColumn::Array(target), RecordColumn::Array(source)) => {
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
        &self,
        path: &str,
        left: &RecordValue,
        right: &RecordValue,
        case: bool,
    ) -> Result<IrExpr, String> {
        self.record_shapes_match(path, left, right)?;
        let mut equality: Option<IrExpr> = None;
        for ((_, left), (_, right)) in left.columns.iter().zip(&right.columns) {
            let column = match (left, right) {
                (RecordColumn::Array(left), RecordColumn::Array(right)) => IrExpr::new(
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
                (RecordColumn::Leaf(leaf), _) | (_, RecordColumn::Leaf(leaf))
                    if leaf.object.is_some() =>
                {
                    return Err(format!(
                        "equality of column-layout records with string or chandle members is not supported in `{path}`"
                    ));
                }
                _ => {
                    let (left, right) = (
                        self.record_column_read(left)?,
                        self.record_column_read(right)?,
                    );
                    if case {
                        if left.is_real() || right.is_real() {
                            return Err(format!(
                                "case equality on real aggregate member in `{path}` is not supported"
                            ));
                        }
                        cmp_expr_ir(IrBinOp::CaseEq, left, right)
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
                return Err(format!(
                    "a column-layout record value in `{path}` can only be assigned to record storage of the same type"
                ));
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
        Ok(Some(IrStmt::Block(out)))
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
        if self.assignment_pattern_operands(path, node)?.is_some() {
            let temporary = self.record_temporary(path, &dst.descriptor, out)?;
            self.lower_record_pattern_into(path, &temporary, node, out)?;
            return self.record_copy(path, dst, &temporary, nba, out);
        }
        if let Some(statements) = self.lower_record_call_into(path, dst, node, nba)? {
            out.extend(statements);
            return Ok(());
        }
        Err(format!(
            "column-layout record value in `{path}` must be record storage, a pattern, a conditional or a function call"
        ))
    }

    /// The value of `node` as storage, or as a temporary built from it.
    fn record_value_of(
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
    /// immediate member that is equal (no X/Z) in both arms, giving the
    /// member's default-uninitialized value otherwise — the rule packed
    /// structure merges apply to records within packed capacity.
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
            let equal = self.record_equality(path, &from_left, &from_right, false)?;
            let mut keep = Vec::new();
            self.record_copy(path, &target, &from_left, nba, &mut keep)?;
            let mut reset = Vec::new();
            self.record_fill_uninitialized(path, member, &target, nba, &mut reset)?;
            out.push(IrStmt::If {
                cond: equal,
                then_: keep,
                els: Some(reset),
                check: IrUniquePriorityCheck::None,
            });
        }
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
                RecordColumn::Array(array) => {
                    let TypeShape::FixedArray { element, .. } = &leaf.shape else {
                        return Err(format!("record merge column is not an array in `{path}`"));
                    };
                    let width = Self::fixed_descriptor_width(element)
                        .ok_or("record merge element has no width")?;
                    let value = Self::fixed_descriptor_uninitialized(element)
                        .unwrap_or_else(|| uniform_constant(width, two_state || element.two_state));
                    out.push(IrStmt::FixedArrayFill {
                        array: self.reference_array(*array),
                        value: IrExpr::new(IrExprKind::Const(value), width, false, None),
                        nba,
                    });
                }
                _ => {
                    let lhs = self.record_column_lhs(column)?;
                    let width = packed_lhs_width(&self.model, &lhs)
                        .ok_or("record merge leaf has no packed width")?;
                    let value = Self::fixed_descriptor_uninitialized(&leaf)
                        .unwrap_or_else(|| uniform_constant(width, two_state));
                    out.push(IrStmt::Assign {
                        rhs: IrExpr::new(IrExprKind::Const(value), width, false, None),
                        lhs,
                        nba,
                    });
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
        if !matches!(element.shape, TypeShape::PackedAtom { .. }) {
            return Err(format!(
                "record pattern fill of a member array with structured elements is not supported in `{path}`"
            ));
        }
        let width =
            Self::fixed_descriptor_width(element).ok_or("record fill element has no width")?;
        let value = IrExpr::convert_to(self.lower_expr(path, value)?, width, element.info.signed);
        out.push(IrStmt::FixedArrayFill {
            array,
            value,
            nba: false,
        });
        Ok(())
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
    fn lower_record_call_into(
        &mut self,
        _path: &str,
        _dst: &RecordValue,
        _node: NodeId,
        _nba: bool,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        Ok(None)
    }
}

fn uniform_constant(width: u32, two_state: bool) -> IrConst {
    let words = width.div_ceil(64) as usize;
    let x = if two_state {
        Vec::new()
    } else {
        let mut x = vec![u64::MAX; words];
        if width % 64 != 0 {
            x[words - 1] = (1u64 << (width % 64)) - 1;
        }
        x
    };
    IrConst::packed(vec![0; words], x, Vec::new(), width, false, None)
        .expect("checked record member width")
}
