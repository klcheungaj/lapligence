//! Initialization.

use super::*;

mod effects;
mod order;

impl<'a> Codegen<'a> {
    pub(super) fn declaration_init_phase(&self) -> IrInitPhase {
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
    pub(super) fn owner_instance(&self, node: NodeId) -> Option<NodeId> {
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

    /// The class whose method declares `node`, if any.
    fn owner_class(&self, node: NodeId) -> Option<NodeId> {
        let mut current = Some(node);
        while let Some(id) = current {
            if matches!(self.kind(id), NodeKind::ClassDef) {
                return Some(id);
            }
            current = self.node(id).parent;
        }
        None
    }

    /// Build the typed runtime identity for a named block, task, or fork
    /// scope. Unowned synthetic nodes fail closed instead of using a name.
    /// A block inside a class method has no instance or package: its class
    /// (each specialization is its own node) is the environment, since every
    /// activation of the method shares the one declaration (SV 9.6.2).
    pub(super) fn activation_target(
        &self,
        declaration: NodeId,
    ) -> Result<crate::sim::ir::IrActivationTarget, String> {
        let instance = self
            .owner_instance(declaration)
            .or_else(|| self.owner_class(declaration));
        let instance = instance.ok_or_else(|| {
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
        self.record_initializer_source(declaration, initializer);
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
        self.validate_static_initializer_effects()?;
        // Value and statement initializers share one schedule: each runs
        // after the static declarations it reads (SV §§6.21, 10.5).
        // String declaration values are assignments in the same schedule.
        let mut string_values = Vec::new();
        for (object, declaration) in &self.object_initializers {
            if let Some(value) = model.objects[*object].initial.take() {
                string_values.push((
                    *declaration,
                    IrStmt::Object(Box::new(crate::sim::ir::IrObjectStmt::StringAssign(
                        *object, value,
                    ))),
                ));
            }
        }
        let declarations = self
            .declaration_inits
            .iter()
            .map(IrInitialization::declaration)
            .chain(
                self.declaration_statements
                    .iter()
                    .chain(&string_values)
                    .map(|(declaration, _)| *declaration),
            )
            .collect::<Vec<_>>();
        let values = self.declaration_inits.len();
        let statements = self.declaration_statements.len();
        for entry in self.initializer_schedule(&declarations) {
            model.init_steps.push(match entry.checked_sub(values) {
                None => IrInitStep::Initialize(self.declaration_inits[entry].clone()),
                Some(statement) => {
                    let (declaration, body) = match statement.checked_sub(statements) {
                        None => self.declaration_statements[statement].clone(),
                        Some(string) => string_values[string].clone(),
                    };
                    IrInitStep::Execute {
                        declaration,
                        body: Box::new(body),
                        random_scope: self.initializer_random_scopes.get(&declaration).cloned(),
                    }
                }
            });
        }
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
            .map(|sample| (sample.source.ir, sample.sample.ir))
            .collect();
        sampled_sources.sort_unstable();
        sampled_sources.dedup_by_key(|(source, _)| *source);
        for source in &self.sampled_value_signals {
            // A clocking source's history registration also samples its value.
            if sampled_sources
                .binary_search_by_key(source, |(source, _)| *source)
                .is_err()
            {
                model
                    .init_steps
                    .push(IrInitStep::RegisterSampledValue(*source));
            }
        }
        for (source, _) in sampled_sources {
            // Only skewed inputs read past slots; the runtime keeps each
            // source's history no deeper than its largest skew.
            model
                .init_steps
                .push(match self.clocking_history_ticks.get(&source) {
                    Some(&ticks) => IrInitStep::RegisterSampledHistory { sig: source, ticks },
                    None => IrInitStep::RegisterSampled(source),
                });
        }
        for source in &self.sampled_real_signals {
            model.init_steps.push(IrInitStep::RegisterSampled(*source));
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
            IrInitTarget::Fixed(lhs) => Ok(*lhs.clone()),
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

    /// Start a whole delayed driver at X: its output is unknown until its
    /// first propagation event matures. Net-array cells, gate outputs and
    /// scalar drivers alike (decision S2-D4). A sequential UDP output with an
    /// `initial` value starts at that value instead
    /// ([`Self::delayed_driver_start`](Codegen)).
    pub(super) fn initialize_delayed_driver(&mut self, index: usize) -> Result<(), String> {
        use crate::sim::ir::IrInitStep;
        let signal = self.model.signal(index);
        let width = signal.ty.width();
        let limbs = (width as usize).div_ceil(64);
        let mut driven = vec![u64::MAX; limbs];
        if !width.is_multiple_of(64) {
            driven[limbs - 1] = (1u64 << (width % 64)) - 1;
        }
        let (bits, x) = match self.delayed_driver_start {
            Some(true) => (driven, vec![0; limbs]),
            Some(false) => (vec![0; limbs], vec![0; limbs]),
            None => (vec![0; limbs], driven),
        };
        let value = IrConst::packed(bits, x, vec![0; limbs], width, signal.ty.signed(), None)
            .map_err(|error| error.to_string())?;
        self.delayed_driver_inits.push(match signal.net_driver {
            Some((group, slot)) => IrInitStep::WriteNet { group, slot, value },
            None => IrInitStep::SetScalar { sig: index, value },
        });
        Ok(())
    }

    /// Start the bits a delayed driver site drives at X. A whole target is
    /// [`Self::initialize_delayed_driver`]; a constant selection of a net
    /// driver slot (a selected continuous assignment or a gate-array
    /// instance output) marks only its own bits, leaving the rest of its
    /// contribution high impedance, and each concatenation part is marked
    /// on its own. Variable targets keep their storage initialization.
    pub(super) fn initialize_delayed_lhs(&mut self, lhs: &IrLhs) -> Result<(), String> {
        let (index, high, low) = match lhs {
            IrLhs::Whole(index) => return self.initialize_delayed_driver(*index),
            IrLhs::Part(index, left, right, _) => (*index, *left, *right),
            IrLhs::Bit(index, bit, _) => match super::statements::force_constant_i64(bit) {
                Some(bit) => (*index, bit, bit),
                None => return Ok(()),
            },
            IrLhs::Stream { parts, .. } => {
                for (part, _) in parts {
                    self.initialize_delayed_lhs(part)?;
                }
                return Ok(());
            }
            _ => return Ok(()),
        };
        use crate::sim::ir::IrInitStep;
        let signal = self.model.signal(index);
        let Some((group, slot)) = signal.net_driver else {
            return Ok(());
        };
        let width = signal.ty.width();
        if low < 0 || high < low || high >= i64::from(width) {
            return Ok(());
        }
        let limbs = (width as usize).div_ceil(64);
        let mut bits = vec![0u64; limbs];
        let mut x = vec![0u64; limbs];
        let mut z = vec![0u64; limbs];
        for bit in 0..width {
            let (limb, offset) = ((bit / 64) as usize, bit % 64);
            if !(low..=high).contains(&i64::from(bit)) {
                z[limb] |= 1 << offset;
            } else if self.delayed_driver_start == Some(true) {
                bits[limb] |= 1 << offset;
            } else if self.delayed_driver_start.is_none() {
                x[limb] |= 1 << offset;
            }
        }
        let value = IrConst::packed(bits, x, z, width, signal.ty.signed(), None)
            .map_err(|error| error.to_string())?;
        self.delayed_driver_inits
            .push(IrInitStep::WriteNet { group, slot, value });
        Ok(())
    }
}
