//! Formatting.

use super::*;

impl EmitCtx<'_, '_> {
    /// Collect the signal storage that can trigger a monitor. The format
    /// string is display metadata, and symbolic time queries have no signal
    /// reads, so only value arguments contribute to this list.
    pub(super) fn collect_monitor_reads(
        &self,
        args: &[NodeId],
    ) -> Result<Vec<crate::sim::ir::IrDependency>, String> {
        let mut fmt_seen = false;
        let mut reads = Vec::new();
        let mut seen = HashSet::new();
        for arg in args {
            let is_fmt = matches!(
                self.cg.kind(*arg),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::String,
                    ..
                })
            );
            if is_fmt && !fmt_seen {
                fmt_seen = true;
                continue;
            }
            // Native strings publish a stable dependency marker rather than
            // an sv4 value.  Keep that marker in a monitor's trigger set so
            // changing a string argument causes the deferred formatter to
            // re-evaluate it, just like a packed or real argument.
            if let Some(object) = self.cg.object_of(&self.path, *arg) {
                let dependency = crate::sim::ir::IrDependency::Object(object);
                if seen.insert(dependency.clone()) {
                    reads.push(dependency);
                }
                continue;
            }
            if self.cg.is_string_expr(&self.path, *arg) {
                continue;
            }
            for read in self.cg.collect_read_signals(&self.path, *arg)? {
                if seen.insert(read.clone()) {
                    reads.push(read);
                }
            }
        }
        Ok(reads)
    }

    /// Parse a display-family call into a C format string and typed values.
    /// Values remain packed, real, or owned strings until the shared runtime
    /// formatter consumes them. A literal first string is the format; when it
    /// is absent each argument gets its family default conversion.
    pub(super) fn parse_display_call(
        &mut self,
        name: &str,
        args: &[NodeId],
        default_radix: IrDisplayRadix,
    ) -> Result<(String, Vec<crate::sim::ir::IrDisplayArg>), String> {
        let mut fmt_arg: Option<String> = None;
        let mut display_args = Vec::new();
        for a in args {
            let is_fmt = self.literal_string(*a, name)?.is_some() && fmt_arg.is_none();
            if is_fmt && fmt_arg.is_none() {
                fmt_arg = self.literal_string(*a, name)?;
            } else {
                display_args.push(self.lower_display_arg(*a)?);
            }
        }
        let Some(fmt) = fmt_arg else {
            let mut c_fmt = String::from("\"");
            for arg in &display_args {
                c_fmt.push('%');
                c_fmt.push(match arg {
                    crate::sim::ir::IrDisplayArg::Real(_) => 'f',
                    crate::sim::ir::IrDisplayArg::String(_) => 's',
                    crate::sim::ir::IrDisplayArg::Packed(_) => default_radix.specifier(),
                });
            }
            c_fmt.push('"');
            return Ok((c_fmt, display_args));
        };
        let mut c_fmt = String::from("\"");
        let mut arg_idx = 0usize;
        let mut chars = fmt.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                c_fmt.push_str(&escaped_char(ch));
                continue;
            }
            let mut spec = String::from("%");
            while let Some(&n) = chars.peek() {
                // Slang's SystemVerilog formatter grammar admits only the
                // left-justify and zero-pad flags.  Other C printf flags are
                // not display-task syntax and are rejected by the frontend.
                if matches!(n, '-' | '0' | '.') || n.is_ascii_digit() {
                    spec.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            let Some(conv) = chars.next() else {
                return Err(format!(
                    "incomplete {name} format at end of `{}`",
                    self.path
                ));
            };
            spec.push(conv);
            let conversion = conv.to_ascii_lowercase();
            match conversion {
                'd' | 'h' | 'x' | 'b' | 'o' | 'c' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(
                        &display_args[arg_idx],
                        crate::sim::ir::IrDisplayArg::Packed(_)
                            | crate::sim::ir::IrDisplayArg::String(_)
                    ) {
                        return Err(format!(
                            "{name} integer format `%{conv}` requires a packed or string argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    // Keep all legal width/precision/flag text.  The old
                    // lowering retained it only for strings/reals, which
                    // silently changed integer formatting before the runtime
                    // ever saw the directive.
                    c_fmt.push_str(&spec[..spec.len() - conv.len_utf8()]);
                    c_fmt.push(conversion);
                }
                'u' | 'z' | 'v' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(
                        &display_args[arg_idx],
                        crate::sim::ir::IrDisplayArg::Packed(_)
                    ) {
                        return Err(format!(
                            "{name} format `%{conv}` requires a packed argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec[..spec.len() - conv.len_utf8()]);
                    c_fmt.push(conversion);
                }
                's' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(
                        &display_args[arg_idx],
                        crate::sim::ir::IrDisplayArg::String(_)
                    ) {
                        return Err(format!(
                            "{name} format `%s` requires a string argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec[..spec.len() - conv.len_utf8()]);
                    c_fmt.push(conversion);
                }
                'f' | 'e' | 'g' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(
                        &display_args[arg_idx],
                        crate::sim::ir::IrDisplayArg::Real(_)
                    ) {
                        return Err(format!(
                            "{name} real format `%{conv}` requires a real argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec[..spec.len() - conv.len_utf8()]);
                    c_fmt.push(conversion);
                }
                't' => {
                    // %t consumes an integral or real time value.  The runtime
                    // applies the design-wide `$timeformat` conversion using
                    // the owning scope's unit metadata.
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(
                        &display_args[arg_idx],
                        crate::sim::ir::IrDisplayArg::Packed(_)
                            | crate::sim::ir::IrDisplayArg::Real(_)
                    ) {
                        return Err(format!(
                            "{name} format `%t` requires a packed or real argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec[..spec.len() - conv.len_utf8()]);
                    c_fmt.push(conversion);
                }
                'p' => {
                    // Pattern formatting for unpacked aggregates is still
                    // outside the owned IR, but scalar packed and native
                    // string values can use the runtime's textual pattern
                    // representation.
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(
                        &display_args[arg_idx],
                        crate::sim::ir::IrDisplayArg::Packed(_)
                            | crate::sim::ir::IrDisplayArg::String(_)
                    ) {
                        return Err(format!(
                            "{name} pattern format `%{conv}` requires a packed or string argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec[..spec.len() - conv.len_utf8()]);
                    c_fmt.push(conversion);
                }
                'm' | 'l' => {
                    // `%m` and `%l` are scope/library queries and consume no
                    // value argument.
                    c_fmt.push_str(&spec[..spec.len() - conv.len_utf8()]);
                    c_fmt.push(conversion);
                }
                '%' => c_fmt.push_str(&spec),
                other => {
                    return Err(format!(
                        "unsupported {name} format specifier `%{other}` in `{}`",
                        self.path
                    ))
                }
            }
        }
        while arg_idx < display_args.len() {
            c_fmt.push('%');
            c_fmt.push(match &display_args[arg_idx] {
                crate::sim::ir::IrDisplayArg::Real(_) => 'f',
                crate::sim::ir::IrDisplayArg::String(_) => 's',
                crate::sim::ir::IrDisplayArg::Packed(_) => default_radix.specifier(),
            });
            arg_idx += 1;
        }
        c_fmt.push('"');
        Ok((c_fmt, display_args))
    }

    /// Lower a string-producing formatter used by `$swrite*`.  Its argument
    /// list follows the display-task convention: literal strings are parsed
    /// as format segments, while unformatted values use the task's radix
    /// default.
    pub(super) fn lower_string_output_format(
        &mut self,
        name: &str,
        args: &[NodeId],
        default_radix: IrDisplayRadix,
    ) -> Result<IrStringExpr, String> {
        let lowered = args
            .iter()
            .map(|value| self.lower_display_arg(*value))
            .collect::<Result<Vec<_>, _>>()?;
        let mut format = String::new();
        let mut values = Vec::new();
        let mut source_idx = 0usize;
        while source_idx < args.len() {
            if let Some(text) = self.literal_string(args[source_idx], name)? {
                let remaining = &lowered[source_idx + 1..];
                let (segment, consumed) =
                    self.parse_format_text(name, &text, remaining, default_radix, false)?;
                format.push_str(&segment);
                values.extend(remaining.iter().take(consumed).cloned());
                source_idx += consumed + 1;
            } else {
                let value = &lowered[source_idx];
                format.push('%');
                format.push(match value {
                    crate::sim::ir::IrDisplayArg::Real(_) => 'f',
                    crate::sim::ir::IrDisplayArg::String(_) => 's',
                    crate::sim::ir::IrDisplayArg::Packed(_) => default_radix.specifier(),
                });
                values.push(value.clone());
                source_idx += 1;
            }
        }
        Ok(IrStringExpr::Format {
            format: Box::new(IrStringExpr::Literal(format.into_bytes())),
            args: values,
            scope: self.path.clone(),
        })
    }

    /// Lower `$sformat`'s explicit format argument.  Dynamic format strings
    /// remain dynamic and are interpreted by the same runtime formatter as
    /// `$sformatf`; literal formats are checked against their typed values at
    /// lowering time and retain any extra display values in source order.
    pub(super) fn lower_explicit_string_format(
        &mut self,
        name: &str,
        format_node: NodeId,
        args: &[NodeId],
    ) -> Result<IrStringExpr, String> {
        let values = args
            .iter()
            .map(|value| self.lower_display_arg(*value))
            .collect::<Result<Vec<_>, _>>()?;
        let format = if let Some(text) = self.literal_string(format_node, name)? {
            IrStringExpr::Literal(
                self.parse_format_text(name, &text, &values, IrDisplayRadix::Decimal, true)?
                    .0
                    .into_bytes(),
            )
        } else {
            self.cg.lower_string(&self.path, format_node)?
        };
        Ok(IrStringExpr::Format {
            format: Box::new(format),
            args: values,
            scope: self.path.clone(),
        })
    }

    /// Assign a formatted string to either native string storage or a packed
    /// string-like lvalue.  Packed destinations use the existing object query
    /// conversion, so truncation and zero padding follow normal assignment
    /// width rules for every destination width.
    pub(super) fn lower_string_format_target(
        &mut self,
        target: NodeId,
        value: IrStringExpr,
    ) -> Result<IrStmt, String> {
        let target = match self.cg.kind(target) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if !operands.is_empty() => operands[0],
            _ => target,
        };
        if self.cg.is_string_expr(&self.path, target) {
            self.cg.ensure_string_actual_writable(&self.path, target)?;
            let target = self
                .cg
                .lower_string_actual_address(&self.path, target)?
                .trim_start_matches('&')
                .to_owned();
            return Ok(IrStmt::Object(IrObjectStmt::StringAssignLocal(
                target, value,
            )));
        }
        let lhs = self.cg.lower_lhs(&self.path, target).map_err(|error| {
            format!(
                "string formatting destination has unsupported LHS in `{}`: {error} (target kind: {:?})",
                self.path,
                // The frontend's string-like destination may be a packed
                // expression wrapper; retain its owned node kind in the
                // lowering diagnostic when that wrapper is unsupported.
                self.cg.kind(target)
            )
        })?;
        let (width, signed, _two_state, const_ref) =
            self.cg.ref_lhs_type(&lhs).ok_or_else(|| {
                format!(
                    "string formatting destination is not packed storage in `{}`",
                    self.path
                )
            })?;
        if const_ref {
            return Err(format!(
                "string formatting destination cannot be a const ref in `{}`",
                self.path
            ));
        }
        let rhs = object_query(IrObjectQuery::StringPacked(value), width, signed);
        Ok(IrStmt::Assign {
            lhs: lhs.clone(),
            rhs: apply_lhs_assignment_context(&self.cg.model, &lhs, rhs),
            nba: false,
        })
    }

    /// Validate a literal format and return the normalized formatter text
    /// consumed by both display-family tasks and string-producing calls.
    fn parse_format_text(
        &self,
        name: &str,
        fmt: &str,
        display_args: &[crate::sim::ir::IrDisplayArg],
        default_radix: IrDisplayRadix,
        append_extras: bool,
    ) -> Result<(String, usize), String> {
        let mut normalized = String::new();
        let mut arg_idx = 0usize;
        let mut chars = fmt.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                normalized.push(ch);
                continue;
            }
            let mut spec = String::from("%");
            while let Some(&next) = chars.peek() {
                if matches!(next, '-' | '0' | '.') || next.is_ascii_digit() {
                    if let Some(next) = chars.next() {
                        spec.push(next);
                    }
                } else {
                    break;
                }
            }
            let Some(conversion) = chars.next() else {
                return Err(format!(
                    "incomplete {name} format at end of `{}`",
                    self.path
                ));
            };
            spec.push(conversion);
            let lower = conversion.to_ascii_lowercase();
            match lower {
                'd' | 'h' | 'x' | 'b' | 'o' | 'c' => {
                    self.require_format_arg(name, conversion, arg_idx, display_args, true)?;
                }
                'u' | 'z' | 'v' | 't' => {
                    self.require_format_arg(name, conversion, arg_idx, display_args, false)?;
                }
                's' => {
                    self.require_format_arg(name, conversion, arg_idx, display_args, true)?;
                    if !matches!(
                        display_args[arg_idx],
                        crate::sim::ir::IrDisplayArg::String(_)
                    ) {
                        return Err(format!(
                            "{name} format `%s` requires a string argument in `{}`",
                            self.path
                        ));
                    }
                }
                'f' | 'e' | 'g' => {
                    if !matches!(
                        display_args.get(arg_idx),
                        Some(crate::sim::ir::IrDisplayArg::Real(_))
                    ) {
                        return Err(format!(
                            "{name} real format `%{conversion}` requires a real argument in `{}`",
                            self.path
                        ));
                    }
                }
                'p' => {
                    self.require_format_arg(name, conversion, arg_idx, display_args, true)?;
                }
                'm' | 'l' | '%' => {}
                other => {
                    return Err(format!(
                        "unsupported {name} format specifier `%{other}` in `{}`",
                        self.path
                    ));
                }
            }
            if !matches!(lower, 'm' | 'l' | '%') {
                arg_idx += 1;
            }
            normalized.push_str(&spec);
        }
        if append_extras {
            while arg_idx < display_args.len() {
                normalized.push('%');
                normalized.push(match &display_args[arg_idx] {
                    crate::sim::ir::IrDisplayArg::Real(_) => 'f',
                    crate::sim::ir::IrDisplayArg::String(_) => 's',
                    crate::sim::ir::IrDisplayArg::Packed(_) => default_radix.specifier(),
                });
                arg_idx += 1;
            }
        }
        Ok((normalized, arg_idx))
    }

    fn require_format_arg(
        &self,
        name: &str,
        conversion: char,
        arg_idx: usize,
        display_args: &[crate::sim::ir::IrDisplayArg],
        allow_string: bool,
    ) -> Result<(), String> {
        let Some(arg) = display_args.get(arg_idx) else {
            return Err(format!(
                "{name} format `%{conversion}` in `{}` has no argument",
                self.path
            ));
        };
        let valid = matches!(arg, crate::sim::ir::IrDisplayArg::Packed(_))
            || (allow_string && matches!(arg, crate::sim::ir::IrDisplayArg::String(_)));
        if !valid {
            return Err(format!(
                "{name} format `%{conversion}` has an incompatible argument in `{}`",
                self.path
            ));
        }
        Ok(())
    }

    fn lower_display_arg(&mut self, node: NodeId) -> Result<crate::sim::ir::IrDisplayArg, String> {
        self.cg.lower_format_arg(&self.path, node)
    }

    /// Recover a source string literal through the implicit string cast that
    /// Slang may insert at a system-task argument boundary.
    pub(super) fn literal_string(
        &self,
        node: NodeId,
        context: &str,
    ) -> Result<Option<String>, String> {
        match self.cg.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                value,
                ..
            }) => decoded_string_text(value, context).map(Some),
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => {
                self.literal_string(*operand, context)
            }
            _ => Ok(None),
        }
    }

    pub(super) fn finish_location(&self, node: NodeId) -> String {
        let source = self.cg.node(node);
        if source.line == 0 {
            return self.path.clone();
        }
        format!("{}:{}:{}", self.path, source.line, source.col)
    }

    /// Lower the optional diagnostic level shared by `$finish` and `$stop`.
    /// Slang's control-task rules require a constant integral 0/1/2 value in
    /// the selected language editions, so the validated level is stored in
    /// the IR rather than evaluated by the generated runtime.
    pub(super) fn lower_control_verbosity(
        &mut self,
        node: NodeId,
        task: &str,
        args: &[NodeId],
    ) -> Result<u8, String> {
        match args {
            [] => Ok(1),
            [argument] => {
                let value = self.cg.eval_bits(*argument).map_err(|error| {
                    format!(
                        "{task} argument at {} must be an integral constant 0, 1, or 2: {error}",
                        self.finish_location(node)
                    )
                })?;
                if value.is_unknown() {
                    return Err(format!(
                        "{task} argument at {} must be a known integral constant 0, 1, or 2",
                        self.finish_location(node)
                    ));
                }
                let value = value.to_u128().ok_or_else(|| {
                    format!(
                        "{task} argument at {} must be an integral constant 0, 1, or 2",
                        self.finish_location(node)
                    )
                })?;
                u8::try_from(value)
                    .ok()
                    .filter(|value| *value <= 2)
                    .ok_or_else(|| {
                        format!(
                            "{task} argument at {} must be 0, 1, or 2 (got {value})",
                            self.finish_location(node)
                        )
                    })
            }
            _ => Err(format!(
                "{task} accepts at most one argument at {}",
                self.finish_location(node)
            )),
        }
    }
}
