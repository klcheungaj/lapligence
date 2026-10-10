//! External input.

use super::*;

impl<'a> Codegen<'a> {
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

    pub(super) fn lower_plusarg_text(
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
        what: &str,
    ) -> Result<IrPlusArgTarget, String> {
        if let Some(target) = self.container_element_input_target(scope_path, node)? {
            return Ok(target);
        }
        if self.is_string_expr(scope_path, node) {
            self.ensure_string_actual_writable(scope_path, node)?;
            return Ok(IrPlusArgTarget::String {
                address: self.lower_string_actual_address(scope_path, node)?,
            });
        }
        if self.is_chandle_expr(scope_path, node) {
            return Err(format!(
                "{what} destination must be packed, real, or string storage in `{scope_path}`"
            ));
        }
        let lhs = self.lower_lhs(scope_path, node)?;
        if let IrLhs::Ref {
            const_ref: true, ..
        } = &lhs
        {
            return Err(format!(
                "{what} destination cannot be a const ref in `{scope_path}`"
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
            format!("{what} destination is not writable storage in `{scope_path}`")
        })?
        else {
            return Err(format!(
                "{what} destination is not a supported packed lvalue in `{scope_path}`"
            ));
        };
        if width == 0 {
            return Err(format!(
                "{what} destination has no resolved type in `{scope_path}`"
            ));
        }
        Ok(IrPlusArgTarget::Packed {
            lhs: Box::new(lhs),
            width,
            signed,
            two_state,
        })
    }

    /// A packed element of a queue, dynamic array or associative array as
    /// a scan destination: the scan writes through a retained element cell
    /// bound when the call starts (SIM-008).
    fn container_element_input_target(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<Option<IrPlusArgTarget>, String> {
        let container = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, .. }) => {
                self.container_of_select(node, *base)
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                self.container_of_select(node, *base)
            }
            _ => None,
        };
        let Some(container) = container.map(|container| container.ir) else {
            return Ok(None);
        };
        if self.model.containers[container].element.is_real() {
            return Err(format!(
                "real element of a queue, dynamic or associative array as an input destination in `{scope_path}` is not supported (SIM-008)"
            ));
        }
        // Other element types keep their own destination diagnostics.
        let crate::sim::ir::IrContainerElement::Packed {
            width,
            signed,
            two_state,
        } = self.model.containers[container].element
        else {
            return Ok(None);
        };
        let read = self.lower_expr(scope_path, node)?;
        let IrExprKind::Container(operation) = read.kind() else {
            return Ok(None);
        };
        if !matches!(
            operation.as_ref(),
            IrContainerExpr::Get { container: read, .. }
                | IrContainerExpr::GetString { container: read, .. } if *read == container
        ) {
            return Ok(None);
        }
        Ok(Some(IrPlusArgTarget::Element {
            read: Box::new(read),
            width,
            signed,
            two_state,
        }))
    }

    pub(super) fn lower_file_input_target(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<IrFileInputTarget, String> {
        match self.lower_plusarg_target(scope_path, node, "file input")? {
            IrPlusArgTarget::Element {
                read,
                width,
                signed,
                two_state,
            } => Ok(IrFileInputTarget::Element {
                read,
                width,
                signed,
                two_state,
            }),
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

    /// Lower a `$fread` destination: a fixed memory, a whole packed dynamic
    /// array or queue, a packed variable or selection, a packed container
    /// element, or a staged selection of one (SIM-026).
    pub(super) fn lower_file_read_destination(
        &mut self,
        scope_path: &str,
        node: NodeId,
        tag: &str,
    ) -> Result<(IrFileReadTarget, Option<StagedInput>), String> {
        if let Some(array) = self.array_of(node).cloned() {
            let array = self.reference_array(array.ir);
            if self.model.array(array).real {
                return Err(format!(
                    "$fread destination array must contain packed elements in `{scope_path}`"
                ));
            }
            return Ok((IrFileReadTarget::Array { array }, None));
        }
        if let Some(container) = self.container_of(node) {
            let container = container.ir;
            let packed = matches!(
                self.model.containers[container].element,
                crate::sim::ir::IrContainerElement::Packed { .. }
            );
            let addressed = matches!(
                self.model.containers[container].kind,
                IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
            );
            if !packed || !addressed {
                return Err(format!(
                    "$fread destination container must be a dynamic array or queue of packed elements in `{scope_path}`"
                ));
            }
            return Ok((IrFileReadTarget::Container { container }, None));
        }
        if let Some(input) = self.container_select_input_stage(scope_path, node, tag)? {
            let (prelude, lhs, width, store) =
                (input.prelude, input.target, input.width, input.store);
            // The element copy and its selection are declared four-state.
            let two_state = false;
            return Ok((
                IrFileReadTarget::Packed {
                    lhs: Box::new(lhs),
                    width,
                    signed: false,
                    two_state,
                },
                Some(StagedInput { prelude, store }),
            ));
        }
        match self.lower_plusarg_target(scope_path, node, "$fread")? {
            IrPlusArgTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            } => Ok((
                IrFileReadTarget::Packed {
                    lhs,
                    width,
                    signed,
                    two_state,
                },
                None,
            )),
            IrPlusArgTarget::Element {
                read,
                width,
                signed,
                two_state,
            } => Ok((
                IrFileReadTarget::Element {
                    read,
                    width,
                    signed,
                    two_state,
                },
                None,
            )),
            IrPlusArgTarget::Real { .. } | IrPlusArgTarget::String { .. } => Err(format!(
                "$fread destination must be a packed variable or unpacked array in `{scope_path}`"
            )),
        }
    }

    /// Lower `$sscanf` source text or a scan format. String-typed operands
    /// stay strings; an unpacked byte array reads as its bytes in index
    /// order; any other integral operand is packed text evaluated once at
    /// the call, so its unknown bits can make the scan return EOF (SV
    /// 21.3.4.3).
    pub(super) fn lower_scan_text(
        &mut self,
        scope_path: &str,
        node: NodeId,
        context: &str,
    ) -> Result<IrPlusArgText, String> {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                ..
            })
        ) || self.is_string_expr(scope_path, node)
        {
            return self.lower_plusarg_text(scope_path, node, context);
        }
        if let Some(value) = self.lower_bitstream_source(scope_path, node)? {
            return Ok(IrPlusArgText::Packed(Box::new(value)));
        }
        if self.is_runtime_stream_source(scope_path, node) {
            let stream = self.lower_bit_stream_source(scope_path, node)?;
            return Ok(IrPlusArgText::Dynamic(IrStringExpr::BitStream {
                stream: Box::new(stream),
                exact: true,
            }));
        }
        let value = self.lower_expr(scope_path, node)?;
        if value.is_real() {
            return Err(format!("{context} requires a string or integral argument"));
        }
        if let IrExprKind::Const(constant) = &value.kind {
            if let Ok(text) = Self::packed_plusarg_text(constant, context) {
                return Ok(IrPlusArgText::Literal(text));
            }
        }
        Ok(IrPlusArgText::Packed(Box::new(value)))
    }

    pub(super) fn file_input_actual(&self, node: NodeId) -> Result<NodeId, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if !operands.is_empty() => Ok(operands[0]),
            _ => Ok(node),
        }
    }

    pub(in super::super) fn lower_plusarg_expr(
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
                    IrExprKind::SysFunc(Box::new(IrSysFunc::TestPlusArgs { pattern })),
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
                let target =
                    self.lower_plusarg_target(scope_path, destination, "$value$plusargs")?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(Box::new(IrSysFunc::ValuePlusArgs { format, target })),
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
    pub(in super::super) fn lower_system_command(
        &mut self,
        scope_path: &str,
        args: &[NodeId],
    ) -> Result<Option<IrStringExpr>, String> {
        match args {
            [] => Ok(None),
            // A literal command keeps its raw bytes so the runtime can reject
            // an embedded NUL; only string-typed values drop "\0" (SV 6.16).
            [arg] => match self.kind(match self.kind(*arg) {
                NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => {
                    *operand
                }
                _ => *arg,
            }) {
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::String,
                    value,
                    ..
                }) => Ok(Some(IrStringExpr::Literal(decoded_string_bytes(value)?))),
                _ => self.lower_string(scope_path, *arg).map(Some),
            },
            _ => Err(format!(
                "$system accepts at most one string argument in `{scope_path}`"
            )),
        }
    }
}

