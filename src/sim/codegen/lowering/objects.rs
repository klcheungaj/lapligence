//! Lower non-integral values without encoding their storage as packed bits.
use super::*;
use crate::sim::ir::{
    IrChandleExpr, IrDisplayRadix, IrObject, IrObjectQuery, IrObjectStmt, IrObjectType,
    IrStringExpr,
};

impl Codegen<'_> {
    pub(super) fn lower_object_display(
        &mut self,
        path: &str,
        args: &[NodeId],
        newline: bool,
        default_radix: IrDisplayRadix,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        if !args.iter().any(|arg| self.is_string_expr(path, *arg)) {
            return Ok(None);
        }
        let Some((first, rest)) = args.split_first() else {
            return Ok(None);
        };
        let (fmt, values) = match self.kind(*first) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                value,
                ..
            }) => (decoded_string_bytes(value)?, rest),
            _ => (Vec::new(), args),
        };
        let fmt = String::from_utf8(fmt)
            .map_err(|_| "string display format must contain valid UTF-8".to_owned())?;
        let mut result = Vec::new();
        let mut text = String::new();
        let mut arg = 0usize;
        let mut chars = fmt.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                text.push(ch);
                continue;
            }
            if chars.peek() == Some(&'%') {
                text.push('%');
                chars.next();
                continue;
            }
            if !text.is_empty() {
                result.push(display_text(&text, default_radix));
                text.clear();
            }
            let mut spec = String::from("%");
            while chars
                .peek()
                .is_some_and(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.'))
            {
                spec.push(chars.next().unwrap());
            }
            let conversion = chars.next().ok_or("incomplete string display format")?;
            spec.push(conversion);
            let node = *values.get(arg).ok_or("missing string display argument")?;
            arg += 1;
            if self.is_string_expr(path, node) {
                if conversion != 's' || !matches!(spec.as_str(), "%s" | "%0s") {
                    return Err("string display currently supports only %s and %0s".to_owned());
                }
                result.push(IrStmt::Object(IrObjectStmt::StringPrint(
                    self.lower_string(path, node)?,
                )));
            } else {
                let value = self.lower_expr(path, node)?;
                if !matches!(
                    conversion,
                    's' | 'h' | 'b' | 'o' | 'd' | 't' | 'f' | 'e' | 'g'
                ) {
                    return Err("unsupported display conversion".to_owned());
                }
                if value.is_real() != matches!(conversion, 'f' | 'e' | 'g') {
                    return Err("display conversion has incompatible argument type".to_owned());
                }
                result.push(IrStmt::Display {
                    fmt: c_quoted(&spec),
                    args: vec![(value.clone(), value.is_real())],
                    newline: false,
                    default_radix,
                });
            }
        }
        while arg < values.len() {
            let node = values[arg];
            arg += 1;
            if self.is_string_expr(path, node) {
                result.push(IrStmt::Object(IrObjectStmt::StringPrint(
                    self.lower_string(path, node)?,
                )));
            } else {
                let value = self.lower_expr(path, node)?;
                let spec = if value.is_real() {
                    "%f".to_owned()
                } else {
                    format!("%{}", default_radix.specifier())
                };
                result.push(IrStmt::Display {
                    fmt: c_quoted(&spec),
                    args: vec![(value.clone(), value.is_real())],
                    newline: false,
                    default_radix,
                });
            }
        }
        if !text.is_empty() {
            result.push(display_text(&text, default_radix));
        }
        if newline {
            result.push(IrStmt::Display {
                fmt: "\"\"".to_owned(),
                args: vec![],
                newline: true,
                default_radix,
            });
        }
        Ok(Some(result))
    }
    fn object_int_argument(
        &mut self,
        path: &str,
        node: NodeId,
        width: u32,
    ) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        ir_to_storage(value, width, true, true)
    }
    pub(super) fn lower_boolean_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        if self.is_chandle_expr(path, node) {
            let equal = object_query(
                IrObjectQuery::ChandleEq(self.lower_chandle(path, node)?, IrChandleExpr::Null),
                1,
                false,
            );
            return Ok(IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::LogNot,
                    a: Box::new(equal),
                },
                1,
                false,
                None,
            ));
        }
        self.lower_expr(path, node)
    }
    pub(super) fn collect_object(&mut self, path: &str, node: NodeId) -> Result<bool, String> {
        let ty = match self.kind(node) {
            NodeKind::Var { ty } => match ty.kind.as_str() {
                "string" => IrObjectType::String,
                "chandle" => IrObjectType::Chandle,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        if self.object_globals.contains_key(&node) {
            return Ok(true);
        }
        let name = self.node(node).name.clone();
        let index = self.model.objects.len();
        self.model.objects.push(IrObject {
            c_name: format!("O_{}_{}", ident(path), ident(&name)),
            ty,
            initial: None,
        });
        self.object_globals.insert(node, index);
        self.scope_object_names
            .entry(path.to_owned())
            .or_default()
            .insert(name, index);
        if let Some(init) = self.db.var_initializer(node) {
            match ty {
                IrObjectType::String => {
                    self.model.objects[index].initial = Some(self.lower_string(path, init)?)
                }
                IrObjectType::Chandle => {
                    if self.lower_chandle(path, init)? != IrChandleExpr::Null {
                        return Err("chandle declaration initializer must be null".to_owned());
                    }
                }
            }
        }
        Ok(true)
    }

    pub(super) fn object_of(&self, path: &str, node: NodeId) -> Option<usize> {
        if let Some((target, _kind, member)) = self.unpacked_member_info(node) {
            if let Some(index) = member.object {
                return Some(self.reference_object(index));
            }
            // A recursive aggregate leaf is still declaration-owned even when
            // its representation is a packed/real signal. Do not fall
            // through to display-name lookup for those values.
            if self.unpacked_aggregates.contains_key(&target) {
                return None;
            }
        }
        if let Some(index) = self.object_globals.get(&node) {
            return Some(self.reference_object(*index));
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            return self
                .object_globals
                .get(target)
                .copied()
                .map(|index| self.reference_object(index));
        }
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { target: None })
        ) {
            return None;
        }
        self.scope_object_names
            .get(path)?
            .get(&self.node(node).name)
            .copied()
            .map(|index| self.reference_object(index))
    }

    pub(super) fn is_string_expr(&self, path: &str, node: NodeId) -> bool {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if self.func.as_ref().is_some_and(|function| {
            target.is_some_and(|target| function.string_read.contains_key(&target))
                || matches!(
                    self.kind(node),
                    NodeKind::Expr(ExprKind::Ref { target: None })
                ) && function
                    .string_read
                    .keys()
                    .any(|target| self.node(*target).name == self.node(node).name)
        }) {
            return true;
        }
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { target: None })
        ) && self.func.as_ref().is_some_and(|function| {
            let name = &self.node(node).name;
            function
                .arg_ir
                .keys()
                .chain(function.locals.keys())
                .any(|target| self.node(*target).name == *name)
                || function.ret.as_ref().is_some_and(|ret| {
                    ret.node
                        .is_some_and(|target| self.node(target).name == *name)
                })
        }) {
            return false;
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            if self
                .resolve_callee_env(self.inst, &self.node(node).name, false, *callee)
                .ok()
                .and_then(|(function, _)| self.func_meta.get(&function))
                .is_some_and(|meta| meta.ret_string)
            {
                return true;
            }
        }
        if self
            .object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::String)
        {
            return true;
        }
        if self.is_container_string_expr(node) {
            return true;
        }
        match self.kind(node) {
            NodeKind::SysCall { name } if name == "$typename" => true,
            NodeKind::Param { ty, .. } => ty.kind == "string",
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => matches!(self.kind(*target),NodeKind::Param{ty,..} if ty.kind=="string"),
            NodeKind::Expr(ExprKind::Cast { ty, .. }) => ty.kind == "string",
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
            } => {
                matches!(name.as_str(), "toupper" | "tolower" | "substr")
                    && self.is_string_expr(path, *receiver)
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat | Operation::MultiConcat,
                operands,
                ..
            }) => operands
                .iter()
                .any(|operand| self.is_string_expr(path, *operand)),
            _ => false,
        }
    }

    /// Return whether an expression is a native chandle without requiring it
    /// to be backed by a model-global object. Function formals, automatic
    /// locals, and chandle-returning calls all live in the function context.
    pub(super) fn is_chandle_expr(&self, path: &str, node: NodeId) -> bool {
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if ty.kind == "chandle" {
                return true;
            }
            if self.is_chandle_expr(path, *operand) {
                return false;
            }
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some(function) = &self.func {
            if target.is_some_and(|target| function.chandle_read.contains_key(&target)) {
                return true;
            }
            if matches!(self.kind(node), NodeKind::Expr(ExprKind::Ref { target: None }))
                && function
                    .chandle_read
                    .keys()
                    .any(|target| self.node(*target).name == self.node(node).name)
            {
                return true;
            }
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::Chandle {
                return true;
            }
        }
        if self.is_container_chandle_expr(node) {
            return true;
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            return self
                .resolve_callee_env(self.inst, &self.node(node).name, false, *callee)
                .ok()
                .and_then(|(function, _)| self.func_meta.get(&function))
                .is_some_and(|meta| meta.ret_chandle);
        }
        false
    }

    /// Resolve a chandle lvalue to its native pointer identity and its
    /// caller-owned pointer slot. The address is never an integer encoding.
    pub(super) fn lower_chandle_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(ChandleTarget, IrChandleExpr), String> {
        let read = self.lower_chandle(path, node)?;
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        let mapped = self.func.as_ref().and_then(|function| {
            target
                .and_then(|target| function.chandle_write.get(&target).cloned())
                .or_else(|| {
                    matches!(self.kind(node), NodeKind::Expr(ExprKind::Ref { target: None }))
                        .then(|| {
                            function
                                .chandle_write
                                .iter()
                                .find(|(target, _)| {
                                    self.node(**target).name == self.node(node).name
                                })
                                .map(|(_, target)| target.clone())
                        })
                        .flatten()
                })
        });
        if let Some(target) = mapped {
            return Ok((target, read));
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::Chandle {
                return Ok((ChandleTarget::Object(index), read));
            }
        }
        Err("chandle output/ref actual must be a named chandle lvalue".to_owned())
    }

    pub(super) fn chandle_target_address(&self, target: &ChandleTarget) -> String {
        match target {
            ChandleTarget::Object(index) => format!("&{}", self.model.objects[*index].c_name),
            ChandleTarget::Local(name) if name.starts_with('*') => format!("&({name})"),
            ChandleTarget::Local(name) => format!("&{name}"),
        }
    }

    pub(super) fn lower_chandle(&mut self, path: &str, node: NodeId) -> Result<IrChandleExpr, String> {
        if let Some(value) = self.lower_container_chandle_query(path, node)? {
            return Ok(value);
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if ty.kind == "chandle" {
                return self.lower_chandle(path, *operand);
            }
        }
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) {
            return Ok(IrChandleExpr::Null);
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.chandle_read.get(&target)) {
                return Ok(value.clone());
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .chandle_read
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok(value.clone());
                }
            }
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            let (ft, _callee_inst) =
                self.resolve_callee_env(self.inst, &self.node(node).name, false, *callee)?;
            let meta = self
                .func_meta
                .get(&ft)
                .cloned()
                .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
            if !meta.ret_chandle {
                return Err(format!(
                    "function `{}` does not return chandle",
                    self.node(ft).name
                ));
            }
            let args = self.node(node).children.clone();
            let bound = self.bind_call_args(self.inst, &meta.formals, &args)?;
            let mut out_args = Vec::new();
            let mut in_args = Vec::new();
            let mut arg_codes = vec![None; meta.formals.len()];
            let mut arg_irs = vec![None; meta.formals.len()];
            for (idx, (formal, is_out)) in meta.formals.iter().enumerate() {
                let is_chandle = matches!(
                    self.kind(*formal),
                    NodeKind::FuncArg { ty, .. } if ty.kind == "chandle"
                );
                let is_ref = matches!(
                    self.kind(*formal),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if is_ref || *is_out {
                    if !is_chandle {
                        return Err(format!(
                            "chandle function `{}` does not support packed output/ref formals",
                            self.node(ft).name
                        ));
                    }
                    let (target, _) = self.lower_chandle_lvalue(path, bound[idx].expr)?;
                    let address = self.chandle_target_address(&target);
                    if is_ref {
                        out_args.push(IrCallArg::ChandleRefAddr(address));
                    } else {
                        out_args.push(IrCallArg::ChandleAddr(address));
                    }
                } else if is_chandle {
                    in_args.push(IrCallArg::ChandleVal(
                        self.lower_chandle(path, bound[idx].expr)?,
                    ));
                } else {
                    let (_, value) = self.lower_bound_arg_code(
                        path,
                        &meta.formals,
                        &bound,
                        idx,
                        &mut arg_codes,
                        &mut arg_irs,
                    )?;
                    in_args.push(IrCallArg::Val(value));
                }
            }
            out_args.extend(in_args);
            return Ok(IrChandleExpr::Call {
                function: meta.ir,
                args: out_args,
                depth: parse_depth(&self.depth_arg),
            });
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::Chandle {
                return Ok(IrChandleExpr::Read(index));
            }
        }
        Err("chandle values can only be copied from chandle or null".to_owned())
    }

    pub(super) fn lower_string(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStringExpr, String> {
        if let Some(value) = self.lower_container_string_query(path, node)? {
            return Ok(value);
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
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
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            let (ft, _) =
                self.resolve_callee_env(self.inst, &self.node(node).name, false, *callee)?;
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
            let actuals = self.node(node).children.clone();
            let bound = self.bind_call_args(self.inst, &formals, &actuals)?;
            let mut arg_codes = vec![None; formals.len()];
            let mut arg_irs = vec![None; formals.len()];
            let mut out_args = Vec::new();
            let mut in_args = Vec::new();
            for (idx, (io, is_out)) in formals.iter().enumerate() {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg { direction: DbDirection::Ref, .. }
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
                        NodeKind::FuncArg { const_ref: true, .. }
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
                        NodeKind::FuncArg { direction: DbDirection::Inout, .. }
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
                    in_args.push(IrCallArg::StringVal(self.lower_string(path, bound[idx].expr)?));
                } else {
                    let (_, arg) = self.lower_bound_arg_code(
                        path,
                        &formals,
                        &bound,
                        idx,
                        &mut arg_codes,
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
                    function: meta.ir,
                    args,
                    depth: parse_depth(&self.depth_arg),
                });
            }
            return Ok(IrStringExpr::TypedCall {
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
            NodeKind::MethodCall { name, receiver: Some(receiver) } => {
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

    pub(super) fn lower_object_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let query = match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::LogicalNot
                    && operands.len() == 1
                    && self.is_chandle_expr(path, operands[0]) =>
            {
                (
                    IrObjectQuery::ChandleEq(
                        self.lower_chandle(path, operands[0])?,
                        IrChandleExpr::Null,
                    ),
                    1,
                    false,
                )
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index })
                if self.is_string_expr(path, *base) =>
            {
                let (base, index) = (*base, *index);
                (
                    IrObjectQuery::StringGetc(
                        self.lower_string(path, base)?,
                        Box::new(self.object_int_argument(path, index, 32)?),
                    ),
                    8,
                    true,
                )
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. })
                if self.is_string_expr(path, *operand) && ty.kind != "string" =>
            {
                let operand = *operand;
                let ty = ty.clone();
                let width = ty
                    .width
                    .ok_or("string cast requires resolved integral target width")?;
                (
                    IrObjectQuery::StringPacked(self.lower_string(path, operand)?),
                    width,
                    ty.signed,
                )
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
            } if self.is_string_expr(path, *receiver) => {
                let name = name.clone();
                let receiver = *receiver;
                let args = self.node(node).children[1..].to_vec();
                let value = self.lower_string(path, receiver)?;
                match (name.as_str(), args.as_slice()) {
                    ("len", []) => (IrObjectQuery::StringLen(value), 32, true),
                    ("atoreal", []) => (IrObjectQuery::StringAtoreal(value), REAL_EXPR_WIDTH, true),
                    ("getc", [index]) => (
                        IrObjectQuery::StringGetc(
                            value,
                            Box::new(self.object_int_argument(path, *index, 32)?),
                        ),
                        8,
                        true,
                    ),
                    ("compare" | "icompare", [other]) => (
                        IrObjectQuery::StringCompare(
                            value,
                            self.lower_string(path, *other)?,
                            name == "icompare",
                        ),
                        32,
                        true,
                    ),
                    ("atoi" | "atohex" | "atooct" | "atobin", []) => (
                        IrObjectQuery::StringAtoi(
                            value,
                            match name.as_str() {
                                "atohex" => 16,
                                "atooct" => 8,
                                "atobin" => 2,
                                _ => 10,
                            },
                        ),
                        32,
                        true,
                    ),
                    _ => {
                        return Err(format!(
                            "unsupported string value method or argument count: {name}"
                        ))
                    }
                }
            }
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if operands.len() == 2 && *op != Operation::Inside =>
            {
                let op = *op;
                let (a, b) = (operands[0], operands[1]);
                let is_chandle = [a, b]
                    .iter()
                    .any(|node| self.is_chandle_expr(path, *node));
                if is_chandle {
                    if matches!(op, Operation::LogicalAnd | Operation::LogicalOr) {
                        let a = self.lower_boolean_expr(path, a)?;
                        let b = self.lower_boolean_expr(path, b)?;
                        return Ok(Some(IrExpr::new(
                            IrExprKind::Bin {
                                op: if op == Operation::LogicalAnd {
                                    IrBinOp::LogAnd
                                } else {
                                    IrBinOp::LogOr
                                },
                                a: Box::new(a),
                                b: Box::new(b),
                            },
                            1,
                            false,
                            None,
                        )));
                    }
                    if !matches!(
                        op,
                        Operation::Equal
                            | Operation::NotEqual
                            | Operation::CaseEqual
                            | Operation::CaseNotEqual
                    ) {
                        return Err("operator is not valid for chandle".to_owned());
                    }
                    let value = object_query(
                        IrObjectQuery::ChandleEq(
                            self.lower_chandle(path, a)?,
                            self.lower_chandle(path, b)?,
                        ),
                        1,
                        false,
                    );
                    return Ok(Some(
                        if matches!(op, Operation::NotEqual | Operation::CaseNotEqual) {
                            IrExpr::new(
                                IrExprKind::Un {
                                    op: IrUnOp::LogNot,
                                    a: Box::new(value),
                                },
                                1,
                                false,
                                None,
                            )
                        } else {
                            value
                        },
                    ));
                }
                if !self.is_string_expr(path, a) && !self.is_string_expr(path, b) {
                    return Ok(None);
                }
                let bin = match op {
                    Operation::Equal => IrBinOp::Eq,
                    Operation::NotEqual => IrBinOp::Neq,
                    Operation::Less => IrBinOp::Lt,
                    Operation::LessEqual => IrBinOp::Le,
                    Operation::Greater => IrBinOp::Gt,
                    Operation::GreaterEqual => IrBinOp::Ge,
                    _ => return Err("operator is not valid for string".to_owned()),
                };
                let compare = object_query(
                    IrObjectQuery::StringCompare(
                        self.lower_string(path, a)?,
                        self.lower_string(path, b)?,
                        false,
                    ),
                    32,
                    true,
                );
                let zero = IrExpr::new(
                    IrExprKind::Const(
                        IrConst::packed(vec![0], vec![], vec![], 32, true, None)
                            .map_err(|e| e.to_string())?,
                    ),
                    32,
                    true,
                    None,
                );
                return Ok(Some(IrExpr::new(
                    IrExprKind::Bin {
                        op: bin,
                        a: Box::new(compare),
                        b: Box::new(zero),
                    },
                    1,
                    false,
                    None,
                )));
            }
            _ => return Ok(None),
        };
        Ok(Some(object_query(query.0, query.1, query.2)))
    }

    pub(super) fn lower_object_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let indexed = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => Some((*base, *index)),
            _ => None,
        };
        let object_node = indexed.map_or(lhs, |(base, _)| base);
        let target_node = match self.kind(object_node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(object_node),
        };
        let chandle_target = self.func.as_ref().and_then(|function| {
            target_node
                .and_then(|target| function.chandle_write.get(&target).cloned())
                .or_else(|| {
                    matches!(
                        self.kind(object_node),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .chandle_write
                            .iter()
                            .find(|(target, _)| {
                                self.node(**target).name == self.node(object_node).name
                            })
                            .map(|(_, target)| target.clone())
                    })
                    .flatten()
                })
        });
        let string_target = self.func.as_ref().and_then(|function| {
            target_node
                .and_then(|target| function.string_write.get(&target).cloned())
                .or_else(|| {
                    matches!(
                        self.kind(object_node),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .string_write
                            .iter()
                            .find(|(target, _)| {
                                self.node(**target).name == self.node(object_node).name
                            })
                            .map(|(_, target)| target.clone())
                    })
                    .flatten()
                })
        });
        let string_const_ref = self.func.as_ref().is_some_and(|function| {
            target_node.is_some_and(|target| {
                function.string_read.contains_key(&target)
                    && !function.string_write.contains_key(&target)
            })
        });
        let index = self.object_of(path, object_node);
        if string_const_ref && string_target.is_none() {
            return Err("cannot mutate a const-ref string formal".to_owned());
        }
        if index.is_none() && chandle_target.is_none() && string_target.is_none() {
            return Ok(None);
        }
        if !blocking {
            return Err(
                "nonblocking assignment to dynamic string/chandle storage is not supported"
                    .to_owned(),
            );
        }
        if op != Operation::Assignment {
            return Err("compound assignment to non-integral storage is unsupported".to_owned());
        }
        if let Some(target) = chandle_target {
            if indexed.is_some() {
                return Err("chandle cannot be indexed".to_owned());
            }
            let value = self.lower_chandle(path, rhs)?;
            return Ok(Some(IrStmt::Object(match target {
                ChandleTarget::Object(index) => IrObjectStmt::ChandleAssign(index, value),
                ChandleTarget::Local(name) => IrObjectStmt::ChandleAssignLocal(name, value),
            })));
        }
        if let Some(target) = string_target {
            if let Some((_, position)) = indexed {
                return Ok(Some(IrStmt::Object(IrObjectStmt::StringPutcLocal(
                    target,
                    self.object_int_argument(path, position, 32)?,
                    self.object_int_argument(path, rhs, 8)?,
                ))));
            }
            return Ok(Some(IrStmt::Object(IrObjectStmt::StringAssignLocal(
                target,
                self.lower_string(path, rhs)?,
            ))));
        }
        let index = index.expect("object target checked above");
        let operation = match self.model.objects[index].ty {
            IrObjectType::String => match indexed {
                Some((_, position)) => IrObjectStmt::StringPutc(
                    index,
                    self.object_int_argument(path, position, 32)?,
                    self.object_int_argument(path, rhs, 8)?,
                ),
                None => IrObjectStmt::StringAssign(index, self.lower_string(path, rhs)?),
            },
            IrObjectType::Chandle => {
                if indexed.is_some() {
                    return Err("chandle cannot be indexed".to_owned());
                }
                IrObjectStmt::ChandleAssign(index, self.lower_chandle(path, rhs)?)
            }
        };
        Ok(Some(IrStmt::Object(operation)))
    }

    pub(super) fn lower_object_method(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStmt, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
            } => (name.clone(), *receiver),
            _ => return Err("object method has no receiver".to_owned()),
        };
        let index = self.object_of(path, receiver);
        let local = if index.is_none() {
            let target = match self.kind(receiver) {
                NodeKind::Expr(ExprKind::Ref { target: Some(target) }) => Some(*target),
                _ => Some(receiver),
            };
            self.func.as_ref().and_then(|function| {
                target
                    .and_then(|target| function.string_write.get(&target).cloned())
                    .or_else(|| {
                        function
                            .string_write
                            .iter()
                            .find(|(target, _)| self.node(**target).name == self.node(receiver).name)
                            .map(|(_, value)| value.clone())
                    })
            })
        } else {
            None
        };
        if index.is_none() && local.is_none() {
            let const_ref = self.func.as_ref().is_some_and(|function| {
                let target = match self.kind(receiver) {
                    NodeKind::Expr(ExprKind::Ref { target: Some(target) }) => Some(*target),
                    _ => Some(receiver),
                };
                target.is_some_and(|target| {
                    function.string_read.contains_key(&target)
                        && !function.string_write.contains_key(&target)
                })
            });
            if const_ref {
                return Err("cannot mutate a const-ref string formal".to_owned());
            }
            return Err("unsupported object method receiver".to_owned());
        }
        if let Some(index) = index {
            if self.model.objects[index].ty != IrObjectType::String {
                return Err("chandle has no built-in methods".to_owned());
            }
        }
        let args = self.node(node).children[1..].to_vec();
        let operation = match (name.as_str(), args.as_slice()) {
            ("realtoa", [value]) => {
                let value = self.lower_expr(path, *value)?;
                let value = if value.is_real() {
                    value
                } else {
                    IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(value),
                            shortreal: false,
                        },
                        REAL_EXPR_WIDTH,
                        true,
                        None,
                    )
                };
                match index {
                    Some(index) => IrObjectStmt::StringRealtoa(index, value),
                    None => IrObjectStmt::StringRealtoaLocal(local.clone().unwrap(), value),
                }
            }
            ("putc", [position, value]) => match index {
                Some(index) => IrObjectStmt::StringPutc(
                    index,
                    self.object_int_argument(path, *position, 32)?,
                    self.object_int_argument(path, *value, 8)?,
                ),
                None => IrObjectStmt::StringPutcLocal(
                    local.clone().unwrap(),
                    self.object_int_argument(path, *position, 32)?,
                    self.object_int_argument(path, *value, 8)?,
                ),
            },
            ("itoa" | "hextoa" | "octtoa" | "bintoa", [value]) => {
                let value = ir_to_storage(self.lower_expr(path, *value)?, 32, true, false)?;
                let base = match name.as_str() {
                    "hextoa" => 16,
                    "octtoa" => 8,
                    "bintoa" => 2,
                    _ => 10,
                };
                match index {
                    Some(index) => IrObjectStmt::StringItoa(index, value, base),
                    None => IrObjectStmt::StringItoaLocal(local.clone().unwrap(), value, base),
                }
            }
            _ => {
                return Err(format!(
                    "unsupported string statement method or argument count: {name}"
                ))
            }
        };
        Ok(IrStmt::Object(operation))
    }
}

pub(super) fn object_query(query: IrObjectQuery, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::ObjectQuery(Box::new(query)),
        width,
        signed,
        None,
    )
}

fn c_quoted(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .chars()
            .map(|byte| format!("\\{:03o}", u32::from(byte)))
            .collect::<String>()
    )
}
fn display_text(value: &str, default_radix: IrDisplayRadix) -> IrStmt {
    IrStmt::Display {
        fmt: c_quoted(&value.replace('%', "%%")),
        args: vec![],
        newline: false,
        default_radix,
    }
}
