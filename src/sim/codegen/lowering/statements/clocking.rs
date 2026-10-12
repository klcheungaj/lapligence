//! Clocking.

use super::*;

impl<'c, 'a> EmitCtx<'c, 'a> {
    fn lower_cycle_wait(&mut self, count_node: NodeId) -> Result<IrStmt, String> {
        let block = self.cg.default_clocking_block(self.inst).ok_or_else(|| {
            format!(
                "`##` cycle delay in `{}` requires a resolved default clocking block",
                self.path
            )
        })?;
        let block_info = self
            .cg
            .db
            .clocking_block(block)
            .ok_or_else(|| "resolved default clocking block metadata disappeared".to_owned())?
            .clone();
        if block_info.event_implicit || block_info.event_specs.is_empty() {
            return Err(format!(
                "`##` cycle delay in `{}` requires an explicit default clocking event",
                self.path
            ));
        }
        let count = self.cg.lower_expr(&self.path, count_node)?;
        if count.is_real() {
            return Err(format!(
                "`##` cycle delay count in `{}` must be integral",
                self.path
            ));
        }
        let event = self
            .cg
            .event_globals
            .get(&block)
            .ok_or_else(|| "default clocking block has no published event".to_owned())?;
        let specs = vec![(IrWaitSrc::Event(IrEventRef::Static(event.ir)), IrEdge::Any)];
        Ok(IrStmt::ClockingCycleWait { count, specs })
    }

    pub(super) fn lower_clocking_drive_specs(
        &mut self,
        target: NodeId,
    ) -> Result<Vec<(IrWaitSrc, IrEdge)>, String> {
        let var = self.cg.db.clocking_var(target).ok_or_else(|| {
            format!(
                "clocking member `{}` has no owned declaration in `{}`",
                self.cg.node(target).name,
                self.path
            )
        })?;
        let block = self
            .cg
            .db
            .clocking_block(var.block)
            .ok_or_else(|| {
                format!(
                    "clocking member `{}` has no owning block in `{}`",
                    self.cg.node(target).name,
                    self.path
                )
            })?
            .clone();
        if block.event_implicit || block.event_specs.is_empty() {
            return Err(format!(
                "clocking drive for `{}` in `{}` requires an explicit clocking event",
                self.cg.node(target).name,
                self.path
            ));
        }
        let output_edge = self.cg.clocking_output_edge(target, &self.path)?;
        let mut specs = self.lower_event_specs(&block.event_specs)?;
        if specs
            .iter()
            .any(|(source, _)| !matches!(source, IrWaitSrc::Sig(_) | IrWaitSrc::Event(_)))
        {
            return Err(format!(
                "clocking drive for `{}` in `{}` requires a simple signal or named-event clocking event",
                self.cg.node(target).name,
                self.path
            ));
        }
        if !matches!(output_edge, ClockingEdge::None) {
            if specs.len() != 1 {
                return Err(format!(
                    "edge-qualified clocking output for `{}` in `{}` requires a singular clocking event",
                    self.cg.node(target).name,
                    self.path
                ));
            }
            if !matches!(specs[0].0, IrWaitSrc::Sig(_)) {
                return Err(format!(
                    "edge-qualified clocking output for `{}` in `{}` requires a signal clocking event",
                    self.cg.node(target).name,
                    self.path
                ));
            }
            match output_edge {
                ClockingEdge::Posedge => specs[0].1 = IrEdge::Posedge,
                ClockingEdge::Negedge => specs[0].1 = IrEdge::Negedge,
                ClockingEdge::BothEdges => {
                    let source = specs[0].0.clone();
                    specs = vec![(source.clone(), IrEdge::Posedge), (source, IrEdge::Negedge)];
                }
                ClockingEdge::None => unreachable!("edge-qualified output checked above"),
            }
        }
        Ok(specs)
    }

