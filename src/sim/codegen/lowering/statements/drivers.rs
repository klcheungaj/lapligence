//! Drivers.

use super::super::collection::EventEvaluation;
use super::*;
use crate::sim::ir::IrRuntimeQuery;
use crate::sim::legacy_unsupported::LegacyFamily;

impl EmitCtx<'_, '_> {
    /// Lower `force lhs = rhs;` into a live runtime binding. The target keeps
    /// its canonical assignment shape so selected and concatenated net
    /// targets can be overlaid without losing their underlying drivers.
    pub(super) fn lower_force(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let NodeKind::Stmt(StmtKind::Force { lhs, rhs }) = self.cg.kind(h) else {
            return Err("expected a typed force statement".to_string());
        };
        let diagnostic_path = self.force_diagnostic_path(h);
        let (lhs, rhs) = (*lhs, *rhs);
        if let Some(target) = self.nested_subroutine_auto_ref(lhs) {
            return Err(format!(
                "force target in `{diagnostic_path}` cannot refer to automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        if self.cg.nested_proc_local_ref(rhs).is_some() {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture an automatic procedural local"
            ));
        }
        if let Some(target) = self.nested_subroutine_auto_ref(rhs) {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        if let Some(target) = self.cg.nested_capture_ref(rhs) {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture activation storage `{}`",
                self.cg.node(target).name
            ));
        }
        // A force evaluator is re-run by the runtime inside the write that
        // changed its source, so it must be a read-only callback. A legal
        // helper with effects is instead evaluated by processes into a hidden
        // source that the callback merely reads (see `effectful_force`).
        let effectful = matches!(
            self.cg.classify_event_expression(rhs, &diagnostic_path),
            Ok(EventEvaluation::Process(_))
        );
        let lh = self
            .cg
            .lower_lhs(&self.path, lhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        let target_real = validate_force_lhs(&self.cg.model, &lh, &diagnostic_path)?;
        let value = self.force_value(rhs, &lh, target_real, &diagnostic_path)?;
        if effectful {
            return self.effectful_force(h, rhs, lh, value, target_real, &diagnostic_path);
        }
        let eval = self.cg.new_fn_name(&self.path, "force_eval");
        self.pre_fns.push(crate::sim::ir::IrPreFn::ForceEval {
            c_name: eval.clone(),
            value: value.clone(),
            real: target_real,
        });
        let (read_names, dependencies) =
            self.cg.collect_force_read_signals(&diagnostic_path, rhs)?;
        let mut reads = Vec::with_capacity(read_names.len());
        for name in read_names {
            let index = self
                .cg
                .model
                .signals
                .iter()
                .position(|signal| signal.c_name == name)
                .or_else(|| {
                    name.strip_prefix("llg_net_alias_")
                        .and_then(|name| name.strip_suffix(".visible"))
                        .and_then(|index| index.parse::<usize>().ok())
                        .filter(|index| {
                            self.cg
                                .model
                                .signals
                                .get(*index)
                                .is_some_and(|signal| !signal.net_alias.is_empty())
                        })
                })
                .ok_or_else(|| {
                    format!(
                        "force dependency `{name}` in `{diagnostic_path}` has no lowered storage"
                    )
                })?;
            if !reads.contains(&index) {
                reads.push(index);
            }
        }
        Ok(IrStmt::Force {
            lhs: lh,
            value: Box::new(value),
            eval,
            reads,
            dependencies,
        })
    }

    /// The force RHS converted to its target: a real cast for a real target,
    /// otherwise the packed target width and signedness.
    fn force_value(
        &mut self,
        rhs: NodeId,
        lh: &IrLhs,
        target_real: bool,
        diagnostic_path: &str,
    ) -> Result<IrExpr, String> {
        let rhs_ir = self
            .cg
            .lower_expr(&self.path, rhs)
            .map_err(|error| self.force_error(diagnostic_path, error))?;
        Ok(if target_real {
            let IrLhs::Whole(index) = lh else {
                return Err(format!(
                    "real force target in `{diagnostic_path}` must be a whole variable"
                ));
            };
            let shortreal = match self.cg.model.signal(*index).ty {
                IrType::Real { shortreal } => shortreal,
                _ => false,
            };
            IrExpr::new(
                IrExprKind::CastToReal {
                    a: Box::new(rhs_ir),
                    shortreal,
                },
                0,
                true,
                None,
            )
        } else {
            let width = packed_lhs_width(&self.cg.model, lh)
                .ok_or_else(|| format!("force target in `{}` has no packed width", self.path))?;
            let signed = force_lhs_signed(&self.cg.model, lh);
            let rhs_ir = apply_assignment_expression_width(rhs_ir, width);
            // Force uses the same value-preserving RHS-to-target conversion as
            // ordinary assignments (LRM §10.7). Per-part two-state coercion is
            // applied by the runtime when a force spans a concatenation.
            ir_to_vector(rhs_ir, width, signed)?
        })
    }

