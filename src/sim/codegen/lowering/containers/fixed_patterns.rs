//! Descriptor-backed pattern construction captures values before publication.

use super::*;

const DESCRIPTOR_PATTERN_DEFAULT_MIN_OCCURRENCES: usize = 2;

#[derive(Clone)]
enum FixedPatternPlan {
    Value(NodeId),
    /// An array-valued item copied through descriptor view transport.
    Array(NodeId),
    Rows {
        bounds: (i32, i32),
        default: Option<Box<Self>>,
        entries: Vec<(u64, Self)>,
        repeat: u64,
        stride: u64,
    },
}

impl Codegen<'_> {
    pub(super) fn lower_descriptor_pattern(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        array: &ArrayInfo,
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        let node = self.p30_unwrap_cast(rhs);
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern | Operation::MultiAssignmentPattern,
                ..
            })
        ) {
            return Ok(None);
        }
        let descriptor = self
            .query_descriptor(lhs)
            .cloned()
            .ok_or("descriptor pattern has no target type")?;
        let plan = self.descriptor_pattern_plan(path, node, &descriptor)?;
        let mut snapshot = self.model.arrays[array.ir].clone();
        snapshot.activation = true;
        snapshot.hdl_name.clear();
        snapshot.c_name = self.new_fn_name(path, "pattern_snapshot");
        let temporary = self.model.arrays.len();
        self.model.arrays.push(snapshot);
        let mut captures = vec![IrStmt::FixedArrayDeclare(temporary)];
        let mut values = HashMap::new();
        self.capture_descriptor_pattern_values(path, &plan, array, &mut values, &mut captures)?;
        captures.extend(self.emit_descriptor_pattern_plan(path, temporary, &plan, &values, &[])?);
        captures.push(IrStmt::FixedArrayCopy {
            dst: array.ir,
            src: temporary,
            nba,
            slice: 0,
        });
        Ok(Some(IrStmt::Block(captures)))
    }

    /// Construct an assignment-pattern source directly into declared
    /// activation storage `array` whose shape is `descriptor`. Item values are
    /// captured before any cell is placed. Returns `None` for non-patterns.
    pub(in super::super) fn lower_descriptor_pattern_into(
        &mut self,
        path: &str,
        rhs: NodeId,
        descriptor: &TypeDescriptor,
        array: &ArrayInfo,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        let node = self.p30_unwrap_cast(rhs);
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern | Operation::MultiAssignmentPattern,
                ..
            })
        ) {
            return Ok(None);
        }
        let plan = self.descriptor_pattern_plan(path, node, descriptor)?;
        let mut statements = Vec::new();
        let mut values = HashMap::new();
        self.capture_descriptor_pattern_values(path, &plan, array, &mut values, &mut statements)?;
        statements.extend(self.emit_descriptor_pattern_plan(
            path,
            array.ir,
            &plan,
            &values,
            &[],
        )?);
        Ok(Some(statements))
    }

    fn descriptor_pattern_plan(
        &self,
        path: &str,
        node: NodeId,
        descriptor: &TypeDescriptor,
    ) -> Result<FixedPatternPlan, String> {
        let node = self.p30_unwrap_cast(node);
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return Ok(FixedPatternPlan::Value(node));
        };
        let NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            reordered,
            ..
        }) = self.kind(node)
        else {
            return Ok(FixedPatternPlan::Array(node));
        };
        if !matches!(
            op,
            Operation::AssignmentPattern | Operation::MultiAssignmentPattern
        ) {
            return Ok(FixedPatternPlan::Array(node));
        }
        let bounds = dimensions
            .first()
            .copied()
            .ok_or("descriptor pattern has no array bounds")?;
        let extent = i64::from(bounds.0).abs_diff(i64::from(bounds.1)) + 1;
        let next = if dimensions.len() == 1 {
            element.as_ref().clone()
        } else {
            TypeDescriptor {
                shape: TypeShape::FixedArray {
                    dimensions: dimensions[1..].to_vec(),
                    element: element.clone(),
                },
                ..descriptor.clone()
            }
        };
        let mut operands = operands.clone();
        if *reordered {
            operands.reverse();
        }
        let mut repeat = 1;
        if *op == Operation::MultiAssignmentPattern {
            let count = operands
                .first()
                .ok_or("descriptor pattern has no replication count")?;
            repeat = self
                .eval_bound_i128(*count)
                .ok()
                .and_then(|value| u64::try_from(value).ok())
                .filter(|value| *value != 0)
                .ok_or("descriptor pattern replication count must be positive")?;
            operands.remove(0);
        }
        let tagged = operands.iter().any(|operand| {
            matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        });
        let mut entries = Vec::new();
        let mut fallback = None;
        let stride;
        if !tagged {
            stride = operands.len() as u64;
            if stride.checked_mul(repeat) != Some(extent) {
                return Err(format!("descriptor pattern shape mismatch in `{path}`"));
            }
            for (offset, operand) in operands.into_iter().enumerate() {
                entries.push((
                    offset as u64,
                    self.descriptor_pattern_plan(path, operand, &next)?,
                ));
            }
        } else {
            if repeat != 1 {
                return Err("replicated keyed descriptor pattern is invalid".into());
            }
            stride = extent;
            let mut default = None;
            let mut type_value = None;
            let mut indices = HashSet::new();
            for operand in operands {
                let NodeKind::Expr(ExprKind::TaggedPattern {
                    key,
                    key_type,
                    value,
                    ..
                }) = self.kind(operand)
                else {
                    return Err("mixed keyed/positional descriptor pattern is invalid".into());
                };
                let value = value.ok_or("descriptor pattern key has no value")?;
                if key.as_deref() == Some("default") {
                    if default.replace(value).is_some() {
                        return Err("duplicate default pattern key".into());
                    }
                } else if let Some(index) = self.assignment_pattern_index_key(path, operand)? {
                    let offset = if bounds.0 >= bounds.1 {
                        i128::from(bounds.0).checked_sub(index)
                    } else {
                        index.checked_sub(i128::from(bounds.0))
                    }
                    .ok_or("descriptor pattern index arithmetic overflow")?;
                    let offset = u64::try_from(offset)
                        .ok()
                        .filter(|offset| *offset < extent)
                        .ok_or_else(|| {
                            format!("descriptor pattern index outside bounds in `{path}`")
                        })?;
                    if !indices.insert(offset) {
                        return Err("duplicate semantic descriptor pattern index".into());
                    }
                    entries.push((offset, self.descriptor_pattern_plan(path, value, &next)?));
                } else if let Some(key_type) = key_type {
                    if super::super::collection::pattern_key_matches_descriptor(
                        key_type,
                        &next,
                        next.two_state,
                        None,
                    ) {
                        type_value = Some(value);
                    }
                } else {
                    return Err("descriptor pattern key has no type or index".into());
                }
            }
            if entries.len() as u64 != extent {
                let value = type_value.or(default).ok_or_else(|| {
                    format!("descriptor pattern has uncovered elements in `{path}`")
                })?;
                fallback = Some(Box::new(self.descriptor_pattern_plan(path, value, &next)?));
            }
        }
        if fallback.is_none() && repeat == 1 && !entries.is_empty() {
            let mut frequencies = HashMap::<NodeId, usize>::new();
            for (_, plan) in &entries {
                if let FixedPatternPlan::Value(value) = plan {
                    *frequencies.entry(*value).or_default() += 1;
                }
            }
            if let Some((value, count)) = frequencies
                .into_iter()
                .max_by_key(|(node, count)| (*count, std::cmp::Reverse(node.index())))
            {
                if count >= DESCRIPTOR_PATTERN_DEFAULT_MIN_OCCURRENCES {
                    fallback = Some(Box::new(FixedPatternPlan::Value(value)));
                    entries.retain(|(_, plan)| !matches!(plan, FixedPatternPlan::Value(other) if *other == value));
                }
            }
        }
        // Uniform scalar defaults and replications retain the sparse default;
        // their generated source and storage do not scale with the extent.
        let uniform = fallback
            .as_deref()
            .and_then(|plan| match plan {
                FixedPatternPlan::Value(value) => Some(*value),
                _ => None,
            })
            .or_else(|| {
                entries.first().and_then(|(_, plan)| match plan {
                    FixedPatternPlan::Value(value) => Some(*value),
                    _ => None,
                })
            });
        if let Some(value) = uniform {
            if entries
                .iter()
                .all(|(_, plan)| matches!(plan, FixedPatternPlan::Value(other) if *other == value))
            {
                return Ok(FixedPatternPlan::Value(value));
            }
        }
        Ok(FixedPatternPlan::Rows {
            bounds,
            default: fallback,
            entries,
            repeat,
            stride,
        })
    }

    fn capture_descriptor_pattern_values(
        &mut self,
        path: &str,
        plan: &FixedPatternPlan,
        array: &ArrayInfo,
        values: &mut HashMap<NodeId, PatternCapture>,
        captures: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        match plan {
            FixedPatternPlan::Array(node) => {
                if let std::collections::hash_map::Entry::Vacant(entry) = values.entry(*node) {
                    // Each item is evaluated once into a lexical snapshot before
                    // any placement, so calls and overlapping sources keep their
                    // pre-assignment values even when a default repeats the item.
                    let snapshot = self.fixed_activation_array(*node)?;
                    let source = self.lower_fixed_value(path, *node)?;
                    captures.push(IrStmt::FixedArrayDeclare(snapshot.ir));
                    captures.push(IrStmt::FixedValueAssign {
                        dst: self.fixed_view_at(snapshot.ir, &[]),
                        src: Box::new(source),
                        nba: false,
                    });
                    entry.insert(PatternCapture::Array(snapshot.ir));
                }
            }
            FixedPatternPlan::Value(node) => {
                if let std::collections::hash_map::Entry::Vacant(entry) = values.entry(*node) {
                    let value = self.lower_expr(path, *node)?;
                    let value = ir_to_storage(
                        value,
                        array.elem_width,
                        array.signed,
                        self.model.arrays[array.ir].two_state,
                    )?;
                    let name = self.new_fn_name(path, "pattern_value");
                    let (width, signed) = (value.width, value.signed);
                    captures.push(IrStmt::DeclLocal {
                        name: name.clone(),
                        width,
                        signed,
                        two_state: false,
                        init: Some(Box::new(value)),
                    });
                    entry.insert(PatternCapture::Value(IrExpr::new(
                        IrExprKind::LocalRead(name),
                        width,
                        signed,
                        None,
                    )));
                }
            }
            FixedPatternPlan::Rows {
                default, entries, ..
            } => {
                if let Some(default) = default {
                    self.capture_descriptor_pattern_values(path, default, array, values, captures)?;
                }
                for (_, plan) in entries {
                    self.capture_descriptor_pattern_values(path, plan, array, values, captures)?;
                }
            }
        }
        Ok(())
    }

    fn emit_descriptor_pattern_plan(
        &mut self,
        path: &str,
        array: usize,
        plan: &FixedPatternPlan,
        values: &HashMap<NodeId, PatternCapture>,
        coordinates: &[IrExpr],
    ) -> Result<Vec<IrStmt>, String> {
        match plan {
            FixedPatternPlan::Array(node) => {
                let PatternCapture::Array(source) = values[node] else {
                    return Err("array pattern item was captured as a packed value".into());
                };
                let target = &self.model.arrays[array];
                let remaining = target.dims[coordinates.len()..]
                    .iter()
                    .try_fold(1u64, |total, (left, right)| {
                        total.checked_mul(u64::from(left.abs_diff(*right)) + 1)
                    })
                    .ok_or("descriptor pattern extent overflows")?;
                let item = self.model.arrays[source].total;
                if remaining == item && coordinates.len() < target.dims.len() {
                    return Ok(vec![IrStmt::FixedValueAssign {
                        dst: self.fixed_view_at(array, coordinates),
                        src: Box::new(crate::sim::ir::IrFixedValue::Array(
                            self.fixed_view_at(source, &[]),
                        )),
                        nba: false,
                    }]);
                }
                if remaining < item || coordinates.len() + 1 >= target.dims.len() {
                    return Err(format!(
                        "array-valued descriptor pattern item shape mismatch in `{path}`"
                    ));
                }
                let bounds = target.dims[coordinates.len()];
                let name = self.new_fn_name(path, "pattern_index");
                let offset = IrExpr::new(IrExprKind::LocalRead(name.clone()), 64, true, None);
                let mut next = coordinates.to_vec();
                next.push(pattern_coordinate(bounds, offset));
                let body = self.emit_descriptor_pattern_plan(path, array, plan, values, &next)?;
                Ok(pattern_loop(
                    name,
                    i64::from(bounds.0).abs_diff(i64::from(bounds.1)) + 1,
                    body,
                ))
            }
            FixedPatternPlan::Value(node) if coordinates.is_empty() => {
                Ok(vec![IrStmt::FixedArrayFill {
                    array,
                    value: values[node].packed()?,
                    nba: false,
                }])
            }
            FixedPatternPlan::Value(node) => {
                if coordinates.len() == self.model.arrays[array].dims.len() {
                    return Ok(vec![IrStmt::Assign {
                        lhs: IrLhs::ArrayElem {
                            arr: array,
                            indices: coordinates.to_vec(),
                            elem_sel: IrElemSel::Whole,
                        },
                        rhs: values[node].packed()?,
                        nba: false,
                    }]);
                }
                let bounds = self.model.arrays[array].dims[coordinates.len()];
                let name = self.new_fn_name(path, "pattern_index");
                let offset = IrExpr::new(IrExprKind::LocalRead(name.clone()), 64, true, None);
                let mut next = coordinates.to_vec();
                next.push(pattern_coordinate(bounds, offset.clone()));
                let body = self.emit_descriptor_pattern_plan(path, array, plan, values, &next)?;
                Ok(pattern_loop(
                    name,
                    i64::from(bounds.0).abs_diff(i64::from(bounds.1)) + 1,
                    body,
                ))
            }
            FixedPatternPlan::Rows {
                bounds,
                default,
                entries,
                repeat,
                stride,
            } => {
                let mut statements = Vec::new();
                if let Some(default) = default {
                    if coordinates.is_empty()
                        && matches!(default.as_ref(), FixedPatternPlan::Value(_))
                    {
                        statements.extend(self.emit_descriptor_pattern_plan(
                            path,
                            array,
                            default,
                            values,
                            coordinates,
                        )?);
                    } else {
                        let name = self.new_fn_name(path, "pattern_index");
                        let offset =
                            IrExpr::new(IrExprKind::LocalRead(name.clone()), 64, true, None);
                        let mut next = coordinates.to_vec();
                        next.push(pattern_coordinate(*bounds, offset));
                        let body =
                            self.emit_descriptor_pattern_plan(path, array, default, values, &next)?;
                        statements.extend(pattern_loop(
                            name,
                            i64::from(bounds.0).abs_diff(i64::from(bounds.1)) + 1,
                            body,
                        ));
                    }
                }
                let name = self.new_fn_name(path, "pattern_repeat");
                let index = IrExpr::new(IrExprKind::LocalRead(name.clone()), 64, true, None);
                let mut body = Vec::new();
                for (offset, plan) in entries {
                    let offset = if *repeat == 1 {
                        pattern_integer(i128::from(*offset))
                    } else {
                        common_bin_expr(
                            IrBinOp::Add,
                            common_bin_expr(
                                IrBinOp::Mul,
                                index.clone(),
                                pattern_integer(i128::from(*stride)),
                            ),
                            pattern_integer(i128::from(*offset)),
                        )
                    };
                    let mut next = coordinates.to_vec();
                    next.push(pattern_coordinate(*bounds, offset));
                    body.extend(
                        self.emit_descriptor_pattern_plan(path, array, plan, values, &next)?,
                    );
                }
                if *repeat == 1 {
                    statements.extend(body);
                } else {
                    statements.extend(pattern_loop(name, *repeat, body));
                }
                Ok(statements)
            }
        }
    }
}

