//! Clocking context.

use super::*;

impl<'a> Codegen<'a> {

    pub(super) fn sampled_signal_of(&self, id: NodeId) -> Option<&SignalInfo> {
        let id = self.db.resolve_clocking_member(id).unwrap_or(id);
        self.clocking_samples.get(&id).map(|sample| &sample.sample)
    }

    pub(super) fn clocking_var_source_info(&self, target: NodeId) -> Option<&SignalInfo> {
        let source = self.db.clocking_var(target)?.source;
        self.signal_of(source)
    }

    pub(super) fn clocking_var_read_source_info(&self, target: NodeId) -> Result<Option<&SignalInfo>, String> {
        let Some(target) = self
            .clocking_var_target(target)
            .or_else(|| self.db.is_clocking_var(target).then_some(target))
        else {
            return Ok(None);
        };
        let Some(var) = self.db.clocking_var(target) else {
            return Ok(None);
        };
        if matches!(var.direction, DbDirection::Output) {
            return Err(format!(
                "clocking output member `{}` is write-only",
                self.node(target).name
            ));
        }
        Ok(self.clocking_var_source_info(target))
    }

    pub(super) fn ensure_clocking_readable(&self, node: NodeId) -> Result<(), String> {
        let _ = self.clocking_var_read_source_info(node)?;
        Ok(())
    }

