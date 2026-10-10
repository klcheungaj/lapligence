//! Clocking context.

use super::*;

impl<'a> Codegen<'a> {
    pub(super) fn sampled_signal_of(&self, id: NodeId) -> Option<&SignalInfo> {
        let id = self.db.resolve_clocking_member(id).unwrap_or(id);
        self.clocking_samples.get(&id).map(|sample| &sample.sample)
    }

    pub(super) fn clocking_var_source_info(&self, target: NodeId) -> Option<&SignalInfo> {
        let source = self.db.clocking_var(target)?.source?;
        self.signal_of(source)
    }

    pub(super) fn clocking_var_read_source_info(
        &self,
        target: NodeId,
    ) -> Result<Option<&SignalInfo>, String> {
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
        if let Some(block) = self
            .owner_instance(inst)
            .and_then(|inst| self.db.default_clocking(inst))
        {
            return Some(block);
        }
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

    pub(super) fn clocking_output_edge(
        &self,
        target: NodeId,
        path: &str,
    ) -> Result<ClockingEdge, String> {
        Ok(self.clocking_output_skew(target, path)?.edge)
    }

    pub(super) fn clocking_output_delay(
        &mut self,
        target: NodeId,
        path: &str,
    ) -> Result<IrDelay, String> {
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
    /// canonical storage identity. A clockvar bound to anything but one whole
    /// packed signal also gets a hidden packed image of its expression, which
    /// `emit_clocking_processes` keeps current; the sample has the clockvar's
    /// own type (SV §14.5).
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
            let block_name = self.node(block).name.clone();
            let event = self.new_event_info(format!("E_clocking_{}", block.index()));
            self.event_globals.insert(block, event);
            for var in self.node(block).children.iter().copied() {
                let Some(var_info) = self.db.clocking_var(var).cloned() else {
                    continue;
                };
                if !matches!(var_info.direction, DbDirection::Input | DbDirection::Inout) {
                    continue;
                }
                if self.clocking_samples.contains_key(&var) {
                    continue;
                }
                let var_name = self.node(var).name.clone();
                let direct = var_info
                    .source
                    .and_then(|source| self.signal_of(source))
                    .filter(|source| !source.real)
                    .cloned();
                let sample_global = |cg: &Self, prefix: &str| {
                    cg.c_name(prefix, &scope, &[&block_name, &var_name, "sample"])
                };
                let (source, sample) = if let Some(source) = direct {
                    if source.width > LLG_MAX_WIDTH {
                        return Err(format!(
                            "clocking input `{var_name}` in `{scope}` exceeds the runtime width limit"
                        ));
                    }
                    let global = sample_global(self, "G");
                    let sample = self.push_clocking_signal(
                        global,
                        IrType::Packed {
                            width: source.width,
                            signed: source.signed,
                            two_state: source.two_state,
                        },
                    );
                    (source, sample)
                } else {
                    let ty = self.clocking_var_value_type(var, &scope)?;
                    let real = matches!(ty, IrType::Real { .. });
                    // A real value travels through the packed sampler as its
                    // exact IEEE image; a shortreal widens to real exactly.
                    let image_ty = if real {
                        IrType::Packed {
                            width: 64,
                            signed: false,
                            two_state: true,
                        }
                    } else {
                        ty
                    };
                    let image_global = self.c_name("G", &scope, &[&block_name, &var_name, "image"]);
                    let image = self.push_clocking_signal(image_global, image_ty);
                    let global = sample_global(self, if real { "D" } else { "G" });
                    let sample = self.push_clocking_signal(global, ty);
                    self.clocking_expression_sources
                        .push(ClockingExpressionSource {
                            variable: var,
                            expression: var_info.expression,
                            image: image.clone(),
                            real,
                        });
                    (image, sample)
                };
                self.clocking_samples
                    .insert(var, ClockingSampleInfo { source, sample });
            }
        }
        Ok(())
    }

    /// The sampled storage type of an expression-backed clockvar: its own
    /// declared type, limited to packed integral and real values.
    fn clocking_var_value_type(&self, var: NodeId, scope: &str) -> Result<IrType, String> {
        let name = &self.node(var).name;
        let NodeKind::Var { ty } = self.kind(var) else {
            return Err(format!(
                "clocking member `{name}` in `{scope}` is not a variable declaration"
            ));
        };
        if ty.kind == "chandle" {
            // A clocking input must be a legal input port connection (SV
            // §14.5), and ports shall not have the chandle type (§6.14).
            return Err(format!(
                "chandle clocking input `{name}` in `{scope}` is illegal: chandles cannot be \
                 clocking signals (SV 6.14, 14.5)"
            ));
        }
        let unsupported = || {
            format!(
                "clocking input `{name}` in `{scope}` has type `{}`; sampled clockvars are \
                 limited to packed integral, real and shortreal values",
                self.db
                    .type_descriptor(var)
                    .map_or(ty.kind.as_str(), |descriptor| descriptor.name.as_str())
            )
        };
        match self
            .db
            .type_descriptor(var)
            .map(|descriptor| &descriptor.shape)
        {
            Some(TypeShape::Real { shortreal }) => {
                return Ok(IrType::Real {
                    shortreal: *shortreal,
                })
            }
            Some(TypeShape::PackedAtom { .. }) | None => {}
            Some(_) => return Err(unsupported()),
        }
        if is_real_kind(&ty.kind) {
            return Ok(IrType::Real {
                shortreal: ty.kind == "shortreal",
            });
        }
        let width = self
            .signal_width(scope, name, ty)
            .map_err(|_| unsupported())?;
        if width == 0 {
            return Err(unsupported());
        }
        Ok(IrType::Packed {
            width,
            signed: ty.signed,
            two_state: self.db.is_two_state_type(var) || is_two_state_kind(&ty.kind),
        })
    }