impl Codegen<'_> {
    /// Copy one row of descriptor `source` (first coordinate `row`) into a
    /// dense array view, cell by cell in nested loops whose code does not
    /// scale with the extent. The view's selectors are already frozen; each
    /// cell takes the destination element's state domain.
    pub(in super::super) fn dense_row_scatter(
        &mut self,
        path: &str,
        target: &IrMemoryView,
        source: usize,
        row: i32,
        nba: bool,
    ) -> Result<Vec<IrStmt>, String> {
        let array = self.model.arrays[target.array].clone();
        let source_dims = self.model.arrays[source].dims[1..].to_vec();
        let extents = |dims: &[(i32, i32)]| {
            dims.iter()
                .map(|(left, right)| left.abs_diff(*right))
                .collect::<Vec<_>>()
        };
        if target.selectors.len() + target.dims.len() != array.dims.len()
            || extents(&target.dims) != extents(&source_dims)
        {
            return Err(format!(
                "assignment-pattern lvalue row shape mismatch in `{path}`"
            ));
        }
        let mut names = Vec::with_capacity(target.dims.len());
        let mut target_indices: Vec<IrExpr> = target
            .selectors
            .iter()
            .map(|selector| selector.value.clone())
            .collect();
        let mut source_indices = vec![pattern_integer(i128::from(row))];
        for (dimension, bounds) in target.dims.iter().enumerate() {
            let name = self.new_fn_name(path, "pattern_cell");
            let offset = IrExpr::new(IrExprKind::LocalRead(name.clone()), 64, true, None);
            target_indices.push(pattern_coordinate(*bounds, offset.clone()));
            source_indices.push(pattern_coordinate(source_dims[dimension], offset));
            names.push(name);
        }
        let value = IrExpr::new(
            IrExprKind::ArrayRead {
                arr: source,
                indices: source_indices,
                elem_sel: IrElemSel::Whole,
            },
            self.model.arrays[source].elem_width,
            self.model.arrays[source].signed,
            None,
        );
        let mut body = vec![IrStmt::Assign {
            lhs: IrLhs::ArrayElem {
                arr: target.array,
                indices: target_indices,
                elem_sel: IrElemSel::Whole,
            },
            rhs: ir_to_storage(value, array.elem_width, array.signed, array.two_state)?,
            nba,
        }];
        for (name, bounds) in names.into_iter().zip(&target.dims).rev() {
            body = pattern_loop(name, u64::from(bounds.0.abs_diff(bounds.1)) + 1, body);
        }
        Ok(body)
    }
}

