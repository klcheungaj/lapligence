//! Strings.

use super::*;

impl Codegen<'_> {
    pub(in super::super) fn lower_string(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStringExpr, String> {
        if let Some(target) = self.class_field_string_lvalue(path, node)? {
            return Ok(IrStringExpr::LocalRead(target));
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
                return Ok(IrStringExpr::RandomState);
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
            let (ft, _) =
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
            let mut arg_irs = vec![None; formals.len()];
            let mut out_args = Vec::new();
            let mut in_args = Vec::new();
            for (idx, (io, is_out)) in formals.iter().enumerate() {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
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
                    if !const_ref {
                        self.ensure_string_actual_writable(path, bound[idx].expr)?;
                    }
                    let addr = self.lower_string_actual_address(path, bound[idx].expr)?;
                    out_args.push(IrCallArg::StringRefAddr { addr, const_ref });
                } else if *is_out {
                    if !bound[idx].string {
                        return Err(format!(
                            "string-returning function `{}` does not support packed output formals",
                            self.node(ft).name
                        ));
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
                    let arg = self.lower_bound_arg(
                        path,
                        &formals,
                        &bound,
                        idx,
                        &mut arg_irs,
                    )?;
                    in_args.push(IrCallArg::Val(arg));
                }
            }
            out_args.extend(in_args);
            let typed = out_args.iter().any(|arg| {
                matches!(
                    arg,
                    IrCallArg::StringVal(_)
                        | IrCallArg::StringOutAddr(_)
                        | IrCallArg::StringRefAddr { .. }
                        | IrCallArg::StringOutTemp { .. }
                )
            });
            if !typed {
                let args = out_args
                    .into_iter()
                    .map(|arg| match arg {
                        IrCallArg::Val(value) => Ok(value),
                        _ => Err("internal non-packed argument in string call".to_owned()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(IrStringExpr::Call {
                    receiver: self.class_method_receiver(node)?.map(Box::new),
                    virtual_dispatch: self.class_method_virtual_dispatch(node),
                    function: meta.ir,
                    args,
                    depth: parse_depth(&self.depth_arg),
                });
            }
            return Ok(IrStringExpr::TypedCall {
                receiver: self.class_method_receiver(node)?.map(Box::new),
                virtual_dispatch: self.class_method_virtual_dispatch(node),
                function: meta.ir,
                args: out_args,
                depth: parse_depth(&self.depth_arg),
            });
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::String {
                return Ok(IrStringExpr::Read(index));
            }
            return Err("chandle cannot be converted to string".to_owned());
        }
        match self.kind(node) {
            NodeKind::SysCall { name } if name == "$sformatf" => {
                let args = self.call_argument_nodes(node);
                let Some((format, values)) = args.split_first() else {
                    return Err(format!("$sformatf requires a format argument in `{path}`"));
                };
                let format = self.lower_string(path, *format)?;
                let args = values
                    .iter()
                    .map(|value| self.lower_format_arg(path, *value))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrStringExpr::Format {
                    format: Box::new(format),
                    args,
                    scope: path.to_owned(),
                })
            }
            NodeKind::SysCall { name } if name == "$typename" => {
                let [argument] = self.node(node).children.as_slice() else {
                    return Err(format!("$typename requires exactly one argument in `{path}`"));
                };
                let descriptor = self.query_descriptor(*argument).ok_or_else(|| {
                    format!(
                        "$typename argument has no owned type metadata in `{path}`"
                    )
                })?;
                Ok(IrStringExpr::Literal(descriptor.name.as_bytes().to_vec()))
            }
            NodeKind::Expr(ExprKind::Constant { const_type: ConstantType::String, value, .. }) => Ok(IrStringExpr::Literal(decoded_string_bytes(value)?)),
            NodeKind::Expr(ExprKind::Ref {target:Some(target)}) if matches!(self.kind(*target),NodeKind::Param {ty,..} if ty.kind=="string") => self.lower_string(path,*target),
            NodeKind::Param {ty,value,..} if ty.kind=="string" => {
                match self.param_vals.get(&node).or(value.as_ref()) {
                    Some(Val::Str(value))=>Ok(IrStringExpr::Literal(decode_verilog_string(value)?)),
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
                    return Ok(IrStringExpr::Literal(decoded_string_bytes(value)?));
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
                    ("substr",[first,last]) => Ok(IrStringExpr::Substr(value,Box::new(self.object_int_argument(path,*first,32)?),Box::new(self.object_int_argument(path,*last,32)?))),
                    _ => Err(format!("unsupported string method or argument count: {name}")),
                }
            }
            _ => Err(format!("string assignment requires a string expression, literal, or explicit cast in `{path}`")),
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
