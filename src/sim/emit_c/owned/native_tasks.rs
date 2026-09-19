//! Borrowed numeric runtime calls; native text is constructed only at the
//! final consuming call, after every potentially nonlocal expression finishes.
use super::stores::Target;
use super::*;

impl Frame<'_, '_> {
    pub(super) fn system_command(
        &mut self,
        command: Option<&IrStringExpr>,
    ) -> Result<Value, String> {
        if self.read_only_callback {
            return Err(pending("host commands in read-only callbacks"));
        }
        let text = if let Some(command) = command {
            self.string(command)?
        } else {
            self.native_value(
                super::native::NativeKind::String,
                "(llg_string_t){0}".to_owned(),
            )
        };
        let result = self.value(
            format!(
                "llg_system({}, {})",
                text.take_string(),
                u8::from(command.is_some())
            ),
            32,
            true,
        );
        self.native_discard(text);
        Ok(result)
    }

    pub(super) fn time_format(
        &mut self,
        units: &IrExpr,
        precision: &IrExpr,
        suffix: &IrStringExpr,
        field_width: &IrExpr,
    ) -> Result<(), String> {
        let units = self.expression(units)?;
        let precision = self.expression(precision)?;
        let text = self.string(suffix)?;
        let width = self.expression(field_width)?;
        if [units.width, precision.width, width.width].contains(&0) {
            return Err("timeformat numeric arguments must be integral".to_owned());
        }
        self.line(format!(
            "llg_timeformat({}, {}, {}, {});",
            units.code,
            precision.code,
            text.take_string(),
            width.code
        ));
        self.native_discard(text);
        self.discard(units);
        self.discard(precision);
        self.discard(width);
        Ok(())
    }

    pub(super) fn vpi_call(
        &mut self,
        site: usize,
        name: &str,
        args: &[IrExpr],
        result: Option<(u32, bool)>,
    ) -> Result<Option<Value>, String> {
        let mut owners = Vec::new();
        for arg in args {
            owners.push(self.expression(arg)?);
        }
        let array = if owners.is_empty() {
            "NULL".to_owned()
        } else {
            let array = self.name("vpi_args");
            let entries = owners.iter().map(|value| {
                if value.width == 0 {
                    // The unused packed field owns nothing; never allocate an
                    // X payload merely to describe a native real argument.
                    format!("{{ .kind = LLG_FMT_REAL, .width = 0, .is_signed = {}, .is_real = 1, .packed = SV4_EMPTY, .real = {} }}", u8::from(value.signed), value.code)
                } else {
                    format!("{{ .kind = LLG_FMT_PACKED, .width = {}, .is_signed = {}, .is_real = 0, .packed = {}, .real = 0.0 }}", value.width, u8::from(value.signed), value.code)
                }
            }).collect::<Vec<_>>();
            self.line(format!(
                "llg_vpi_arg_t {array}[] = {{ {} }}; /* borrowed argument snapshots */",
                entries.join(", ")
            ));
            array
        };
        let arguments = format!(
            "{site}ULL, {}, {array}, {}",
            c_string_literal(name),
            args.len()
        );
        let value = match result {
            Some((0, signed)) => Some(self.value(
                format!("llg_vpi_call_real_function_site({arguments})"),
                0,
                signed,
            )),
            Some((width, signed)) => Some(self.value(
                format!(
                    "llg_vpi_call_function_site({arguments}, {width}, {})",
                    u8::from(signed)
                ),
                width,
                signed,
            )),
            None => {
                self.line(format!("(void)llg_vpi_call_task_site({arguments});"));
                None
            }
        };
        for owner in owners {
            self.discard(owner);
        }
        Ok(value)
    }

