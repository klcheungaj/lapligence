//! System tasks.

use super::*;

impl EmitCtx<'_, '_> {
    /// Lower system-task calls ($display/$monitor/$strobe/$finish/…).
    /// Skippable constructs warn here and produce no statements.
    fn lower_dumpvars(&mut self, args: &[NodeId]) -> Result<IrStmt, String> {
        let (depth, first_selection) = match args.first().copied() {
            Some(first) => match self.cg.eval_bound_i128(first) {
                Ok(value) => {
                    let depth = u32::try_from(value).map_err(|_| {
                        format!(
                            "$dumpvars depth must be a non-negative 32-bit constant in `{}`",
                            self.path
                        )
                    })?;
                    (depth, 1usize)
                }
                Err(_) => (0, 0usize),
            },
            None => (0, 0usize),
        };
        let mut names = args[first_selection..]
            .iter()
            .map(|argument| self.cg.waveform_selection_name(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        // `$dumpvars(depth)` uses the current elaborated module instance as
        // its implicit scope.  Keep this as owned hierarchy metadata rather
        // than converting the textual process path at runtime.
        if names.is_empty() && depth != 0 {
            names.push(self.cg.waveform_name_for(self.inst));
        }
        Ok(IrStmt::WaveDumpVars(crate::sim::ir::IrWaveDumpVars::new(
            depth, names,
        )))
    }

    fn lower_memory_task(&mut self, name: &str, args: &[NodeId]) -> Result<IrStmt, String> {
        if !(2..=4).contains(&args.len()) {
            return Err(format!(
                "{name} requires two to four arguments in `{}`",
                self.path
            ));
        }
        let write = matches!(name, "$writememb" | "$writememh");
        if write && self.cg.db.edition() == LanguageEdition::Verilog2001 {
            return Err(format!(
                "{name} is a SystemVerilog memory task and is not available in Verilog-2001 in `{}`",
                self.path
            ));
        }
        let path = self.cg.lower_string(&self.path, args[0])?;
        let array = self.cg.array_of(args[1]).cloned().ok_or_else(|| {
            if self.cg.container_of(args[1]).is_some() {
                format!(
                    "{name} does not support dynamic arrays, queues, or associative arrays in `{}`",
                    self.path
                )
            } else {
                format!(
                    "{name} requires a fixed one-dimensional packed memory in `{}`",
                    self.path
                )
            }
        })?;
        if array.dims.len() != 1 {
            return Err(format!(
                "{name} requires a one-dimensional memory; `{}` has {} dimensions in `{}`",
                self.cg.node(args[1]).name,
                array.dims.len(),
                self.path
            ));
        }
        if array.real {
            return Err(format!(
                "{name} does not support real or shortreal memory elements in `{}`",
                self.path
            ));
        }
        if array.is_net {
            return Err(format!(
                "{name} requires a variable memory, not a net, in `{}`",
                self.path
            ));
        }
        let mut lower_bound = |node| {
            let value = self.cg.lower_expr(&self.path, node)?;
            if value.is_real() {
                return Err(format!(
                    "{name} start/finish bounds must be packed integers in `{}`",
                    self.path
                ));
            }
            Ok(value)
        };
        let start = args.get(2).copied().map(&mut lower_bound).transpose()?;
        let finish = args.get(3).copied().map(&mut lower_bound).transpose()?;
        let radix = match name {
            "$readmemb" | "$writememb" => IrMemoryRadix::Binary,
            "$readmemh" | "$writememh" => IrMemoryRadix::Hex,
            _ => unreachable!(),
        };
        Ok(IrStmt::Memory {
            write,
            path,
            array: array.ir,
            radix,
            start,
            finish,
        })
    }

    fn assertion_control_target_scope(&self, target: NodeId, task: &str) -> Result<String, String> {
        if !matches!(
            self.cg.kind(target),
            NodeKind::ModuleInst { .. }
                | NodeKind::GenScopeArray
                | NodeKind::GenScope
                | NodeKind::Stmt(StmtKind::Begin)
                | NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. })
        ) {
            return Err(format!(
                "{task} scope argument must resolve to a hierarchy or assertion in `{}`",
                self.path
            ));
        }
        let path = self.cg.waveform_name_for(target).replace('\u{1f}', ".");
        if path.is_empty() {
            Err(format!(
                "{task} scope has an empty hierarchy in `{}`",
                self.path
            ))
        } else {
            Ok(path)
        }
    }

