//! Delays.

use super::*;

impl<'a> Codegen<'a> {
    /// The operand of every `min:typ:max` expression that the simulation
    /// uses (IEEE 1364-2001 4.3, IEEE 1800-2009 11.12). Constant folding,
    /// runtime lowering and driver/net delays all select through this one
    /// choice, which is the compilation's `--delay-select` (typical by
    /// default) that the frontend also used for elaborated parameters.
    pub(super) fn min_typ_max_operand(&self) -> usize {
        self.db.delay_selection().operand()
    }

    /// Lower a delayed continuous driver whose target is not one packed
    /// driver (a concatenation or a selection split across alias groups).
    /// The LRM computes one delay from the whole left-hand-side value
    /// (IEEE 1364-2001 6.1.3, IEEE 1800-2009 10.3.3), so the value goes
    /// through one inertial driver on a hidden packed signal; a zero-delay
    /// process sensitive to that signal performs `scatter`, the undelayed
    /// writes of its value, at the propagation time. The hidden signal
    /// starts X like every pending delayed driver. Returns the statement
    /// that schedules the hidden driver; the scatter process is pushed.
    pub(super) fn delayed_shadow_driver(
        &mut self,
        path: &str,
        source: NodeId,
        value: IrExpr,
        delay: IrInertialDelay,
        scatter: impl FnOnce(IrExpr) -> Vec<IrStmt>,
    ) -> Result<IrStmt, String> {
        if value.is_real() || value.width() == 0 || value.width() == LLG_MAX_WIDTH {
            return Err(format!(
                "delayed continuous assignment in `{path}` needs a fixed-width packed value"
            ));
        }
        let shadow = self.model.signals.len();
        let c_name = format!("llg_delay_shadow_{shadow}");
        self.model.signals.push(IrSignal {
            fixed_default: None,
            c_name: c_name.clone(),
            hdl_name: None,
            ty: IrType::Packed {
                width: value.width(),
                signed: value.signed(),
                two_state: false,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        self.initialize_delayed_driver(shadow)?;
        let read = IrExpr::new(
            IrExprKind::SigRead(shadow),
            value.width(),
            value.signed(),
            None,
        );
        let body = scatter(read);
        let fn_name = self.new_fn_name(path, "ca");
        let origin = self.origin(source);
        self.model.processes.push(IrProcess::new_with_origin(
            fn_name,
            format!("{}.assign", self.source_path(path)),
            IrShape::SensLoop {
                reads: vec![IrDependency::Scalar(c_name)],
            },
            Vec::new(),
            body,
            origin,
        ));
        Ok(IrStmt::InertialAssign {
            lhs: IrLhs::Whole(shadow),
            rhs: value,
            delay,
        })
    }

    pub(super) fn lower_procedural_delay(
        &mut self,
        path: &str,
        delay_node: NodeId,
        expression: NodeId,
    ) -> Result<IrDelay, String> {
        let resolved = self.eval_decl_value(expression);
        let constant_nonnegative = match &resolved {
            Ok(Val::Bits(value)) => {
                value.to_u128().is_some()
                    && (!value.signed || value.to_i128().is_some_and(|value| value >= 0))
            }
            // A finite negative real is legal (V 9.7.1, SV 9.4.1) and becomes
            // two's-complement unsigned time, which the runtime conversion
            // shared with runtime-valued delays performs. Nonfinite constants
            // stay on the constant path, which rejects them.
            Ok(Val::Real(value)) => !value.is_finite() || *value >= 0.0,
            _ => false,
        };
        if constant_nonnegative {
            return self
                .procedural_delay_ticks(delay_node, expression)
                .map(IrDelay::Constant);
        }
        let timescale = self.timescale_of_node(delay_node);
        if timescale.unit_fs == 0 || timescale.precision_fs == 0 || self.design_precision_fs == 0 {
            return Err(format!("delay in `{path}` has an invalid time scale"));
        }
        let value = self.lower_expr_for_delay(path, expression)?;
        Ok(IrDelay::Runtime {
            value: Box::new(value),
            unit_ticks: timescale.unit_fs / self.design_precision_fs,
            precision_ticks: timescale.precision_fs / self.design_precision_fs,
        })
    }

    /// Evaluate a typed delay expression and round it once to the owning
    /// module's precision before converting to design scheduler ticks.
    pub(super) fn procedural_delay_ticks(
        &mut self,
        delay_node: NodeId,
        expression: NodeId,
    ) -> Result<u64, String> {
        let timescale = self.timescale_of_node(delay_node);
        let exact_literal = match self.kind(expression) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Time,
                source: ConstantSource::Exact(source),
                time_scale: Some(scale),
                ..
            }) => Some((source.clone(), *scale)),
            _ => None,
        };
        let value = self
            .eval_decl_value_without_time_rounding(expression)
            .map_err(|error| {
                format!(
                    "procedural delay in `{}` is runtime-valued or uses an unsupported \
                 constant expression: {error}",
                    self.instance_path_of(self.inst)
                )
            })?;
        self.delay_value_ticks(value, exact_literal, timescale)
    }

