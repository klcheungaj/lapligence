//! Casts.

use super::*;
use crate::core::db::TypeDescriptor;
use crate::sim::codegen::lowering::collection::HoistedValue;
use crate::sim::ir::IrDynamicCast;

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
            IrLhs::TaggedSelect {
                target,
                steps,
                signed,
                two_state,
                ..
            } => (
                steps.last().map_or(0, |step| step.selection.width),
                *signed,
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

    pub(in super::super) fn class_node_for_type_expr(&self, node: NodeId) -> Option<NodeId> {
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
        let node = self.overload_operand(node);
        // Callers lower a conversion as an expression. Looking through it for
        // a storage array would erase the cast, even when its result is packed.
        if matches!(self.kind(node), NodeKind::Expr(ExprKind::Cast { .. })) {
            return Ok(None);
        }
        if let Some(value) = self.overload_current_read(path, node) {
            return value.map(Some);
        }
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

    /// Lower the two-argument `$cast` system subroutine. A destination with
    /// a storage slot is retained as an IR LHS so the emitter can evaluate
    /// selectors once and leave it unchanged when enum membership validation
    /// fails. Any other destination is written through its assignment family
    /// (`lower_cast_through_assignment`). `failure_location` selects the task
    /// form: a failed cast is then a run-time error (SV 6.24.2).
    pub(in super::super) fn lower_dynamic_cast(
        &mut self,
        path: &str,
        args: &[NodeId],
        failure_location: Option<String>,
    ) -> Result<IrExpr, String> {
        let [destination, source] = args else {
            return Err(format!("$cast requires exactly two arguments in `{path}`"));
        };
        // Slang represents the output argument of a system task as the
        // assignment expression that binds its hidden output temporary.  The
        // first operand is the user's actual lvalue; the second is an owned
        // placeholder and must not be lowered as a destination.
        let (destination, placeholder) = match self.kind(*destination) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if operands.len() == 2
                && matches!(self.kind(operands[1]), NodeKind::Expr(ExprKind::Other)) =>
            {
                (operands[0], Some(operands[1]))
            }
            _ => (*destination, None),
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
            let expected = self
                .class_nodes
                .get(&target_class_node)
                .copied()
                .ok_or_else(|| format!("$cast target class has no execution layout in `{path}"))?;
            let _ = self.lower_chandle(path, destination)?;
            let Some(target) = self.chandle_slot(path, destination)? else {
                let upcast = source_is_null || self.cast_is_upcast(*source, target_class_node);
                let target = DynamicCastTarget::Handle { expected, upcast };
                return self.lower_cast_through_assignment(
                    path,
                    destination,
                    placeholder,
                    *source,
                    target,
                    failure_location,
                );
            };
            let target_address = self.chandle_target_address(&target);
            let source = self.lower_chandle(path, *source)?;
            return Ok(class_cast(
                target_address,
                source,
                expected,
                failure_location,
            ));
        }
        // Check singularity before lowering: an unpacked destination would
        // otherwise lower as an aggregate scatter rather than one value.
        let target_descriptor = self.query_descriptor(destination).cloned();
        if let Some(descriptor) = &target_descriptor {
            match &descriptor.shape {
                TypeShape::PackedAtom { .. } | TypeShape::Real { .. } => {}
                TypeShape::Aggregate(layout)
                    if matches!(
                        layout.kind,
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion
                    ) => {}
                TypeShape::String => {
                    return self.lower_string_cast(
                        path,
                        destination,
                        placeholder,
                        *source,
                        failure_location,
                    );
                }
                // SV 6.24.2 admits only singular destinations.
                TypeShape::Aggregate(_) | TypeShape::FixedArray { .. } => {
                    return Err(format!(
                        "$cast destination must be a singular value in `{path}`"
                    ));
                }
                TypeShape::Container { .. } | TypeShape::Opaque { .. } => {
                    return Err(format!(
                        "$cast destination type is not supported in `{path}`"
                    ));
                }
            }
        }
        let lhs = if self.cast_destination_has_lhs(destination) {
            Some(self.lower_lhs(path, destination)?)
        } else {
            None
        };
        let (target_width, target_signed, target_two_state, target_shortreal) = match &lhs {
            Some(lhs) => self.dynamic_cast_lhs_shape(lhs)?,
            None => self.cast_value_shape(path, destination, target_descriptor.as_ref())?,
        };
        let source_descriptor = self.query_descriptor(*source).cloned();
        let source_is_string =
            self.is_string_expr(path, *source) && !self.is_string_literal(*source);
        if let Some(descriptor) = &source_descriptor {
            let unsupported = matches!(
                descriptor.shape,
                TypeShape::FixedArray { .. }
                    | TypeShape::Container { .. }
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
        // A string converts to a packed value and never to a real one.
        if source_is_string && target_width == 0 {
            return self.lower_failing_cast(path, *source, failure_location);
        }
        let mut rhs = if source_is_string {
            object_query(
                IrObjectQuery::StringPacked(self.lower_string(path, *source)?),
                target_width,
                false,
            )
        } else {
            self.lower_expr(path, *source)?
        };

        let mut valid_values = Vec::new();
        let target_is_enum = target_descriptor
            .as_ref()
            .is_some_and(|descriptor| descriptor.info.kind == "enum");
        if target_is_enum {
            if rhs.is_real() {
                // A real source converts as to `int` (SV 6.12.2: rounded,
                // ties away from zero, low 32 bits kept) and that complete
                // value is checked for membership (decision S11-D3).
                rhs = ir_to_vector(rhs, 32, true)?;
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
        let Some(lhs) = lhs else {
            let target = DynamicCastTarget::Value(Box::new(ValueCast {
                rhs,
                width: target_width,
                signed: target_signed,
                two_state: target_two_state,
                shortreal: target_shortreal,
                valid_values,
            }));
            return self.lower_cast_through_assignment(
                path,
                destination,
                placeholder,
                *source,
                target,
                failure_location,
            );
        };
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
                failure_location,
            })),
            1,
            false,
            None,
        ))
    }

    /// Whether `lower_lhs` serves a `$cast` destination: anything but a
    /// selection through an element of a resizable container.
    fn cast_destination_has_lhs(&self, destination: NodeId) -> bool {
        let mut node = destination;
        loop {
            if self.element_select(node).is_some() {
                return false;
            }
            match self.kind(node) {
                NodeKind::Expr(
                    ExprKind::BitSelect { base, .. }
                    | ExprKind::PartSelect { base, .. }
                    | ExprKind::IndexedPartSelect { base, .. }
                    | ExprKind::ArraySelect { base, .. }
                    | ExprKind::MemberSelect { base, .. },
                ) => node = *base,
                _ => return true,
            }
        }
    }

    /// Width, signedness and state domain of a destination with no IR LHS,
    /// read from its value; zero width names real storage.
    fn cast_value_shape(
        &mut self,
        path: &str,
        destination: NodeId,
        descriptor: Option<&TypeDescriptor>,
    ) -> Result<(u32, bool, bool, bool), String> {
        if let Some(TypeShape::Real { shortreal }) = descriptor.map(|d| &d.shape) {
            return Ok((0, true, false, *shortreal));
        }
        let value = self.lower_expr(path, destination)?;
        let two_state = descriptor.is_some_and(|descriptor| descriptor.two_state);
        Ok((value.width, value.signed, two_state, false))
    }

    /// `$cast` into a string: a string source is an ordinary assignment and
    /// any other source fails, leaving the destination unchanged.
    fn lower_string_cast(
        &mut self,
        path: &str,
        destination: NodeId,
        placeholder: Option<NodeId>,
        source: NodeId,
        failure_location: Option<String>,
    ) -> Result<IrExpr, String> {
        if !self.is_cast_string_source(path, source) {
            return self.lower_failing_cast(path, source, failure_location);
        }
        let placeholder = cast_placeholder(path, placeholder)?;
        let temporary = format!("_llg_cast{}", placeholder.0);
        let mut statements = vec![IrStmt::DeclString {
            name: temporary.clone(),
            init: Some(self.lower_string(path, source)?),
        }];
        self.hoisted_values
            .insert(placeholder, HoistedValue::String(temporary));
        let store = self.lower_cast_store(path, destination, placeholder);
        self.hoisted_values.remove(&placeholder);
        statements.push(store?);
        Ok(IrExpr::new(
            IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                statements,
                value: super::super::containers::pattern_key_expr(1, 1, false, false),
            })),
            1,
            false,
            None,
        ))
    }

    fn is_string_literal(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                ..
            })
        )
    }

    /// Whether a `$cast` source is string typed; a literal has a packed type
    /// that an assignment to a string converts.
    fn is_cast_string_source(&self, path: &str, source: NodeId) -> bool {
        self.is_string_literal(source) || self.is_string_expr(path, source)
    }

    /// A cast no value of its source type can satisfy. The source still runs.
    fn lower_failing_cast(
        &mut self,
        path: &str,
        source: NodeId,
        failure_location: Option<String>,
    ) -> Result<IrExpr, String> {
        let mut statements = Vec::new();
        if !self.side_effect_free(source) {
            let name = format!("_llg_cast_src{}", source.0);
            statements.push(self.declare_cast_copy(path, source, name)?.0);
        }
        let name = format!("_llg_cast_never{}", source.0);
        statements.push(IrStmt::DeclLocal {
            name: name.clone(),
            width: 1,
            signed: false,
            two_state: true,
            init: None,
        });
        Ok(IrExpr::new(
            IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                statements,
                value: self.lower_failing_cast_status(name, failure_location),
            })),
            1,
            false,
            None,
        ))
    }

    /// The zero status of a cast that writes nothing; `slot` is a declared
    /// one-bit two-state local.
    fn lower_failing_cast_status(&self, name: String, failure_location: Option<String>) -> IrExpr {
        let cast = IrDynamicCast {
            lhs: IrLhs::WholeRef {
                addr: format!("&{name}"),
                width: 1,
                signed: false,
                two_state: true,
                shortreal: false,
            },
            rhs: super::super::containers::pattern_key_expr(0, 1, false, false),
            target_width: 1,
            target_signed: false,
            target_two_state: true,
            target_shortreal: false,
            // An unreachable member makes the check fail at run time, so the
            // task form reports the error of every other failed cast.
            valid_values: vec![super::super::containers::pattern_key_expr(
                1, 1, false, false,
            )],
            class_target: None,
            class_source: None,
            class_expected: None,
            failure_location,
        };
        IrExpr::new(IrExprKind::DynamicCast(Box::new(cast)), 1, false, None)
    }

    /// A local holding the value of `node`, and the hoisted form of that local.
    fn declare_cast_copy(
        &mut self,
        path: &str,
        node: NodeId,
        name: String,
    ) -> Result<(IrStmt, HoistedValue), String> {
        if self.is_string_expr(path, node) {
            let init = Some(self.lower_string(path, node)?);
            return Ok((
                IrStmt::DeclString {
                    name: name.clone(),
                    init,
                },
                HoistedValue::String(name),
            ));
        }
        if self.is_chandle_expr(path, node) {
            let value = self.lower_chandle(path, node)?;
            return Ok((
                IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    name.clone(),
                    Some(value),
                ))),
                HoistedValue::Handle(name),
            ));
        }
        let value = self.lower_expr(path, node)?;
        let (width, signed) = (value.width, value.signed);
        Ok((
            IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed,
                two_state: false,
                init: Some(Box::new(value)),
            },
            HoistedValue::Packed(name, width, signed),
        ))
    }

    /// The ordinary blocking assignment of the hoisted `placeholder` to
    /// `destination`, in whichever statement family owns that storage.
    fn lower_cast_store(
        &mut self,
        path: &str,
        destination: NodeId,
        placeholder: NodeId,
    ) -> Result<IrStmt, String> {
        let function = self.func.clone();
        let depth = self.depth_arg.clone();
        let instance = self.inst;
        EmitCtx::new(self, path.to_owned(), instance, &depth, function, false)
            .lower_assignment_operands(destination, placeholder, true, Operation::Assignment, false)
    }

    /// Lower a cast whose destination has no storage slot: the value is cast
    /// into a temporary of the destination type, and an ordinary assignment
    /// stores that temporary only when the cast succeeds, so a failed cast
    /// creates no element. A checked destination (a class handle or an enum)
    /// evaluates its selectors before the source; any other destination
    /// assigns as usual, source first.
    fn lower_cast_through_assignment(
        &mut self,
        path: &str,
        destination: NodeId,
        placeholder: Option<NodeId>,
        source: NodeId,
        target: DynamicCastTarget,
        failure_location: Option<String>,
    ) -> Result<IrExpr, String> {
        let placeholder = cast_placeholder(path, placeholder)?;
        let temporary = format!("_llg_cast{}", placeholder.0);
        let status = format!("_llg_cast_ok{}", placeholder.0);
        let mut statements = Vec::new();
        let mut captured = Vec::new();
        let mut receiver = None;
        // The outcome of a constant value or an upcast is known here, so the
        // destination is not bound first.
        let checked = match &target {
            DynamicCastTarget::Handle { upcast, .. } => !upcast,
            DynamicCastTarget::Value(value) => {
                !value.valid_values.is_empty() && !matches!(value.rhs.kind, IrExprKind::Const(_))
            }
        };
        let result = (|| {
            if checked {
                receiver = self.capture_cast_receiver(path, destination, &mut statements)?;
                self.capture_cast_selectors(
                    path,
                    destination,
                    source,
                    &mut statements,
                    &mut captured,
                )?;
                let location = self.cast_location(path, destination);
                statements.extend(self.cast_bounds_stop(path, destination, location)?);
                let default = format!("{temporary}_default");
                let (declare, hoisted) = match &target {
                    DynamicCastTarget::Handle { .. } => (
                        IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                            default.clone(),
                            None,
                        ))),
                        HoistedValue::Handle(default),
                    ),
                    DynamicCastTarget::Value(value) => (
                        IrStmt::DeclLocal {
                            name: default.clone(),
                            width: value.width,
                            signed: value.signed,
                            two_state: value.two_state,
                            init: None,
                        },
                        HoistedValue::Packed(default, value.width, value.signed),
                    ),
                };
                statements.extend(self.create_cast_element(
                    path,
                    destination,
                    placeholder,
                    declare,
                    hoisted,
                )?);
            }
            let (cast, hoisted) = match target {
                DynamicCastTarget::Handle { expected, .. } => {
                    statements.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                        temporary.clone(),
                        None,
                    ))));
                    let source = self.lower_chandle(path, source)?;
                    (
                        class_cast(format!("&{temporary}"), source, expected, failure_location),
                        HoistedValue::Handle(temporary.clone()),
                    )
                }
                DynamicCastTarget::Value(value) => {
                    let ValueCast {
                        rhs,
                        width,
                        signed,
                        two_state,
                        shortreal,
                        valid_values,
                    } = *value;
                    // Every value converts to real: only an integral target
                    // has a check.
                    if width == 0 {
                        let init = if rhs.is_real() {
                            rhs
                        } else {
                            IrExpr::new(
                                IrExprKind::CastToReal {
                                    a: Box::new(rhs),
                                    shortreal,
                                },
                                REAL_EXPR_WIDTH,
                                true,
                                None,
                            )
                        };
                        statements.push(IrStmt::DeclLocal {
                            name: temporary.clone(),
                            width: REAL_EXPR_WIDTH,
                            signed: true,
                            two_state: false,
                            init: Some(Box::new(init)),
                        });
                        let hoisted =
                            HoistedValue::Packed(temporary.clone(), REAL_EXPR_WIDTH, true);
                        (
                            super::super::containers::pattern_key_expr(1, 1, false, false),
                            hoisted,
                        )
                    } else {
                        statements.push(IrStmt::DeclLocal {
                            name: temporary.clone(),
                            width,
                            signed,
                            two_state,
                            init: None,
                        });
                        let cast = IrDynamicCast {
                            lhs: IrLhs::WholeRef {
                                addr: format!("&{temporary}"),
                                width,
                                signed,
                                two_state,
                                shortreal,
                            },
                            rhs,
                            target_width: width,
                            target_signed: signed,
                            target_two_state: two_state,
                            target_shortreal: shortreal,
                            valid_values,
                            class_target: None,
                            class_source: None,
                            class_expected: None,
                            failure_location,
                        };
                        (
                            IrExpr::new(IrExprKind::DynamicCast(Box::new(cast)), 1, false, None),
                            HoistedValue::Packed(temporary.clone(), width, signed),
                        )
                    }
                }
            };
            statements.push(IrStmt::DeclLocal {
                name: status.clone(),
                width: 1,
                signed: false,
                two_state: true,
                init: Some(Box::new(cast)),
            });
            self.hoisted_values.insert(placeholder, hoisted);
            self.lower_cast_store(path, destination, placeholder)
        })();
        self.hoisted_values.remove(&placeholder);
        for node in captured {
            self.hoisted_values.remove(&node);
        }
        if let Some(base) = receiver {
            self.computed_receivers.remove(&base);
        }
        statements.push(IrStmt::If {
            cond: IrExpr::new(IrExprKind::LocalRead(status.clone()), 1, false, None),
            then_: vec![result?],
            els: None,
            check: IrUniquePriorityCheck::None,
        });
        Ok(IrExpr::new(
            IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                statements,
                value: IrExpr::new(IrExprKind::LocalRead(status), 1, false, None),
            })),
            1,
            false,
            None,
        ))
    }

    /// Whether the static class of `source` is `target` or derives from it.
    fn cast_is_upcast(&self, source: NodeId, target: NodeId) -> bool {
        let mut class = self
            .query_descriptor(source)
            .filter(|descriptor| descriptor.info.kind == "class")
            .and_then(|descriptor| self.db.class_for_type(descriptor.id));
        while let Some(current) = class {
            if current == target {
                return true;
            }
            class = self
                .db
                .class_metadata(current)
                .and_then(|metadata| metadata.base);
        }
        false
    }

    fn names_queue_end(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::Expr(ExprKind::Unbounded))
            || self
                .node(node)
                .children
                .iter()
                .any(|child| self.names_queue_end(*child))
    }

    fn cast_location(&self, path: &str, node: NodeId) -> String {
        let source = self.node(node);
        format!("{path}:{}:{}", source.line, source.col)
    }

    /// A checked destination binds its element before the source runs: an
    /// index past the end of a queue or dynamic array ends the run, whichever
    /// form the cast has. One past the end of a queue is the element the
    /// store appends.
    fn cast_bounds_stop(
        &mut self,
        path: &str,
        destination: NodeId,
        location: String,
    ) -> Result<Vec<IrStmt>, String> {
        let Some((container, index)) = self
            .element_select(destination)
            .map(|(container, index)| (container.ir, index))
        else {
            return Ok(Vec::new());
        };
        let limit_extra = match self.model.containers[container].kind {
            IrContainerKind::Queue { .. } => 1,
            IrContainerKind::Dynamic => 0,
            IrContainerKind::Associative { .. } => return Ok(Vec::new()),
        };
        let index = self.lower_container_top_index(path, container, index)?;
        let index = IrExpr::convert_to(index, CAST_INDEX_WIDTH, true);
        let size = cast_container_size(container);
        let limit = bin_expr(
            IrBinOp::Add,
            size,
            super::super::containers::pattern_key_expr(limit_extra, CAST_INDEX_WIDTH, true, false),
        );
        let in_range = bin_expr(
            IrBinOp::LogAnd,
            bin_expr(
                IrBinOp::Ge,
                index.clone(),
                super::super::containers::pattern_key_expr(0, CAST_INDEX_WIDTH, true, false),
            ),
            bin_expr(IrBinOp::Lt, index, limit),
        );
        let slot = format!("_llg_cast_slot{}", destination.0);
        Ok(vec![IrStmt::If {
            cond: un_expr(IrUnOp::LogNot, in_range),
            then_: vec![
                IrStmt::DeclLocal {
                    name: slot.clone(),
                    width: 1,
                    signed: false,
                    two_state: true,
                    init: None,
                },
                IrStmt::DeclLocal {
                    name: format!("_llg_cast_stop{}", destination.0),
                    width: 1,
                    signed: false,
                    two_state: true,
                    init: Some(Box::new(self.lower_failing_cast_status(
                        slot,
                        Some(format!(
                            "{location}: the destination element does not exist"
                        )),
                    ))),
                },
            ],
            els: None,
            check: IrUniquePriorityCheck::None,
        }])
    }

    /// A bound destination exists before the source runs: a missing
    /// associative entry or the queue element one past the end holds the
    /// default value of its type whether or not the cast succeeds.
    fn create_cast_element(
        &mut self,
        path: &str,
        destination: NodeId,
        placeholder: NodeId,
        declare: IrStmt,
        default: HoistedValue,
    ) -> Result<Vec<IrStmt>, String> {
        let Some((container, selector)) = self
            .element_select(destination)
            .map(|(container, selector)| (container.ir, selector))
        else {
            return Ok(Vec::new());
        };
        let missing = match self.model.containers[container].kind {
            IrContainerKind::Dynamic => return Ok(Vec::new()),
            IrContainerKind::Queue { .. } => {
                let index = self.lower_container_top_index(path, container, selector)?;
                bin_expr(
                    IrBinOp::Eq,
                    IrExpr::convert_to(index, CAST_INDEX_WIDTH, true),
                    cast_container_size(container),
                )
            }
            IrContainerKind::Associative { ref key } => {
                let exists = match key {
                    IrAssocKey::String => IrContainerExpr::ExistsString {
                        container,
                        key: self.lower_string(path, selector)?,
                    },
                    _ => IrContainerExpr::Exists {
                        container,
                        key: Box::new(self.lower_container_index(path, selector)?),
                    },
                };
                un_expr(
                    IrUnOp::LogNot,
                    IrExpr::new(IrExprKind::Container(Box::new(exists)), 32, true, None),
                )
            }
        };
        self.hoisted_values.insert(placeholder, default);
        let store = self.lower_cast_store(path, destination, placeholder);
        self.hoisted_values.remove(&placeholder);
        Ok(vec![IrStmt::If {
            cond: missing,
            then_: vec![declare, store?],
            els: None,
            check: IrUniquePriorityCheck::None,
        }])
    }

    /// Evaluate a computed handle the destination goes through (`get().q[i]`)
    /// into a local the store then uses.
    fn capture_cast_receiver(
        &mut self,
        path: &str,
        destination: NodeId,
        statements: &mut Vec<IrStmt>,
    ) -> Result<Option<NodeId>, String> {
        let Some(base) = self.effectful_computed_receiver(destination) else {
            return Ok(None);
        };
        let name = format!("_llg_recv{}", base.0);
        let handle = self.lower_chandle(path, base)?;
        statements.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
            name.clone(),
            Some(handle),
        ))));
        self.computed_receivers
            .insert(base, IrChandleExpr::LocalRead(name));
        Ok(Some(base))
    }

    /// Evaluate the destination's selectors now. A selector needs a local
    /// when it has effects of its own, which must happen even if the cast
    /// fails, or when the source could change it.
    fn capture_cast_selectors(
        &mut self,
        path: &str,
        destination: NodeId,
        source: NodeId,
        statements: &mut Vec<IrStmt>,
        captured: &mut Vec<NodeId>,
    ) -> Result<(), String> {
        let source_is_free = self.side_effect_free(source);
        let mut selectors = Vec::new();
        let mut node = destination;
        loop {
            match self.kind(node) {
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    selectors.extend(indices.iter().copied());
                    node = *base;
                }
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    selectors.push(*index);
                    node = *base;
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base, base_expr, ..
                }) => {
                    selectors.push(*base_expr);
                    node = *base;
                }
                NodeKind::Expr(
                    ExprKind::PartSelect { base, .. } | ExprKind::MemberSelect { base, .. },
                ) => node = *base,
                _ => break,
            }
        }
        // Outermost selector first, as written. `$` has meaning only as a
        // queue selector, so that selector stays in place.
        for selector in selectors.into_iter().rev() {
            let constant = matches!(
                self.kind(selector),
                NodeKind::Expr(ExprKind::Constant { .. })
            );
            if constant
                || (source_is_free && self.side_effect_free(selector))
                || self.names_queue_end(selector)
            {
                continue;
            }
            let name = format!("_llg_cast_sel{}", selector.0);
            let (statement, hoisted) = self.declare_cast_copy(path, selector, name)?;
            statements.push(statement);
            for node in [selector, self.p30_unwrap_cast(selector)] {
                self.hoisted_values.insert(node, hoisted.clone());
                captured.push(node);
            }
        }
        Ok(())
    }
}