    fn assertion_control_scope(&self, node: NodeId, task: &str) -> Result<String, String> {
        match self.cg.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .next()
                .copied()
                .map(|target| self.assertion_control_target_scope(target, task))
                .unwrap_or_else(|| {
                    Err(format!(
                        "{task} scope argument has no resolved hierarchy in `{}`",
                        self.path
                    ))
                }),
            NodeKind::Expr(ExprKind::ScopeRef { target }) => {
                self.assertion_control_target_scope(*target, task)
            }
            _ => Err(format!(
                "{task} scope argument must be a resolved hierarchy or assertion name in `{}`",
                self.path
            )),
        }
    }

    fn lower_assertion_control(
        &mut self,
        task: &str,
        kind: IrAssertionControlKind,
        args: &[NodeId],
    ) -> Result<IrStmt, String> {
        let (integral_count, scope_start) = if kind == IrAssertionControlKind::Control {
            if args.is_empty() {
                return Err(format!(
                    "$assertcontrol requires one to four integral control arguments in `{}`",
                    self.path
                ));
            }
            // Slang captures the optional assertion/directive/levels fields as
            // positional integral arguments. A hierarchy selector can only
            // follow the fourth field. With fewer than four arguments all
            // captured arguments are therefore integral; otherwise the first
            // four are controls and the remainder are selectors.
            let integral_count = args.len().min(4);
            (integral_count, integral_count)
        } else {
            let has_level = !args.is_empty();
            (has_level as usize, has_level as usize)
        };
        let mut lowered = Vec::with_capacity(integral_count);
        for argument in args.iter().take(integral_count) {
            let value = self.cg.lower_expr(&self.path, *argument)?;
            if value.is_real() {
                return Err(format!(
                    "{task} control arguments must be integral in `{}`",
                    self.path
                ));
            }
            lowered.push(value);
        }
        let scopes = args[scope_start..]
            .iter()
            .map(|argument| self.assertion_control_scope(*argument, task))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(IrStmt::AssertionControl {
            kind,
            args: lowered,
            scopes,
        })
    }

    pub(super) fn lower_sys_call(&mut self, h: NodeId, name: &str) -> Result<Vec<IrStmt>, String> {
        let args: Vec<NodeId> = self.cg.node(h).children.clone();
        if let Some(level) = severity_task_variant(name) {
            let first_is_string = match args.first().copied() {
                Some(first) => {
                    self.literal_string(first, name)?.is_some()
                        || self.cg.is_string_expr(&self.path, first)
                }
                None => false,
            };
            let (message_args, fatal_finish_number) = if level.is_fatal() {
                match args.first().copied() {
                    None => (&args[..], Some(1)),
                    Some(first)
                        if first_is_string
                            || self.cg.query_descriptor(first).is_some_and(|descriptor| {
                                matches!(descriptor.shape, TypeShape::Real { .. })
                            }) =>
                    {
                        // A string/real first argument starts the message
                        // list, rather than supplying the optional finish
                        // number; the fatal task still uses its default level
                        // 1.
                        (&args[..], Some(1))
                    }
                    Some(first) => {
                        let value = self.cg.eval_bits(first).map_err(|error| {
                            format!(
                                "$fatal finish number at {} must be an integral constant 0, 1, or 2: {error}",
                                self.finish_location(h)
                            )
                        })?;
                        if value.is_unknown() {
                            return Err(format!(
                                "$fatal finish number at {} must be a known integral constant 0, 1, or 2",
                                self.finish_location(h)
                            ));
                        }
                        let value = value.to_u128().ok_or_else(|| {
                            format!(
                                "$fatal finish number at {} must be an integral constant 0, 1, or 2",
                                self.finish_location(h)
                            )
                        })?;
                        let finish_number = u8::try_from(value)
                            .ok()
                            .filter(|value| *value <= 2)
                            .ok_or_else(|| {
                                format!(
                                    "$fatal finish number at {} must be 0, 1, or 2 (got {value})",
                                    self.finish_location(h)
                                )
                            })?;
                        (&args[1..], Some(finish_number))
                    }
                }
            } else {
                (&args[..], None)
            };
            let (fmt, severity_args) =
                self.parse_display_call(name, message_args, IrDisplayRadix::Decimal)?;
            return Ok(vec![IrStmt::Severity {
                level,
                fmt,
                args: severity_args,
                scope: self.path.clone(),
                location: self.finish_location(h),
                fatal_finish_number,
                runtime_failure: false,
            }]);
        }
        if let Some((task_kind, default_radix)) = display_task_variant(name) {
            match task_kind {
                DisplayTaskKind::Immediate { newline, file } => {
                    let (descriptor, display_args) = if file {
                        let descriptor = args.first().ok_or_else(|| {
                            format!("{name} requires a file descriptor in `{}`", self.path)
                        })?;
                        let descriptor = self.cg.lower_expr(&self.path, *descriptor)?;
                        if descriptor.is_real() {
                            return Err(format!(
                                "{name} requires a packed file descriptor in `{}`",
                                self.path
                            ));
                        }
                        (Some(descriptor), &args[1..])
                    } else {
                        (None, args.as_slice())
                    };
                    let (fmt, display_args) =
                        self.parse_display_call(name, display_args, default_radix)?;
                    return Ok(vec![IrStmt::DisplayTyped {
                        fmt,
                        args: display_args,
                        scope: self.path.clone(),
                        newline,
                        default_radix,
                        descriptor,
                        time_unit_fs: self.cg.timescale_of_node(h).unit_fs,
                    }]);
                }
                DisplayTaskKind::Deferred { strobe, file } => {
                    if self.func.is_some() || self.inline.is_some() {
                        return Err(format!(
                            "{name} in `{}` cannot escape a function or task activation",
                            self.path
                        ));
                    }
                    if let Some(local) = args
                        .iter()
                        .find_map(|arg| self.cg.nested_proc_local_ref(*arg))
                    {
                        return Err(format!(
                            "{name} cannot defer a reference to inline loop variable `{}` in `{}`",
                            self.cg.node(local).name,
                            self.path
                        ));
                    }
                    if let Some(local) =
                        args.iter().find_map(|arg| self.cg.nested_capture_ref(*arg))
                    {
                        return Err(format!(
                            "{name} in `{}` cannot defer a reference to activation storage `{}`",
                            self.path,
                            self.cg.node(local).name
                        ));
                    }
                    if self.in_final {
                        return Err(format!(
                            "{name} inside a final block in `{}` is not supported: \
                             no scheduled output events execute after final procedures",
                            self.path
                        ));
                    }
                    let (descriptor, display_args_source) = if file {
                        let descriptor = args.first().ok_or_else(|| {
                            format!("{name} requires a file descriptor in `{}`", self.path)
                        })?;
                        let descriptor = self.cg.lower_expr(&self.path, *descriptor)?;
                        if descriptor.is_real() {
                            return Err(format!(
                                "{name} requires a packed file descriptor in `{}`",
                                self.path
                            ));
                        }
                        (Some(descriptor), &args[1..])
                    } else {
                        (None, args.as_slice())
                    };
                    let (fmt, display_args) =
                        self.parse_display_call(name, display_args_source, default_radix)?;
                    let reads = if !strobe {
                        self.collect_monitor_reads(display_args_source)?
                    } else {
                        Vec::new()
                    };
                    // Generated re-evaluator: reads the CURRENT values of the
                    // displayed arguments each time the runtime prints (after an
                    // NBA commit for the monitor, at the end of the time step for
                    // $strobe).  Attached ahead of the enclosing function/process.
                    let eval_name = self.cg.new_fn_name(&self.path, "mon");
                    self.pre_fns.push(crate::sim::ir::IrPreFn::DisplayEval {
                        c_name: eval_name.clone(),
                        args: display_args.clone(),
                        time_unit_fs: self.cg.timescale_of_node(h).unit_fs,
                    });
                    return Ok(vec![IrStmt::MonitorSet {
                        strobe,
                        fmt,
                        eval: eval_name,
                        n_args: display_args.len(),
                        reads,
                        default_radix,
                        scope: self.path.clone(),
                        descriptor,
                    }]);
                }
            }
        }
        if name.starts_with("$q_") {
            return Ok(vec![self.lower_stochastic_task(name, &args)?]);
        }
        match name {
            "$asserton" => Ok(vec![self.lower_assertion_control(
                name,
                IrAssertionControlKind::On,
                &args,
            )?]),
            "$assertoff" => Ok(vec![self.lower_assertion_control(
                name,
                IrAssertionControlKind::Off,
                &args,
            )?]),
            "$assertkill" => Ok(vec![self.lower_assertion_control(
                name,
                IrAssertionControlKind::Kill,
                &args,
            )?]),
            "$assertcontrol" => Ok(vec![self.lower_assertion_control(
                name,
                IrAssertionControlKind::Control,
                &args,
            )?]),
            "$assertpasson"
            | "$assertpassoff"
            | "$assertfailon"
            | "$assertfailoff"
            | "$assertnonvacuouson"
            | "$assertvacuousoff" => Err(format!(
                "assertion control task `{name}` is outside the bounded simulator subset in `{}`",
                self.path
            )),
            "$swrite" | "$swriteb" | "$swriteo" | "$swriteh" => {
                let Some((target, values)) = args.split_first() else {
                    return Err(format!("{name} requires a destination in `{}`", self.path));
                };
                let value = self.lower_string_output_format(
                    name,
                    values,
                    match name {
                        "$swriteb" => IrDisplayRadix::Binary,
                        "$swriteo" => IrDisplayRadix::Octal,
                        "$swriteh" => IrDisplayRadix::Hex,
                        _ => IrDisplayRadix::Decimal,
                    },
                )?;
                Ok(vec![self.lower_string_format_target(*target, value)?])
            }
            "$sformat" => {
                let [target, format, values @ ..] = args.as_slice() else {
                    return Err(format!(
                        "$sformat requires a destination and format in `{}`",
                        self.path
                    ));
                };
                let value = self.lower_explicit_string_format(name, *format, values)?;
                Ok(vec![self.lower_string_format_target(*target, value)?])
            }
            "$cast" => {
                let status = self.cg.lower_dynamic_cast(&self.path, &args)?;
                Ok(vec![IrStmt::DeclLocal {
                    name: format!("_llg_cast_status_{}", h.0),
                    width: 1,
                    signed: false,
                    init: Some(Box::new(status)),
                    two_state: false,
                }])
            }
            "$test$plusargs" | "$value$plusargs" => Ok(vec![IrStmt::PlusArg(
                self.cg.lower_plusarg_expr(&self.path, name, h)?,
            )]),
            "$readmemb" | "$readmemh" | "$writememb" | "$writememh" => {
                Ok(vec![self.lower_memory_task(name, &args)?])
            }
            "$system" => Ok(vec![IrStmt::System(
                self.cg.lower_system_command(&self.path, &args)?,
            )]),
            "$fgetc" | "$ungetc" | "$fgets" | "$fscanf" | "$sscanf" | "$fread" => {
                let value = self.cg.lower_sys_func_expr(&self.path, name, h)?;
                Ok(vec![IrStmt::DeclLocal {
                    name: format!("_llg_file_input_{}", h.0),
                    width: value.width,
                    signed: value.signed,
                    init: Some(Box::new(value)),
                    two_state: false,
                }])
            }
            "$fclose" | "$fflush" | "$rewind" => {
                let (op, optional) = match name {
                    "$fclose" => (crate::sim::ir::IrFileOp::Close, false),
                    "$fflush" => (crate::sim::ir::IrFileOp::Flush, true),
                    "$rewind" => (crate::sim::ir::IrFileOp::Rewind, false),
                    _ => unreachable!(),
                };
                if (!optional && args.len() != 1) || (optional && args.len() > 1) {
                    return Err(format!(
                        "{name} requires {} file descriptor argument{} in `{}`",
                        if optional { "zero or one" } else { "one" },
                        if optional { "s" } else { "" },
                        self.path
                    ));
                }
                let descriptor = args
                    .first()
                    .map(|arg| self.cg.lower_expr(&self.path, *arg))
                    .transpose()?;
                if descriptor.as_ref().is_some_and(IrExpr::is_real) {
                    return Err(format!(
                        "{name} requires a packed file descriptor in `{}`",
                        self.path
                    ));
                }
                Ok(vec![IrStmt::FileControl { op, descriptor }])
            }
            "$monitoron" => Ok(vec![IrStmt::MonitorEnable(true)]),
            "$monitoroff" => Ok(vec![IrStmt::MonitorEnable(false)]),
            "$dumpfile" => {
                if args.len() != 1 {
                    return Err(format!(
                        "$dumpfile requires exactly one literal string argument in `{}`",
                        self.path
                    ));
                }
                let path = self
                    .literal_string(args[0], "$dumpfile path")?
                    .ok_or_else(|| {
                        format!(
                            "$dumpfile requires a literal string argument in `{}`",
                            self.path
                        )
                    })?;
                let lower_path = path.to_ascii_lowercase();
                if !lower_path.ends_with(".vcd") && !lower_path.ends_with(".fst") {
                    return Err(format!(
                        "$dumpfile path must end in .vcd or .fst in `{}`",
                        self.path
                    ));
                }
                self.cg.model.waveform = true;
                Ok(vec![IrStmt::WaveFile(path)])
            }
            "$dumpvars" => {
                self.cg.model.waveform = true;
                Ok(vec![self.lower_dumpvars(&args)?])
            }
            "$dumpon" | "$dumpoff" | "$dumpall" | "$dumpflush" => {
                if !args.is_empty() {
                    return Err(format!("{name} takes no arguments in `{}`", self.path));
                }
                self.cg.model.waveform = true;
                Ok(vec![match name {
                    "$dumpon" => IrStmt::WaveOn,
                    "$dumpoff" => IrStmt::WaveOff,
                    "$dumpall" => IrStmt::WaveDumpAll,
                    "$dumpflush" => IrStmt::WaveFlush,
                    _ => unreachable!(),
                }])
            }
            "$dumplimit" => {
                if args.len() != 1 {
                    return Err(format!(
                        "$dumplimit requires exactly one packed expression in `{}`",
                        self.path
                    ));
                }
                let limit = self.cg.lower_expr(&self.path, args[0])?;
                if limit.is_real() {
                    return Err(format!(
                        "$dumplimit requires a packed expression, not real, in `{}`",
                        self.path
                    ));
                }
                self.cg.model.waveform = true;
                Ok(vec![IrStmt::WaveLimit(limit)])
            }
            "$finish" => {
                let verbosity = self.lower_control_verbosity(h, "$finish", &args)?;
                Ok(vec![IrStmt::FinishControl {
                    verbosity,
                    location: self.finish_location(h),
                }])
            }
            "$exit" => {
                if !args.is_empty() {
                    return Err(format!("$exit takes no arguments in `{}`", self.path));
                }
                if self.in_final {
                    return Err(format!(
                        "$exit inside a final block in `{}` is not supported",
                        self.path
                    ));
                }
                // The caller's process origin determines $exit semantics, not
                // the declaration scope of a task containing the call.
                Ok(vec![IrStmt::ProgramExit])
            }
            "$stop" => {
                if self.in_final {
                    return Err(format!(
                        "$stop inside a final block in `{}` is not supported: final procedures cannot suspend",
                        self.path
                    ));
                }
                let verbosity = self.lower_control_verbosity(h, "$stop", &args)?;
                Ok(vec![IrStmt::StopControl {
                    verbosity,
                    location: self.finish_location(h),
                }])
            }
            "$printtimescale" => {
                let ts = self.cg.timescale_of_node(h);
                Ok(vec![IrStmt::PrintTimescale {
                    unit_fs: ts.unit_fs,
                    precision_fs: ts.precision_fs,
                    label: self.path.clone(),
                }])
            }
            "$timeformat" => {
                if !args.is_empty() && args.len() != 4 {
                    return Err(format!(
                        "$timeformat requires either zero or four arguments in `{}`",
                        self.path
                    ));
                }
                let default_units = (-15..=0)
                    .find(|exponent| time_exponent_to_fs(*exponent) == self.cg.design_precision_fs)
                    .ok_or_else(|| {
                        format!(
                            "$timeformat default units cannot represent design precision in `{}`",
                            self.path
                        )
                    })?;
                let units = args
                    .first()
                    .map(|node| self.cg.lower_expr(&self.path, *node))
                    .transpose()?
                    .unwrap_or_else(|| lhs_integer_expr(i128::from(default_units)));
                let precision = args
                    .get(1)
                    .map(|node| self.cg.lower_expr(&self.path, *node))
                    .transpose()?
                    .unwrap_or_else(|| lhs_integer_expr(0));
                let suffix = args
                    .get(2)
                    .map(|node| self.cg.lower_string(&self.path, *node))
                    .transpose()?
                    .unwrap_or_else(|| IrStringExpr::Literal(Vec::new()));
                let minimum_field_width = args
                    .get(3)
                    .map(|node| self.cg.lower_expr(&self.path, *node))
                    .transpose()?
                    .unwrap_or_else(|| lhs_integer_expr(20));
                for (label, value) in [
                    ("units", &units),
                    ("precision", &precision),
                    ("minimum field width", &minimum_field_width),
                ] {
                    if value.is_real() {
                        return Err(format!(
                            "$timeformat {label} argument must be integral in `{}`",
                            self.path
                        ));
                    }
                }
                Ok(vec![IrStmt::TimeFormat {
                    units,
                    precision,
                    suffix,
                    minimum_field_width,
                }])
            }
            "$displayon" | "$displayoff" => {
                self.cg
                    .warnings
                    .push(format!("{name} in `{}` skipped (not supported)", self.path));
                Ok(vec![])
            }
            _ if name.starts_with('$') => {
                let args = args
                    .into_iter()
                    .map(|arg| self.cg.lower_expr(&self.path, arg))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some((index, _)) = args
                    .iter()
                    .enumerate()
                    .find(|(_, arg)| arg.width > LLG_MAX_WIDTH)
                {
                    return Err(format!(
                        "VPI system task `{name}` argument {index} exceeds the supported width in `{}`",
                        self.path
                    ));
                }
                let site = self.cg.model.vpi_compile_calls.len();
                self.cg.model.vpi_compile_calls.push(IrVpiCompileCall::new(
                    name.to_owned(),
                    args.iter()
                        .map(|arg| IrVpiCompileArg {
                            width: arg.width,
                            signed: arg.signed,
                            real: arg.is_real(),
                        })
                        .collect(),
                ));
                self.cg.model.vpi_compile_calls[site].time_unit_fs =
                    self.cg.timescale_of_node(h).unit_fs;
                Ok(vec![IrStmt::VpiCall {
                    site,
                    name: name.to_owned(),
                    args,
                }])
            }
            _ => Err(format!("unsupported system task {name} in `{}`", self.path)),
        }
    }

    fn lower_stochastic_input(&mut self, node: NodeId, label: &str) -> Result<IrExpr, String> {
        let value = self.cg.lower_expr(&self.path, node)?;
        if value.is_real() {
            return Err(format!(
                "{label} must be a packed integer expression in `{}`",
                self.path
            ));
        }
        Ok(value)
    }

    fn lower_stochastic_output(&mut self, node: NodeId, label: &str) -> Result<IrLhs, String> {
        self.cg.lower_stochastic_output(&self.path, node, label)
    }

    fn lower_stochastic_task(&mut self, name: &str, args: &[NodeId]) -> Result<IrStmt, String> {
        let wrong_arity = || {
            Err(format!(
                "{name} requires exactly four arguments in `{}`",
                self.path
            ))
        };
        match name {
            "$q_initialize" => {
                let [q_id, q_type, max_length, status] = args else {
                    return wrong_arity();
                };
                Ok(IrStmt::Stochastic(Box::new(IrStochasticStmt::Initialize {
                    q_id: self.lower_stochastic_input(*q_id, "$q_initialize q_id")?,
                    q_type: self.lower_stochastic_input(*q_type, "$q_initialize q_type")?,
                    max_length: self
                        .lower_stochastic_input(*max_length, "$q_initialize max_length")?,
                    status: self.lower_stochastic_output(*status, "$q_initialize status")?,
                })))
            }
            "$q_add" => {
                let [q_id, job_id, inform_id, status] = args else {
                    return wrong_arity();
                };
                Ok(IrStmt::Stochastic(Box::new(IrStochasticStmt::Add {
                    q_id: self.lower_stochastic_input(*q_id, "$q_add q_id")?,
                    job_id: self.lower_stochastic_input(*job_id, "$q_add job_id")?,
                    inform_id: self.lower_stochastic_input(*inform_id, "$q_add inform_id")?,
                    status: self.lower_stochastic_output(*status, "$q_add status")?,
                })))
            }
            "$q_remove" => {
                let [q_id, job_id, inform_id, status] = args else {
                    return wrong_arity();
                };
                Ok(IrStmt::Stochastic(Box::new(IrStochasticStmt::Remove {
                    q_id: self.lower_stochastic_input(*q_id, "$q_remove q_id")?,
                    job_id: self.lower_stochastic_output(*job_id, "$q_remove job_id")?,
                    inform_id: self.lower_stochastic_output(*inform_id, "$q_remove inform_id")?,
                    status: self.lower_stochastic_output(*status, "$q_remove status")?,
                })))
            }
            "$q_exam" => {
                let [q_id, stat_code, stat_value, status] = args else {
                    return wrong_arity();
                };
                Ok(IrStmt::Stochastic(Box::new(IrStochasticStmt::Exam {
                    q_id: self.lower_stochastic_input(*q_id, "$q_exam q_id")?,
                    stat_code: self.lower_stochastic_input(*stat_code, "$q_exam stat_code")?,
                    stat_value: self.lower_stochastic_output(*stat_value, "$q_exam stat_value")?,
                    status: self.lower_stochastic_output(*status, "$q_exam status")?,
                })))
            }
            _ => Err(format!("unsupported system task {name} in `{}`", self.path)),
        }
    }
}