    pub(super) fn lower_cycle_delay(
        &mut self,
        h: NodeId,
        count_node: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        if self.in_final {
            return Err(format!(
                "`##` cycle delay inside a final block in `{}` is not allowed",
                self.path
            ));
        }
        if self.timing_forbidden() {
            return Err(format!(
                "`##` cycle delay inside a function body in `{}` is not supported \
                 (ordinary functions cannot suspend)",
                self.path
            ));
        }
        let wait = self.lower_cycle_wait(count_node)?;
        self.saw_wait = true;
        let mut out = vec![wait];
        if let Some(body) = self.cg.node(h).children.first().copied() {
            if !matches!(self.cg.kind(body), NodeKind::Stmt(StmtKind::Empty)) {
                out.extend(self.lower_stmt(body)?);
            }
        }
        Ok(out)
    }

    pub(super) fn lower_cycle_assignment(
        &mut self,
        h: NodeId,
        blocking: bool,
        count_node: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        if self.in_final {
            return Err(format!(
                "cycle-delayed assignment inside a final block in `{}` is not allowed",
                self.path
            ));
        }
        if self.timing_forbidden() {
            return Err(format!(
                "cycle-delayed assignment inside a function body in `{}` is not supported",
                self.path
            ));
        }
        let (lhs, rhs, op) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => (
                self.cg
                    .node(h)
                    .children
                    .first()
                    .copied()
                    .ok_or_else(|| "assignment without LHS".to_owned())?,
                self.cg
                    .node(h)
                    .children
                    .get(1)
                    .copied()
                    .ok_or_else(|| "assignment without RHS".to_owned())?,
                *op,
            ),
            _ => unreachable!("non-assignment passed to lower_cycle_assignment"),
        };
        if op != Operation::Assignment {
            return Err(format!(
                "compound cycle-delayed assignment in `{}` is not supported",
                self.path
            ));
        }
        let mut clocking_targets = Vec::new();
        let all_clocking = self.cg.clocking_lhs_targets(lhs, &mut clocking_targets);
        if !clocking_targets.is_empty() && !all_clocking {
            return Err(format!(
                "clocking output/inout concatenations cannot mix ordinary targets in `{}`",
                self.path
            ));
        }
        if blocking && !clocking_targets.is_empty() {
            return Err(format!(
                "clocking output/inout member drives in `{}` require nonblocking `<=`",
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
        if let Some(target) = clocking_targets.first().copied() {
            // A synchronous drive with a cycle delay does not block: the
            // value and count are taken now and the drive matures on the
            // target clockvar's own clocking events (SV 14.16).
            if let Some(drive) =
                self.lower_virtual_clocking_drive(lhs, rhs, op, Some(count_node))?
            {
                return Ok(vec![drive]);
            }
            let lh = self.cg.lower_lhs(&self.path, lhs)?;
            let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
            let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
            let cycles = self.lower_drive_cycle_count(count_node)?;
            let drive_specs = self.lower_clocking_drive_specs(target)?;
            for other_target in clocking_targets.iter().skip(1) {
                if self.lower_clocking_drive_specs(*other_target)? != drive_specs {
                    return Err(format!(
                        "clocking concatenation in `{}` uses different clocking events",
                        self.path
                    ));
                }
            }
            let ticks = self.cg.clocking_output_delay(target, &self.path)?;
            for other_target in clocking_targets.iter().skip(1) {
                if self.cg.clocking_output_delay(*other_target, &self.path)? != ticks {
                    return Err(format!(
                        "clocking concatenation in `{}` uses different output skews",
                        self.path
                    ));
                }
            }
            return Ok(vec![IrStmt::ClockingDrive {
                lhs: lh,
                rhs: rhs_ir,
                ticks,
                specs: drive_specs,
                cycles: Some(Box::new(cycles)),
            }]);
        }
        // SV 14.11: a cycle delay is not a legal intra-assignment delay of
        // an ordinary blocking or nonblocking assignment.
        Err(format!(
            "intra-assignment cycle delay in `{}` requires a clocking output/inout target",
            self.path
        ))
    }

    /// The cycle count of a synchronous drive's `##n`, which counts the
    /// target's clocking events and needs no default clocking.
    fn lower_drive_cycle_count(&mut self, count_node: NodeId) -> Result<IrExpr, String> {
        let count = self.cg.lower_expr(&self.path, count_node)?;
        if count.is_real() {
            return Err(format!(
                "`##` cycle delay count in `{}` must be integral",
                self.path
            ));
        }
        Ok(count)
    }

    /// Lower a synchronous drive through a virtual-interface handle (SV
    /// 14.16, 25.9). The handle selects the bound instance when the drive
    /// issues; that instance's arm drives its own clockvar with its own
    /// clocking event, skew and net driver, so rebinding the handle later
    /// never redirects a queued drive. Only the selected arm evaluates the
    /// value, selectors and cycle count.
    pub(super) fn lower_virtual_clocking_drive(
        &mut self,
        lhs: NodeId,
        rhs: NodeId,
        op: Operation,
        cycles: Option<NodeId>,
    ) -> Result<Option<IrStmt>, String> {
        let Some(target) = self.cg.virtual_clocking_target(&self.path, lhs)? else {
            return Ok(None);
        };
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment to a clocking output/inout in `{}` is not supported",
                self.path
            ));
        }
        let Some(first) = target.instances.first().copied() else {
            return Err(format!(
                "virtual interface clocking drive in `{}` has no interface instance to bind",
                self.path
            ));
        };
        let steps = if target.selects.is_empty() {
            None
        } else {
            let dimensions = self
                .cg
                .db
                .packed_dimensions(target.clockvar)
                .or_else(|| {
                    self.cg
                        .db
                        .clocking_var(first)
                        .and_then(|var| var.source)
                        .and_then(|source| self.cg.db.packed_dimensions(source))
                })
                .map(<[_]>::to_vec)
                .unwrap_or_default();
            Some(self.cg.packed_selection_steps_over(
                &self.path,
                &dimensions,
                target.selects,
                target.width,
            )?)
        };
        let cycles = cycles
            .map(|count| self.lower_drive_cycle_count(count))
            .transpose()?;
        let mut rhs_ir = None;
        let mut items = Vec::with_capacity(target.instances.len());
        for (instance, clockvar) in target.instances.iter().copied().enumerate() {
            let root = self
                .cg
                .clocking_drive_root(&self.path, clockvar, steps.is_some())?;
            let lh = match &steps {
                Some(steps) => IrLhs::PackedSelect {
                    target: Box::new(root),
                    steps: steps.clone(),
                    signed: false,
                    two_state: target.two_state,
                },
                None => root,
            };
            let value = match &rhs_ir {
                Some(value) => IrExpr::clone(value),
                None => {
                    let value = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
                    let value = apply_lhs_assignment_context(&self.cg.model, &lh, value);
                    rhs_ir = Some(value.clone());
                    value
                }
            };
            let specs = self.lower_clocking_drive_specs(clockvar)?;
            let ticks = self.cg.clocking_output_delay(clockvar, &self.path)?;
            items.push(IrCaseItem::new(
                vec![lhs_integer_expr(instance as i128)],
                vec![IrStmt::ClockingDrive {
                    lhs: lh,
                    rhs: value,
                    ticks,
                    specs,
                    cycles: cycles.clone().map(Box::new),
                }],
            ));
        }
        Ok(Some(IrStmt::Case {
            sel: IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::VirtualInterfaceInstance {
                    handle: target.handle,
                    interface: target.descriptor,
                    site: target.site,
                })),
                32,
                false,
                None,
            ),
            kind: IrCaseKind::Exact,
            items,
            check: IrUniquePriorityCheck::None,
        }))
    }
}