    /// A delay value in scheduler ticks: an integer counts time units, a
    /// real is rounded to the time precision (an exact time literal by its
    /// own spelling).
    fn delay_value_ticks(
        &self,
        value: Val,
        exact_literal: Option<(String, crate::core::db::TimeLiteralScale)>,
        timescale: Timescale,
    ) -> Result<u64, String> {
        let (ticks, unit_fs) = match value {
            Val::Bits(value) => {
                let raw = if value.signed {
                    let signed = value
                        .to_i128()
                        .ok_or_else(|| "procedural delay must be a known integer".to_owned())?;
                    u128::try_from(signed).map_err(|_| {
                        "procedural delay must be a known nonnegative integer".to_owned()
                    })?
                } else {
                    value.to_u128().ok_or_else(|| {
                        "procedural delay must be a known nonnegative integer".to_owned()
                    })?
                };
                let ticks = u64::try_from(raw)
                    .map_err(|_| "procedural delay exceeds 64 bits".to_owned())?;
                (ticks, timescale.unit_fs)
            }
            Val::Real(value) => {
                let ticks = match exact_literal {
                    Some((source, scale)) => {
                        time_literal_delay_ticks(value, &source, scale, timescale)?
                    }
                    None => real_delay_ticks(value, timescale)?,
                };
                (ticks, timescale.precision_fs)
            }
            Val::Str(_) => return Err("procedural delay cannot be a string".to_owned()),
        };
        scale_delay_ticks(
            ticks,
            unit_fs,
            self.design_precision_fs,
            &self.instance_path_of(self.inst),
        )
    }

    /// A `PATHPULSE$` pulse limit of the specparam `node` in scheduler
    /// ticks, in the time unit of its module like a path delay; a negative
    /// limit is zero.
    pub(super) fn path_limit_ticks(&self, node: NodeId, value: &Val) -> Result<u64, String> {
        match value {
            Val::Bits(bits) if bits.signed && bits.to_i128().is_some_and(|value| value < 0) => {
                return Ok(0)
            }
            Val::Real(value) if *value < 0.0 => return Ok(0),
            _ => {}
        }
        self.delay_value_ticks(value.clone(), None, self.timescale_of_node(node))
    }

    /// One module path delay value in scheduler ticks. Path delays are
    /// constant expressions of literals and specparams; a negative value is
    /// treated as zero (IEEE 1364-2001 14.3.1, IEEE 1800-2009 30.5.1) and
    /// min:typ:max takes the selected operand like any other delay.
    pub(super) fn path_delay_ticks(
        &mut self,
        path_node: NodeId,
        expression: NodeId,
    ) -> Result<u64, String> {
        match self.eval_decl_value_without_time_rounding(expression) {
            Ok(Val::Bits(value))
                if value.signed && value.to_i128().is_some_and(|value| value < 0) =>
            {
                return Ok(0)
            }
            Ok(Val::Real(value)) if value < 0.0 => return Ok(0),
            _ => {}
        }
        self.procedural_delay_ticks(path_node, expression)
    }

    /// One timing check limit in scheduler ticks, in the check's module time
    /// unit with min:typ:max selection as for path delays; `true` with zero
    /// ticks for a negative value, which each check treats by its own rule.
    pub(super) fn timing_limit_ticks(
        &mut self,
        check_node: NodeId,
        expression: NodeId,
    ) -> Result<(u64, bool), String> {
        let negative = match self.eval_decl_value_without_time_rounding(expression) {
            Ok(Val::Bits(value)) => value.signed && value.to_i128().is_some_and(|value| value < 0),
            Ok(Val::Real(value)) => value < 0.0,
            _ => false,
        };
        if negative {
            return Ok((0, true));
        }
        Ok((self.procedural_delay_ticks(check_node, expression)?, false))
    }