    pub(super) fn clocking_var_target(&self, node: NodeId) -> Option<NodeId> {
        if let Some(target) = self.db.resolve_clocking_member(node) {
            return self.db.is_clocking_var(target).then_some(target);
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => {
                target.filter(|target| self.db.is_clocking_var(*target))
            }
            NodeKind::Expr(ExprKind::ScopeRef { target }) => {
                self.db.is_clocking_var(*target).then_some(*target)
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .find(|target| self.db.is_clocking_var(**target))
                .copied(),
            _ => None,
        }
    }

    /// Collect clocking variables covered by an assignment target. A target
    /// containing any clocking member must consist entirely of clocking
    /// members; mixing ordinary and clocking destinations would require
    /// different scheduling rules within one concatenation.
    pub(super) fn clocking_lhs_targets(&self, node: NodeId, targets: &mut Vec<NodeId>) -> bool {
        if let Some(target) = self.clocking_var_target(node) {
            targets.push(target);
            return true;
        }
        match self.kind(node) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.clocking_lhs_targets(*base, targets),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) => {
                let mut all_clocking = true;
                for operand in operands {
                    if !self.clocking_lhs_targets(*operand, targets) {
                        all_clocking = false;
                    }
                }
                all_clocking
            }
            NodeKind::Expr(ExprKind::Streaming { streams, .. }) => {
                let mut all_clocking = true;
                for stream in streams {
                    if !self.clocking_lhs_targets(stream.value, targets) {
                        all_clocking = false;
                    }
                }
                all_clocking
            }
            _ => false,
        }
    }

    pub(super) fn default_clocking_block(&self, inst: NodeId) -> Option<NodeId> {
        let mut current = Some(inst);
        while let Some(scope) = current {
            if let Some(block) = self.node(scope).children.iter().copied().find(|child| {
                self.db
                    .clocking_block(*child)
                    .is_some_and(|info| info.is_default)
            }) {
                return Some(block);
            }
            current = self.node(scope).parent;
        }
        None
    }

    fn clocking_output_skew(&self, target: NodeId, path: &str) -> Result<ClockingSkew, String> {
        let var = self.db.clocking_var(target).ok_or_else(|| {
            format!(
                "clocking member `{}` is not available in `{path}`",
                self.node(target).name
            )
        })?;
        let block = self.db.clocking_block(var.block).ok_or_else(|| {
            format!(
                "clocking member `{}` has no owning block in `{path}`",
                self.node(target).name
            )
        })?;
        let skew = if var.output.delay.is_some() || !matches!(var.output.edge, ClockingEdge::None) {
            &var.output
        } else {
            &block.default_output
        };
        Ok(skew.clone())
    }

    pub(super) fn clocking_output_edge(&self, target: NodeId, path: &str) -> Result<ClockingEdge, String> {
        Ok(self.clocking_output_skew(target, path)?.edge)
    }

    pub(super) fn clocking_output_delay(&mut self, target: NodeId, path: &str) -> Result<IrDelay, String> {
        let skew = self.clocking_output_skew(target, path)?;
        let Some(delay) = skew.delay else {
            // An omitted clocking output skew is the LRM default `#0`.
            return Ok(IrDelay::Constant(0));
        };
        if self.db.semantic_detail(delay) == Some("OneStepDelay") {
            return Ok(IrDelay::Constant(1));
        }
        let expression = skew.delay_expression.ok_or_else(|| {
            format!(
                "clocking output skew for `{}` in `{path}` is not a constant timing control",
                self.node(target).name
            )
        })?;
        Ok(IrDelay::Constant(
            self.procedural_delay_ticks(delay, expression)?,
        ))
    }

    /// Allocate one hidden sampled storage cell for every input/inout clocking
    /// member. Sources are resolved through the ordinary signal table after
    /// net groups have been built, so aliases and resolved nets retain their
    /// canonical storage identity.
    pub(super) fn collect_clocking_storage(&mut self) -> Result<(), String> {
        let design: HashSet<NodeId> = self.design_nodes().into_iter().collect();
        let blocks: Vec<NodeId> = self
            .design_nodes()
            .into_iter()
            .filter(|id| self.db.clocking_block(*id).is_some())
            .collect();
        for block in blocks {
            let parent = self
                .node(block)
                .parent
                .filter(|parent| design.contains(parent));
            let scope = parent
                .map(|parent| self.instance_path_of(parent))
                .unwrap_or_else(|| self.design_name.clone());
            let block_name = ident(&self.node(block).name);
            let event = self.new_event_info(format!("E_clocking_{}", block.index()));
            self.event_globals.insert(block, event);
            for var in self.node(block).children.iter().copied() {
                let Some(var_info) = self.db.clocking_var(var) else {
                    continue;
                };
                if !matches!(var_info.direction, DbDirection::Input | DbDirection::Inout) {
                    continue;
                }
                if self.clocking_samples.contains_key(&var) {
                    continue;
                }
                let source = self.signal_of(var_info.source).cloned().ok_or_else(|| {
                    format!(
                        "clocking variable `{}` source `{}` is not a collected packed signal in `{scope}`",
                        self.node(var).name,
                        self.node(var_info.source).full_name
                    )
                })?;
                if source.real {
                    return Err(format!(
                        "real-valued clocking input `{}` is not supported in `{scope}`",
                        self.node(var).name
                    ));
                }
                if source.width > LLG_MAX_WIDTH {
                    return Err(format!(
                        "clocking input `{}` in `{scope}` exceeds the runtime width limit",
                        self.node(var).name
                    ));
                }
                let storage_name = format!("{}_{}_sample", block_name, ident(&self.node(var).name));
                let global = global_name(&scope, &storage_name);
                let ir = self.model.signals.len();
                let sample = SignalInfo {
                    global: global.clone(),
                    width: source.width,
                    signed: source.signed,
                    two_state: source.two_state,
                    real: false,
                    shortreal: false,
                    net_driver: None,
                    ir,
                };
                self.model.signals.push(IrSignal {
                    c_name: global,
                    hdl_name: None,
                    ty: IrType::Packed {
                        width: source.width,
                        signed: source.signed,
                        two_state: source.two_state,
                    },
                    net_driver: None,
                    net_alias: Vec::new(),
                    alias: None,
                    omit: false,
                });
                self.signals.push(sample.clone());
                self.clocking_samples.insert(
                    var,
                    ClockingSampleInfo {
                        source: var_info.source,
                        sample,
                    },
                );
            }
        }
        Ok(())
    }

    fn clocking_sample_mode(
        &mut self,
        skew: &ClockingSkew,
        path: &str,
    ) -> Result<IrClockingSampleMode, String> {
        let Some(delay) = skew.delay else {
            return Ok(IrClockingSampleMode::OneStep);
        };
        if matches!(self.kind(delay), NodeKind::Other)
            && self.db.semantic_detail(delay) == Some("OneStepDelay")
        {
            return Ok(IrClockingSampleMode::OneStep);
        }
        let expression = skew.delay_expression.ok_or_else(|| {
            format!("clocking skew in `{path}` has unsupported non-constant timing control")
        })?;
        let ticks = self.procedural_delay_ticks(delay, expression)?;
        if ticks == 0 {
            Ok(IrClockingSampleMode::Observed)
        } else {
            Ok(IrClockingSampleMode::History(ticks))
        }
    }

    /// Emit one synthetic sampler process per clocking block. The process
    /// waits on the underlying edge, updates input members, then publishes the
    /// distinct block event after its Observed samples;
    /// input skews override block defaults and an omitted skew means #1step.
    pub(super) fn emit_clocking_processes(&mut self) -> Result<(), String> {
        let blocks: Vec<NodeId> = self
            .design_nodes()
            .into_iter()
            .filter(|id| self.db.clocking_block(*id).is_some())
            .collect();
        for block in blocks {
            let Some(block_info) = self.db.clocking_block(block).cloned() else {
                continue;
            };
            let Some(parent) = self.node(block).parent else {
                return Err(format!(
                    "clocking block `{}` has no owning instance",
                    self.node(block).name
                ));
            };
            let path = self.instance_path_of(parent);
            let process_name =
                self.new_fn_name(&path, &format!("clocking_{}", self.node(block).name));
            let (event_specs, pre_fns) = {
                let mut ctx = EmitCtx::new(self, path.clone(), parent, "0", None, None, false);
                let event_specs = ctx.lower_event_specs(&block_info.event_specs)?;
                (event_specs, std::mem::take(&mut ctx.pre_fns))
            };
            let mut body = vec![IrStmt::WaitEvents { specs: event_specs }];
            for var in self.node(block).children.iter().copied() {
                let Some(var_info) = self.db.clocking_var(var) else {
                    continue;
                };
                if !matches!(var_info.direction, DbDirection::Input | DbDirection::Inout) {
                    continue;
                }
                let Some(storage) = self.clocking_samples.get(&var).cloned() else {
                    continue;
                };
                let skew = if var_info.input.delay.is_some()
                    || !matches!(var_info.input.edge, ClockingEdge::None)
                {
                    &var_info.input
                } else {
                    &block_info.default_input
                };
                let mode = self.clocking_sample_mode(skew, &path)?;
                body.push(IrStmt::ClockingSample {
                    source: self
                        .signal_of(storage.source)
                        .ok_or_else(|| "clocking source storage disappeared".to_owned())?
                        .ir,
                    sample: storage.sample.ir,
                    mode,
                });
            }
            body.push(IrStmt::ClockingEventTrigger {
                ev: IrEventRef::Static(self.event_globals[&block].ir),
            });
            self.model.processes.push(IrProcess::new_with_origin(
                process_name,
                format!("{}.clocking", path),
                IrShape::Loop,
                pre_fns,
                body,
                crate::sim::semantic::Origin::Synthetic {
                    reason: format!("clocking input sampler for {}", self.node(block).full_name),
                },
            ));
        }
        Ok(())
    }
}