    pub(super) fn legacy_random(
        &mut self,
        kind: IrRandomFunc,
        seed: Option<&IrLhs>,
        args: &[IrExpr],
    ) -> Result<Value, String> {
        let seed_target = seed.map(|lhs| self.target(lhs)).transpose()?;
        let seed_name = if let Some(target) = &seed_target {
            if target.width == 0 {
                return Err("legacy random seed must be packed storage".to_owned());
            }
            let value = self.read_target(target);
            let value = self.convert(value, 32, true, false, false);
            let scalar = self.scalar("int32_t", format!("(int32_t)sv4_to_i64({})", value.code));
            self.discard(value);
            Some(scalar)
        } else {
            None
        };
        let mut parameters = Vec::new();
        for argument in args {
            let value = self.expression(argument)?;
            if value.width == 0 {
                return Err("legacy random parameters must be packed".to_owned());
            }
            let value = self.convert(value, 32, true, false, false);
            parameters.push(self.scalar("int32_t", format!("(int32_t)sv4_to_i64({})", value.code)));
            self.discard(value);
        }
        let call = if let Some(seed) = &seed_name {
            let mut all = vec![format!("&{seed}")];
            all.extend(parameters);
            format!("{}({})", kind.runtime_name(), all.join(", "))
        } else if kind == IrRandomFunc::Random {
            "llg_random_default()".to_owned()
        } else {
            return Err("legacy distribution requires a seed".to_owned());
        };
        let result = self.scalar("int32_t", call);
        if let (Some(target), Some(seed)) = (seed_target, seed_name) {
            let value = self.value(format!("sv4_from_i64((int64_t){seed}, 32)"), 32, true);
            self.store(&target, value, false, "0")?;
            self.release_target(target);
        }
        Ok(self.value(format!("sv4_from_i64((int64_t){result}, 32)"), 32, true))
    }

    fn queue_output(&mut self, lhs: &IrLhs) -> Result<(Target, String), String> {
        let target = self.target(lhs)?;
        if target.width == 0
            || target.selection.is_some()
            || target.net.is_some()
            || target.sequence_local
        {
            return Err("stochastic queue output requires a whole packed variable".to_owned());
        }
        let address = self.scalar(
            "sv4_t*",
            format!("({}) ? {} : NULL", target.valid, target.binding.address),
        );
        Ok((target, address))
    }

    pub(super) fn queue_full(&mut self, id: &IrExpr, status: &IrLhs) -> Result<Value, String> {
        let id = self.expression(id)?;
        let (target, address) = self.queue_output(status)?;
        let value = self.value(format!("llg_q_full({}, {address})", id.code), 32, true);
        self.release_target(target);
        self.discard(id);
        Ok(value)
    }

    pub(super) fn stochastic(&mut self, op: &IrStochasticStmt) -> Result<(), String> {
        let (name, inputs, outputs): (&str, Vec<&IrExpr>, Vec<&IrLhs>) = match op {
            IrStochasticStmt::Initialize {
                q_id,
                q_type,
                max_length,
                status,
            } => (
                "llg_q_initialize",
                vec![q_id, q_type, max_length],
                vec![status],
            ),
            IrStochasticStmt::Add {
                q_id,
                job_id,
                inform_id,
                status,
            } => ("llg_q_add", vec![q_id, job_id, inform_id], vec![status]),
            IrStochasticStmt::Remove {
                q_id,
                job_id,
                inform_id,
                status,
            } => ("llg_q_remove", vec![q_id], vec![job_id, inform_id, status]),
            IrStochasticStmt::Exam {
                q_id,
                stat_code,
                stat_value,
                status,
            } => (
                "llg_q_exam",
                vec![q_id, stat_code],
                vec![stat_value, status],
            ),
        };
        let mut owners = Vec::new();
        let mut targets = Vec::new();
        let mut arguments = Vec::new();
        for expression in inputs {
            let value = self.expression(expression)?;
            if value.width == 0 {
                return Err("stochastic queue input must be integral".to_owned());
            }
            arguments.push(value.code.clone());
            owners.push(value);
        }
        for lhs in outputs {
            let (target, address) = self.queue_output(lhs)?;
            arguments.push(address);
            targets.push(target);
        }
        self.line(format!("{name}({});", arguments.join(", ")));
        for target in targets {
            self.release_target(target);
        }
        for owner in owners {
            self.discard(owner);
        }
        Ok(())
    }
}