    /// Lower a force whose RHS calls a legal helper with effects (visible or
    /// persistent writes, unproved static results or descriptor transport).
    ///
    /// SV 10.6 treats a force RHS like a continuous assignment that is
    /// re-evaluated whenever an operand changes while the force is in effect.
    /// The force statement evaluates the RHS in its own process into a hidden
    /// per-site source, then binds the target to that source with an ordinary
    /// read-only evaluator, so the force takes effect immediately (SV 10.6.2).
    /// A synthetic guard process per site waits on the RHS read set and
    /// re-evaluates it into the source only while a live force binding still
    /// reads that source. The helper's effects therefore publish from a
    /// process in the Active region, like a continuous assignment's, and never
    /// run after release or replacement. Evaluation counts are unspecified:
    /// one at the force statement and one per guard wake while in effect.
    fn effectful_force(
        &mut self,
        h: NodeId,
        rhs: NodeId,
        lhs: IrLhs,
        value: IrExpr,
        real: bool,
        diagnostic_path: &str,
    ) -> Result<IrStmt, String> {
        let source = self.cg.model.signals.len();
        let ty = if real {
            let IrExprKind::CastToReal { shortreal, .. } = value.kind() else {
                return Err("real force value lost its real conversion".to_owned());
            };
            IrType::Real {
                shortreal: *shortreal,
            }
        } else {
            IrType::Packed {
                width: value.width,
                signed: value.signed,
                two_state: false,
            }
        };
        self.cg.model.signals.push(IrSignal {
            fixed_default: None,
            c_name: format!("llg_force_src_{source}"),
            hdl_name: None,
            ty,
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        let read = IrExpr::new(IrExprKind::SigRead(source), ty.width(), ty.signed(), None);
        // The guard is a top-level process: re-lower the RHS at process call
        // depth, since a force inside a task body lowers its calls relative
        // to that body's frame.
        let saved_depth = std::mem::replace(&mut self.cg.depth_arg, "0".to_owned());
        let guard_value = self.force_value(rhs, &lhs, real, diagnostic_path);
        self.cg.depth_arg = saved_depth;
        let guard_value = guard_value?;
        let evaluate = IrStmt::Assign {
            lhs: IrLhs::Whole(source),
            rhs: value,
            nba: false,
        };
        let sens = self.cg.collect_evaluator_sensitivity(&self.path, rhs)?;
        let guard = vec![
            IrStmt::WaitAny { sens },
            IrStmt::If {
                cond: IrExpr::new(
                    IrExprKind::RuntimeQuery(IrRuntimeQuery::ForceSourceActive(source)),
                    1,
                    false,
                    None,
                ),
                then_: vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(source),
                    rhs: guard_value,
                    nba: false,
                }],
                els: None,
                check: IrUniquePriorityCheck::None,
            },
        ];
        let guard_name = self.cg.new_fn_name(&self.path, "force_source");
        let origin = self.cg.origin(h);
        self.cg.model.processes.push(IrProcess::new_with_origin(
            guard_name,
            format!("{}.force_source", self.path),
            IrShape::Loop,
            Vec::new(),
            guard,
            origin,
        ));
        let eval = self.cg.new_fn_name(&self.path, "force_eval");
        self.pre_fns.push(crate::sim::ir::IrPreFn::ForceEval {
            c_name: eval.clone(),
            value: read.clone(),
            real,
        });
        Ok(IrStmt::Block(vec![
            evaluate,
            IrStmt::Force {
                lhs,
                value: Box::new(read),
                eval,
                reads: vec![source],
                dependencies: Vec::new(),
            },
        ]))
    }

    /// Lower `release lhs;` against the same canonical target descriptor used
    /// by `force`, so selected nets release only their matching overlay.
    pub(super) fn lower_release(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let NodeKind::Stmt(StmtKind::Release { lhs }) = self.cg.kind(h) else {
            return Err("expected a typed release statement".to_string());
        };
        let diagnostic_path = self.force_diagnostic_path(h);
        let lhs = *lhs;
        if let Some(target) = self.nested_subroutine_auto_ref(lhs) {
            return Err(format!(
                "release target in `{diagnostic_path}` cannot refer to automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        let lh = self
            .cg
            .lower_lhs(&self.path, lhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        validate_force_lhs(&self.cg.model, &lh, &diagnostic_path)?;
        Ok(IrStmt::Release { lhs: lh })
    }

    pub(super) fn nested_subroutine_auto_ref(&self, node: NodeId) -> Option<NodeId> {
        if let Some(target) = self.cg.subroutine_auto_ref(node) {
            return Some(target);
        }
        self.cg
            .node(node)
            .children
            .iter()
            .find_map(|child| self.nested_subroutine_auto_ref(*child))
    }

    fn force_error(&self, diagnostic_path: &str, error: String) -> String {
        if diagnostic_path == self.path {
            error
        } else {
            format!("{error} (source location: {diagnostic_path})")
        }
    }

    fn force_diagnostic_path(&self, node: NodeId) -> String {
        let source = self.cg.node(node);
        match (source.file.as_deref(), source.line, source.col) {
            (Some(file), line, col) if line > 0 => {
                format!("{} at {file}:{line}:{col}", self.path)
            }
            _ => self.path.clone(),
        }
    }

    /// Source-located rejection of a construct unsupported by design.
    pub(super) fn legacy_unsupported(
        &self,
        node: NodeId,
        construct: &str,
        family: LegacyFamily,
    ) -> String {
        self.cg
            .legacy_unsupported_at(&self.path, node, construct, family)
    }

    /// Procedural `assign`/`deassign` (IEEE 1364-2001 9.3.1) is unsupported
    /// in every form by user decision (2026-10-09). `legacy_unsupported::scan`
    /// reports every reachable statement before lowering; this covers
    /// statements the reachability walk does not visit (for example class
    /// method bodies).
    pub(super) fn reject_procedural_assign(&self, h: NodeId, stmt: &str) -> String {
        self.legacy_unsupported(
            h,
            &format!("procedural `{stmt}`"),
            LegacyFamily::ProceduralAssign,
        )
    }
}