    /// One timing check limit in signed scheduler ticks: a negative value
    /// keeps its magnitude in the check's time unit (`$setuphold`/`$recrem`
    /// limits, `$nochange` offsets; 31.9, 31.4.6).
    pub(super) fn timing_limit_ticks_signed(
        &mut self,
        check_node: NodeId,
        expression: NodeId,
    ) -> Result<i64, String> {
        let (ticks, negative) = self.timing_limit_ticks(check_node, expression)?;
        let ticks = if negative {
            let magnitude = match self.eval_decl_value_without_time_rounding(expression)? {
                Val::Bits(value) => Val::Real(
                    -(value
                        .to_i128()
                        .ok_or_else(|| "timing check limit must be known".to_owned())?
                        as f64),
                ),
                Val::Real(value) => Val::Real(-value),
                other => other,
            };
            let ticks =
                self.delay_value_ticks(magnitude, None, self.timescale_of_node(check_node))?;
            return i64::try_from(ticks)
                .map(|ticks| -ticks)
                .map_err(|_| "timing check limit exceeds 63 bits".to_owned());
        } else {
            ticks
        };
        i64::try_from(ticks).map_err(|_| "timing check limit exceeds 63 bits".to_owned())
    }

    fn lower_expr_for_delay(&mut self, path: &str, expression: NodeId) -> Result<IrExpr, String> {
        let previous = self.round_time_literals;
        self.round_time_literals = false;
        let result = self.lower_expr(path, expression);
        self.round_time_literals = previous;
        result
    }

    fn eval_decl_value_without_time_rounding(&mut self, expression: NodeId) -> Result<Val, String> {
        let previous = self.round_time_literals;
        self.round_time_literals = false;
        let result = self.eval_decl_value(expression);
        self.round_time_literals = previous;
        result
    }

    pub(super) fn rounded_time_literal(
        &self,
        node: NodeId,
        value: f64,
        source: &ConstantSource,
        scale: Option<crate::core::db::TimeLiteralScale>,
    ) -> Result<f64, String> {
        let source = match source {
            ConstantSource::Exact(source) => source,
            ConstantSource::NotCaptured | ConstantSource::Unavailable => {
                return Err(format!(
                    "time literal at {}:{}:{} has no admitted source provenance",
                    self.node(node).file.as_deref().unwrap_or("<unknown>"),
                    self.node(node).line,
                    self.node(node).col
                ));
            }
        };
        let scale = scale.ok_or_else(|| {
            format!(
                "time literal at {}:{}:{} has no resolved unit scale",
                self.node(node).file.as_deref().unwrap_or("<unknown>"),
                self.node(node).line,
                self.node(node).col
            )
        })?;
        round_time_literal(value, source, scale, self.timescale_of_node(node))
    }

    /// Delays of a continuous assignment, gate or UDP instance. Constant
    /// values convert to ticks once. SystemVerilog allows runtime-valued
    /// expressions (IEEE 1800-2009 28.16.1: "These can be any three
    /// expressions"); the driver evaluates them each time it schedules an
    /// update, not when only a delay operand changes (decision S2-D9).
    /// IEEE 1364-2001 7.14.1 requires constant expressions.
    /// Whether any value of a driver or net delay is runtime-valued.
    pub(super) fn driver_delay_is_runtime(&mut self, delay: DriverDelay) -> bool {
        let values = match delay {
            DriverDelay::Single(value) => vec![value],
            DriverDelay::RiseFall(rise, fall) => vec![rise, fall],
            DriverDelay::RiseFallTurnOff(rise, fall, turn_off) => vec![rise, fall, turn_off],
        };
        values
            .into_iter()
            .any(|value| self.eval_decl_value(value).is_err())
    }