#[derive(Clone)]
enum PatternCapture {
    Value(IrExpr),
    Array(usize),
}

impl PatternCapture {
    fn packed(&self) -> Result<IrExpr, String> {
        match self {
            Self::Value(value) => Ok(value.clone()),
            Self::Array(_) => Err("packed pattern item was captured as an array".into()),
        }
    }
}

fn pattern_coordinate(bounds: (i32, i32), offset: IrExpr) -> IrExpr {
    common_bin_expr(
        if bounds.0 >= bounds.1 {
            IrBinOp::Sub
        } else {
            IrBinOp::Add
        },
        pattern_integer(i128::from(bounds.0)),
        offset,
    )
}

fn pattern_loop(name: String, count: u64, body: Vec<IrStmt>) -> Vec<IrStmt> {
    let read = IrExpr::new(IrExprKind::LocalRead(name.clone()), 64, true, None);
    vec![IrStmt::Block(vec![
        IrStmt::DeclLocal {
            name: name.clone(),
            width: 64,
            signed: true,
            two_state: true,
            init: Some(Box::new(pattern_integer(0))),
        },
        IrStmt::For {
            init: Vec::new(),
            cond: common_bin_expr(
                IrBinOp::Lt,
                read.clone(),
                pattern_integer(i128::from(count)),
            ),
            incr: vec![IrStmt::Assign {
                lhs: IrLhs::WholeRef {
                    addr: format!("&{name}"),
                    width: 64,
                    signed: true,
                    two_state: true,
                    shortreal: false,
                },
                rhs: common_bin_expr(IrBinOp::Add, read, pattern_integer(1)),
                nba: false,
            }],
            body,
        },
    ])]
}

fn pattern_integer(value: i128) -> IrExpr {
    IrExpr::convert_to(lhs_integer_expr(value), 64, true)
}
