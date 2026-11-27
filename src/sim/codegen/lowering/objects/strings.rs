//! Strings.

use super::*;
use crate::sim::codegen::lowering::collection::CallElementCopies;

impl Codegen<'_> {
    /// `value` after `statements`, which run as the predicate of a
    /// conditional that always selects it.
    pub(super) fn string_after_statements(
        statements: Vec<IrStmt>,
        value: IrStringExpr,
    ) -> IrStringExpr {
        if statements.is_empty() {
            return value;
        }
        IrStringExpr::Conditional {
            predicate: Box::new(IrExpr::new(
                IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                    statements,
                    value: super::super::containers::pattern_key_expr(1, 1, false, false),
                })),
                1,
                false,
                None,
            )),
            then: Box::new(value),
            otherwise: Box::new(IrStringExpr::Literal(Vec::new())),
        }
    }

    pub(in super::super) fn lower_string(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStringExpr, String> {
        self.check_stack(node)?;
        // A string element through a computed handle that an effect of the
        // statement precedes binds its view before its index (VM-D1).
        if let Some(bindings) = self.take_placed_interface_bindings(node) {
            let value = self.lower_string(path, node)?;
            return self
                .string_after_bindings(value, bindings)
                .map_err(|error| self.locate_error(node, error));
        }
        self.lower_string_unlocated(path, node)
            .map_err(|error| self.locate_error(node, error))
    }

    /// A string call operand after the statements `copies` runs before it,
    /// and, with copy-outs after it, the call's value held in a temporary
    /// while they store the elements its actuals name (SV 13.5.2, decision
    /// EO-D3).
    fn string_call_operand(
        &mut self,
        path: &str,
        copies: CallElementCopies,
        call: IrStringExpr,
    ) -> IrStringExpr {
        let CallElementCopies { mut before, after } = copies;
        if after.is_empty() {
            return Self::string_after_statements(before, call);
        }
        let name = self.new_fn_name(path, "string_call");
        before.push(IrStmt::DeclString {
            name: name.clone(),
            init: Some(call),
        });
        before.extend(after);
        Self::string_after_statements(before, IrStringExpr::LocalRead(name))
    }

    /// [`Self::lower_string`] before its failure is located.
    fn lower_string_unlocated(&mut self, path: &str, node: NodeId) -> Result<IrStringExpr, String> {
        if let Some(HoistedValue::String(name)) = self.hoisted_value(node) {
            let value = IrStringExpr::LocalRead(name.clone());
            self.note_hoisted_read(node);
            return Ok(value);
        }
        if let Some(error) = self.nested_assignment_in_place_error(node) {
            return Err(error);
        }
        if let Some(value) = self
            .inline_string_key_read(node)
            .or_else(|| self.inline_string_index_read(node))
        {
            return Ok(value);
        }
        if let Some(value) = self.lower_parameter_string(path, node)? {
            return Ok(value);
        }
        if let Some(operands) = self.enter_call_operands(path, node)? {
            let value = self.lower_string(path, node);
            let before = self.leave_call_operands(operands);
            let value = value?;
            if before.is_empty() {
                return Ok(value);
            }
            return Ok(IrStringExpr::Conditional {
                predicate: Box::new(always_after(before)),
                then: Box::new(value),
                otherwise: Box::new(IrStringExpr::Literal(Vec::new())),
            });
        }
        if let Some(before) = self.enter_receiver_root(node) {
            let value = self.lower_string(path, node);
            self.leave_receiver_root(node, &before);
            return Ok(IrStringExpr::Conditional {
                predicate: Box::new(always_after(before)),
                then: Box::new(value?),
                otherwise: Box::new(IrStringExpr::Literal(Vec::new())),
            });
        }
        // Element container members read outside a staged statement are
        // copied out first; the copies run as the predicate of a conditional
        // that always selects the value.
        if self.element_staging_possible {
            if let Some((mut before, after)) = self.stage_expression_members(path, node)? {
                let value = self.lower_string(path, node);
                self.end_element_member_staging();
                let mut value = value?;
                // The value is read before the changed element is stored.
                if !after.is_empty() {
                    let name = format!("_llg_ss{}", node.0);
                    before.push(IrStmt::DeclString {
                        name: name.clone(),
                        init: Some(value),
                    });
                    before.extend(after);
                    value = IrStringExpr::LocalRead(name);
                }
                return Ok(Self::string_after_statements(before, value));
            }
        }
        if let Some(value) = self.lower_object_random_state(path, node)? {
            return Ok(value);
        }
        // Covergroup string options (SIM-046).
        if let Some(value) = self.lower_coverage_text(path, node)? {
            return Ok(value);
        }
        if let Some(value) = self.native_tagged_string(path, node)? {
            return Ok(value);
        }
        if let Some(select) = self.native_member_select(path, node)? {
            if let crate::sim::ir::IrNativeLeafExpr::String(value) =
                self.native_member_select_read(path, &select)?
            {
                return Ok(value);
            }
        }
        if self.element_leaf_kind(node) == Some(IrClassFieldType::String) {
            if let Some((name, _)) = self.element_leaf_symbol(path, node, false)? {
                return Ok(IrStringExpr::LocalRead(name));
            }
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            if let Some(name) = self.proc_string_local_name(*target) {
                return Ok(IrStringExpr::LocalRead(name.to_owned()));
            }
        }
        if let Some(target) = self.class_field_string_lvalue(path, node)? {
            return Ok(IrStringExpr::LocalRead(target));
        }
        if let Some((predicate, then, otherwise)) =
            self.lower_native_conditional_parts(path, node, |shape| *shape == TypeShape::String)?
        {
            return Ok(IrStringExpr::Conditional {
                predicate: Box::new(predicate),
                then: Box::new(self.lower_string(path, then)?),
                otherwise: Box::new(self.lower_string(path, otherwise)?),
            });
        }
        if let NodeKind::MethodCall {
            name,
            receiver: Some(receiver),
            ..
        } = self.kind(node)
        {
            if name == "get_randstate"
                && self.is_process_rng_receiver(*receiver)
                && self.node(node).children.len() == 1
            {
                self.model.random.threads = true;
                return Ok(IrStringExpr::RandomState);
            }
            if name == "get_randstate"
                && self.is_process_value(path, *receiver)
                && self.node(node).children.len() == 1
            {
                self.model.random.threads = true;
                return Ok(IrStringExpr::ProcessRandState(Box::new(
                    self.process_receiver(path, node, *receiver, name)?,
                )));
            }
        }
        if let Some(value) = self.lower_container_string_query(path, node)? {
            return Ok(value);
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some((_, name)) = self.lexical_proc_string_local(node) {
            return Ok(IrStringExpr::LocalRead(name.to_owned()));
        }
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.string_read.get(&target)) {
                return Ok(value.clone());
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .string_read
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok(value.clone());
                }
            }
        }
        let callable = match self.kind(node) {
            NodeKind::FuncCall {
                is_task: false,
                callee,
                ..
            } => Some(*callee),
            NodeKind::MethodCall { callee, .. } if self.is_class_method_call(node) => Some(*callee),
            _ => None,
        };
        if let Some(callee) = callable {
            let (ft, callee_inst) =
                self.resolve_callee_env(self.inst, &self.node(node).name, false, callee)?;
            let meta = self
                .func_meta
                .get(&ft)
                .cloned()
                .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
            if !meta.ret_string {
                return Err(format!(
                    "function `{}` does not return string",
                    self.node(ft).name
                ));
            }
            let formals = meta.formals.clone();
            let actuals = self.call_argument_nodes(node);
            let bound = self.bind_call_args(self.inst, &formals, &actuals)?;
            let dispatch = self.record_ref_dispatch(node, false);
            let function =
                self.record_ref_callee(ft, callee_inst, &formals, &bound, meta.ir, dispatch)?;
            let mut arg_irs = vec![None; formals.len()];
            let mut out_args = Vec::new();
            let mut in_args = Vec::new();
            // Columns after the first of column-layout record formals, and
            // the temporaries of record arguments that are no record storage.
            let mut record_out_args = Vec::new();
            let mut record_in_args = Vec::new();
            // The statements before the call (record temporaries, inputs
            // evaluated early) and after it (element copy-outs).
            let mut copies = CallElementCopies::default();
            let mut record_after = Vec::new();
            // Inputs evaluated before the copy-ins and bindings after them
            // (decision EO-D3).
            let until = self.early_actuals_until(&formals, &bound);
            for (idx, (io, is_out)) in formals.iter().enumerate() {
                if let Some((statement, actual)) = self.early_input_actual(
                    path,
                    &formals,
                    &bound,
                    idx,
                    until,
                    &mut arg_irs,
                    format!("_llg_ea{}_{idx}", node.0),
                )? {
                    copies.before.push(statement);
                    in_args.push(actual.argument());
                    continue;
                }
                if self.record_declaration(*io) {
                    let address = *is_out || self.is_ref_formal(*io);
                    let mut columns = self
                        .record_call_columns(
                            path,
                            *io,
                            bound[idx].expr,
                            Some((&mut copies.before, &mut record_after)),
                        )?
                        .into_iter();
                    let first = columns
                        .next()
                        .ok_or("column-layout record formal has no columns")?;
                    if address {
                        out_args.push(first);
                        record_out_args.extend(columns);
                    } else {
                        in_args.push(first);
                        record_in_args.extend(columns);
                    }
                    continue;
                }
                if bound[idx].is_event {
                    let name = self.node(ft).name.clone();
                    let (address, argument) =
                        self.event_call_argument(path, &name, *io, *is_out, bound[idx].expr)?;
                    if address {
                        out_args.push(argument);
                    } else {
                        in_args.push(argument);
                    }
                    continue;
                }
                // See `lower_call_stmts`: storage of the actual's own type.
                if let Some(argument) = self.dpi_open_storage_argument(*io, bound[idx].expr)? {
                    if *is_out {
                        out_args.push(argument);
                    } else {
                        in_args.push(argument);
                    }
                    continue;
                }
                if self.is_subroutine_container(*io) {
                    // A member of a record element read after an effect is
                    // bound after the actuals before it (decisions EO-D3,
                    // EO-D4).
                    if let Some(bindings) = self.take_placed_interface_bindings(bound[idx].expr) {
                        copies.before.extend(bindings);
                    }
                    let argument =
                        self.container_call_argument(path, *io, bound[idx].expr, None, None)?;
                    // `ref` formals pass by address with the outputs.
                    if *is_out || self.is_ref_formal(*io) {
                        out_args.push(argument);
                    } else {
                        in_args.push(argument);
                    }
                    continue;
                }
                if self.is_native_declaration(*io) {
                    let argument = self.native_expression_argument(path, *io, bound[idx].expr)?;
                    if *is_out || self.is_ref_formal(*io) {
                        out_args.push(argument);
                    } else {
                        in_args.push(argument);
                    }
                    continue;
                }
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind))
                {
                    // Handle formals keep their identity (SV 13.5); process
                    // formals bind as in other expression calls.
                    if !is_ref && !*is_out {
                        in_args.push(IrCallArg::ChandleVal(self.lower_handle_argument(
                            path,
                            *io,
                            bound[idx].expr,
                        )?));
                        continue;
                    }
                    if is_ref {
                        if let Some(argument) = self.handle_element_ref(path, bound[idx].expr)? {
                            out_args.push(argument);
                            continue;
                        }
                    }
                    if self.is_process_formal(*io) {
                        if let Some(argument) = self.process_formal_expression_binding(
                            path,
                            *io,
                            bound[idx].expr,
                            is_ref,
                        )? {
                            out_args.push(argument);
                            continue;
                        }
                    }
                    let (target, _) = self.lower_chandle_lvalue(path, bound[idx].expr)?;
                    let address = self.chandle_target_address(&target);
                    out_args.push(if is_ref {
                        IrCallArg::ChandleRefAddr(address)
                    } else {
                        IrCallArg::ChandleAddr(address)
                    });
                    continue;
                }
                if is_ref {
                    if !bound[idx].string {
                        return Err(format!(
                            "string-returning function `{}` only supports string ref formals",
                            self.node(ft).name
                        ));
                    }
                    let const_ref = matches!(
                        self.kind(*io),
                        NodeKind::FuncArg {
                            const_ref: true,
                            ..
                        }
                    );
                    if let Some(argument) =
                        self.string_element_ref(path, bound[idx].expr, const_ref)?
                    {
                        out_args.push(argument);
                        continue;
                    }
                    if !const_ref {
                        self.ensure_string_actual_writable(path, bound[idx].expr)?;
                    }
                    let addr = self.lower_string_actual_address(path, bound[idx].expr)?;
                    out_args.push(IrCallArg::StringRefAddr {
                        addr,
                        const_ref,
                        observed: false,
                    });
                } else if *is_out {
                    if !bound[idx].string {
                        let (mut argument, formal_read) = self.packed_output_temp(
                            path,
                            node,
                            idx,
                            *io,
                            callee_inst,
                            &bound[idx],
                            &mut copies,
                        )?;
                        // Copied in at its place among the actuals (decision
                        // EO-D3).
                        if let (true, IrCallArg::OutTemp { init, .. }) =
                            (until.is_some_and(|until| idx < until), &mut argument)
                        {
                            if let Some(value) = init.take() {
                                *init = Some(Box::new(Self::early_copy_in(
                                    *value,
                                    format!("_llg_ec{}_{idx}", node.0),
                                    &mut copies.before,
                                )));
                            }
                        }
                        arg_irs[idx] = Some(formal_read);
                        out_args.push(argument);
                        continue;
                    }
                    self.ensure_string_actual_writable(path, bound[idx].expr)?;
                    let writeback = self.lower_string_actual_address(path, bound[idx].expr)?;
                    let init = matches!(
                        self.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Inout,
                            ..
                        }
                    )
                    .then(|| self.lower_string(path, bound[idx].expr))
                    .transpose()?;
                    let (storage_addr, storage_read) = self
                        .static_string_formals
                        .get(&(self.inst, *io))
                        .map(|object| {
                            (
                                Some(format!("&{}", self.model.objects[*object].c_name)),
                                Some(Box::new(IrStringExpr::Read(*object))),
                            )
                        })
                        .unwrap_or((None, None));
                    out_args.push(IrCallArg::StringOutTemp {
                        name: format!("_st{}_{}", node.0, idx),
                        init: init.map(Box::new),
                        writeback,
                        storage_addr,
                        storage_read,
                    });
                } else if bound[idx].string {
                    in_args.push(IrCallArg::StringVal(
                        self.lower_string(path, bound[idx].expr)?,
                    ));
                } else {
                    let arg = self.lower_bound_arg(path, &formals, &bound, idx, &mut arg_irs)?;
                    in_args.push(IrCallArg::Val(arg));
                }
            }
            if !record_after.is_empty() {
                return Err(format!(
                    "an output record argument of string function `{}` in `{path}` must be record storage",
                    self.node(ft).name
                ));
            }
            out_args.extend(record_out_args);
            out_args.extend(in_args);
            out_args.extend(record_in_args);
            // Only packed inputs pass by value through the plain string
            // call; any other argument (a real, an address, a copy-out
            // temporary) takes the typed call.
            let typed = out_args
                .iter()
                .any(|arg| !matches!(arg, IrCallArg::Val(value) if !value.is_real()));
            if !typed && copies.before.is_empty() && copies.after.is_empty() {
                let args = out_args
                    .into_iter()
                    .map(|arg| match arg {
                        IrCallArg::Val(value) => Ok(value),
                        _ => Err("internal non-packed argument in string call".to_owned()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(IrStringExpr::Call {
                    receiver: self.class_method_receiver(path, node)?.map(Box::new),
                    virtual_dispatch: self.class_method_virtual_dispatch(node),
                    function,
                    args,
                    depth: parse_depth(&self.depth_arg),
                    vpi_call: self.vpi_call_origin(node),
                });
            }
            let call = IrStringExpr::TypedCall {
                receiver: self.class_method_receiver(path, node)?.map(Box::new),
                virtual_dispatch: self.class_method_virtual_dispatch(node),
                function,
                args: out_args,
                depth: parse_depth(&self.depth_arg),
                vpi_call: self.vpi_call_origin(node),
            };
            return Ok(self.string_call_operand(path, copies, call));
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::String {
                return Ok(IrStringExpr::Read(index));
            }
            return Err("chandle cannot be converted to string".to_owned());
        }
        match self.kind(node) {
            NodeKind::SysCall { name }
                if matches!(
                    name.as_str(),
                    "$sampled" | "$past" | "$stable" | "$changed" | "$rose" | "$fell"
                ) =>
            {
                Err(format!(
                    "{name} of a string expression is illegal in `{path}`: sampled expressions \
                     exclude string operands (SV 16.6.1)"
                ))
            }
            NodeKind::SysCall { name } if name == "$sformatf" => {
                let args = self.call_argument_nodes(node);
                let Some((format, values)) = args.split_first() else {
                    return Err(format!("$sformatf requires a format argument in `{path}`"));
                };
                let library = self
                    .scope_nodes
                    .get(path)
                    .map(|scope| self.library_binding(*scope));
                let (format, values) = (*format, values.to_vec());
                self.lower_explicit_format(path, library.as_deref(), "$sformatf", format, &values)
            }
            NodeKind::SysCall { name } if name == "$typename" => {
                let text = self
                    .db
                    .type_name_text(node)
                    .ok_or_else(|| format!("$typename has no captured text in `{path}`"))?;
                Ok(IrStringExpr::Literal(text.to_vec()))
            }
            NodeKind::Expr(ExprKind::Constant { const_type: ConstantType::String, value, .. }) => Ok(string_literal(decoded_string_bytes(value)?)),
            NodeKind::Expr(ExprKind::Ref {target:Some(target)}) if matches!(self.kind(*target),NodeKind::Param {ty,..} if ty.kind=="string") => self.lower_string(path,*target),
            NodeKind::Param {ty,..} if ty.kind=="string" => {
                match self.parameter_value(node).as_deref() {
                    Some(Val::Str(value))=>Ok(string_literal(decode_verilog_string(value)?)),
                    _=>Err("string parameter has no captured string value".to_owned()),
                }
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => {
                let operand = *operand;
                if self.is_string_expr(path,operand) { return self.lower_string(path,operand); }
                if let NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::String,
                    value,
                    ..
                }) = self.kind(operand)
                {
                    return Ok(string_literal(decoded_string_bytes(value)?));
                }
                // A streaming concatenation fills the string as a dynamic
                // array of bytes, left-aligned (SV 6.24.3, 11.4.14); a
                // resizable or unpacked source is a bit-stream cast and must
                // supply whole bytes.
                if let Some(stream) = self.lower_bit_stream(path, operand)? {
                    return Ok(IrStringExpr::BitStream { stream: Box::new(stream), exact: false });
                }
                if self.is_runtime_stream_source(path, operand) {
                    let stream = self.lower_bit_stream_source(path, operand)?;
                    return Ok(IrStringExpr::BitStream { stream: Box::new(stream), exact: true });
                }
                if let Some(value) = self.lower_bitstream_source(path, operand)? {
                    return Ok(IrStringExpr::BitStream {
                        stream: Box::new(crate::sim::ir::IrBitStream {
                            segments: vec![crate::sim::ir::IrStreamSegment::Packed(value)],
                            slice: 1,
                            direction: IrStreamDirection::LeftToRight,
                            unpack: None,
                        }),
                        exact: true,
                    });
                }
                let value = self.lower_expr(path,operand)?;
                if value.is_real() { return Err("real to string cast is unsupported".to_owned()); }
                Ok(IrStringExpr::FromPacked(Box::new(value)))
            }
            NodeKind::Expr(ExprKind::Operation {op,operands,reordered,..}) if matches!(op, Operation::Concat | Operation::MultiConcat) => {
                let repeat = *op == Operation::MultiConcat;
                let mut operands = operands.clone();
                if *reordered { operands.reverse(); }
                let count = if repeat {
                    if operands.len() < 2 { return Err("malformed string replication".to_owned()); }
                    Some(self.lower_expr(path, operands.remove(0))?)
                } else { None };
                let parts = operands.into_iter().map(|operand| self.lower_string(path,operand)).collect::<Result<Vec<_>,_>>()?;
                let result = IrStringExpr::Concat(parts);
                Ok(match count {Some(count) => IrStringExpr::Repeat(Box::new(result),Box::new(count)),None => result})
            }
            NodeKind::MethodCall { name, receiver: Some(receiver), .. } => {
                if name == "name" {
                    if let Some(value) = self.lower_enum_name(path, node)? {
                        return Ok(value);
                    }
                }
                let name = name.clone(); let receiver = *receiver;
                let args = self.node(node).children[1..].to_vec();
                let value = Box::new(self.lower_string(path,receiver)?);
                match (name.as_str(),args.as_slice()) {
                    ("toupper",[]) => Ok(IrStringExpr::Case(value,true)),
                    ("tolower",[]) => Ok(IrStringExpr::Case(value,false)),
                    ("substr",[first,last]) => Ok(IrStringExpr::Substr(value,Box::new(self.object_index_argument(path,*first)?),Box::new(self.object_index_argument(path,*last)?))),
                    _ => Err(format!("unsupported string method or argument count: {name}")),
                }
            }
            NodeKind::Expr(ExprKind::MemberSelect { .. }) => {
                if let Some(value) = self.lower_native_call_string_select(path, node)? {
                    return Ok(value);
                }
                Err(format!(
                    "a string member of a function result in `{path}` must be read from a record variable holding the result"
                ))
            }
            _ => Err(format!("string assignment requires a string expression, literal, or explicit cast in `{path}`")),
        }
    }

    /// Whether `node` is a string literal with a `\0` in it.
    fn literal_has_zero_byte(&self, node: NodeId) -> Result<bool, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                value,
                ..
            }) => Ok(decoded_string_bytes(value)?.contains(&0)),
            _ => Ok(false),
        }
    }

    /// Lower one argument retained by the shared typed formatter.  Native
    /// strings remain owned string expressions and reals never pass through a
    /// packed conversion.
    pub(in super::super) fn lower_format_arg(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrDisplayArg, String> {
        if self.literal_has_zero_byte(node)? {
            // A string value would lose the zero bytes of the packed literal
            // (SV 5.9, 6.16), which `%s` and `%h` print.
            return Ok(IrDisplayArg::Packed(self.lower_expr(path, node)?));
        }
        if self.is_string_expr(path, node)
            || matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::String,
                    ..
                })
            )
            || self
                .query_descriptor(node)
                .is_some_and(|descriptor| descriptor.shape == TypeShape::String)
        {
            return Ok(IrDisplayArg::String(self.lower_string(path, node)?));
        }
        let value = self.lower_expr(path, node)?;
        Ok(if value.is_real() {
            IrDisplayArg::Real(value)
        } else {
            IrDisplayArg::Packed(value)
        })
    }
}

/// A string-typed value of literal bytes. A string variable never contains
/// "\0" (SV 6.16), so zero bytes of the literal are dropped, as they are by
/// packed-to-string conversion.
fn string_literal(mut bytes: Vec<u8>) -> IrStringExpr {
    bytes.retain(|byte| *byte != 0);
    IrStringExpr::Literal(bytes)
}

/// A true predicate that runs `statements` first, for values with no
/// sequence form of their own.
pub(in super::super) fn always_after(statements: Vec<IrStmt>) -> IrExpr {
    IrExpr::new(
        IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
            statements,
            value: super::super::containers::pattern_key_expr(1, 1, false, false),
        })),
        1,
        false,
        None,
    )
}
