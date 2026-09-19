//! Casts.

use super::*;

impl<'a> Codegen<'a> {
    fn dynamic_cast_lhs_shape(&self, lhs: &IrLhs) -> Result<(u32, bool, bool, bool), String> {
        Ok(match lhs {
            IrLhs::PackedSelect {
                target,
                steps,
                two_state,
                ..
            } => (
                steps.last().map_or(0, |step| step.width),
                false,
                *two_state || self.dynamic_cast_lhs_shape(target)?.2,
                false,
            ),
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
                    IrElemSel::PackedChain(steps) => (
                        steps.last().map_or(0, |step| step.width),
                        false,
                        array.two_state,
                        false,
                    ),
                }
            }
            IrLhs::Stream { .. } => {
                return Err("$cast destination cannot be a streaming assignment target".to_owned())
            }
        })
    }

    fn class_node_for_type_expr(&self, node: NodeId) -> Option<NodeId> {
        let candidate = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => target.unwrap_or(node),
            NodeKind::Expr(ExprKind::NewClass { class_type, .. }) => {
                return class_type.and_then(|type_id| self.db.class_for_type(type_id));
            }
            _ => node,
        };
        self.db
            .type_descriptor(candidate)
            .filter(|descriptor| descriptor.info.kind == "class")
            .and_then(|descriptor| self.db.class_for_type(descriptor.id))
            .or_else(|| {
                matches!(self.kind(candidate), NodeKind::Var { ty } if ty.kind == "class")
                    .then(|| {
                        self.db
                            .classes()
                            .iter()
                            .find(|class| self.node(**class).name == self.node(candidate).name)
                            .copied()
                    })
                    .flatten()
            })
    }

    /// Flatten one fixed-size unpacked value in the declaration order required
    /// by a bit-stream cast. Dynamic containers, strings, real leaves, and
    /// unions remain outside this fixed-size lowering boundary.
    pub(in super::super) fn lower_bitstream_source(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(
                &descriptor.shape,
                TypeShape::FixedArray { .. }
                    | TypeShape::Aggregate(crate::core::db::AggregateLayout {
                        kind: AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion,
                        ..
                    })
            )
        }) {
            if let Some(value) = self.fixed_activation_read(path, node)? {
                return Ok(Some(value));
            }
        }
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

    pub(in super::super) fn join_bitstream_parts(
        path: &str,
        parts: Vec<IrExpr>,
    ) -> Result<IrExpr, String> {
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
    pub(in super::super) fn lower_dynamic_cast(
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
        if let Some(target_class_node) = self.class_node_for_type_expr(destination) {
            let source_is_null = matches!(
                self.kind(*source),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::Null,
                    ..
                })
            );
            if !source_is_null && !self.is_chandle_expr(path, *source) {
                return Err(format!(
                    "$cast class source is not a class handle in `{path}`"
                ));
            }
            let (target, _) = self.lower_chandle_lvalue(path, destination)?;
            let target_address = self.chandle_target_address(&target);
            let source = self.lower_chandle(path, *source)?;
            let expected = self
                .class_nodes
                .get(&target_class_node)
                .copied()
                .ok_or_else(|| format!("$cast target class has no execution layout in `{path}"))?;
            return Ok(IrExpr::new(
                IrExprKind::DynamicCast(Box::new(crate::sim::ir::IrDynamicCast {
                    lhs: IrLhs::WholeRef {
                        addr: target_address.clone(),
                        width: 0,
                        signed: false,
                        two_state: false,
                        shortreal: false,
                    },
                    rhs: IrExpr::new(
                        IrExprKind::Const(IrConst {
                            bits: vec![0],
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
                    ),
                    target_width: 0,
                    target_signed: false,
                    target_two_state: false,
                    target_shortreal: false,
                    valid_values: Vec::new(),
                    class_target: Some(target_address),
                    class_source: Some(source),
                    class_expected: Some(expected),
                })),
                1,
                false,
                None,
            ));
        }
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
                class_target: None,
                class_source: None,
                class_expected: None,
            })),
            1,
            false,
            None,
        ))
    }
}