/// A file-input destination with no direct runtime descriptor (SIM-026).
/// The input writes a lexical temporary (or a selection of one); `prelude`
/// runs before the call, freezing the destination's selectors and keys and
/// declaring the temporary, and `store` publishes the temporary after the
/// call once the destination has been assigned.
pub(super) struct StagedInput {
    pub(super) prelude: Vec<IrStmt>,
    pub(super) store: IrStmt,
}

fn int_value(value: i128) -> IrExpr {
    super::super::containers::pattern_key_expr(value, 32, true, false)
}

impl Codegen<'_> {
    /// Lower one `$fscanf`/`$sscanf`/`$fgets` destination. Whole packed,
    /// real and string storage, packed selections and packed container
    /// elements are written in place through runtime descriptors; string and
    /// real container elements and selections of packed container elements
    /// are staged (SIM-026). `tag` names the staging locals uniquely.
    pub(super) fn lower_scan_destination(
        &mut self,
        scope_path: &str,
        node: NodeId,
        tag: &str,
    ) -> Result<(IrFileInputTarget, Option<StagedInput>), String> {
        if let Some(staged) = self.container_scalar_input_stage(scope_path, node, tag)? {
            return Ok(staged);
        }
        if let Some(input) = self.container_select_input_stage(scope_path, node, tag)? {
            let (prelude, lhs, width, store) =
                (input.prelude, input.target, input.width, input.store);
            // The element copy and its selection are declared four-state.
            let two_state = false;
            return Ok((
                IrFileInputTarget::Packed {
                    lhs: Box::new(lhs),
                    width,
                    signed: false,
                    two_state,
                },
                Some(StagedInput { prelude, store }),
            ));
        }
        if self.class_field_target(node).is_some() && self.is_string_expr(scope_path, node) {
            if let Some(field) = self.class_field_string_lvalue(scope_path, node)? {
                // A class property is reached through its receiver at the
                // store, like an ordinary assignment to it.
                let value_name = format!("_siv{tag}");
                return Ok((
                    IrFileInputTarget::String {
                        address: format!("&{value_name}"),
                    },
                    Some(StagedInput {
                        prelude: vec![IrStmt::DeclString {
                            name: value_name.clone(),
                            init: None,
                        }],
                        store: IrStmt::Object(Box::new(IrObjectStmt::StringAssignLocal(
                            field,
                            IrStringExpr::LocalRead(value_name),
                        ))),
                    }),
                ));
            }
        }
        self.check_scan_destination_shape(scope_path, node)?;
        Ok((self.lower_file_input_target(scope_path, node)?, None))
    }

    /// Reject destinations SV 21.3.4.3 excludes: integer conversions "shall
    /// not be used with any unpacked aggregate data type", and `%s` admits
    /// only an unpacked array of bytes among unpacked types (read as its
    /// bytes, like an integral destination). A character of a string is not
    /// a supported scan destination.
    fn check_scan_destination_shape(&self, scope_path: &str, node: NodeId) -> Result<(), String> {
        if let NodeKind::Expr(ExprKind::BitSelect { base, .. }) = self.kind(node) {
            if self.is_string_expr(scope_path, *base) {
                return Err(format!(
                    "a character of a string as a file input destination in `{scope_path}` is not supported; read into a byte variable and assign it"
                ));
            }
        }
        let Some(descriptor) = self.query_descriptor(node) else {
            return Ok(());
        };
        let byte_array = match &descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                dimensions.len() == 1
                    && matches!(element.shape, TypeShape::PackedAtom { .. })
                    && element.info.width == Some(8)
            }
            TypeShape::Aggregate(crate::core::db::AggregateLayout {
                kind: AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion,
                ..
            }) => false,
            _ => return Ok(()),
        };
        if byte_array {
            Ok(())
        } else {
            Err(format!(
                "file input destination of unpacked aggregate type `{}` in `{scope_path}` is illegal: scan conversions assign integral, real or string variables, or an unpacked array of bytes (SV 21.3.4.3)",
                descriptor.name
            ))
        }
    }

    /// Stage a string or real element of a queue, dynamic array, fixed
    /// string array or associative array: its key is evaluated once before
    /// the call and the element is stored as by an ordinary assignment.
    fn container_scalar_input_stage(
        &mut self,
        scope_path: &str,
        node: NodeId,
        tag: &str,
    ) -> Result<Option<(IrFileInputTarget, Option<StagedInput>)>, String> {
        let selected = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => self
                .container_of_select(node, *base)
                .map(|container| (container.ir, *index)),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => self
                .container_of_select(node, *base)
                .map(|container| (container.ir, indices[0])),
            _ => None,
        };
        let Some((container, index)) = selected else {
            return Ok(None);
        };
        let element = self.model.containers[container].element.clone();
        let shortreal = match element {
            crate::sim::ir::IrContainerElement::String => None,
            crate::sim::ir::IrContainerElement::Real { shortreal } => Some(shortreal),
            _ => return Ok(None),
        };
        let mut prelude = Vec::new();
        let string_key = matches!(
            self.model.containers[container].kind,
            IrContainerKind::Associative {
                key: crate::sim::ir::IrAssocKey::String
            }
        );
        let key_name = format!("_sik{tag}");
        let key = if string_key {
            let key = self.lower_string(scope_path, index)?;
            prelude.push(IrStmt::DeclString {
                name: key_name.clone(),
                init: Some(key),
            });
            None
        } else {
            let key = self.lower_container_top_index(scope_path, container, index)?;
            let read = IrExpr::new(
                IrExprKind::LocalRead(key_name.clone()),
                key.width,
                key.signed,
                None,
            );
            prelude.push(IrStmt::DeclLocal {
                name: key_name.clone(),
                width: key.width,
                signed: key.signed,
                two_state: false,
                init: Some(Box::new(key)),
            });
            Some(read)
        };
        let value_name = format!("_siv{tag}");
        let string_key_read = || IrStringExpr::LocalRead(key_name.clone());
        let (target, store) = match shortreal {
            None => {
                prelude.push(IrStmt::DeclString {
                    name: value_name.clone(),
                    init: None,
                });
                let value = IrStringExpr::LocalRead(value_name.clone());
                let store = match key {
                    Some(index) => IrContainerStmt::SetStringValue {
                        container,
                        index,
                        value,
                    },
                    None => IrContainerStmt::SetStringString {
                        container,
                        key: string_key_read(),
                        value,
                    },
                };
                (
                    IrFileInputTarget::String {
                        address: format!("&{value_name}"),
                    },
                    store,
                )
            }
            Some(shortreal) => {
                prelude.push(IrStmt::DeclLocal {
                    name: value_name.clone(),
                    width: 0,
                    signed: false,
                    two_state: false,
                    init: Some(Box::new(super::super::real_literal_expr(0.0))),
                });
                let value = IrExpr::new(IrExprKind::LocalRead(value_name.clone()), 0, false, None);
                let store = match key {
                    Some(index) => IrContainerStmt::SetReal {
                        container,
                        index,
                        value,
                    },
                    None => IrContainerStmt::SetStringReal {
                        container,
                        key: string_key_read(),
                        value,
                    },
                };
                (
                    IrFileInputTarget::Real {
                        lhs: Box::new(IrLhs::WholeRef {
                            addr: format!("&{value_name}"),
                            width: 0,
                            signed: false,
                            two_state: false,
                            shortreal,
                        }),
                        shortreal,
                    },
                    store,
                )
            }
        };
        Ok(Some((
            target,
            Some(StagedInput {
                prelude,
                store: IrStmt::Container(Box::new(store)),
            }),
        )))
    }

    /// Wrap a lowered file-input call whose destinations include staged ones.
    /// Every prelude runs first, in argument order, so all selectors are
    /// frozen at the call as for direct destinations; each staged store runs
    /// only when the call assigned that destination: input functions fill
    /// destinations in argument order, so destination `k` was assigned
    /// exactly when the returned count exceeds `k`.
    pub(super) fn staged_input_call(
        call: IrExpr,
        stages: Vec<(usize, StagedInput)>,
        tag: &str,
    ) -> IrExpr {
        if stages.is_empty() {
            return call;
        }
        let (width, signed) = (call.width, call.signed);
        let result = format!("_sir{tag}");
        let mut statements = Vec::new();
        let mut stores = Vec::new();
        for (ordinal, stage) in stages {
            statements.extend(stage.prelude);
            stores.push((ordinal, stage.store));
        }
        statements.push(IrStmt::DeclLocal {
            name: result.clone(),
            width,
            signed,
            two_state: false,
            init: Some(Box::new(call)),
        });
        let read = IrExpr::new(IrExprKind::LocalRead(result), width, signed, None);
        for (ordinal, store) in stores {
            let ordinal = i128::try_from(ordinal).unwrap_or(i128::MAX);
            statements.push(IrStmt::If {
                cond: IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::Gt,
                        a: Box::new(read.clone()),
                        b: Box::new(int_value(ordinal)),
                    },
                    1,
                    false,
                    None,
                ),
                then_: vec![store],
                els: None,
                check: crate::sim::ir::IrUniquePriorityCheck::None,
            });
        }
        IrExpr::new(
            IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                statements,
                value: read,
            })),
            width,
            signed,
            None,
        )
    }
}