    pub(super) fn driver_delay(
        &mut self,
        path: &str,
        node: NodeId,
        delay: DriverDelay,
    ) -> Result<IrInertialDelay, String> {
        let values = match delay {
            DriverDelay::Single(value) => vec![value],
            DriverDelay::RiseFall(rise, fall) => vec![rise, fall],
            DriverDelay::RiseFallTurnOff(rise, fall, turn_off) => vec![rise, fall, turn_off],
        };
        let runtime = values
            .iter()
            .map(|value| self.eval_decl_value(*value).is_err())
            .collect::<Vec<_>>();
        if !runtime.contains(&true) {
            return self
                .driver_delay_ticks(node, delay)
                .map(IrInertialDelay::Constant);
        }
        if self.db.edition() != LanguageEdition::SystemVerilog2009 {
            return Err(
                "the delay is runtime-valued or uses an unsupported constant expression; \
                 IEEE 1364-2001 7.14.1 requires constant delay expressions on continuous \
                 assignments, gates and nets"
                    .to_owned(),
            );
        }
        let mut lowered = Vec::with_capacity(values.len());
        for (value, runtime) in values.into_iter().zip(runtime) {
            lowered.push(if runtime {
                self.lower_procedural_delay(path, node, value)?
            } else {
                IrDelay::Constant(self.procedural_delay_ticks(node, value)?)
            });
        }
        let mut lowered = lowered.into_iter();
        let rise = lowered
            .next()
            .ok_or_else(|| "driver delay has no value".to_owned())?;
        Ok(IrInertialDelay::Runtime(Box::new(
            crate::sim::ir::IrRuntimeTransitionDelay {
                rise,
                fall: lowered.next(),
                turn_off: lowered.next(),
                writable: None,
            },
        )))
    }

    pub(super) fn driver_delay_ticks(
        &mut self,
        node: NodeId,
        delay: DriverDelay,
    ) -> Result<IrTransitionDelay, String> {
        match delay {
            DriverDelay::Single(expression) => Ok(IrTransitionDelay::uniform(
                self.procedural_delay_ticks(node, expression)?,
            )),
            DriverDelay::RiseFall(rise, fall) => {
                let rise = self.procedural_delay_ticks(node, rise)?;
                let fall = self.procedural_delay_ticks(node, fall)?;
                Ok(IrTransitionDelay {
                    rise,
                    fall,
                    turn_off: rise.min(fall),
                })
            }
            DriverDelay::RiseFallTurnOff(rise, fall, turn_off) => Ok(IrTransitionDelay {
                rise: self.procedural_delay_ticks(node, rise)?,
                fall: self.procedural_delay_ticks(node, fall)?,
                turn_off: self.procedural_delay_ticks(node, turn_off)?,
            }),
        }
    }

    /// Resolved timescale of the nearest owning module instance, package,
    /// compilation unit or class (its declaring scope's). Slang has already applied compilation-unit and
    /// declaration inheritance.
    pub(super) fn timescale_of_node(&self, node: NodeId) -> Timescale {
        let mut current = Some(node);
        while let Some(id) = current {
            if let NodeKind::ModuleInst {
                timeunit,
                timeprecision,
                ..
            } = self.kind(id)
            {
                // A checker declared in `$unit` without a time unit
                // declaration runs at the `$unit` default (F2-D2).
                if self
                    .db
                    .checker_instance(id)
                    .is_some_and(|checker| checker.unit_time_scale)
                {
                    return self.unit_default_timescale;
                }
                return Timescale {
                    unit_fs: time_exponent_to_fs(*timeunit),
                    precision_fs: time_exponent_to_fs(*timeprecision),
                };
            }
            // Package, `$unit` and class code scales time by its own
            // declaration scope (SV 3.14.2.3), never by the caller. Class
            // nodes are detached from their declaring scope, so the Db
            // records the scale they inherited.
            if self.is_runtime_environment(id) || matches!(self.kind(id), NodeKind::ClassDef) {
                // A scope with no recorded scale is `$unit` (or a class in
                // it) without a time unit declaration.
                return self.db.declaration_time_scale(id).map_or(
                    self.unit_default_timescale,
                    |(unit, precision)| Timescale {
                        unit_fs: time_exponent_to_fs(unit),
                        precision_fs: time_exponent_to_fs(precision),
                    },
                );
            }
            current = self.node(id).parent;
        }
        Timescale::DEFAULT
    }

    /// Hierarchical name of the module instance, package or `$unit` whose
    /// time scale [`Self::timescale_of_node`] reports for `node`. Class
    /// nodes have no owned declaring scope, so they yield `None`.
    pub(super) fn timescale_scope_label(&self, node: NodeId) -> Option<String> {
        let mut current = Some(node);
        while let Some(id) = current {
            match self.kind(id) {
                NodeKind::ModuleInst { .. } => {
                    let path = self.instance_path_of(id);
                    return Some(self.display_path(&path).to_owned());
                }
                NodeKind::ClassDef => return None,
                _ if self.is_compilation_unit(id) => return Some("$unit".to_owned()),
                _ if self.is_runtime_environment(id) => return Some(self.namespace_path(id)),
                _ => current = self.node(id).parent,
            }
        }
        None
    }