    /// Register one hidden clocking storage cell in the model.
    fn push_clocking_signal(&mut self, global: String, ty: IrType) -> SignalInfo {
        let ir = self.model.signals.len();
        let (width, signed, two_state, real, shortreal) = match ty {
            IrType::Packed {
                width,
                signed,
                two_state,
            } => (width, signed, two_state, false, false),
            IrType::Real { shortreal } => (0, false, false, true, shortreal),
        };
        let info = SignalInfo {
            global: global.clone(),
            width,
            signed,
            two_state,
            real,
            shortreal,
            net_driver: None,
            ir,
        };
        self.model.signals.push(IrSignal {
            fixed_default: None,
            c_name: global,
            hdl_name: None,
            ty,
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        self.signals.push(info.clone());
        info
    }

    /// The packed image value of an expression-backed clockvar.
    fn clocking_image_value(
        &mut self,
        path: &str,
        source: &ClockingExpressionSource,
    ) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, source.expression)?;
        if source.real {
            let value = if value.is_real() {
                value
            } else {
                IrExpr::new(
                    IrExprKind::SysFunc(Box::new(IrSysFunc::Itor(Box::new(value)))),
                    0,
                    false,
                    None,
                )
            };
            return Ok(IrExpr::new(
                IrExprKind::SysFunc(Box::new(IrSysFunc::RealToBits(Box::new(value)))),
                64,
                false,
                None,
            ));
        }
        let image = &source.image;
        let value = apply_assignment_expression_width(value, image.width);
        ir_to_storage(value, image.width, image.signed, image.two_state)
    }

    /// Keep every expression-backed clockvar image equal to its expression:
    /// a static initializer gives the Preponed region of time zero the
    /// declaration-time value, and a synthetic continuous evaluation follows
    /// later changes, so every input skew samples the settled expression.
    fn emit_clocking_expression_sources(&mut self) -> Result<(), String> {
        for source in self.clocking_expression_sources.clone() {
            let block = self
                .db
                .clocking_var(source.variable)
                .map(|info| info.block)
                .ok_or_else(|| "clocking expression source lost its declaration".to_owned())?;
            let parent = self.node(block).parent.ok_or_else(|| {
                format!(
                    "clocking block `{}` has no owning instance",
                    self.node(block).name
                )
            })?;
            let path = self.instance_path_of(parent);
            // Callee resolution and hierarchical names use the owning instance.
            self.inst = parent;
            let initial = self.clocking_image_value(&path, &source)?;
            self.record_initializer_source(source.variable, source.expression);
            let initialization = IrInitialization::new(
                self.declaration_identity(source.variable)?,
                StorageLifetime::Static,
                self.declaration_init_phase(),
                IrInitTarget::Signal(source.image.ir),
                initial,
                self.origin(source.variable),
            );
            self.declaration_inits.push(initialization);
            let value = self.clocking_image_value(&path, &source)?;
            let reads = self.collect_read_signals(&path, source.expression)?;
            let shape = if reads.is_empty() {
                IrShape::RunOnce
            } else {
                IrShape::SensLoop { reads }
            };
            let variable = self.node(source.variable).name.clone();
            let block_name = self.node(block).name.clone();
            let process_name =
                self.new_fn_name(&path, &format!("clocking_{block_name}_{variable}_image"));
            self.model.processes.push(IrProcess::new_with_origin(
                process_name,
                format!("{path}.{block_name}.{variable}"),
                shape,
                Vec::new(),
                vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(source.image.ir),
                    rhs: value,
                    nba: false,
                }],
                crate::sim::semantic::Origin::Synthetic {
                    reason: format!(
                        "clocking input expression of {}",
                        self.node(source.variable).full_name
                    ),
                },
            ));
        }
        Ok(())
    }

    fn clocking_sample_mode(
        &mut self,
        skew: &ClockingSkew,
        variable: NodeId,
        path: &str,
    ) -> Result<IrClockingSampleMode, String> {
        if !matches!(skew.edge, ClockingEdge::None) {
            // SV §14.3 allows an edge skew but does not define which sample
            // an input takes at it; reject rather than silently use #1step.
            return Err(format!(
                "edge-qualified input skew of clocking input `{}` in `{path}` is not supported",
                self.node(variable).name
            ));
        }
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
        self.emit_clocking_expression_sources()?;
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
            let mut body = vec![IrStmt::WaitEvents {
                specs: event_specs,
                refresh: false,
            }];
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
                let mode = self.clocking_sample_mode(skew, var, &path)?;
                let source = storage.source.ir;
                if let IrClockingSampleMode::History(ticks) = mode {
                    let deepest = self.clocking_history_ticks.entry(source).or_default();
                    *deepest = (*deepest).max(ticks);
                }
                body.push(IrStmt::ClockingSample {
                    source,
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