/// Width that holds any container index and size with their signs.
const CAST_INDEX_WIDTH: u32 = 64;

/// What a cast into a destination with no storage slot produces.
enum DynamicCastTarget {
    Handle {
        expected: usize,
        /// The source's static type is the destination's or derives from it.
        upcast: bool,
    },
    Value(Box<ValueCast>),
}

/// The value of a packed or real cast, already converted for checking.
struct ValueCast {
    rhs: IrExpr,
    width: u32,
    signed: bool,
    two_state: bool,
    shortreal: bool,
    valid_values: Vec<IrExpr>,
}

fn cast_container_size(container: usize) -> IrExpr {
    IrExpr::convert_to(
        IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::Size(container))),
            32,
            true,
            None,
        ),
        CAST_INDEX_WIDTH,
        true,
    )
}

fn cast_placeholder(path: &str, placeholder: Option<NodeId>) -> Result<NodeId, String> {
    placeholder.ok_or_else(|| format!("$cast destination has no output binding in `{path}`"))
}

/// The class form of `$cast`. The scalar fields stay a validation placeholder
/// (see [`IrDynamicCast`]).
fn class_cast(
    target_address: String,
    source: IrChandleExpr,
    expected: usize,
    failure_location: Option<String>,
) -> IrExpr {
    IrExpr::new(
        IrExprKind::DynamicCast(Box::new(IrDynamicCast {
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
            failure_location,
        })),
        1,
        false,
        None,
    )
}
