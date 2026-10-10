//! Formatting.

use super::*;
use crate::sim::codegen::lowering::objects::c_format_literal;

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

    /// A formal, local or return variable of the subroutine being lowered
    /// that lives in the activation rather than in model storage, if `node`
    /// reads one. Static subroutine variables have persistent storage and are
    /// not reported.
    pub(super) fn activation_bound_ref(&self, node: NodeId) -> Option<NodeId> {
        let function = self.func.as_ref()?;
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.cg.kind(node)
        {
            let target = *target;
            let bound = function.arg_ir.contains_key(&target)
                || function.arg_read.contains_key(&target)
                || function.arg_write.contains_key(&target)
                || function.event_args.contains_key(&target)
                || function.const_refs.contains(&target)
                || function.string_read.contains_key(&target)
                || function.string_write.contains_key(&target)
                || function.string_addr.contains_key(&target)
                || function.chandle_read.contains_key(&target)
                || function.process_read.contains_key(&target)
                || function.locals.contains_key(&target)
                || function.ret_node == Some(target);
            if bound && !function.persistent.contains_key(&target) {
                return Some(target);
            }
        }
        self.cg
            .node(node)
            .children
            .iter()
            .find_map(|child| self.activation_bound_ref(*child))
    }

    /// Parse a display-family call into a C format string and typed values.
    /// Values remain packed, real, or owned strings until the shared runtime
    /// formatter consumes them. Every string-literal argument is a format
    /// segment; other arguments get the family's default conversion.
    pub(super) fn parse_display_call(
        &mut self,
        name: &str,
        args: &[NodeId],
        default_radix: IrDisplayRadix,
    ) -> Result<(String, Vec<crate::sim::ir::IrDisplayArg>), String> {
        let library = self.cg.library_binding(self.inst);
        let (format, values) =
            self.cg
                .lower_format_segments(&self.path, Some(&library), name, args, default_radix)?;
        Ok((c_format_literal(&format), values))
    }

    /// Lower a string-producing formatter used by `$swrite*`.  Its argument
    /// list follows the display-task convention: literal strings are parsed
    /// as format segments, while unformatted values use the task's radix
    /// default.
    pub(super) fn lower_string_output_format(
        &mut self,
        site: NodeId,
        name: &str,
        args: &[NodeId],
        default_radix: IrDisplayRadix,
    ) -> Result<IrStringExpr, String> {
        let library = self.cg.library_binding(self.inst);
        let (format, values) =
            self.cg
                .lower_format_segments(&self.path, Some(&library), name, args, default_radix)?;
        Ok(IrStringExpr::Format {
            format: Box::new(IrStringExpr::Literal(format)),
            args: values,
            scope: self.cg.format_scope(&self.path, site),
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
        let library = self.cg.library_binding(self.inst);
        self.cg
            .lower_explicit_format(&self.path, Some(&library), name, format_node, args)
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
            return Ok(IrStmt::Object(Box::new(IrObjectStmt::StringAssignLocal(
                target, value,
            ))));
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
