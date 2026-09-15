//! Drivers.

use super::*;

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
        let lh = self
            .cg
            .lower_lhs(&self.path, lhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        let target_real = validate_force_lhs(&self.cg.model, &lh, &diagnostic_path)?;
        let rhs_ir = self
            .cg
            .lower_expr(&self.path, rhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        let value = if target_real {
            let IrLhs::Whole(index) = &lh else {
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
            let width = packed_lhs_width(&self.cg.model, &lh)
                .ok_or_else(|| format!("force target in `{}` has no packed width", self.path))?;
            let signed = force_lhs_signed(&self.cg.model, &lh);
            let rhs_ir = apply_assignment_expression_width(rhs_ir, width);
            // Force uses the same value-preserving RHS-to-target conversion as
            // ordinary assignments (LRM §10.7). Per-part two-state coercion is
            // applied by the runtime when a force spans a concatenation.
            ir_to_vector(rhs_ir, width, signed)?
        };
        let eval = self.cg.new_fn_name(&self.path, "force_eval");
        self.pre_fns.push(crate::sim::ir::IrPreFn::ForceEval {
            c_name: eval.clone(),
            value: value.clone(),
            real: target_real,
        });
        let read_names = self.cg.collect_force_read_signals(&diagnostic_path, rhs)?;
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
            value,
            eval,
            reads,
        })
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

    fn nested_subroutine_auto_ref(&self, node: NodeId) -> Option<NodeId> {
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

    /// Collect the plain source nodes that make up a legal ordinary packed
    /// concatenation target. Streaming targets are a different language
    /// construct and remain outside procedural continuous assignment support.
    fn pca_source_nodes(&self, lhs: NodeId, out: &mut Vec<NodeId>) -> Result<(), String> {
        match self.cg.kind(lhs) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                reordered,
                operands,
                ..
            }) => {
                let mut operands = operands.clone();
                if *reordered {
                    operands.reverse();
                }
                for operand in operands {
                    self.pca_source_nodes(operand, out)?;
                }
                Ok(())
            }
            NodeKind::Expr(ExprKind::Streaming { .. }) => Err(format!(
                "procedural continuous assignment on a streaming target in `{}` is not \
                 supported",
                self.path
            )),
            _ => {
                out.push(lhs);
                Ok(())
            }
        }
    }

    fn pca_lhs_parts(&self, lhs: &IrLhs, out: &mut Vec<usize>, stmt: &str) -> Result<(), String> {
        match lhs {
            IrLhs::Whole(index) => out.push(*index),
            IrLhs::Stream { parts, .. } => {
                for (part, _) in parts {
                    self.pca_lhs_parts(part, out, stmt)?;
                }
            }
            _ => {
                return Err(format!(
                    "procedural continuous `{stmt}` on a select in `{}` is not supported \
                     (whole variables and ordinary concatenations only)",
                    self.path
                ));
            }
        }
        Ok(())
    }

    /// Resolve the target of a procedural continuous `assign` / `deassign`.
    /// Variables only (LRM 1364-1995 §9.4): nets, selects/part-selects/
    /// array elements, hierarchical paths and streaming targets are rejected
    /// cleanly. Whole real variables and ordinary packed concatenations are
    /// represented as several target bindings sharing one site identity.
    fn pca_targets(&mut self, lhs: NodeId, stmt: &str) -> Result<Vec<(usize, SignalInfo)>, String> {
        let mut source_nodes = Vec::new();
        self.pca_source_nodes(lhs, &mut source_nodes)?;
        for source in &source_nodes {
            if matches!(
                self.cg.kind(*source),
                NodeKind::Expr(ExprKind::HierPath { .. })
            ) {
                return Err(format!(
                    "procedural continuous `{stmt}` on a hierarchical target in `{}` is not \
                     supported (variables of the current scope only)",
                    self.path
                ));
            }
            // Net targets: the elaborated ref normally binds the declaration,
            // so the net/var distinction is read straight off the arena node.
            // A module-level `reg` can be captured as a net node, but its
            // semantic net type still identifies it as variable storage.
            if let NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) = self.cg.kind(*source)
            {
                if let NodeKind::Net { net_type, .. } = self.cg.kind(*target) {
                    if *net_type != NetType::Reg {
                        return Err(format!(
                            "procedural continuous `{stmt}` on net `{}` in `{}` is not supported \
                             (variables only)",
                            self.cg.node(*target).name,
                            self.path
                        ));
                    }
                }
            }
            if !matches!(
                self.cg.kind(*source),
                NodeKind::Var { .. } | NodeKind::Expr(ExprKind::Ref { .. })
            ) {
                return Err(format!(
                    "procedural continuous `{stmt}` on a select in `{}` is not supported \
                     (whole variables and ordinary concatenations only)",
                    self.path
                ));
            }
        }

        let lowered = self.cg.lower_lhs(&self.path, lhs)?;
        let mut indices = Vec::new();
        self.pca_lhs_parts(&lowered, &mut indices, stmt)?;
        if indices.len() != source_nodes.len() {
            return Err(format!(
                "procedural continuous `{stmt}` target shape in `{}` is not supported",
                self.path
            ));
        }
        let mut targets = Vec::with_capacity(indices.len());
        for sig_idx in indices {
            let info = self
                .cg
                .signals
                .iter()
                .find(|info| info.ir == sig_idx)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "cannot collect `{}` in `{}` as a procedural continuous assignment \
                         target (whole variables of the current scope only)",
                        self.cg.node(lhs).name,
                        self.path
                    )
                })?;
            targets.push((sig_idx, info));
        }
        if targets.len() > 1 && targets.iter().any(|(_, info)| info.real) {
            return Err(format!(
                "procedural continuous `{stmt}` concatenation in `{}` must contain only \
                 packed variables",
                self.path
            ));
        }
        Ok(targets)
    }

    /// Pre-scan phase 1 for one process body: resolve each collected
    /// ProcContAssign target and allocate its site (enable signal) without
    /// lowering anything. Distinct sites may target the same variable; the
    /// runtime replaces the active binding when one executes.
    pub(in super::super) fn claim_pca_sites(&mut self, nodes: &[NodeId]) -> Result<(), String> {
        for n in nodes {
            let NodeKind::Stmt(StmtKind::ProcContAssign { lhs, .. }) = self.cg.kind(*n) else {
                return Err("expected a typed procedural continuous assignment".to_string());
            };
            let lhs = *lhs;
            let targets = self.pca_targets(lhs, "assign")?;
            let existing = targets.iter().find_map(|(sig_idx, _)| {
                self.cg
                    .pca_sites
                    .get(&(*n, *sig_idx))
                    .map(|site| (site.en, site.site))
            });
            let (en, site) = existing.unwrap_or_else(|| {
                let site = self.cg.pca_seq;
                let en = self.cg.new_pca_enable(&self.path);
                (en, site)
            });
            for (sig_idx, _) in targets {
                self.cg.pca_sites.entry((*n, sig_idx)).or_insert(PcaSite {
                    en,
                    site,
                    guarded_by: None,
                });
            }
        }
        Ok(())
    }

    /// Lower `assign <variable> = expr;` (procedural continuous assignment,
    /// LRM 1364-1995 §9.4).  Decided model, one process per SITE:
    ///
    /// ```text
    /// for (;;) {                       // IrShape::Loop guard process
    ///     wait_any(rhs_reads ∪ en);    // en changes wake the guard too
    ///     if (en) write(lhs, rhs_now); // disabled wakes skip silently
    /// }
    /// ```
    ///
    /// The statement activates its runtime binding and performs an immediate
    /// write (so the value updates in the same delta); `deassign` removes the
    /// binding and the variable KEEPS its last value (LRM). Distinct sites
    /// share one target-level binding, so a later execution replaces the
    /// earlier site.
    ///
    /// NBA-vs-PCA interplay: while a site is enabled, ordinary procedural
    /// writes to the variable — blocking AND non-blocking — are suppressed by
    /// the runtime; the guard never wakes on changes of the TARGET itself.
    ///
    /// Sites are pre-allocated by [`Codegen::prescan_pca_sites`] before any
    /// body lowers; this method claims the pre-allocated site by materializing
    /// its guard process.
    pub(super) fn lower_proc_cont_assign(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let NodeKind::Stmt(StmtKind::ProcContAssign { lhs, rhs }) = self.cg.kind(h) else {
            return Err("expected a typed procedural continuous assignment".to_string());
        };
        let (lhs, rhs) = (*lhs, *rhs);
        if self.func.is_some() || self.inline.is_some() {
            return Err(format!(
                "procedural continuous assignment in `{}` cannot escape a function or task activation",
                self.path
            ));
        }
        if let Some(target) = self.cg.nested_capture_ref(rhs) {
            return Err(format!(
                "procedural continuous assignment in `{}` cannot capture activation storage `{}`",
                self.path,
                self.cg.node(target).name
            ));
        }
        let targets = self.pca_targets(lhs, "assign")?;
        let (first_sig, first_info) = targets.first().cloned().ok_or_else(|| {
            format!(
                "procedural continuous assignment in `{}` has no target",
                self.path
            )
        })?;

        // The dedicated guard process: waits on the RHS read set ∪ {en} and
        // re-writes the CURRENT rhs whenever it wakes while enabled.
        let mut sens = Vec::new();
        let mut seen = HashSet::new();
        let mut visited = HashSet::new();
        self.cg
            .walk_read_signals(&self.path, rhs, &mut seen, &mut visited, &mut sens)?;
        let rhs_ir = self.cg.lower_expr(&self.path, rhs)?;
        let values = if first_info.real {
            if targets.len() != 1 {
                return Err(format!(
                    "procedural continuous assignment target in `{}` mixes real and packed values",
                    self.path
                ));
            }
            vec![(first_sig, rhs_ir)]
        } else {
            let total_width = targets
                .iter()
                .try_fold(0u32, |width, (_, info)| width.checked_add(info.width))
                .ok_or_else(|| {
                    format!(
                        "procedural continuous assignment target width overflows in `{}`",
                        self.path
                    )
                })?;
            let rhs_ir = apply_assignment_expression_width(rhs_ir, total_width);
            let rhs_ir = ir_to_storage(rhs_ir, total_width, false, false)?;
            let mut cursor = total_width;
            let mut values = Vec::with_capacity(targets.len());
            for (sig_idx, info) in &targets {
                let right = cursor.checked_sub(info.width).ok_or_else(|| {
                    format!(
                        "procedural continuous assignment target width underflows in `{}`",
                        self.path
                    )
                })?;
                let part = IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(rhs_ir.clone()),
                        left: i64::from(cursor - 1),
                        right: i64::from(right),
                    },
                    info.width,
                    false,
                    None,
                );
                values.push((
                    *sig_idx,
                    ir_to_storage(part, info.width, info.signed, info.two_state)?,
                ));
                cursor = right;
            }
            values
        };
        // Site bookkeeping. Sites exist for every process-body statement
        // already (pre-scan); a missing entry is only reachable from trees
        // the pre-scan does not walk (defensive fallback with identical
        // semantics). A delay-bearing task body can be lowered at several
        // call sites, so its one guard and site identity are reused.
        let existing = self
            .cg
            .pca_sites
            .get(&(h, first_sig))
            .map(|s| (s.en, s.site, s.guarded_by));
        let (en_ir, site_id) = match existing {
            Some((en, site, Some(_))) => {
                return Ok(values
                    .into_iter()
                    .map(|(sig, value)| IrStmt::PcaAssign {
                        sig,
                        enable: en,
                        site,
                        value,
                    })
                    .collect());
            }
            Some((en, site, None)) => {
                for (sig, _) in &targets {
                    if let Some(pca) = self.cg.pca_sites.get_mut(&(h, *sig)) {
                        pca.guarded_by = Some(h);
                    }
                }
                (en, site)
            }
            None => {
                let site = self.cg.pca_seq;
                let en = self.cg.new_pca_enable(&self.path);
                for (sig, _) in &targets {
                    self.cg.pca_sites.insert(
                        (h, *sig),
                        PcaSite {
                            en,
                            site,
                            guarded_by: Some(h),
                        },
                    );
                }
                (en, site)
            }
        };
        let en_global = self.cg.model.signals[en_ir].c_name.clone();
        let en_dependency = crate::sim::ir::IrDependency::scalar(en_global);
        if !sens.contains(&en_dependency) {
            sens.push(en_dependency);
        }
        let drives = values
            .iter()
            .map(|(sig, value)| IrStmt::PcaDrive {
                sig: *sig,
                enable: en_ir,
                site: site_id,
                value: value.clone(),
            })
            .collect();
        let guard_body = vec![
            IrStmt::WaitAny { sens },
            IrStmt::If {
                cond: IrExpr::new(IrExprKind::SigRead(en_ir), 1, false, None),
                then_: drives,
                els: None,
                check: IrUniquePriorityCheck::None,
            },
        ];
        let guard_name = self.cg.new_fn_name(&self.path, "pca");
        let origin = self.cg.origin(h);
        self.cg.model.processes.push(IrProcess::new_with_origin(
            guard_name,
            format!("{}.pca", self.path),
            IrShape::Loop,
            Vec::new(),
            guard_body,
            origin,
        ));

        // Statement execution activates/replaces the binding and immediately
        // drives the target. The runtime drops the target write while forced
        // but retains the evaluated RHS for release.
        Ok(values
            .into_iter()
            .map(|(sig, value)| IrStmt::PcaAssign {
                sig,
                enable: en_ir,
                site: site_id,
                value,
            })
            .collect())
    }

    /// Lower `deassign <variable>;` — remove the target's active procedural
    /// continuous assignment binding. The variable KEEPS its last assigned
    /// value (LRM 1364-1995 §9.4). Deassign before any assign is harmless;
    /// net/select/hierarchical targets are rejected like `assign` targets.
    pub(super) fn lower_deassign(&mut self, lhs: NodeId) -> Result<Vec<IrStmt>, String> {
        Ok(self
            .pca_targets(lhs, "deassign")?
            .into_iter()
            .map(|(sig, _)| IrStmt::PcaDeassign { sig })
            .collect())
    }
}
