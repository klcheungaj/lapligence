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
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        let (lhs_captures, lh) = self.capture_cycle_lhs(h, lh);
        let tmp = format!("_cycle_rhs_{}", h.0);
        let (width, signed) = (rhs_ir.width, rhs_ir.signed);
        let wait = self.lower_cycle_wait(count_node)?;
        let final_stmt = if let Some(target) = clocking_targets.first().copied() {
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
            IrStmt::ClockingDrive {
                lhs: lh,
                rhs: IrExpr::new(IrExprKind::LocalRead(tmp.clone()), width, signed, None),
                ticks,
                specs: drive_specs,
            }
        } else {
            IrStmt::Assign {
                lhs: lh,
                rhs: IrExpr::new(IrExprKind::LocalRead(tmp.clone()), width, signed, None),
                nba: !blocking,
            }
        };
        self.saw_wait = true;
        let mut body = vec![IrStmt::DeclLocal {
            name: tmp,
            width,
            signed,
            two_state: false,
            init: Some(Box::new(rhs_ir)),
        }];
        body.extend(lhs_captures);
        body.extend([wait, final_stmt]);
        Ok(vec![IrStmt::Block(body)])
    }
}
