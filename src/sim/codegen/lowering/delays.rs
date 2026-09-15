//! Delays.

use super::*;

impl<'a> Codegen<'a> {

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
            Ok(Val::Real(_)) => true,
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
        let (ticks, unit_fs) = match self
            .eval_decl_value_without_time_rounding(expression)
            .map_err(|error| {
                format!(
                    "procedural delay in `{}` is runtime-valued or uses an unsupported \
                 constant expression: {error}",
                    self.instance_path_of(self.inst)
                )
            })? {
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

    /// Resolved timescale of the nearest owning module instance. Slang has
    /// already applied compilation-unit and declaration inheritance.
    pub(super) fn timescale_of_node(&self, node: NodeId) -> Timescale {
        let mut current = Some(node);
        while let Some(id) = current {
            if let NodeKind::ModuleInst {
                timeunit,
                timeprecision,
                ..
            } = self.kind(id)
            {
                return Timescale {
                    unit_fs: time_exponent_to_fs(*timeunit),
                    precision_fs: time_exponent_to_fs(*timeprecision),
                };
            }
            current = self.node(id).parent;
        }
        Timescale::DEFAULT
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
        if self.design_precision_fs == u64::MAX {
            // No source files at all (should not happen for a real design).
            self.design_precision_fs = Timescale::DEFAULT.precision_fs;
        }
        self.model.precision_fs = self.design_precision_fs;
    }
}