    /// Retain a signed marker present in a legacy textual literal payload.
    pub(super) fn signed_based_constant(&self, node: NodeId) -> bool {
        self.signed_based_literal_info(node).0
    }

    /// Read an explicit width and signed marker from a legacy textual literal.
    /// Slang vector values normally carry both directly.
    pub(super) fn signed_based_literal_info(&self, node: NodeId) -> (bool, Option<u32>) {
        let node = self.node(node);
        if is_signed_based_literal(&node.name) {
            return (true, based_literal_width(&node.name));
        }
        (false, None)
    }

    /// Retain an unbased unsized fill literal present in a legacy textual
    /// payload. Slang normally preserves it as a typed operation.
    pub(super) fn source_fill_literal(&self, node: NodeId) -> Option<u8> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                source: ConstantSource::Exact(source),
                ..
            }) => fill_literal_token(source),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.source_fill_literal(*operand),
            _ => fill_literal_token(&self.node(node).name),
        }
    }

    /// Parse the timescale of every distinct source file referenced by the
    /// design and fix the design precision (the finest precision, which sets
    /// the scheduler tick unit).  Runs before any emission so every `#delay`
    /// and `$time` scales consistently.
    pub(super) fn collect_timescales(&mut self) {
        self.design_precision_fs = u64::MAX;
        for top in self.db.tops() {
            self.walk_files(*top);
        }
        for m in self.db.flat_modules() {
            self.walk_files(*m);
        }
        // Package and `$unit` subroutines and classes run at their own
        // namespace precision, so the tick must resolve it. Namespaces that
        // declare no code cannot observe or schedule time and leave the
        // tick (and the default `$timeformat` units) coarser.
        let namespaces = self
            .db
            .node_ids()
            .filter(|id| self.is_runtime_environment(*id) && self.namespace_has_code(*id))
            .collect::<Vec<_>>();
        // A checker declared in `$unit` without a time unit declaration
        // takes the `$unit` default scale (walk_files skips it).
        let mut undeclared_unit = self.db.has_checker_instances()
            && self.db.checker_instances().into_iter().any(|checker| {
                self.db
                    .checker_instance(checker)
                    .is_some_and(|info| info.unit_time_scale)
            });
        for namespace in namespaces {
            match self.db.declaration_time_scale(namespace) {
                Some(_) => {
                    let ts = self.timescale_of_node(namespace);
                    self.design_precision_fs = self.design_precision_fs.min(ts.precision_fs);
                }
                None => undeclared_unit = true,
            }
        }
        // A `$unit` without `timeunit`/`timeprecision` takes the default
        // time unit; a `timescale directive does not set it (SV 3.14.2.3).
        // Its precision is implementation-specific: llg uses the finest
        // precision the design declares, but never one coarser than the
        // default unit, so the design tick stays that of the declared
        // scales (SV 3.14.3) while `$unit` delays keep whole-unit
        // resolution. With nothing declared it is the default precision.
        // Decision F2-D2.
        let declared_precision = self.design_precision_fs;
        self.unit_default_timescale = Timescale {
            unit_fs: Timescale::DEFAULT.unit_fs,
            precision_fs: if declared_precision == u64::MAX {
                Timescale::DEFAULT.precision_fs
            } else {
                declared_precision.min(Timescale::DEFAULT.unit_fs)
            },
        };
        if undeclared_unit {
            self.design_precision_fs = self
                .design_precision_fs
                .min(self.unit_default_timescale.precision_fs);
        }
        if self.design_precision_fs == u64::MAX {
            // No source files at all (should not happen for a real design).
            self.design_precision_fs = Timescale::DEFAULT.precision_fs;
        }
        self.model.precision_fs = self.design_precision_fs;
    }

    fn namespace_has_code(&self, namespace: NodeId) -> bool {
        self.node(namespace).children.iter().any(|child| {
            matches!(
                self.kind(*child),
                NodeKind::FuncTask { .. } | NodeKind::ClassDef
            )
        })
    }
}
