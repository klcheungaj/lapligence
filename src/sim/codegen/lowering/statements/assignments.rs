//! Assignments.

use super::*;
use crate::sim::codegen::lowering::collection::native_values::{NativeEndpoint, NativeTaggedRoot};

impl EmitCtx<'_, '_> {
    /// Lower an assignment without intra-assignment delay (`force_blocking`
    /// pins blocking semantics for for-loop init/increment statements).
    pub(super) fn lower_assignment(
        &mut self,
        h: NodeId,
        force_blocking: bool,
    ) -> Result<IrStmt, String> {
        let (blocking, op) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { blocking, op, .. }) => (*blocking, *op),
            _ => unreachable!("non-assignment passed to lower_assignment"),
        };
        if matches!(
            self.cg.kind(h),
            NodeKind::Stmt(StmtKind::Assign { delay: Some(_), .. })
        ) {
            // Reached only from the for-loop init/increment path; plain
            // statement assignments are classified in lower_stmt.
            return Err(format!(
                "intra-assignment delay on a for-loop init/increment \
                 assignment in `{}` is not supported",
                self.path
            ));
        }
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "assignment without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "assignment without RHS".to_string())?;
        self.lower_assignment_operands(lhs, rhs, force_blocking || blocking, op, force_blocking)
    }

    pub(super) fn lower_assignment_operands(
        &mut self,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
        force_blocking: bool,
    ) -> Result<IrStmt, String> {
        // Tagged unions with string, real or handle members (SIM-007): a
        // tagged construction sets the tag and one member; a member write
        // happens only while that member is active.
        // A container element is replaced through a temporary of its type
        // (`lower_container_record_assignment`), which accepts these sources.
        if let Some(root) = self
            .cg
            .native_tagged_root(lhs)
            .filter(|root| !matches!(root, NativeTaggedRoot::Element(_)))
        {
            if let Some(statement) = self
                .cg
                .lower_native_tagged_construct(&self.path, root, rhs, !blocking)?
            {
                if op != Operation::Assignment {
                    return Err(format!(
                        "compound assignment of a tagged union in `{}` is illegal",
                        self.path
                    ));
                }
                return Ok(statement);
            }
            if !blocking && !matches!(root, NativeTaggedRoot::Module(_)) {
                return Err(format!(
                    "nonblocking assignment to a tagged union with string, real or handle members in subroutine storage in `{}` is not supported",
                    self.path
                ));
            }
            if self.cg.native_tagged_root(rhs).is_none()
                && !self.cg.is_container_record(rhs)
                && !self.cg.native_call_node(rhs)
                && !self
                    .cg
                    .native_record_conditional(self.cg.p30_unwrap_cast(rhs))
            {
                return Err(format!(
                    "a tagged union with string, real or handle members in `{}` can only be assigned a tagged expression, another such union variable, a function result or a conditional",
                    self.path
                ));
            }
        }
        if let Some(select) = self.cg.native_member_select(&self.path, lhs)? {
            if !blocking || op != Operation::Assignment {
                return Err(format!(
                    "nonblocking or compound assignment to a run-time selected native record member element in `{}` is not supported",
                    self.path
                ));
            }
            return self.cg.native_member_select_write(&self.path, &select, rhs);
        }
        if self.cg.native_tagged_access(lhs).is_some() {
            // SV 11.9 checks a member write against the tag current when it
            // is performed; for a nonblocking write that is the commit, and
            // queued native writes carry no commit-time tag check.
            if !blocking {
                return Err(format!(
                    "nonblocking assignment to a member of a tagged union with string, real or handle members in `{}` is not supported: the write needs a commit-time tag check",
                    self.path
                ));
            }
            self.cg.native_tagged_bypass.insert(lhs);
            let statement =
                self.lower_assignment_operands_unchecked(lhs, rhs, blocking, op, force_blocking);
            self.cg.native_tagged_bypass.remove(&lhs);
            let statement = statement?;
            return self
                .cg
                .native_tagged_guarded_write(&self.path, lhs, statement);
        }
        self.lower_assignment_operands_unchecked(lhs, rhs, blocking, op, force_blocking)
    }

    fn lower_assignment_operands_unchecked(
        &mut self,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
        force_blocking: bool,
    ) -> Result<IrStmt, String> {
        // A subprogram declaration initializer can be represented as an
        // assignment whose LHS is the variable declaration itself. Its value is
        // emitted through `IrLocal::initial`; suppress only that structural
        // declaration form. Executable assignments have Ref LHS nodes.
        let is_declaration_initializer = matches!(self.cg.kind(lhs), NodeKind::Var { .. })
            && !force_blocking
            && self
                .func
                .as_ref()
                .is_some_and(|function| function.locals.contains_key(&lhs));
        if is_declaration_initializer {
            return Ok(IrStmt::Nop);
        }
        if matches!(
            op,
            Operation::OverloadUpdate | Operation::OverloadPostUpdate
        ) {
            // IEEE 1800-2009 11.11: a target the frontend binds once (its
            // selectors have side effects) is read and written by one
            // mutation; statement position discards its value.
            return self.lower_discarded_mutation(op, &[lhs, rhs]);
        }
        if let Some(statement) = self.lower_native_overload_update_value(lhs, rhs, blocking)? {
            return Ok(statement);
        }
        if let NodeKind::Expr(ExprKind::Operation {
            op: Operation::OverloadUpdate | Operation::OverloadPostUpdate,
            operands,
            ..
        }) = self.cg.kind(rhs)
        {
            // Aggregate and native destinations otherwise misreport the value.
            if let Some(target) = operands.first() {
                self.cg
                    .check_overloaded_update_target(&self.path, *target)?;
            }
        }
        if let Some(target) = self
            .cg
            .event_target_of(lhs)
            .filter(|target| !self.cg.container_globals.contains_key(&target.declaration))
        {
            if op != Operation::Assignment {
                return Err(format!(
                    "compound assignment to named event `{}` in `{}` is not supported",
                    self.cg.node(target.declaration).name,
                    self.path
                ));
            }
            if !blocking {
                return Err(format!(
                    "nonblocking assignment to named event `{}` in `{}` is not supported",
                    self.cg.node(target.declaration).name,
                    self.path
                ));
            }
            let target = self.cg.event_ref_of(&target, &self.path)?;
            let source = if let Some(source) = self.cg.event_target_of(rhs) {
                Some(self.cg.event_ref_of(&source, &self.path)?)
            } else if let Some((container, back)) = self
                .cg
                .queue_pop_call(rhs)
                .filter(|(container, _)| self.cg.model.containers[*container].element.is_event())
            {
                Some(IrEventRef::Handle(Box::new(IrChandleExpr::QueuePop {
                    container,
                    back,
                })))
            } else if self.cg.is_null_event_expression(rhs) {
                None
            } else {
                return Err(format!(
                    "named event assignment in `{}` requires another event handle or null",
                    self.path
                ));
            };
            return Ok(IrStmt::EventAssign { target, source });
        }
        if let Some(statement) = self
            .cg
            .lower_container_assignment(&self.path, lhs, rhs, blocking, op)?
        {
            return Ok(statement);
        }
        if let Some(statement) = self
            .cg
            .lower_object_assignment(&self.path, lhs, rhs, blocking, op)?
        {
            return Ok(statement);
        }
        let mut clocking_targets = Vec::new();
        let all_clocking = self.cg.clocking_lhs_targets(lhs, &mut clocking_targets);
        if !clocking_targets.is_empty() {
            if !all_clocking {
                return Err(format!(
                    "clocking output/inout concatenations cannot mix ordinary targets in `{}`",
                    self.path
                ));
            }
            if blocking {
                return Err(format!(
                    "clocking output/inout member drives in `{}` require nonblocking `<=`",
                    self.path
                ));
            }
            if op != Operation::Assignment {
                return Err(format!(
                    "compound assignment to a clocking output/inout in `{}` is not supported",
                    self.path
                ));
            }
            clocking_targets.sort_unstable_by_key(|target| target.0);
            clocking_targets.dedup();
            for target in &clocking_targets {
                let Some(var) = self.cg.db.clocking_var(*target) else {
                    return Err(format!(
                        "clocking member `{}` has no owned declaration in `{}`",
                        self.cg.node(*target).name,
                        self.path
                    ));
                };
                if matches!(var.direction, DbDirection::Input) {
                    return Err(format!(
                        "clocking input member `{}` is read-only in `{}`",
                        self.cg.node(*target).name,
                        self.path
                    ));
                }
            }
            if let Some(drive) = self.lower_virtual_clocking_drive(lhs, rhs, op, None)? {
                return Ok(drive);
            }
            let lh = self.cg.lower_lhs(&self.path, lhs)?;
            let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
            let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
            let ticks = self
                .cg
                .clocking_output_delay(clocking_targets[0], &self.path)?;
            let drive_specs = self.lower_clocking_drive_specs(clocking_targets[0])?;
            for target in clocking_targets.iter().skip(1) {
                if self.lower_clocking_drive_specs(*target)? != drive_specs {
                    return Err(format!(
                        "clocking concatenation in `{}` uses different clocking events",
                        self.path
                    ));
                }
                let other = self.cg.clocking_output_delay(*target, &self.path)?;
                if other != ticks {
                    return Err(format!(
                        "clocking concatenation in `{}` uses different output skews",
                        self.path
                    ));
                }
            }
            return Ok(IrStmt::ClockingDrive {
                lhs: lh,
                rhs: rhs_ir,
                ticks,
                specs: drive_specs,
                cycles: None,
            });
        }
        if !blocking {
            self.reject_class_property_nba(lhs)?;
            if let Some(local) = self.cg.proc_local_target(lhs) {
                return Err(format!(
                    "nonblocking assignment to inline loop variable `{}` in `{}` is not supported because the update can outlive its lexical storage",
                    self.cg.node(local).name,
                    self.path
                ));
            }
            if self.cg.native_target(lhs) {
                return Err(format!(
                    "nonblocking assignment to native record subroutine storage in `{}` is not supported",
                    self.path
                ));
            }
            if self.cg.subroutine_auto_target(lhs) {
                return Err(format!(
                    "nonblocking assignment to automatic subroutine storage in `{}` is not supported because the update can outlive its activation",
                    self.path
                ));
            }
        }
        if self.in_final && !blocking {
            return Err(format!(
                "nonblocking assignment inside a final block in `{}` is not \
                 allowed (final permits function statements only)",
                self.path
            ));
        }
        if let Some(aggregate_assignment) = self
            .cg
            .lower_unpacked_aggregate_assignment(&self.path, lhs, rhs, !blocking, op)?
        {
            return Ok(aggregate_assignment);
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        if op != Operation::Assignment
            && matches!(
                lh,
                IrLhs::Bit(..)
                    | IrLhs::Part(..)
                    | IrLhs::IdxPart(..)
                    | IrLhs::ArrayElem { .. }
                    | IrLhs::PackedSelect { .. }
                    | IrLhs::Ref { .. }
            )
        {
            // The canonical mutation expression resolves and stores the
            // select once; its yielded value is discarded in statement
            // position. This keeps `mem[i++] += f()` incrementing once.
            return self.lower_discarded_mutation(op, &[lhs, rhs]);
        }
        let rhs_ir = match self
            .cg
            .lower_packed_aggregate_pattern(&self.path, lhs, rhs, op)?
        {
            Some(value) => value,
            None => self.lower_assignment_rhs(lhs, rhs, op, &lh)?,
        };
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        Ok(IrStmt::Assign {
            lhs: lh,
            rhs: rhs_ir,
            nba: !blocking,
        })
    }

    /// `lhs = <overloaded update>` (IEEE 1800-2009 11.11) whose target is a
    /// native record (string, real, handle or container members) with
    /// side-effect-free selectors. Such a target has no packed mutation
    /// capture, so the update runs as `A = f(A, B)` with the target read
    /// again, and `lhs` receives the updated target or, for a postfix
    /// increment or decrement, a copy of the target taken before the update.
    /// Targets whose selectors have side effects keep their packed-only form.
    fn lower_native_overload_update_value(
        &mut self,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
    ) -> Result<Option<IrStmt>, String> {
        let NodeKind::Expr(ExprKind::Operation {
            op: op @ (Operation::OverloadUpdate | Operation::OverloadPostUpdate),
            operands,
            ..
        }) = self.cg.kind(rhs)
        else {
            return Ok(None);
        };
        let post = *op == Operation::OverloadPostUpdate;
        let (Some(&target), Some(&call)) = (operands.first(), operands.get(1)) else {
            return Ok(None);
        };
        if !self.cg.side_effect_free(target) {
            return Ok(None);
        }
        let native = self.cg.native_layout(target)?.is_some()
            && (self.cg.native_endpoint(target)?.is_some() || self.cg.is_container_record(target));
        // A fixed target above the packed value limit is read again the same
        // way; its old value has no temporary, so its postfix value stays
        // with the packed-only form.
        let oversized = !native
            && self
                .cg
                .check_overloaded_update_target(&self.path, target)
                .is_err();
        if !native && (!oversized || post) {
            return Ok(None);
        }
        let mut statements = Vec::with_capacity(4);
        let old = if post {
            let temporary = self.cg.native_temporary(target)?;
            let copy = NativeEndpoint::Value {
                value: temporary,
                prefix: Vec::new(),
            };
            let descriptor = self.cg.query_descriptor(target).cloned().ok_or_else(|| {
                format!("overloaded update target in `{}` has no type", self.path)
            })?;
            statements.push(IrStmt::NativeValueDeclare(temporary));
            statements.push(self.cg.native_assign_into(
                &self.path,
                &copy,
                &descriptor,
                target,
                false,
            )?);
            Some(copy)
        } else {
            None
        };
        self.cg
            .overload_current
            .push(super::super::OverloadCurrent::Target(target));
        let update =
            self.lower_assignment_operands(target, call, true, Operation::Assignment, false);
        self.cg.overload_current.pop();
        statements.push(update?);
        statements.push(match old {
            Some(copy) => {
                let (destination, _) = self.cg.native_endpoint(lhs)?.ok_or_else(|| {
                    format!(
                        "the value of an overloaded postfix update on a native record in `{}` needs a record destination",
                        self.path
                    )
                })?;
                self.cg
                    .native_transfer(&self.path, &destination, &copy, !blocking)?
            }
            None => self.lower_assignment_operands(
                lhs,
                target,
                blocking,
                Operation::Assignment,
                false,
            )?,
        });
        Ok(Some(IrStmt::Block(statements)))
    }

    /// Emit a compound assignment or increment/decrement to a select or array
    /// element in statement position. The mutation expression performs the
    /// one resolved read/modify/store; capturing its discarded result in a
    /// fresh local keeps every selector evaluation at exactly one site.
    fn lower_discarded_mutation(
        &mut self,
        op: Operation,
        operands: &[NodeId],
    ) -> Result<IrStmt, String> {
        let mutation = self
            .cg
            .lower_mutation_expression(&self.path, op, operands, false)?;
        let (width, signed) = (mutation.width, mutation.signed);
        let name = format!("_mut{}", operands[0].0);
        Ok(IrStmt::Block(vec![IrStmt::DeclLocal {
            name,
            width,
            signed,
            two_state: false,
            init: Some(Box::new(mutation)),
        }]))
    }

    /// Lower the value written by a normal or compound procedural
    /// assignment.  Compound assignments currently require a whole scalar
    /// target, which guarantees the LHS is evaluated once; select and array
    /// targets need index temporaries before they can preserve that rule.
    pub(super) fn lower_assignment_rhs(
        &mut self,
        lhs_node: NodeId,
        rhs_node: NodeId,
        op: Operation,
        lhs: &IrLhs,
    ) -> Result<IrExpr, String> {
        let rhs = self.cg.lower_expr(&self.path, rhs_node)?;
        if op == Operation::Assignment {
            return Ok(rhs);
        }
        if !matches!(lhs, IrLhs::Whole(_) | IrLhs::WholeRef { .. }) {
            return Err(format!(
                "compound assignment to a select or array element in `{}` is not supported yet",
                self.path
            ));
        }
        let current = self.cg.lower_expr(&self.path, lhs_node)?;
        self.lower_compound_expr(op, current, rhs)
    }

    fn lower_compound_expr(
        &self,
        op: Operation,
        lhs: IrExpr,
        rhs: IrExpr,
    ) -> Result<IrExpr, String> {
        super::super::lower_compound_expr_ir(&self.path, op, lhs, rhs)
    }

    /// Lower a statement-position pre/post increment or decrement.  Since
    /// the operation's value is discarded in statement position, pre and
    /// post forms have the same blocking-write behavior.  Expression-valued
    /// forms require a side-effecting expression IR and remain unsupported.
    pub(super) fn lower_inc_dec(
        &mut self,
        op: Operation,
        operands: &[NodeId],
    ) -> Result<IrStmt, String> {
        let operand = match operands {
            [operand] => *operand,
            _ => {
                return Err(format!(
                    "increment/decrement in `{}` must have exactly one operand",
                    self.path
                ))
            }
        };
        if self.cg.is_container_element(operand) {
            let increment = matches!(op, Operation::PostIncrement | Operation::PreIncrement);
            return self.cg.lower_container_element_update(
                &self.path,
                operand,
                &mut |_, current| Ok(inc_dec_value(current, increment)),
            );
        }
        let lhs = self.cg.lower_lhs(&self.path, operand)?;
        if matches!(
            lhs,
            IrLhs::Bit(..)
                | IrLhs::Part(..)
                | IrLhs::IdxPart(..)
                | IrLhs::ArrayElem { .. }
                | IrLhs::PackedSelect { .. }
                | IrLhs::Ref { .. }
        ) {
            // Statement-position pre/post increment discards the yielded value,
            // so both forms commit the same single resolved store.
            return self.lower_discarded_mutation(op, operands);
        }
        if !matches!(
            lhs,
            IrLhs::Whole(_) | IrLhs::WholeRef { .. } | IrLhs::Ref { bit: None, .. }
        ) {
            return Err(format!(
                "increment/decrement of a select or array element in `{}` is not supported yet",
                self.path
            ));
        }
        let current = self.cg.lower_expr(&self.path, operand)?;
        let increment = matches!(op, Operation::PostIncrement | Operation::PreIncrement);
        let rhs = inc_dec_value(current, increment);
        let rhs = apply_lhs_assignment_context(&self.cg.model, &lhs, rhs);
        Ok(IrStmt::Assign {
            lhs,
            rhs,
            nba: false,
        })
    }

    /// Capture a delayed assignment's RHS immediately. Blocking assignments
    /// suspend; nonblocking assignments enqueue a future NBA and continue.
    /// Reject a nonblocking write to an instance class property. SV 6.21:
    /// "members or elements of dynamic variables—class properties and
    /// dynamically sized variables—shall not be written with nonblocking,
    /// continuous, or procedural continuous assignments." The frontend
    /// already rejects dynamically sized elements; static properties are not
    /// members of a dynamic object and stay legal.
    pub(super) fn reject_class_property_nba(&self, lhs: NodeId) -> Result<(), String> {
        let mut node = lhs;
        while let NodeKind::Expr(
            ExprKind::BitSelect { base, .. }
            | ExprKind::PartSelect { base, .. }
            | ExprKind::IndexedPartSelect { base, .. }
            | ExprKind::ArraySelect { base, .. },
        ) = self.cg.kind(node)
        {
            node = *base;
        }
        let Some(field) = self
            .cg
            .class_field_target(node)
            .filter(|field| self.cg.class_fields.contains_key(field))
        else {
            return Ok(());
        };
        Err(format!(
            "nonblocking assignment to class property `{}` at {} in `{}` is illegal \
             (IEEE 1800-2009 6.21: class properties shall not be written with nonblocking assignments)",
            self.cg.node(field).name,
            self.cg.source_location(lhs),
            self.path
        ))
    }

    pub(super) fn lower_delayed_assignment(
        &mut self,
        h: NodeId,
        blocking: bool,
        scaled_ticks: IrDelay,
    ) -> Result<Vec<IrStmt>, String> {
        if self.timing_forbidden() {
            return Err(format!(
                "delay inside a function body in `{}` is not supported \
                 (ordinary functions cannot suspend)",
                self.path
            ));
        }
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "assignment without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "assignment without RHS".to_string())?;
        if blocking {
            return self.lower_blocking_timed_assignment(
                h,
                lhs,
                rhs,
                vec![IrStmt::Delay {
                    ticks: scaled_ticks,
                }],
            );
        }
        self.reject_class_property_nba(lhs)?;
        if self.cg.native_record_target(lhs) {
            return self.lower_delayed_native_record(h, lhs, rhs, false, scaled_ticks);
        }
        let string_target = self.cg.is_string_expr(&self.path, lhs);
        let chandle_target = !string_target && self.cg.is_chandle_expr(&self.path, lhs);
        if string_target || chandle_target {
            return Ok(vec![self.cg.lower_native_nba(
                &self.path,
                lhs,
                rhs,
                scaled_ticks,
            )?]);
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let op = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => *op,
            _ => unreachable!("non-assignment passed to lower_delayed_assignment"),
        };
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        if self.cg.proc_local_target(lhs).is_some()
            || self.cg.subroutine_auto_target(lhs)
            || lh.has_activation_root()
        {
            return Err("nonblocking delayed assignment requires persistent target storage".into());
        }
        Ok(vec![IrStmt::DelayedAssign {
            lhs: lh,
            rhs: rhs_ir,
            ticks: scaled_ticks,
        }])
    }

    /// Lower a blocking assignment with intra-assignment timing whose
    /// timing control is already lowered into `wait` (a delay, an event
    /// control or a repeated event control). The right-hand side is evaluated
    /// into a temporary before `wait` and the destination, including its
    /// selectors and the object a handle names, when `wait` completes:
    /// SV 9.4.5 ("the right-hand expression shall be evaluated before the
    /// delay") and 10.4.1 ("If variable_lvalue requires an evaluation, it
    /// shall be evaluated at the time specified by the intra-assignment
    /// timing control").
    pub(super) fn lower_blocking_timed_assignment(
        &mut self,
        h: NodeId,
        lhs: NodeId,
        rhs: NodeId,
        wait: Vec<IrStmt>,
    ) -> Result<Vec<IrStmt>, String> {
        let op = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => *op,
            _ => unreachable!("non-assignment passed to lower_blocking_timed_assignment"),
        };
        if self.cg.native_record_target(lhs) {
            return self.lower_blocking_native_record(h, lhs, rhs, op, wait);
        }
        if self.container_write_target(lhs) {
            return self.lower_blocking_timed_container_element(h, lhs, rhs, op, wait);
        }
        let string_target = self.cg.is_string_expr(&self.path, lhs);
        let chandle_target = !string_target && self.cg.is_chandle_expr(&self.path, lhs);
        if chandle_target {
            // The blocking form captures the value before suspending and
            // resolves its destination when the delay completes.
            let value = self.cg.lower_chandle(&self.path, rhs)?;
            let tmp = format!("_ch{}", h.0);
            let write = self.cg.lower_object_assignment(
                &self.path,
                lhs,
                rhs,
                true,
                Operation::Assignment,
            )?;
            let Some(IrStmt::Object(write)) = write else {
                return Err(format!(
                    "blocking delayed chandle assignment in `{}` has no chandle destination",
                    self.path
                ));
            };
            let write = match *write {
                IrObjectStmt::ChandleAssign(index, _) => {
                    IrObjectStmt::ChandleAssign(index, IrChandleExpr::LocalRead(tmp.clone()))
                }
                IrObjectStmt::ChandleAssignLocal(name, _) => {
                    IrObjectStmt::ChandleAssignLocal(name, IrChandleExpr::LocalRead(tmp.clone()))
                }
                _ => {
                    return Err(format!(
                        "blocking delayed assignment in `{}` supports only chandle destinations",
                        self.path
                    ))
                }
            };
            self.saw_wait = true;
            return Ok(vec![IrStmt::Block(vec![
                IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    tmp,
                    Some(value),
                ))),
                IrStmt::Block(wait),
                IrStmt::Object(Box::new(write)),
            ])]);
        }
        if string_target {
            self.cg.ensure_string_actual_writable(&self.path, lhs)?;
            let target = self
                .cg
                .lower_string_actual_address(&self.path, lhs)?
                .trim_start_matches('&')
                .to_owned();
            let value = self.cg.lower_string(&self.path, rhs)?;
            self.saw_wait = true;
            let tmp = format!("_st{}", h.0);
            return Ok(vec![IrStmt::Block(vec![
                IrStmt::DeclString {
                    name: tmp.clone(),
                    init: Some(value),
                },
                IrStmt::Block(wait),
                IrStmt::Object(Box::new(IrObjectStmt::StringAssignLocal(
                    target,
                    IrStringExpr::LocalRead(tmp.clone()),
                ))),
                IrStmt::Object(Box::new(IrObjectStmt::StringAssignLocal(
                    tmp,
                    IrStringExpr::Literal(Vec::new()),
                ))),
            ])]);
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        self.saw_wait = true;
        // The temp name is unique per assignment node; each site's Block keeps
        // re-declarations (loops, repeated task inlining) out of one C scope.
        let tmp = format!("_t{}", h.0);
        let (w, s) = (rhs_ir.width, rhs_ir.signed);
        Ok(vec![IrStmt::Block(vec![
            IrStmt::DeclLocal {
                name: tmp.clone(),
                width: w,
                signed: s,
                two_state: false,
                init: Some(Box::new(rhs_ir)),
            },
            IrStmt::Block(wait),
            IrStmt::Assign {
                lhs: lh,
                rhs: IrExpr::new(IrExprKind::LocalRead(tmp), w, s, None),
                nba: false,
            },
        ])])
    }

    /// Whether a destination writes into a resizable container element,
    /// directly or through a select of one.
    fn container_write_target(&self, lhs: NodeId) -> bool {
        let mut node = lhs;
        loop {
            if self.cg.is_container_element(node) {
                return true;
            }
            match self.cg.kind(node) {
                NodeKind::Expr(
                    ExprKind::BitSelect { base, .. }
                    | ExprKind::PartSelect { base, .. }
                    | ExprKind::IndexedPartSelect { base, .. }
                    | ExprKind::ArraySelect { base, .. },
                ) => node = *base,
                _ => return false,
            }
        }
    }

    /// Blocking timed write of one whole element of a resizable container
    /// (`q[i] = @e v`, `aa[key] = #1 s`). The ordinary element store is
    /// lowered and its value moved into a temporary evaluated before `wait`;
    /// the index or key expression stays in the store, which runs after it.
    /// Stores that need temporaries of their own (a select inside an
    /// element) cannot be split this way and are rejected.
    fn lower_blocking_timed_container_element(
        &mut self,
        h: NodeId,
        lhs: NodeId,
        rhs: NodeId,
        op: Operation,
        wait: Vec<IrStmt>,
    ) -> Result<Vec<IrStmt>, String> {
        let path = self.path.clone();
        let unsupported = || {
            format!(
                "intra-assignment timing on a select within a resizable container element or a compound container update in `{path}` is not supported"
            )
        };
        if op != Operation::Assignment {
            return Err(unsupported());
        }
        let mut statement = self.lower_assignment_operands(lhs, rhs, true, op, false)?;
        while let IrStmt::Located {
            statement: inner, ..
        } = statement
        {
            statement = *inner;
        }
        let IrStmt::Container(store) = statement else {
            return Err(unsupported());
        };
        let tmp = format!("_t{}", h.0);
        let packed = |value: IrExpr| {
            let (width, signed) = (value.width, value.signed);
            (
                IrStmt::DeclLocal {
                    name: tmp.clone(),
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(value)),
                },
                IrExpr::new(IrExprKind::LocalRead(tmp.clone()), width, signed, None),
            )
        };
        let string = |value: IrStringExpr| {
            (
                IrStmt::DeclString {
                    name: tmp.clone(),
                    init: Some(value),
                },
                IrStringExpr::LocalRead(tmp.clone()),
            )
        };
        let chandle = |value: IrChandleExpr| {
            (
                IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    tmp.clone(),
                    Some(value),
                ))),
                IrChandleExpr::LocalRead(tmp.clone()),
            )
        };
        let (capture, store) = match *store {
            IrContainerStmt::Set {
                container,
                index,
                value,
            } => {
                let (capture, value) = packed(value);
                let store = IrContainerStmt::Set {
                    container,
                    index,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetReal {
                container,
                index,
                value,
            } => {
                let (capture, value) = packed(value);
                let store = IrContainerStmt::SetReal {
                    container,
                    index,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetNested {
                container,
                indices,
                value,
            } => {
                let (capture, value) = packed(value);
                let store = IrContainerStmt::SetNested {
                    container,
                    indices,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetNestedReal {
                container,
                indices,
                value,
            } => {
                let (capture, value) = packed(value);
                let store = IrContainerStmt::SetNestedReal {
                    container,
                    indices,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetString {
                container,
                key,
                value,
            } => {
                let (capture, value) = packed(value);
                let store = IrContainerStmt::SetString {
                    container,
                    key,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetStringReal {
                container,
                key,
                value,
            } => {
                let (capture, value) = packed(value);
                let store = IrContainerStmt::SetStringReal {
                    container,
                    key,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetStringValue {
                container,
                index,
                value,
            } => {
                let (capture, value) = string(value);
                let store = IrContainerStmt::SetStringValue {
                    container,
                    index,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetNestedString {
                container,
                indices,
                value,
            } => {
                let (capture, value) = string(value);
                let store = IrContainerStmt::SetNestedString {
                    container,
                    indices,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetStringString {
                container,
                key,
                value,
            } => {
                let (capture, value) = string(value);
                let store = IrContainerStmt::SetStringString {
                    container,
                    key,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetChandleValue {
                container,
                index,
                value,
            } => {
                let (capture, value) = chandle(value);
                let store = IrContainerStmt::SetChandleValue {
                    container,
                    index,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetNestedChandle {
                container,
                indices,
                value,
            } => {
                let (capture, value) = chandle(value);
                let store = IrContainerStmt::SetNestedChandle {
                    container,
                    indices,
                    value,
                };
                (capture, store)
            }
            IrContainerStmt::SetStringChandle {
                container,
                key,
                value,
            } => {
                let (capture, value) = chandle(value);
                let store = IrContainerStmt::SetStringChandle {
                    container,
                    key,
                    value,
                };
                (capture, store)
            }
            _ => return Err(unsupported()),
        };
        self.saw_wait = true;
        Ok(vec![IrStmt::Block(vec![
            capture,
            IrStmt::Block(wait),
            IrStmt::Container(Box::new(store)),
        ])])
    }

    /// Lower event/repeat intra-assignment timing. A blocking assignment
    /// evaluates its RHS before registering the wait and evaluates its LHS
    /// when the wait completes. An NBA captures both RHS and destination
    /// selectors at issue time and registers an independent runtime action.
    pub(super) fn lower_event_assignment(
        &mut self,
        h: NodeId,
        blocking: bool,
        timing: &IntraControl,
    ) -> Result<Vec<IrStmt>, String> {
        if self.in_final {
            return Err(format!(
                "intra-assignment event/repeat control inside a final block in `{}` is not allowed",
                self.path
            ));
        }
        if self.timing_forbidden() {
            return Err(format!(
                "event/repeat intra-assignment timing inside a function body in `{}` is not allowed \
                 (functions cannot suspend)",
                self.path
            ));
        }
        let (lhs, rhs, op) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => {
                let lhs = self
                    .cg
                    .node(h)
                    .children
                    .first()
                    .copied()
                    .ok_or_else(|| "assignment without LHS".to_string())?;
                let rhs = self
                    .cg
                    .node(h)
                    .children
                    .get(1)
                    .copied()
                    .ok_or_else(|| "assignment without RHS".to_string())?;
                (lhs, rhs, *op)
            }
            _ => unreachable!("non-assignment passed to lower_event_assignment"),
        };
        if !blocking {
            self.reject_class_property_nba(lhs)?;
        }
        if op != Operation::Assignment {
            return Err(format!(
                "compound event/repeat intra-assignment timing in `{}` is not supported",
                self.path
            ));
        }
        if let Some((specs, count)) = self.process_evaluated_intra_event(timing) {
            return self.lower_process_event_assignment(h, blocking, lhs, rhs, op, &specs, count);
        }
        if blocking {
            // The count and event expressions are evaluated when the control
            // is reached, after the RHS (Table 9-3 equivalence).
            let (specs, repeat) = self.lower_intra_event_timing(timing)?;
            let wait = IrStmt::WaitEvents {
                specs,
                refresh: false,
            };
            let wait = match repeat {
                Some(count) => IrStmt::Repeat {
                    count,
                    body: vec![wait],
                },
                None => wait,
            };
            return self.lower_blocking_timed_assignment(h, lhs, rhs, vec![wait]);
        }
        if self.native_nba_target(lhs) {
            let (specs, count) = self.intra_event_specs(timing)?;
            let count = count
                .map(|count| self.lower_repeat_count(count))
                .transpose()?;
            return self.lower_native_event_nba(h, lhs, rhs, &specs, count);
        }
        let (specs, repeat) = self.lower_intra_event_timing(timing)?;
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);

        if self.cg.proc_local_target(lhs).is_some()
            || self.cg.subroutine_auto_target(lhs)
            || lh.has_activation_root()
        {
            return Err(
                "nonblocking event/repeat intra-assignment timing requires persistent target storage"
                    .to_owned(),
            );
        }
        let frame = self.cg.new_frame_id()?;
        let mut captures = Vec::new();
        let rhs_capture = self.capture_event_assignment_expr(frame, &mut captures, rhs_ir);
        let lhs_capture = self.capture_event_assignment_lhs(frame, &mut captures, lh)?;
        let action = self.cg.new_fn_name(&self.path, "event_assign");
        self.pre_fns.push(crate::sim::ir::IrPreFn::EventAssign {
            c_name: action.clone(),
            frame,
            captures: captures.clone(),
            lhs: lhs_capture.clone(),
            rhs: rhs_capture.clone(),
        });
        Ok(vec![IrStmt::NonblockingEventAssignWhen {
            lhs: lhs_capture,
            rhs: Box::new(rhs_capture),
            specs,
            repeat: repeat.map(Box::new),
            action,
            frame,
            captures,
        }])
    }

    /// The event specs and optional repeat count of an intra-assignment
    /// event control whose helper needs process evaluation (see
    /// [`super::super::collection::EventEvaluation`]). Other forms keep the
    /// runtime-callback path and its diagnostics.
    fn process_evaluated_intra_event(
        &self,
        timing: &IntraControl,
    ) -> Option<(Vec<EventSpec>, Option<NodeId>)> {
        match timing {
            IntraControl::Event {
                specs, implicit, ..
            } if !*implicit && self.event_specs_need_process(specs) => Some((specs.clone(), None)),
            IntraControl::Repeat { count, event, .. } => match event.as_ref() {
                IntraControl::Event {
                    specs, implicit, ..
                } if !*implicit && self.event_specs_need_process(specs) => {
                    Some((specs.clone(), Some(*count)))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Lower an intra-assignment event control whose helper has effects. The
    /// RHS (and an NBA's destination selectors) are evaluated first, then the
    /// control is armed (SV 9.4.5). A blocking assignment waits in its own
    /// process. A nonblocking assignment must not block its issuer, so its
    /// captured values and armed event state move to a detached process that
    /// waits, then issues the NBA (SV 9.4.5, 10.4.2). That process is not a
    /// child of the issuer, so `wait fork`/`disable fork` never observe it,
    /// just as they never observe a runtime-owned pending NBA.
    #[allow(clippy::too_many_arguments)]
    fn lower_process_event_assignment(
        &mut self,
        h: NodeId,
        blocking: bool,
        lhs: NodeId,
        rhs: NodeId,
        op: Operation,
        specs: &[EventSpec],
        count: Option<NodeId>,
    ) -> Result<Vec<IrStmt>, String> {
        let count = count
            .map(|count| self.lower_repeat_count(count))
            .transpose()?;
        if blocking {
            let wait = self.process_event_plan(h, specs)?.blocking();
            let wait = match count {
                Some(count) => IrStmt::Repeat {
                    count,
                    body: vec![wait],
                },
                None => wait,
            };
            return self.lower_blocking_timed_assignment(h, lhs, rhs, vec![wait]);
        }
        if self.native_nba_target(lhs) {
            return self.lower_native_event_nba(h, lhs, rhs, specs, count);
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        if self.cg.proc_local_target(lhs).is_some()
            || self.cg.subroutine_auto_target(lhs)
            || lh.has_activation_root()
        {
            return Err(
                "nonblocking event/repeat intra-assignment timing requires persistent target storage"
                    .to_owned(),
            );
        }
        let frame = self.cg.new_frame_id()?;
        let mut captures = Vec::new();
        let rhs_capture = self.capture_event_assignment_expr(frame, &mut captures, rhs_ir);
        let lhs_capture = self.capture_event_assignment_lhs(frame, &mut captures, lh)?;
        let action = vec![IrStmt::Assign {
            lhs: lhs_capture,
            rhs: rhs_capture,
            nba: true,
        }];
        Ok(vec![self.spawn_process_evaluated_action(
            h,
            specs,
            count,
            frame,
            captures,
            action,
            "event_nba",
        )?])
    }

    /// Arm a process-evaluated event control in the issuing process and
    /// return the statement that spawns the detached process which waits for
    /// it (repeated `count` times) and then runs `action`. `captures` already
    /// holds the issue-time values `action` reads; the repeat count and the
    /// armed event state are appended. The detached process sees only module,
    /// package and static storage plus its captured frame.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_process_evaluated_action(
        &mut self,
        h: NodeId,
        specs: &[EventSpec],
        count: Option<IrExpr>,
        frame: FrameId,
        mut captures: Vec<IrCapture>,
        action: Vec<IrStmt>,
        kind: &str,
    ) -> Result<IrStmt, String> {
        fn expressions(spec: &EventSpec, out: &mut Vec<NodeId>) {
            match spec {
                EventSpec::Qualified { event, condition } => {
                    out.push(*condition);
                    expressions(event, out);
                }
                EventSpec::Named(event) => out.push(*event),
                EventSpec::AnyChange { sig } | EventSpec::Edge { sig, .. } => out.push(*sig),
            }
        }
        let mut nodes = Vec::new();
        for spec in specs {
            expressions(spec, &mut nodes);
        }
        for node in nodes {
            if let Some(target) = self
                .cg
                .nested_proc_local_ref(node)
                .or_else(|| self.cg.nested_capture_ref(node))
                .or_else(|| self.nested_subroutine_auto_ref(node))
            {
                return Err(format!(
                    "nonblocking event control with a process-evaluated helper in `{}` cannot read automatic storage `{}`",
                    self.cg.source_path(&self.path),
                    self.cg.node(target).name
                ));
            }
        }
        let count =
            count.map(|count| self.capture_event_assignment_expr(frame, &mut captures, count));
        let plan = self.process_event_plan(h, specs)?;
        if plan.arm.iter().any(|arm| arm.native.is_some()) {
            return Err(format!(
                "a nonblocking event control on a string or class-handle expression that the waiting process evaluates is not supported in `{}`",
                self.cg.source_path(&self.path)
            ));
        }
        let armed = plan
            .arm
            .iter()
            .map(|arm| self.capture_event_assignment_expr(frame, &mut captures, arm.value.clone()))
            .collect();
        let wait = plan.resumed(armed);
        let mut body = match count {
            Some(count) => vec![IrStmt::Repeat { count, body: wait }],
            None => wait,
        };
        body.extend(action);
        let name = self.cg.new_fn_name(&self.path, kind);
        let label = format!("{}.{kind}", self.cg.source_path(&self.path));
        self.pre_fns.push(crate::sim::ir::IrPreFn::CapturedBranch {
            c_name: name.clone(),
            frame,
            captures: captures.clone(),
            body,
        });
        Ok(IrStmt::CapturedFork {
            join_kind: IrJoinKind::Detached,
            branches: vec![IrCapturedBranch::new(name, label, frame, captures)],
            target: None,
        })
    }

    /// Whether a nonblocking destination is a string or handle object,
    /// which queues an owned native value rather than a packed one.
    fn native_nba_target(&self, lhs: NodeId) -> bool {
        self.cg.is_string_expr(&self.path, lhs) || self.cg.is_chandle_expr(&self.path, lhs)
    }

    /// The explicit event list and optional repeat count of an
    /// intra-assignment event control, with the same shape rules as
    /// [`Self::lower_intra_event_timing`].
    fn intra_event_specs(
        &self,
        timing: &IntraControl,
    ) -> Result<(Vec<EventSpec>, Option<NodeId>), String> {
        match timing {
            IntraControl::Event {
                specs, implicit, ..
            } if !*implicit && !specs.is_empty() => Ok((specs.clone(), None)),
            IntraControl::Repeat { count, event, .. } => match event.as_ref() {
                IntraControl::Event {
                    specs, implicit, ..
                } if !*implicit && !specs.is_empty() => Ok((specs.clone(), Some(*count))),
                _ => Err(format!(
                    "intra-assignment event control in `{}` must contain an explicit event",
                    self.path
                )),
            },
            _ => Err(format!(
                "intra-assignment event control in `{}` must contain an explicit event",
                self.path
            )),
        }
    }

    /// A nonblocking string or handle assignment with an event or repeated
    /// event control (`s <= repeat (2) @(e) t;`). The value is evaluated and
    /// copied at issue (SV 9.4.5) and the issuer continues (SV 10.4.2). The
    /// control is armed at issue and waited for by a detached process that
    /// owns the copied value, then queues the native NBA in the current time
    /// step. Native NBA destinations are whole persistent objects, so no
    /// selector state needs capturing.
    fn lower_native_event_nba(
        &mut self,
        h: NodeId,
        lhs: NodeId,
        rhs: NodeId,
        specs: &[EventSpec],
        count: Option<IrExpr>,
    ) -> Result<Vec<IrStmt>, String> {
        let write = self
            .cg
            .lower_native_nba(&self.path, lhs, rhs, IrDelay::Constant(0))?;
        let frame = self.cg.new_frame_id()?;
        let value = format!("_nbv{}", h.0);
        let (declare, kind, initial, write) = match write {
            IrStmt::DelayedStringAssign { target, rhs, ticks } => (
                IrStmt::DeclString {
                    name: value.clone(),
                    init: Some(rhs),
                },
                StorageKind::String,
                IrExpr::new(IrExprKind::LocalRead(value.clone()), 0, false, None),
                IrStmt::DelayedStringAssign {
                    target,
                    rhs: IrStringExpr::LocalRead(value),
                    ticks,
                },
            ),
            IrStmt::DelayedChandleAssign { target, rhs, ticks } => (
                IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    value.clone(),
                    Some(rhs),
                ))),
                StorageKind::Opaque,
                IrExpr::new(
                    IrExprKind::ObjectQuery(Box::new(IrObjectQuery::HandleCapture(
                        IrChandleExpr::LocalRead(value.clone()),
                    ))),
                    1,
                    false,
                    None,
                ),
                IrStmt::DelayedChandleAssign {
                    target,
                    rhs: IrChandleExpr::LocalRead(value),
                    ticks,
                },
            ),
            _ => {
                return Err(format!(
                "nonblocking event-controlled native assignment in `{}` has no native destination",
                self.path
            ))
            }
        };
        let storage = StorageRef::new(
            frame,
            0,
            StorageLifetime::Automatic,
            StorageOwnership::Owned,
        )
        .with_kind(kind);
        let spawn = self.spawn_process_evaluated_action(
            h,
            specs,
            count,
            frame,
            vec![IrCapture::new(storage, initial)],
            vec![write],
            "event_native_nba",
        )?;
        Ok(vec![IrStmt::Block(vec![declare, spawn])])
    }

    #[allow(clippy::type_complexity)]
    fn lower_intra_event_timing(
        &mut self,
        timing: &IntraControl,
    ) -> Result<(Vec<(IrWaitSrc, IrEdge)>, Option<IrExpr>), String> {
        match timing {
            IntraControl::Event {
                specs, implicit, ..
            } => {
                if *implicit || specs.is_empty() {
                    return Err(format!(
                        "intra-assignment event control in `{}` must contain an explicit event",
                        self.path
                    ));
                }
                Ok((self.lower_event_specs(specs)?, None))
            }
            IntraControl::Repeat { count, event, .. } => {
                let count = self.lower_repeat_count(*count)?;
                let (specs, nested) = self.lower_intra_event_timing(event)?;
                if nested.is_some() {
                    return Err(format!(
                        "nested repeat event controls in `{}` are not supported",
                        self.path
                    ));
                }
                Ok((specs, Some(count)))
            }
            IntraControl::Delay(_) => Err(format!(
                "delay timing reached event-assignment lowering in `{}`",
                self.path
            )),
            IntraControl::Cycle { control, .. } => {
                let node = self.cg.node(*control);
                Err(format!(
                    "cycle delay cannot be nested in an event assignment at {}:{}:{} in `{}`",
                    node.file.as_deref().unwrap_or("<unknown>"),
                    node.line,
                    node.col,
                    self.path
                ))
            }
            IntraControl::Unsupported { control } => {
                let node = self.cg.node(*control);
                Err(format!(
                    "unsupported intra-assignment timing at {}:{}:{} in `{}`",
                    node.file.as_deref().unwrap_or("<unknown>"),
                    node.line,
                    node.col,
                    self.path
                ))
            }
        }
    }

    pub(super) fn capture_event_assignment_expr(
        &self,
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        expression: IrExpr,
    ) -> IrExpr {
        let slot = captures.len() as u32;
        let storage = StorageRef::new(
            frame,
            slot,
            StorageLifetime::Automatic,
            StorageOwnership::Owned,
        )
        .with_kind(if expression.is_real() {
            StorageKind::Real
        } else {
            StorageKind::Packed
        });
        let local = format!("_fc{}_{}", frame.index(), slot);
        captures.push(IrCapture::new(storage, expression.clone()));
        IrExpr::new(
            IrExprKind::LocalRead(local),
            expression.width,
            expression.signed,
            None,
        )
    }

    fn capture_event_assignment_lhs(
        &self,
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        lhs: IrLhs,
    ) -> Result<IrLhs, String> {
        Ok(match lhs {
            IrLhs::PackedSelect {
                target,
                steps,
                signed,
                two_state,
            } => IrLhs::PackedSelect {
                target: Box::new(self.capture_event_assignment_lhs(frame, captures, *target)?),
                steps: steps
                    .into_iter()
                    .map(|mut step| {
                        step.base = self.capture_event_assignment_expr(frame, captures, step.base);
                        step
                    })
                    .collect(),
                signed,
                two_state,
            },
            IrLhs::Bit(index, select, two_state) => IrLhs::Bit(
                index,
                Box::new(self.capture_event_assignment_expr(frame, captures, *select)),
                two_state,
            ),
            IrLhs::IdxPart(index, base, width, selected_width, negative, two_state) => {
                IrLhs::IdxPart(
                    index,
                    Box::new(self.capture_event_assignment_expr(frame, captures, *base)),
                    Box::new(self.capture_event_assignment_expr(frame, captures, *width)),
                    selected_width,
                    negative,
                    two_state,
                )
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => IrLhs::ArrayElem {
                arr,
                indices: indices
                    .into_iter()
                    .map(|index| self.capture_event_assignment_expr(frame, captures, index))
                    .collect(),
                elem_sel: match elem_sel {
                    IrElemSel::PackedChain(steps) => IrElemSel::PackedChain(
                        steps
                            .into_iter()
                            .map(|mut step| {
                                step.base =
                                    self.capture_event_assignment_expr(frame, captures, step.base);
                                step
                            })
                            .collect(),
                    ),
                    IrElemSel::Whole => IrElemSel::Whole,
                    IrElemSel::Part(left, right) => IrElemSel::Part(left, right),
                    IrElemSel::Bit(index) => IrElemSel::Bit(Box::new(
                        self.capture_event_assignment_expr(frame, captures, *index),
                    )),
                    IrElemSel::Indexed {
                        base,
                        width,
                        negative,
                    } => IrElemSel::Indexed {
                        base: Box::new(self.capture_event_assignment_expr(frame, captures, *base)),
                        width,
                        negative,
                    },
                },
            },
            IrLhs::Stream {
                parts,
                width,
                slice,
                direction,
            } => IrLhs::Stream {
                parts: parts
                    .into_iter()
                    .map(|(part, width)| {
                        self.capture_event_assignment_lhs(frame, captures, part)
                            .map(|part| (part, width))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                width,
                slice,
                direction,
            },
            other => other,
        })
    }
}

/// `current + 1` or `current - 1`, in real or packed arithmetic.
fn inc_dec_value(current: IrExpr, increment: bool) -> IrExpr {
    if current.is_real() {
        return real_bin_expr(
            if increment {
                IrRealBinOp::Add
            } else {
                IrRealBinOp::Sub
            },
            current,
            real_literal_expr(1.0),
        );
    }
    let one = IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![1],
            x: vec![0],
            z: vec![0],
            width: 32,
            signed: true,
            real: None,
            fill: None,
        }),
        32,
        true,
        None,
    );
    common_bin_expr(
        if increment {
            IrBinOp::Add
        } else {
            IrBinOp::Sub
        },
        current,
        one,
    )
}
