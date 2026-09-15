//! Initialization.

use super::*;

impl<'a> Codegen<'a> {

    fn declaration_init_phase(&self) -> IrInitPhase {
        if self.db.edition() == LanguageEdition::SystemVerilog2009 {
            IrInitPhase::BeforeProcesses
        } else {
            IrInitPhase::ActiveRegion
        }
    }

    pub(super) fn declaration_identity(&self, node: NodeId) -> Result<u32, String> {
        u32::try_from(node.index()).map_err(|_| {
            format!(
                "declaration `{}` has an identity outside the simulator IR range",
                self.node(node).name
            )
        })
    }

    /// Resolve the owning elaborated environment for a named procedural
    /// declaration. The owned database keeps every instance clone distinct;
    /// package declarations are one shared environment, while module and
    /// interface declarations use their concrete elaborated instance.
    fn owner_instance(&self, node: NodeId) -> Option<NodeId> {
        let mut current = Some(node);
        while let Some(id) = current {
            if matches!(self.kind(id), NodeKind::ModuleInst { .. })
                || self.is_runtime_environment(id)
            {
                return Some(id);
            }
            current = self.node(id).parent;
        }
        None
    }

    /// Build the typed runtime identity for a named block, task, or fork
    /// scope. Unowned synthetic nodes fail closed instead of using a name.
    pub(super) fn activation_target(
        &self,
        declaration: NodeId,
    ) -> Result<crate::sim::ir::IrActivationTarget, String> {
        let instance = self.owner_instance(declaration).ok_or_else(|| {
            format!(
                "named activation `{}` has no elaborated module, interface, or package environment",
                self.node(declaration).full_name()
            )
        })?;
        Ok(crate::sim::ir::IrActivationTarget::new(
            self.declaration_identity(declaration)?,
            self.declaration_identity(instance)?,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_declaration_initializer(
        &mut self,
        path: &str,
        declaration: NodeId,
        initializer: NodeId,
        target: IrInitTarget,
        width: u32,
        signed: bool,
        two_state: bool,
        real: bool,
    ) -> Result<IrInitialization, String> {
        let value = self.lower_expr(path, initializer)?;
        let value = if real {
            value
        } else {
            let value = apply_assignment_expression_width(value, width);
            ir_to_storage(value, width, signed, two_state)?
        };
        Ok(IrInitialization::new(
            self.declaration_identity(declaration)?,
            StorageLifetime::Static,
            self.declaration_init_phase(),
            target,
            value,
            self.origin(declaration),
        ))
    }

    /// Convert the collected declaration initializers into `main()` init
    /// steps, in application order: ungrouped-net defaults, array fills (+ pattern
    /// elements), then scalar net-decl fills, then variable fills, then
    /// collapsed-net member writes.
    pub(super) fn build_init_steps(&self, model: &mut IrModel) -> Result<(), String> {
        use crate::sim::ir::IrInitStep;
        // Ungrouped nets include ordinary ports, interface members and gate
        // outputs. Their initial Z value must not inherit a variable's X.
        for node in self.design_nodes() {
            let Some(info) = self.sig_globals.get(&node) else {
                continue;
            };
            if matches!(self.kind(node), NodeKind::Net { .. })
                && model.signals[info.ir].net_driver.is_none()
                && !info.real
            {
                let limbs = (info.width as usize).div_ceil(64);
                let mut z = vec![u64::MAX; limbs];
                if !info.width.is_multiple_of(64) {
                    z[limbs - 1] = (1u64 << (info.width % 64)) - 1;
                }
                let value = IrConst::packed(
                    vec![0; limbs],
                    vec![0; limbs],
                    z,
                    info.width,
                    info.signed,
                    None,
                )
                .map_err(|error| error.to_string())?;
                model.init_steps.push(IrInitStep::SetScalar {
                    sig: info.ir,
                    value,
                });
            }
        }
        for ai in &self.arrays {
            if self.reference_array(ai.ir) != ai.ir {
                continue;
            }
            model.init_steps.push(if ai.is_net {
                IrInitStep::FillArrayZ(ai.ir)
            } else {
                IrInitStep::FillArrayX(ai.ir)
            });
            if let Some(vals) = &ai.init {
                for (i, c) in vals.iter().enumerate() {
                    model.init_steps.push(IrInitStep::SetArrayElem {
                        arr: ai.ir,
                        index: i as u64,
                        value: c.clone(),
                    });
                }
            }
        }
        for (info, c) in &self.scalar_inits {
            model.init_steps.push(IrInitStep::SetScalar {
                sig: info.ir,
                value: c.clone(),
            });
        }
        for (info, c) in &self.var_inits {
            model.init_steps.push(IrInitStep::SetScalar {
                sig: info.ir,
                value: c.clone(),
            });
        }
        model.init_steps.extend(
            self.declaration_inits
                .iter()
                .cloned()
                .map(IrInitStep::Initialize),
        );
        for (net, slot, c) in &self.net_inits {
            let group = model
                .net_groups
                .iter()
                .position(|g| &g.c_name == net)
                .ok_or_else(|| format!("net group `{net}` not collected"))?;
            model.init_steps.push(IrInitStep::WriteNet {
                group,
                slot: *slot,
                value: c.clone(),
            });
        }
        model
            .init_steps
            .extend(self.delayed_driver_inits.iter().cloned());
        let mut sampled_sources: Vec<(usize, usize)> = self
            .clocking_samples
            .values()
            .filter_map(|sample| {
                self.signal_of(sample.source)
                    .map(|source| (source.ir, sample.sample.ir))
            })
            .collect();
        sampled_sources.sort_unstable();
        sampled_sources.dedup_by_key(|(source, _)| *source);
        for (source, _) in sampled_sources {
            model.init_steps.push(IrInitStep::RegisterSampled(source));
        }

        let active_initializations: Vec<IrInitialization> = model
            .init_steps
            .iter()
            .filter_map(|step| match step {
                IrInitStep::Initialize(initialization)
                    if initialization.phase == IrInitPhase::ActiveRegion =>
                {
                    Some(initialization.clone())
                }
                _ => None,
            })
            .collect();
        for (index, initialization) in active_initializations.into_iter().enumerate() {
            let lhs = self.initialization_lhs(model, &initialization)?;
            let mut process_name = format!("p_{}_decl_init_{index}", ident(&model.design_name));
            let mut suffix = 0usize;
            while model
                .processes
                .iter()
                .any(|process| process.c_name == process_name)
            {
                suffix += 1;
                process_name =
                    format!("p_{}_decl_init_{index}_{suffix}", ident(&model.design_name));
            }
            let label = format!("{}.declaration_init.{}", model.design_name, index);
            model.processes.push(IrProcess::new_with_origin(
                process_name,
                label,
                IrShape::RunOnce,
                Vec::new(),
                vec![IrStmt::Assign {
                    lhs,
                    rhs: initialization.value,
                    nba: false,
                }],
                initialization.origin,
            ));
        }
        Ok(())
    }

    fn initialization_lhs(
        &self,
        model: &IrModel,
        initialization: &IrInitialization,
    ) -> Result<IrLhs, String> {
        match &initialization.target {
            IrInitTarget::Signal(signal) => {
                if *signal >= model.signals.len() {
                    return Err(format!(
                        "declaration initializer references signal index {signal} out of bounds"
                    ));
                }
                self.reference_lhs(IrLhs::Whole(*signal))
            }
            IrInitTarget::StaticLocal { function, name } => {
                let func = model.funcs.get(*function).ok_or_else(|| {
                    format!(
                        "declaration initializer references function index {function} out of bounds"
                    )
                })?;
                let local = func
                    .locals
                    .iter()
                    .find(|local| local.c_name() == name)
                    .ok_or_else(|| {
                        format!("declaration initializer references unknown static local `{name}`")
                    })?;
                Ok(IrLhs::WholeRef {
                    addr: format!("&{name}"),
                    width: local.width(),
                    signed: local.signed(),
                    two_state: local.two_state,
                    shortreal: local.shortreal,
                })
            }
        }
    }

    pub(super) fn initialize_delayed_driver(&mut self, index: usize) -> Result<(), String> {
        use crate::sim::ir::IrInitStep;
        let signal = self.model.signal(index);
        let width = signal.ty.width();
        let limbs = (width as usize).div_ceil(64);
        let mut x = vec![u64::MAX; limbs];
        if !width.is_multiple_of(64) {
            x[limbs - 1] = (1u64 << (width % 64)) - 1;
        }
        let value = IrConst::packed(
            vec![0; limbs],
            x,
            vec![0; limbs],
            width,
            signal.ty.signed(),
            None,
        )
        .map_err(|error| error.to_string())?;
        self.delayed_driver_inits.push(match signal.net_driver {
            Some((group, slot)) => IrInitStep::WriteNet { group, slot, value },
            None => IrInitStep::SetScalar { sig: index, value },
        });
        Ok(())
    }
}
