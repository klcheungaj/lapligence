//! Arguments.

use super::*;

impl<'a> Codegen<'a> {
    /// A call argument a frontend can synthesize for a *missing named* argument: a
    /// location-less `0` constant (genuine `0` literals carry a source line).
    fn is_synthetic_arg(&self, a: NodeId) -> bool {
        if self.node(a).line != 0 {
            return false;
        }
        match self.kind(a) {
            NodeKind::Expr(ExprKind::Constant { value, .. }) => matches!(
                value,
                ValueData::Int(0)
                    | ValueData::UInt(0)
                    | ValueData::Scalar(crate::core::value::ScalarValue::Zero)
            ),
            _ => false,
        }
    }

    /// Bind a call's positional arguments to the callee's formals, in formal
    /// order. Missing (or frontend-synthesized) arguments fall back to the
    /// formal's default expression; a formal without a default errors.
    pub(in super::super) fn bind_call_args(
        &self,
        inst: NodeId,
        formals: &[(NodeId, bool)],
        args: &[NodeId],
    ) -> Result<Vec<BoundArg>, String> {
        let mut bound = Vec::with_capacity(formals.len());
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let (w, s, two_state, real, shortreal, is_event, is_string) = match self.kind(*io) {
                NodeKind::FuncArg { ty, .. } => {
                    if ty.kind == "event" {
                        (0, false, false, false, false, true, false)
                    } else if is_handle_kind(&ty.kind) {
                        (0, false, false, false, false, false, false)
                    } else if ty.kind == "string" {
                        (0, false, false, false, false, false, true)
                    } else {
                        if is_real_kind(&ty.kind) {
                            (0, false, false, true, ty.kind == "shortreal", false, false)
                        } else {
                            match ty.width {
                                Some(w) if w <= LLG_MAX_WIDTH => (
                                    self.effective_decl_width(*io, inst, w),
                                    ty.signed,
                                    self.db.is_two_state_type(*io) || is_two_state_kind(&ty.kind),
                                    false,
                                    false,
                                    false,
                                    false,
                                ),
                                Some(w) => {
                                    return Err(format!(
                                        "formal `{}` is {w} bits wide; the runtime supports \
                             at most {LLG_MAX_WIDTH}",
                                        self.node(*io).name
                                    ))
                                }
                                None => {
                                    return Err(format!(
                                        "formal `{}` has no width",
                                        self.node(*io).name
                                    ))
                                }
                            }
                        }
                    }
                }
                _ => unreachable!("non-FuncArg in formals"),
            };
            let default = match self.kind(*io) {
                NodeKind::FuncArg { default, .. } => *default,
                _ => unreachable!(),
            };
            let (mut expr, is_default) = match args.get(idx) {
                // Slang inserts the formal's owned default expression node
                // directly into the elaborated call argument list.
                Some(a) if Some(*a) == default => (*a, true),
                Some(a) if !self.is_synthetic_arg(*a) => (*a, false),
                _ => (
                    default.ok_or_else(|| {
                        format!(
                            "missing argument for formal `{}` of `{}`",
                            self.node(*io).name,
                            self.node(*io)
                                .parent
                                .map(|p| self.node(p).name.clone())
                                .unwrap_or_default()
                        )
                    })?,
                    true,
                ),
            };
            if *is_out && !is_default {
                if let NodeKind::Expr(ExprKind::Operation { op, operands, .. }) = self.kind(expr) {
                    if *op == Operation::Assignment {
                        let [actual, _converted] = operands.as_slice() else {
                            return Err(format!(
                                "output argument for formal `{}` has a malformed assignment wrapper",
                                self.node(*io).name
                            ));
                        };
                        expr = *actual;
                    }
                }
            }
            bound.push(BoundArg {
                width: w,
                signed: s,
                two_state,
                real,
                shortreal,
                string: is_string,
                expr,
                is_default,
                is_event,
            });
        }
        Ok(bound)
    }

    /// Return source arguments in formal order. Slang keeps a method receiver
    /// as the first structural child, while ordinary function calls contain
    /// only their argument expressions.
    pub(in super::super) fn call_argument_nodes(&self, call: NodeId) -> Vec<NodeId> {
        let children = self.node(call).children.as_slice();
        match self.kind(call) {
            NodeKind::MethodCall {
                receiver: Some(receiver),
                ..
            } if children.first() == Some(receiver) => children[1..].to_vec(),
            _ => children.to_vec(),
        }
    }

    /// Lower the C value expression for bound argument `idx` of a call and
    /// record it in `arg_codes[idx]` (rendered, for the legacy string paths)
    /// and `arg_irs[idx]` (IR) for later formals' default expressions to
    /// reference.
    ///
    /// A formal's default expression (`input logic b = a + 1`) is written in
    /// the callee's scope and may reference earlier formals; it is lowered
    /// under a temporary formal-aware context mapping those formals to their
    /// already-lowered argument expressions.  Caller provided arguments are
    /// lowered in the caller's own context.
    pub(in super::super) fn lower_bound_arg_code(
        &mut self,
        scope_path: &str,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        idx: usize,
        arg_codes: &mut [Option<String>],
        arg_irs: &mut Vec<Option<IrExpr>>,
    ) -> Result<(String, IrExpr), String> {
        let (w, s, two_state) = (bound[idx].width, bound[idx].signed, bound[idx].two_state);
        let e_ir = if bound[idx].is_default {
            let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
            let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
            for (j, (io, _)) in formals.iter().enumerate().take(idx) {
                if arg_codes[j].is_some() {
                    let (wj, sj) = (bound[j].width, bound[j].signed);
                    arg_read.insert(
                        *io,
                        ArgMap {
                            width: wj,
                            signed: sj,
                            two_state: bound[j].two_state,
                        },
                    );
                    if let Some(ir) = arg_irs[j].clone() {
                        arg_ir.insert(*io, ir);
                    }
                }
            }
            let temp_func = FuncCtx {
                name: String::new(),
                is_task: false,
                ret: None,
                arg_read,
                arg_ir,
                event_args: HashMap::new(),
                arg_dependencies: HashMap::new(),
                arg_write: HashMap::new(),
                arg_lhs: HashMap::new(),
                const_refs: HashSet::new(),
                const_ref_lhs: HashMap::new(),
                persistent: HashMap::new(),
                chandle_read: HashMap::new(),
                chandle_write: HashMap::new(),
                process_read: HashMap::new(),
                process_write: HashMap::new(),
                string_read: HashMap::new(),
                string_write: HashMap::new(),
                string_addr: HashMap::new(),
                locals: HashMap::new(),
                ret_node: None,
                class_receiver: None,
                // Formal defaults carry exact references to the same formal
                // NodeIds used as keys above. Avoid remapping them through an
                // executable function context while arguments are being
                // assembled.
                def_node: None,
            };
            let saved = self.func.take();
            self.func = Some(temp_func);
            let res = self.lower_expr(scope_path, bound[idx].expr);
            self.func = saved;
            res?
        } else {
            self.lower_expr(scope_path, bound[idx].expr)?
        };
        let e_ir = apply_assignment_expression_width(e_ir, w);
        let conv_ir = if bound[idx].real {
            IrExpr::new(
                IrExprKind::CastToReal {
                    a: Box::new(e_ir),
                    shortreal: bound[idx].shortreal,
                },
                0,
                false,
                None,
            )
        } else {
            ir_to_storage(e_ir, w, s, two_state)?
        };
        let code = self.render_ir_code(&conv_ir)?;
        arg_codes[idx] = Some(code.clone());
        if arg_irs.len() <= idx {
            arg_irs.resize(idx + 1, None);
        }
        arg_irs[idx] = Some(conv_ir.clone());
        Ok((code, conv_ir))
    }

    fn ir_constant_i128(value: &IrExpr) -> Option<i128> {
        let IrExprKind::Const(value) = value.kind() else {
            return None;
        };
        if value.real_value().is_some()
            || value.x_mask().iter().any(|mask| *mask != 0)
            || value.z_mask().iter().any(|mask| *mask != 0)
            || value.width() > 128
        {
            return None;
        }
        let low = value.bits().first().copied().unwrap_or(0) as u128;
        let high = value.bits().get(1).copied().unwrap_or(0) as u128;
        let raw = low | (high << 64);
        if value.signed() && value.width() < 128 && value.width() != 0 {
            let sign = 1u128 << (value.width() - 1);
            if raw & sign != 0 {
                return Some((raw | (!0u128 << value.width())) as i128);
            }
        }
        Some(raw as i128)
    }

    pub(in super::super) fn array_constant_linear_index(
        array: &ArrayInfo,
        indices: &[IrExpr],
    ) -> Option<u64> {
        if indices.len() != array.dims.len() {
            return None;
        }
        let mut linear = 0u64;
        for ((left, right), index) in array.dims.iter().zip(indices) {
            let index = Self::ir_constant_i128(index)?;
            let lo = i128::from((*left).min(*right));
            let hi = i128::from((*left).max(*right));
            if index < lo || index > hi {
                return None;
            }
            let offset = if left >= right {
                i128::from(*left) - index
            } else {
                index - i128::from(*left)
            };
            let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
            linear = linear.checked_mul(extent)?.checked_add(offset as u64)?;
        }
        Some(linear)
    }

    /// Lower a `ref` actual to a checked canonical descriptor.  Unlike an
    /// output/inout binding this never creates a temporary or a writeback:
    /// the descriptor names the caller's original storage directly.
    fn lower_ref_actual_lhs(&mut self, scope_path: &str, node: NodeId) -> Result<IrLhs, String> {
        let Some(function) = self.func.as_ref() else {
            return self.lower_lhs(scope_path, node);
        };
        let target = self.canonical_func_target(node).unwrap_or(node);
        let target = if function.const_refs.contains(&target) {
            Some(target)
        } else {
            let name = self.node(node).name.as_str();
            function
                .const_refs
                .iter()
                .find(|formal| self.node(**formal).name == name)
                .copied()
        };
        let Some(target) = target else {
            return self.lower_lhs(scope_path, node);
        };
        if let Some(lhs) = function.const_ref_lhs.get(&target) {
            return self.lhs_to_ir(lhs.clone());
        }
        let Some(arg) = function.arg_ir.get(&target) else {
            return Err(format!(
                "const ref actual `{}` has no canonical descriptor in `{scope_path}`",
                self.node(node).name
            ));
        };
        let IrExprKind::FormalRead(index) = arg.kind() else {
            return Err(format!(
                "const ref actual `{}` has no canonical descriptor in `{scope_path}`",
                self.node(node).name
            ));
        };
        let info = function.arg_read.get(&target).ok_or_else(|| {
            format!(
                "const ref actual `{}` has no type metadata in `{scope_path}`",
                self.node(node).name
            )
        })?;
        Ok(IrLhs::Ref {
            addr: format!("r{index}"),
            width: info.width,
            signed: info.signed,
            two_state: info.two_state,
            const_ref: true,
            bit: None,
        })
    }

    pub(in super::super) fn lower_ref_arg(
        &mut self,
        scope_path: &str,
        bound: &BoundArg,
        const_ref: bool,
        ref_static: bool,
    ) -> Result<IrCallArg, String> {
        if bound.is_default {
            return Err(format!(
                "ref formal cannot use a default argument in `{scope_path}`"
            ));
        }
        if ref_static {
            let target = match self.kind(bound.expr) {
                NodeKind::Var { .. } => Some(bound.expr),
                NodeKind::Expr(ExprKind::Ref { target }) => *target,
                NodeKind::Expr(
                    ExprKind::BitSelect { base, .. }
                    | ExprKind::PartSelect { base, .. }
                    | ExprKind::IndexedPartSelect { base, .. }
                    | ExprKind::ArraySelect { base, .. },
                ) => match self.kind(*base) {
                    NodeKind::Expr(ExprKind::Ref { target }) => *target,
                    NodeKind::Var { .. } | NodeKind::Array { .. } => Some(*base),
                    _ => None,
                },
                _ => None,
            }
            .map(|target| self.canonical_func_target(target).unwrap_or(target));
            match target.map(|target| self.db.variable_lifetime(target)) {
                Some(VariableLifetime::Static) => {}
                Some(VariableLifetime::Automatic) => {
                    return Err(format!(
                        "ref static actual in `{scope_path}` cannot bind automatic storage"
                    ));
                }
                Some(VariableLifetime::Unavailable) | None => {
                    return Err(format!(
                        "ref static actual in `{scope_path}` has unavailable storage lifetime"
                    ));
                }
            }
        }

        // Queue elements cannot be represented by a stable `sv4_t *`: any
        // structural queue edit can reallocate or shift the backing storage.
        // Bind a retained element cell while evaluating the actual. Removal
        // detaches that cell without changing its value or other ref aliases.
        let queue_actual = match self.kind(bound.expr) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => self
                .container_of(*base)
                .filter(|container| {
                    matches!(
                        self.model.containers[container.ir].kind,
                        IrContainerKind::Queue { .. }
                    )
                })
                .map(|container| (container.ir, *index)),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => self
                .container_of(*base)
                .filter(|container| {
                    matches!(
                        self.model.containers[container.ir].kind,
                        IrContainerKind::Queue { .. }
                    )
                })
                .map(|container| (container.ir, indices[0])),
            _ => None,
        };
        if let Some((container, index_node)) = queue_actual {
            let (width, signed, two_state) = match self.model.containers[container].element {
                IrContainerElement::Packed {
                    width,
                    signed,
                    two_state,
                } => (width, signed, two_state),
                _ => {
                    return Err(format!(
                        "ref actual queue element in `{scope_path}` must have a packed integral type"
                    ));
                }
            };
            if width == 0 {
                return Err(format!(
                    "ref actual queue element in `{scope_path}` has zero width"
                ));
            }
            if (width, signed, two_state) != (bound.width, bound.signed, bound.two_state) {
                return Err(format!(
                    "ref actual type does not exactly match formal in `{scope_path}`"
                ));
            }
            let index = self.lower_queue_index(scope_path, container, index_node)?;
            let index_code = self.render_ir_code(&index)?;
            let queue = &self.model.containers[container];
            let lhs = IrLhs::WholeRef {
                // This typed placeholder is used for dependency/type analysis;
                // the emitted descriptor deliberately uses `.queue` instead
                // of this address because the data pointer is relocatable.
                addr: format!("&{}.data[sv4_to_index({index_code})]", queue.c_name),
                width,
                signed,
                two_state,
                shortreal: false,
            };
            let descriptor = format!(
                "llg_ref_queue(&{}, sv4_to_index({index_code}))",
                queue.c_name
            );
            return Ok(IrCallArg::RefAddr {
                addr: descriptor,
                width,
                signed,
                two_state,
                const_ref,
                lhs: Box::new(lhs),
                read: Box::new(self.lower_expr(scope_path, bound.expr)?),
            });
        }

        let lhs = self.lower_ref_actual_lhs(scope_path, bound.expr)?;
        let read = self.lower_expr(scope_path, bound.expr)?;
        let (base, width, signed, two_state, actual_const, kind, fields) = match &lhs {
            IrLhs::Whole(index) => {
                let signal = self.model.signals.get(*index).ok_or_else(|| {
                    format!("reference actual signal {index} is out of bounds in `{scope_path}`")
                })?;
                if signal.net_driver.is_some() {
                    return Err(format!(
                        "ref actual in `{scope_path}` must be a variable, not a net"
                    ));
                }
                let IrType::Packed {
                    width,
                    signed,
                    two_state,
                } = signal.ty
                else {
                    return Err(format!(
                        "ref actual in `{scope_path}` must have a packed integral type"
                    ));
                };
                (
                    format!("&{}", signal.c_name),
                    width,
                    signed,
                    two_state,
                    false,
                    "LLG_REF_WHOLE",
                    String::new(),
                )
            }
            IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                ..
            } => (
                addr.clone(),
                *width,
                *signed,
                *two_state,
                false,
                "LLG_REF_WHOLE",
                String::new(),
            ),
            IrLhs::Ref {
                addr,
                width,
                signed,
                two_state,
                const_ref: actual_const,
                bit: None,
            } => (
                addr.clone(),
                *width,
                *signed,
                *two_state,
                *actual_const,
                "LLG_REF_NESTED",
                String::new(),
            ),
            IrLhs::Ref { bit: Some(_), .. } => {
                return Err("packed bit selects cannot be passed by reference".to_owned());
            }
            IrLhs::Bit(index, bit, two_state) => {
                let signal = self.model.signals.get(*index).ok_or_else(|| {
                    format!("reference actual signal {index} is out of bounds in `{scope_path}`")
                })?;
                if signal.net_driver.is_some() {
                    return Err(format!("ref actual in `{scope_path}` must be a variable"));
                }
                let IrType::Packed { .. } = signal.ty else {
                    return Err(format!("ref actual in `{scope_path}` must be integral"));
                };
                let bit = self.render_ir_code(bit)?;
                (
                    format!("&{}", signal.c_name),
                    1,
                    false,
                    *two_state,
                    false,
                    "LLG_REF_BIT",
                    format!(".index = sv4_to_index({bit})"),
                )
            }
            IrLhs::Part(index, left, right, two_state) => {
                let signal = self.model.signals.get(*index).ok_or_else(|| {
                    format!("reference actual signal {index} is out of bounds in `{scope_path}`")
                })?;
                if signal.net_driver.is_some() {
                    return Err(format!("ref actual in `{scope_path}` must be a variable"));
                }
                let IrType::Packed { .. } = signal.ty else {
                    return Err(format!("ref actual in `{scope_path}` must be integral"));
                };
                let width = left.abs_diff(*right) as u32 + 1;
                (
                    format!("&{}", signal.c_name),
                    width,
                    false,
                    *two_state,
                    false,
                    "LLG_REF_PART",
                    format!(".left = {left}, .right = {right}"),
                )
            }
            IrLhs::IdxPart(index, base_index, _, width, negative, two_state) => {
                let signal = self.model.signals.get(*index).ok_or_else(|| {
                    format!("reference actual signal {index} is out of bounds in `{scope_path}`")
                })?;
                if signal.net_driver.is_some() {
                    return Err(format!("ref actual in `{scope_path}` must be a variable"));
                }
                let IrType::Packed { .. } = signal.ty else {
                    return Err(format!("ref actual in `{scope_path}` must be integral"));
                };
                let base_index = self.render_ir_code(base_index)?;
                (
                    format!("&{}", signal.c_name),
                    *width,
                    false,
                    *two_state,
                    false,
                    "LLG_REF_INDEXED",
                    format!(
                        ".index = sv4_to_index({base_index}), .indexed_width = {width}, \
                         .indexed_negative = {}",
                        *negative as u8
                    ),
                )
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self.model.arrays.get(*arr).ok_or_else(|| {
                    format!("reference actual array {arr} is out of bounds in `{scope_path}`")
                })?;
                let array_info = ArrayInfo {
                    global: array.c_name.clone(),
                    elem_width: array.elem_width,
                    signed: array.signed,
                    real: array.real,
                    shortreal: array.shortreal,
                    is_net: false,
                    dims: array.dims.clone(),
                    init: None,
                    ir: *arr,
                };
                let constant_linear = Self::array_constant_linear_index(&array_info, indices);
                if matches!(elem_sel, IrElemSel::Whole) && constant_linear.is_none() {
                    if indices
                        .iter()
                        .all(|index| Self::ir_constant_i128(index).is_some())
                    {
                        return Err(format!(
                            "ref actual array index in `{scope_path}` must be a constant in range"
                        ));
                    }
                    let index_codes = indices
                        .iter()
                        .map(|index| self.render_ir_code(index))
                        .collect::<Result<Vec<_>, _>>()?;
                    let (decls, condition, linear) =
                        array_guard(array, &index_codes).ok_or_else(|| {
                            format!("ref actual array index in `{scope_path}` has no dimensions")
                        })?;
                    let index = format!(
                        "({{ {decls} ({condition}) ? (uint64_t)({linear}) : UINT64_MAX; }})"
                    );
                    (
                        array.c_name.clone(),
                        array.elem_width,
                        array.signed,
                        array.two_state,
                        false,
                        "LLG_REF_ARRAY",
                        format!(".array_size = {}ULL, .index = {index}", array.total),
                    )
                } else {
                    let linear = Self::array_constant_linear_index(&array_info, indices)
                        .ok_or_else(|| {
                            format!(
                        "ref actual array index in `{scope_path}` must be a constant in range"
                    )
                        })?;
                    let base = format!("&{}[{}]", array.c_name, linear);
                    match elem_sel {
                        IrElemSel::Whole => (
                            base,
                            array.elem_width,
                            array.signed,
                            array.two_state,
                            false,
                            "LLG_REF_WHOLE",
                            String::new(),
                        ),
                        IrElemSel::Part(left, right) => (
                            base,
                            left.abs_diff(*right) as u32 + 1,
                            false,
                            array.two_state,
                            false,
                            "LLG_REF_PART",
                            format!(".left = {left}, .right = {right}"),
                        ),
                        IrElemSel::Bit(index) => {
                            let index = self.render_ir_code(index)?;
                            (
                                base,
                                1,
                                false,
                                array.two_state,
                                false,
                                "LLG_REF_BIT",
                                format!(".index = sv4_to_index({index})"),
                            )
                        }
                        IrElemSel::Indexed {
                            base: index,
                            width,
                            negative,
                        } => {
                            let index = self.render_ir_code(index)?;
                            (
                                base,
                                *width,
                                false,
                                array.two_state,
                                false,
                                "LLG_REF_INDEXED",
                                format!(
                                    ".index = sv4_to_index({index}), .indexed_width = {width}, \
                                 .indexed_negative = {}",
                                    *negative as u8
                                ),
                            )
                        }
                    }
                }
            }
            IrLhs::Stream { .. } => {
                return Err(format!(
                    "streaming concatenation is not a legal ref actual in `{scope_path}`"
                ));
            }
        };
        if width != bound.width || signed != bound.signed || two_state != bound.two_state {
            return Err(format!(
                "ref actual type does not exactly match formal in `{scope_path}`"
            ));
        }
        if actual_const && !const_ref {
            return Err(format!(
                "const ref actual cannot bind to writable ref formal in `{scope_path}`"
            ));
        }
        let descriptor = if kind == "LLG_REF_NESTED" {
            base
        } else {
            format!(
                "&(llg_ref_t){{ .base = {base}, .width = {width}, .is_signed = {}, \
                 .two_state = {}, .kind = {kind}, {fields} }}",
                signed as u8, two_state as u8
            )
        };
        Ok(IrCallArg::RefAddr {
            addr: descriptor,
            width,
            signed,
            two_state,
            const_ref: actual_const,
            lhs: Box::new(lhs),
            read: Box::new(read),
        })
    }

    /// Lower a caller-side output/inout temp initializer from an already
    /// evaluated actual read.  Callers use this after freezing selected-LHS
    /// indices so copy-in and copy-out share one actual identity.
    pub(in super::super) fn lower_call_temp_init_from_expr(
        &self,
        io: NodeId,
        b: &BoundArg,
        e: IrExpr,
    ) -> Result<(String, Option<IrExpr>), String> {
        match self.kind(io) {
            NodeKind::FuncArg {
                direction: DbDirection::Inout,
                ..
            } => {
                let e = apply_assignment_expression_width(e, b.width);
                let conv = if b.real {
                    IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(e),
                            shortreal: b.shortreal,
                        },
                        0,
                        false,
                        None,
                    )
                } else {
                    ir_to_storage(e, b.width, b.signed, b.two_state)?
                };
                let code = self.render_ir_code(&conv)?;
                Ok((code, Some(conv)))
            }
            _ if b.real => Ok(("0.0".to_string(), None)),
            _ => Ok((format!("sv4_x({}, {})", b.width, b.signed as u8), None)),
        }
    }

    /// Resolve a whole native string actual to its caller-owned C slot.
    pub(in super::super) fn lower_string_actual_address(
        &self,
        path: &str,
        actual: NodeId,
    ) -> Result<String, String> {
        let actual = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => *operand,
            _ => actual,
        };
        if matches!(
            self.kind(actual),
            NodeKind::Expr(ExprKind::BitSelect { .. })
        ) {
            return Err(format!(
                "string output/ref actual in `{path}` must be a whole string lvalue"
            ));
        }
        if let Some(index) = self.object_of(path, actual) {
            let object = self
                .model
                .objects
                .get(index)
                .ok_or_else(|| "string object index is out of bounds".to_owned())?;
            if object.ty != IrObjectType::String {
                return Err(format!("actual in `{path}` is not string storage"));
            }
            return Ok(format!("&{}", object.c_name));
        }
        if let Some((_, name)) = self.lexical_proc_string_local(actual) {
            return Ok(format!("&{name}"));
        }
        let target = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(actual),
        };
        let target = self.func.as_ref().and_then(|function| {
            target
                .and_then(|target| {
                    function
                        .string_write
                        .get(&target)
                        .cloned()
                        .or_else(|| function.string_addr.get(&target).cloned())
                })
                .or_else(|| {
                    matches!(
                        self.kind(actual),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .string_write
                            .iter()
                            .find(|(target, _)| self.node(**target).name == self.node(actual).name)
                            .map(|(_, value)| value.clone())
                            .or_else(|| {
                                function
                                    .string_addr
                                    .iter()
                                    .find(|(target, _)| {
                                        self.node(**target).name == self.node(actual).name
                                    })
                                    .map(|(_, value)| value.clone())
                            })
                    })
                    .flatten()
                })
        });
        target
            .map(|target| format!("&{target}"))
            .ok_or_else(|| format!("string output/ref actual in `{path}` is not a writable lvalue"))
    }

    /// Reject a string output/inout or mutable-ref actual that is only a
    /// readable const-ref alias.  Address formation itself remains available
    /// for const-ref formals and therefore must not perform this check.
    pub(in super::super) fn ensure_string_actual_writable(
        &self,
        path: &str,
        actual: NodeId,
    ) -> Result<(), String> {
        let actual = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => *operand,
            _ => actual,
        };
        if let Some(index) = self.object_of(path, actual) {
            let object = self
                .model
                .objects
                .get(index)
                .ok_or_else(|| "string object index is out of bounds".to_owned())?;
            return if object.ty == IrObjectType::String {
                Ok(())
            } else {
                Err(format!("actual in `{path}` is not string storage"))
            };
        }
        let target = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(actual),
        };
        let writable = self.func.as_ref().is_some_and(|function| {
            target
                .and_then(|target| function.string_write.get(&target))
                .is_some()
                || (matches!(
                    self.kind(actual),
                    NodeKind::Expr(ExprKind::Ref { target: None })
                ) && function
                    .string_write
                    .keys()
                    .any(|target| self.node(*target).name == self.node(actual).name))
        });
        if writable || self.lexical_proc_string_local(actual).is_some() {
            Ok(())
        } else {
            Err(format!(
                "string output/ref actual in `{path}` is a const-ref or non-writable lvalue"
            ))
        }
    }

    /// Lower one output/inout actual once.  Runtime selector expressions are
    /// captured in caller-side locals before the callee starts; the returned
    /// read and writeback LHS then use those locals rather than re-evaluating
    /// an index after the call.
    #[allow(clippy::type_complexity)]
    pub(in super::super) fn lower_call_actual(
        &mut self,
        path: &str,
        actual: NodeId,
        tag: &str,
    ) -> Result<(IrLhs, IrExpr, Vec<(String, u32, bool, bool, IrExpr)>), String> {
        let lhs = self.lower_lhs(path, actual)?;
        let mut captures = Vec::new();
        let mut sequence = 0usize;
        let (lhs, read) = self.freeze_call_lhs(lhs, tag, &mut sequence, &mut captures)?;
        let read = read.unwrap_or(self.lower_expr(path, actual)?);
        Ok((lhs, read, captures))
    }

    fn freeze_call_lhs(
        &self,
        lhs: IrLhs,
        tag: &str,
        sequence: &mut usize,
        captures: &mut Vec<(String, u32, bool, bool, IrExpr)>,
    ) -> Result<(IrLhs, Option<IrExpr>), String> {
        let mut capture = |expr: IrExpr| -> Result<IrExpr, String> {
            if expr.is_real() {
                return Err(format!(
                    "real-valued selector in subroutine output/inout actual `{tag}` is not supported"
                ));
            }
            let name = format!("_call_idx_{tag}_{}", *sequence);
            *sequence += 1;
            let frozen = IrExpr::new(
                IrExprKind::LocalRead(name.clone()),
                expr.width,
                expr.signed,
                None,
            );
            captures.push((name, expr.width, expr.signed, false, expr));
            Ok(frozen)
        };
        let read_signal = |signal: usize| -> Result<IrExpr, String> {
            let ty = self
                .model
                .signals
                .get(signal)
                .ok_or_else(|| format!("subroutine actual signal {signal} is out of bounds"))?
                .ty;
            Ok(IrExpr::new(
                IrExprKind::SigRead(signal),
                ty.width(),
                ty.signed(),
                None,
            ))
        };
        match lhs {
            IrLhs::Whole(signal) => Ok((lhs, Some(read_signal(signal)?))),
            IrLhs::Bit(signal, index, two_state) => {
                let index = capture(index)?;
                let read = IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(read_signal(signal)?),
                        idx: Box::new(index.clone()),
                    },
                    1,
                    false,
                    None,
                );
                Ok((IrLhs::Bit(signal, index, two_state), Some(read)))
            }
            IrLhs::Part(signal, left, right, two_state) => {
                let width = left
                    .abs_diff(right)
                    .checked_add(1)
                    .and_then(|width| u32::try_from(width).ok())
                    .ok_or_else(|| "part-select width exceeds the supported range".to_string())?;
                let read = IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(read_signal(signal)?),
                        left,
                        right,
                    },
                    width,
                    false,
                    None,
                );
                Ok((IrLhs::Part(signal, left, right, two_state), Some(read)))
            }
            IrLhs::IdxPart(signal, base, width_expr, width, negative, two_state) => {
                let base = capture(base)?;
                let width_expr = capture(width_expr)?;
                let read = IrExpr::new(
                    IrExprKind::IdxPartSel {
                        base: Box::new(read_signal(signal)?),
                        base_idx: Box::new(base.clone()),
                        width_expr: Box::new(width_expr.clone()),
                        neg: negative,
                    },
                    width,
                    false,
                    None,
                );
                Ok((
                    IrLhs::IdxPart(signal, base, width_expr, width, negative, two_state),
                    Some(read),
                ))
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self
                    .model
                    .arrays
                    .get(arr)
                    .ok_or_else(|| format!("subroutine actual array {arr} is out of bounds"))?;
                if array.real {
                    return Err(
                        "real unpacked-array output/inout actuals are not supported".to_string()
                    );
                }
                let indices = indices
                    .into_iter()
                    .map(&mut capture)
                    .collect::<Result<Vec<_>, _>>()?;
                let elem_sel = match elem_sel {
                    IrElemSel::Whole => IrElemSel::Whole,
                    IrElemSel::Part(left, right) => IrElemSel::Part(left, right),
                    IrElemSel::Bit(index) => IrElemSel::Bit(Box::new(capture(*index)?)),
                    IrElemSel::Indexed {
                        base,
                        width,
                        negative,
                    } => IrElemSel::Indexed {
                        base: Box::new(capture(*base)?),
                        width,
                        negative,
                    },
                };
                let (width, signed) = match elem_sel {
                    IrElemSel::Whole => (array.elem_width, array.signed),
                    IrElemSel::Part(left, right) => (
                        left.abs_diff(right)
                            .checked_add(1)
                            .and_then(|width| u32::try_from(width).ok())
                            .ok_or_else(|| {
                                "array element part-select width exceeds the supported range"
                                    .to_string()
                            })?,
                        false,
                    ),
                    IrElemSel::Bit(_) => (1, false),
                    IrElemSel::Indexed { width, .. } => (width, false),
                };
                let read = IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr,
                        indices: indices.clone(),
                        elem_sel: elem_sel.clone(),
                    },
                    width,
                    signed,
                    None,
                );
                Ok((
                    IrLhs::ArrayElem {
                        arr,
                        indices,
                        elem_sel,
                    },
                    Some(read),
                ))
            }
            lhs @ (IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::Stream { .. }) => {
                Ok((lhs, None))
            }
        }
    }
}
