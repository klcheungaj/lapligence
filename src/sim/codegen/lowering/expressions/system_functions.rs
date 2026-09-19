//! System functions.

use super::*;

impl<'a> Codegen<'a> {
    fn lower_sampled_func_expr(
        &mut self,
        scope_path: &str,
        name: &str,
        args: &[NodeId],
    ) -> Result<IrExpr, String> {
        use crate::sim::ir::{IrSampledCall, IrSampledFunc};

        if name == "$sampled" {
            let [argument] = args else {
                return Err(format!(
                    "$sampled requires exactly one argument in `{scope_path}`"
                ));
            };
            let argument = self.lower_expr(scope_path, *argument)?;
            if argument.is_real() || !super::super::assertions::sampled_compatible(&argument) {
                return Err(format!(
                    "$sampled argument must be a static packed expression in `{scope_path}`"
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::Sampled(IrSampledCall::new(
                    IrSampledFunc::Sampled,
                    argument.clone(),
                    None,
                    0,
                ))),
                argument.width,
                argument.signed,
                None,
            ));
        }

        let (kind, global, future) = match name {
            "$rose" => (IrSampledFunc::Rose, false, false),
            "$fell" => (IrSampledFunc::Fell, false, false),
            "$stable" => (IrSampledFunc::Stable, false, false),
            "$changed" => (IrSampledFunc::Changed, false, false),
            "$past" => (IrSampledFunc::Past, false, false),
            "$past_gclk" => (IrSampledFunc::Past, true, false),
            "$rose_gclk" => (IrSampledFunc::Rose, true, false),
            "$fell_gclk" => (IrSampledFunc::Fell, true, false),
            "$stable_gclk" => (IrSampledFunc::Stable, true, false),
            "$changed_gclk" => (IrSampledFunc::Changed, true, false),
            "$future_gclk" | "$rising_gclk" | "$falling_gclk" | "$steady_gclk"
            | "$changing_gclk" => (IrSampledFunc::Past, true, true),
            _ => return Err(format!("unsupported sampled-value function `{name}`")),
        };
        if future {
            return Err(format!(
                "future global sampled-value function `{name}` is not supported; future values are never read from live storage in `{scope_path}`"
            ));
        }

        let expected = if kind == IrSampledFunc::Past && !global {
            1..=4
        } else if global {
            1..=1
        } else {
            1..=2
        };
        if !expected.contains(&args.len()) {
            return Err(format!(
                "{name} has invalid argument count in `{scope_path}`"
            ));
        }
        let argument = self.lower_expr(scope_path, args[0])?;
        if argument.is_real() || !super::super::assertions::sampled_compatible(&argument) {
            return Err(format!(
                "{name} argument must be a static packed expression in `{scope_path}`"
            ));
        }

        let mut ticks = 0;
        let mut gate = None;
        let mut explicit_clock = None;
        if kind == IrSampledFunc::Past && !global {
            if let Some(node) = args.get(1) {
                if !matches!(self.kind(*node), NodeKind::Expr(ExprKind::Other)) {
                    let value = self.eval_bound_i128(*node).map_err(|_| {
                        format!("$past tick count must be a positive constant in `{scope_path}`")
                    })?;
                    ticks = u64::try_from(value).map_err(|_| {
                        format!("$past tick count is outside the supported range in `{scope_path}`")
                    })?;
                    if ticks == 0 {
                        return Err(format!(
                            "$past tick count must be positive in `{scope_path}`"
                        ));
                    }
                }
            }
            if ticks == 0 {
                ticks = 1;
            }
            if let Some(node) = args.get(2) {
                if !matches!(self.kind(*node), NodeKind::Expr(ExprKind::Other)) {
                    gate = Some(self.lower_boolean_expr(scope_path, *node)?);
                }
            }
            explicit_clock = args.get(3).copied();
        } else if !global {
            explicit_clock = args.get(1).copied();
        }
        if global && kind == IrSampledFunc::Past {
            ticks = 1;
        }

        let mut clock = if global {
            self.lower_global_sampled_clock(scope_path)?
        } else if let Some(node) = explicit_clock {
            self.lower_sampled_clock_event(scope_path, node)?
        } else {
            self.sampled_clock
                .or(self.lower_default_sampled_clock(scope_path)?)
                .ok_or_else(|| {
                    format!(
                        "{name} requires an explicit clocking event outside a clocked assertion in `{scope_path}`"
                    )
                })?
        };
        if let Some(event_gate) = clock.gate.take() {
            let event_gate = self.lower_boolean_expr(scope_path, event_gate)?;
            if !super::super::assertions::sampled_compatible(&event_gate) {
                return Err(format!(
                    "sampled clock gate must be a static packed expression in `{scope_path}`"
                ));
            }
            gate = Some(match gate {
                Some(gate) => IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::LogAnd,
                        a: Box::new(gate),
                        b: Box::new(event_gate),
                    },
                    1,
                    false,
                    None,
                ),
                None => event_gate,
            });
        }
        if let Some(gate) = &gate {
            if !super::super::assertions::sampled_compatible(gate) {
                return Err(format!(
                    "$past gate must be a static packed expression in `{scope_path}`"
                ));
            }
        }
        let domain = self.lower_sampled_domain(
            scope_path,
            SampledClock {
                signal: clock.signal,
                posedge: clock.posedge,
                gate: None,
            },
            argument.clone(),
            gate,
        )?;
        let (width, signed) = if kind == IrSampledFunc::Past {
            (argument.width, argument.signed)
        } else {
            (1, false)
        };
        Ok(IrExpr::new(
            IrExprKind::SysFunc(IrSysFunc::Sampled(IrSampledCall::new(
                kind,
                argument,
                Some(domain),
                ticks,
            ))),
            width,
            signed,
            None,
        ))
    }

    /// Lower system-function expressions ($system/$clog2/$time/$stime/$bits/
    /// $signed/$unsigned); timescale scaling happens here.
    pub(in super::super) fn lower_sys_func_expr(
        &mut self,
        scope_path: &str,
        name: &str,
        call: NodeId,
    ) -> Result<IrExpr, String> {
        let args: Vec<NodeId> = self.node(call).children.clone();
        if matches!(
            name,
            "$sampled"
                | "$rose"
                | "$fell"
                | "$stable"
                | "$changed"
                | "$past"
                | "$past_gclk"
                | "$rose_gclk"
                | "$fell_gclk"
                | "$stable_gclk"
                | "$changed_gclk"
                | "$future_gclk"
                | "$rising_gclk"
                | "$falling_gclk"
                | "$steady_gclk"
                | "$changing_gclk"
        ) {
            return self.lower_sampled_func_expr(scope_path, name, &args);
        }
        if name == "$q_full" {
            let [q_id, status] = args.as_slice() else {
                return Err(format!(
                    "$q_full requires exactly two arguments in `{scope_path}`"
                ));
            };
            let q_id = self.lower_expr(scope_path, *q_id)?;
            if q_id.is_real() {
                return Err(format!(
                    "$q_full q_id must be a packed integer in `{scope_path}`"
                ));
            }
            let status = self.lower_stochastic_output(scope_path, *status, "$q_full status")?;
            return Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::QFull {
                    q_id: Box::new(q_id),
                    status: Box::new(status),
                }),
                32,
                true,
                None,
            ));
        }
        if let Some(kind) = Self::legacy_random_kind(name) {
            return self.lower_legacy_random_expr(scope_path, kind, &args);
        }
        if name == "index" {
            if let Some(iterator) = self.container_iterator {
                let [receiver] = args.as_slice() else {
                    return Err(format!(
                        "array-method iterator index in `{scope_path}` has an invalid argument list"
                    ));
                };
                let is_iterator = matches!(
                    self.kind(*receiver),
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target)
                    }) if *target == iterator.node
                );
                if !is_iterator {
                    return Err(format!(
                        "array-method iterator index in `{scope_path}` has an unresolved binding"
                    ));
                }
                if iterator.index_width == 0 {
                    return Err(format!(
                        "array-method iterator index in `{scope_path}` is not representable for this receiver"
                    ));
                }
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead("__llg_method_index".to_owned()),
                    iterator.index_width,
                    iterator.index_signed,
                    None,
                ));
            }
        }
        use crate::sim::ir::IrMathFunc;
        let math = match name {
            "$ln" => Some(IrMathFunc::Ln),
            "$log10" => Some(IrMathFunc::Log10),
            "$exp" => Some(IrMathFunc::Exp),
            "$sqrt" => Some(IrMathFunc::Sqrt),
            "$pow" => Some(IrMathFunc::Pow),
            "$floor" => Some(IrMathFunc::Floor),
            "$ceil" => Some(IrMathFunc::Ceil),
            "$sin" => Some(IrMathFunc::Sin),
            "$cos" => Some(IrMathFunc::Cos),
            "$tan" => Some(IrMathFunc::Tan),
            "$asin" => Some(IrMathFunc::Asin),
            "$acos" => Some(IrMathFunc::Acos),
            "$atan" => Some(IrMathFunc::Atan),
            "$atan2" => Some(IrMathFunc::Atan2),
            "$hypot" => Some(IrMathFunc::Hypot),
            "$sinh" => Some(IrMathFunc::Sinh),
            "$cosh" => Some(IrMathFunc::Cosh),
            "$tanh" => Some(IrMathFunc::Tanh),
            "$asinh" => Some(IrMathFunc::Asinh),
            "$acosh" => Some(IrMathFunc::Acosh),
            "$atanh" => Some(IrMathFunc::Atanh),
            _ => None,
        };
        if let Some(kind) = math {
            if args.len() != kind.arity() {
                return Err(format!(
                    "{name} requires {} arguments in `{scope_path}`",
                    kind.arity()
                ));
            }
            let args = args
                .into_iter()
                .map(|arg| self.lower_expr(scope_path, arg))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::Math { kind, args }),
                0,
                true,
                None,
            ));
        }
        match name {
            "$urandom" => {
                if args.len() > 1 {
                    return Err(format!(
                        "$urandom accepts zero or one argument in `{scope_path}`"
                    ));
                }
                let seed = args
                    .first()
                    .map(|arg| self.lower_expr(scope_path, *arg))
                    .transpose()?;
                let seed = seed
                    .map(|value| {
                        if value.is_real() {
                            Err(format!("$urandom seed must be integral in `{scope_path}`"))
                        } else {
                            Ok(IrExpr::convert_to(value, 32, false))
                        }
                    })
                    .transpose()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Urandom {
                        seed: seed.map(Box::new),
                    }),
                    32,
                    false,
                    None,
                ))
            }
            "$urandom_range" => {
                if !(1..=2).contains(&args.len()) {
                    return Err(format!(
                        "$urandom_range requires one or two arguments in `{scope_path}`"
                    ));
                }
                let max = self.lower_expr(scope_path, args[0])?;
                if max.is_real() {
                    return Err(format!(
                        "$urandom_range maximum must be integral in `{scope_path}`"
                    ));
                }
                let min = args
                    .get(1)
                    .map(|arg| self.lower_expr(scope_path, *arg))
                    .transpose()?;
                if min.as_ref().is_some_and(IrExpr::is_real) {
                    return Err(format!(
                        "$urandom_range minimum must be integral in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::UrandomRange {
                        max: Box::new(IrExpr::convert_to(max, 32, false)),
                        min: min.map(|value| Box::new(IrExpr::convert_to(value, 32, false))),
                    }),
                    32,
                    false,
                    None,
                ))
            }
            "$cast" => self.lower_dynamic_cast(scope_path, &args),
            "$test$plusargs" | "$value$plusargs" => self.lower_plusarg_expr(scope_path, name, call),
            "$system" => Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::System(
                    self.lower_system_command(scope_path, &args)?,
                )),
                32,
                true,
                None,
            )),
            "$fopen" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(format!(
                        "$fopen requires one or two string arguments in `{scope_path}`"
                    ));
                }
                let path = self.lower_string(scope_path, args[0])?;
                let mode = args
                    .get(1)
                    .map(|argument| self.lower_string(scope_path, *argument))
                    .transpose()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileOpen { path, mode }),
                    32,
                    true,
                    None,
                ))
            }
            "$ftell" | "$feof" => {
                let [argument] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one file descriptor in `{scope_path}`"
                    ));
                };
                let descriptor = self.lower_expr(scope_path, *argument)?;
                if descriptor.is_real() {
                    return Err(format!(
                        "{name} requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                let function = if name == "$ftell" {
                    IrSysFunc::FileTell(Box::new(descriptor))
                } else {
                    IrSysFunc::FileEof(Box::new(descriptor))
                };
                let (width, signed) = if name == "$ftell" {
                    (64, true)
                } else {
                    (32, true)
                };
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(function),
                    width,
                    signed,
                    None,
                ))
            }
            "$fseek" => {
                let [descriptor, offset, operation] = args.as_slice() else {
                    return Err(format!(
                        "$fseek requires descriptor, offset, and operation in `{scope_path}`"
                    ));
                };
                let descriptor = self.lower_expr(scope_path, *descriptor)?;
                let offset = self.lower_expr(scope_path, *offset)?;
                let operation = self.lower_expr(scope_path, *operation)?;
                if descriptor.is_real() || offset.is_real() || operation.is_real() {
                    return Err(format!(
                        "$fseek requires packed arguments in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileSeek {
                        descriptor: Box::new(descriptor),
                        offset: Box::new(offset),
                        operation: Box::new(operation),
                    }),
                    32,
                    true,
                    None,
                ))
            }
            "$ferror" => {
                if args.len() != 1 && args.len() != 2 {
                    return Err(format!(
                        "$ferror requires a descriptor and optional string output in `{scope_path}`"
                    ));
                }
                let descriptor = self.lower_expr(scope_path, args[0])?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$ferror requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                let message = args
                    .get(1)
                    .map(|argument| {
                        let argument = match self.kind(*argument) {
                            NodeKind::Expr(ExprKind::Operation {
                                op: Operation::Assignment,
                                operands,
                                ..
                            }) => operands.first().copied().ok_or_else(|| {
                                format!("$ferror output argument is malformed in `{scope_path}`")
                            })?,
                            _ => *argument,
                        };
                        self.ensure_string_actual_writable(scope_path, argument)?;
                        self.lower_string_actual_address(scope_path, argument)
                    })
                    .transpose()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileError {
                        descriptor: Box::new(descriptor),
                        message,
                    }),
                    32,
                    true,
                    None,
                ))
            }
            "$fgetc" => {
                let [descriptor] = args.as_slice() else {
                    return Err(format!(
                        "$fgetc requires exactly one file descriptor in `{scope_path}`"
                    ));
                };
                let descriptor = self.lower_expr(scope_path, *descriptor)?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$fgetc requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::Getc {
                        descriptor: Box::new(descriptor),
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$ungetc" => {
                let [character, descriptor] = args.as_slice() else {
                    return Err(format!(
                        "$ungetc requires a character and file descriptor in `{scope_path}`"
                    ));
                };
                let character = self.lower_expr(scope_path, *character)?;
                let descriptor = self.lower_expr(scope_path, *descriptor)?;
                if character.is_real() || descriptor.is_real() {
                    return Err(format!(
                        "$ungetc requires packed arguments in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::Ungetc {
                        character: Box::new(character),
                        descriptor: Box::new(descriptor),
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$fgets" => {
                let [destination, descriptor] = args.as_slice() else {
                    return Err(format!(
                        "$fgets requires a string destination and file descriptor in `{scope_path}`"
                    ));
                };
                let destination = self.file_input_actual(*destination)?;
                let target = if self.is_string_expr(scope_path, destination) {
                    self.ensure_string_actual_writable(scope_path, destination)?;
                    IrFileInputTarget::String {
                        address: self.lower_string_actual_address(scope_path, destination)?,
                    }
                } else {
                    self.lower_file_input_target(scope_path, destination)?
                };
                if matches!(target, IrFileInputTarget::Real { .. }) {
                    return Err(format!(
                        "$fgets destination must be packed or string storage in `{scope_path}`"
                    ));
                }
                let descriptor = self.lower_expr(scope_path, *descriptor)?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$fgets requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::Gets {
                        descriptor: Box::new(descriptor),
                        target,
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$fscanf" => {
                if args.len() < 2 {
                    return Err(format!(
                        "$fscanf requires a descriptor, format, and optional destinations in `{scope_path}`"
                    ));
                }
                let descriptor = self.lower_expr(scope_path, args[0])?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$fscanf requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                let format = self.lower_plusarg_text(scope_path, args[1], "$fscanf format")?;
                let targets = args[2..]
                    .iter()
                    .map(|argument| {
                        let actual = self.file_input_actual(*argument)?;
                        self.lower_file_input_target(scope_path, actual)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::ScanFile {
                        descriptor: Box::new(descriptor),
                        format,
                        targets,
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$sscanf" => {
                if args.len() < 2 {
                    return Err(format!(
                        "$sscanf requires a source string, format, and optional destinations in `{scope_path}`"
                    ));
                }
                let source = self.lower_string(scope_path, args[0])?;
                let format = self.lower_plusarg_text(scope_path, args[1], "$sscanf format")?;
                let targets = args[2..]
                    .iter()
                    .map(|argument| {
                        let actual = self.file_input_actual(*argument)?;
                        self.lower_file_input_target(scope_path, actual)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::ScanString {
                        source,
                        format,
                        targets,
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$fread" => {
                if !(2..=4).contains(&args.len()) {
                    return Err(format!(
                        "$fread requires destination, descriptor, and optional start/count in `{scope_path}`"
                    ));
                }
                let destination = self.file_input_actual(args[0])?;
                let target = self.lower_file_read_target(scope_path, destination)?;
                let descriptor = self.lower_expr(scope_path, args[1])?;
                if descriptor.is_real() {
                    return Err(format!(
                        "$fread requires a packed file descriptor in `{scope_path}`"
                    ));
                }
                let start = match args.get(2).copied() {
                    Some(node) if matches!(self.kind(node), NodeKind::Expr(ExprKind::Other)) => {
                        None
                    }
                    Some(node) => Some(self.lower_expr(scope_path, node)?),
                    None => None,
                };
                let count = args
                    .get(3)
                    .map(|node| self.lower_expr(scope_path, *node))
                    .transpose()?;
                if let Some(value) = start.as_ref().or(count.as_ref()) {
                    if value.is_real() {
                        return Err(format!(
                            "$fread start/count must be packed expressions in `{scope_path}`"
                        ));
                    }
                }
                if matches!(target, IrFileReadTarget::Packed { .. })
                    && (start.is_some() || count.is_some())
                {
                    return Err(format!(
                        "$fread start/count bounds require an unpacked array destination in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::FileInput(IrFileInput::Read {
                        descriptor: Box::new(descriptor),
                        target,
                        start: start.map(Box::new),
                        count: count.map(Box::new),
                    })),
                    32,
                    true,
                    None,
                ))
            }
            "$dimensions" | "$unpacked_dimensions" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one argument in `{scope_path}`"
                    ));
                };
                let descriptor = self.query_descriptor(*arg).ok_or_else(|| {
                    format!("{name} argument has no owned type metadata in `{scope_path}`")
                })?;
                let count = if name == "$dimensions" {
                    Self::query_dimensions_for(descriptor).len()
                } else {
                    usize::try_from(Self::query_unpacked_dimensions_for(descriptor))
                        .unwrap_or(usize::MAX)
                };
                Ok(Self::query_integer(i128::try_from(count).map_err(
                    |_| format!("{name} dimension count is too large in `{scope_path}`"),
                )?))
            }
            "$isunbounded" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "$isunbounded requires exactly one argument in `{scope_path}`"
                    ));
                };
                let target = match self.kind(*arg) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) => *target,
                    _ => *arg,
                };
                let target_kind = self.kind(target);
                let is_unbounded = matches!(target_kind, NodeKind::Expr(ExprKind::Unbounded))
                    || matches!(target_kind, NodeKind::Param { value: None, .. })
                    || matches!(
                        target_kind,
                        NodeKind::Param { ty, .. }
                            if ty.kind == "unbounded"
                                || ty.type_name.as_deref() == Some("$")
                    )
                    || self.query_descriptor(target).is_some_and(|descriptor| {
                        descriptor.info.kind == "unbounded"
                            || descriptor.name == "$"
                            || descriptor.name.contains("unbounded")
                    });
                // IEEE 1800-2009 20.6.3 takes a constant_expression: an
                // elaborated parameter or literal is legal and reports
                // false unless it denotes `$`; a runtime variable is not.
                let is_constant = matches!(
                    target_kind,
                    NodeKind::Param { .. }
                        | NodeKind::Expr(ExprKind::Constant { .. })
                        | NodeKind::Expr(ExprKind::Unbounded)
                );
                if !is_constant {
                    return Err(format!(
                        "$isunbounded requires a constant parameter or unbounded literal in `{scope_path}`"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Const(IrConst {
                        bits: vec![u64::from(is_unbounded)],
                        x: vec![0],
                        z: vec![0],
                        width: 1,
                        signed: false,
                        real: None,
                        fill: None,
                    }),
                    1,
                    false,
                    None,
                ))
            }
            "$left" | "$right" | "$low" | "$high" | "$increment" | "$size" => {
                self.lower_array_query(scope_path, name, &args)
            }
            "$realtime" => Ok(IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::Realtime {
                    precision_fs: self.design_precision_fs,
                    unit_fs: self.timescale_of_node(call).unit_fs,
                }),
                0,
                true,
                None,
            )),
            "$rtoi" | "$itor" | "$realtobits" | "$bitstoreal" | "$shortrealtobits"
            | "$bitstoshortreal" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one argument in `{scope_path}`"
                    ));
                };
                let arg = self.lower_expr(scope_path, *arg)?;
                match name {
                    "$rtoi" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::Rtoi(Box::new(arg))),
                        32,
                        true,
                        None,
                    )),
                    "$itor" => {
                        if arg.is_real() {
                            return Ok(arg);
                        }
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::Itor(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                    "$realtobits" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::RealToBits(Box::new(arg))),
                        64,
                        false,
                        None,
                    )),
                    "$bitstoreal" => {
                        if arg.is_real() || arg.width != 64 {
                            return Err(format!(
                                "$bitstoreal requires an exactly 64-bit packed argument in `{scope_path}`"
                            ));
                        }
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::BitsToReal(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                    "$shortrealtobits" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::ShortRealToBits(Box::new(arg))),
                        32,
                        false,
                        None,
                    )),
                    _ => {
                        if arg.is_real() || arg.width != 32 {
                            return Err(format!(
                                "$bitstoshortreal requires an exactly 32-bit packed argument in `{scope_path}`"
                            ));
                        }
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::BitsToShortReal(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                }
            }
            "$countones" | "$onehot" | "$onehot0" | "$isunknown" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one argument in `{scope_path}`"
                    ));
                };
                let arg = self.lower_expr(scope_path, *arg)?;
                if arg.is_real() {
                    return Err(format!(
                        "{name} requires a packed integral argument in `{scope_path}`"
                    ));
                }
                let kind = match name {
                    "$countones" => IrBitQuery::CountOnes,
                    "$onehot" => IrBitQuery::OneHot,
                    "$onehot0" => IrBitQuery::OneHot0,
                    _ => IrBitQuery::IsUnknown,
                };
                let (width, signed) = kind.result_type();
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::BitQuery {
                        kind,
                        arg: Box::new(arg),
                    }),
                    width,
                    signed,
                    None,
                ))
            }
            "$clog2" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("$clog2 without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "clog2 on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Clog2(Box::new(a))),
                    32,
                    false,
                    None,
                ))
            }
            "$time" | "$stime" => {
                // Both functions return the current time in the CALLING
                // module's unit; `$stime` is the 32-bit form. `llg_time()` is
                // in design-precision ticks (1 tick = design_precision_fs
                // fs), so now_fs = now * P.
                let unit_fs = self.timescale_of_node(call).unit_fs;
                let kind = if name == "$stime" {
                    IrTimeKind::STime
                } else {
                    IrTimeKind::Time
                };
                let width = kind.width();
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Time {
                        precision_fs: self.design_precision_fs,
                        unit_fs,
                        kind,
                    }),
                    width,
                    false,
                    None,
                ))
            }
            "$bits" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("$bits without argument in `{scope_path}`"))?;
                if let Some(descriptor) = self.query_descriptor(a).cloned() {
                    if let Some(width) = descriptor.fixed_size_bits() {
                        return Ok(Self::query_integer(i128::from(width)));
                    }
                    if let Some(container) = self.container_of(a) {
                        let element_width = match &self.model.containers[container.ir].element {
                            crate::sim::ir::IrContainerElement::Packed { width, .. } => *width,
                            _ => {
                                return Err(format!(
                                    "$bits on a non-packed container is not supported in `{scope_path}`"
                                ))
                            }
                        };
                        let size = IrExpr::new(
                            IrExprKind::Container(Box::new(IrContainerExpr::Size(container.ir))),
                            32,
                            true,
                            None,
                        );
                        return Ok(IrExpr::new(
                            IrExprKind::Bin {
                                op: IrBinOp::Mul,
                                a: Box::new(size),
                                b: Box::new(Self::query_integer(i128::from(element_width))),
                            },
                            32,
                            true,
                            None,
                        ));
                    }
                    if descriptor.shape == TypeShape::String
                        && self.object_of(scope_path, a).is_some_and(|index| {
                            self.model.objects[index].ty == IrObjectType::String
                        })
                    {
                        let len = IrExpr::new(
                            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringLen(
                                self.lower_string(scope_path, a)?,
                            ))),
                            32,
                            true,
                            None,
                        );
                        return Ok(IrExpr::new(
                            IrExprKind::Bin {
                                op: IrBinOp::Mul,
                                a: Box::new(len),
                                b: Box::new(Self::query_integer(8)),
                            },
                            32,
                            true,
                            None,
                        ));
                    }
                }
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "bits on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Bits(Box::new(a))),
                    32,
                    true,
                    None,
                ))
            }
            "$signed" | "$unsigned" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("{name} without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "{name} on real value in `{scope_path}` is not supported"
                    ));
                }
                let s = name == "$signed";
                let w = a.width;
                Ok(IrExpr::resize_to(a, w, s))
            }
            _ if name.starts_with('$') => {
                let (width, signed) = match self.query_descriptor(call) {
                    Some(descriptor) => match descriptor.shape {
                        TypeShape::Real { .. } => (0, true),
                        TypeShape::PackedAtom { .. } | TypeShape::Aggregate(_) => {
                            let width = descriptor.fixed_size_bits().ok_or_else(|| {
                                format!(
                                    "VPI system function `{name}` has an unsupported non-packed result in `{scope_path}`"
                                )
                            })?;
                            let width = u32::try_from(width).map_err(|_| {
                                format!(
                                    "VPI system function `{name}` result is wider than 32 bits in `{scope_path}`"
                                )
                            })?;
                            if width == 0 || width > LLG_MAX_WIDTH {
                                return Err(format!(
                                    "VPI system function `{name}` result width {width} is outside the supported range in `{scope_path}`"
                                ));
                            }
                            (width, descriptor.info.signed)
                        }
                        _ => {
                            return Err(format!(
                                "VPI system function `{name}` has an unsupported result type in `{scope_path}`"
                            ));
                        }
                    },
                    // Slang does not always attach a descriptor to an
                    // unregistered system call. VPI's default integer result
                    // remains useful until the plugin's sizetf is consulted.
                    None => (32, true),
                };
                let args = args
                    .into_iter()
                    .map(|arg| self.lower_expr(scope_path, arg))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some((index, _)) = args
                    .iter()
                    .enumerate()
                    .find(|(_, arg)| arg.width > LLG_MAX_WIDTH)
                {
                    return Err(format!(
                        "VPI system function `{name}` argument {index} exceeds the supported width in `{scope_path}`"
                    ));
                }
                let site = self.model.vpi_compile_calls.len();
                self.model.vpi_compile_calls.push(IrVpiCompileCall::new(
                    name.to_owned(),
                    args.iter()
                        .map(|arg| IrVpiCompileArg {
                            width: arg.width,
                            signed: arg.signed,
                            real: arg.is_real(),
                        })
                        .collect(),
                ));
                self.model.vpi_compile_calls[site].time_unit_fs =
                    self.timescale_of_node(call).unit_fs;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::VpiCall {
                        site,
                        name: name.to_owned(),
                        args,
                    }),
                    width,
                    signed,
                    None,
                ))
            }
            _ => Err(format!(
                "unsupported system function {name} in `{scope_path}`"
            )),
        }
    }

    /// Lower one output argument of an IEEE stochastic queue task. The C ABI
    /// receives a direct `sv4_t*`, so only whole packed storage is admitted;
    /// selected aliases and real values remain explicit unsupported lowering
    /// diagnostics rather than silently writing a temporary.
    pub(in super::super) fn lower_stochastic_output(
        &mut self,
        path: &str,
        node: NodeId,
        label: &str,
    ) -> Result<IrLhs, String> {
        // Slang wraps output actuals in an assignment conversion whose first
        // operand is the caller's storage (the second is a frontend-only
        // converted placeholder). Match ordinary output-formal binding and
        // lower the actual itself as the direct runtime destination.
        let node = match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::Assignment =>
            {
                operands
                    .first()
                    .copied()
                    .ok_or_else(|| format!("{label} has a malformed output argument in `{path}`"))?
            }
            _ => node,
        };
        let lhs = self.lower_lhs(path, node).map_err(|error| {
            let children = self
                .node(node)
                .children
                .iter()
                .map(|child| format!("{child:?}={:?}", self.kind(*child)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{label} has an unsupported target ({:?}) children=[{children}] in `{path}`: {error}",
                self.kind(node)
            )
        })?;
        match &lhs {
            IrLhs::Whole(index) => {
                let signal = self.model.signal(*index);
                if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                    return Err(format!(
                        "{label} must name a whole packed integer variable, not a net, in `{path}`"
                    ));
                }
            }
            IrLhs::WholeRef { width, .. } if *width != 0 => {}
            IrLhs::WholeRef { .. } => {
                return Err(format!(
                    "{label} must name a whole packed integer variable in `{path}`"
                ));
            }
            _ => {
                return Err(format!(
                    "{label} must name a whole packed integer variable in `{path}`"
                ));
            }
        }
        Ok(lhs)
    }

    fn legacy_random_kind(name: &str) -> Option<crate::sim::ir::IrRandomFunc> {
        use crate::sim::ir::IrRandomFunc;
        Some(match name {
            "$random" => IrRandomFunc::Random,
            "$dist_uniform" => IrRandomFunc::Uniform,
            "$dist_normal" => IrRandomFunc::Normal,
            "$dist_exponential" => IrRandomFunc::Exponential,
            "$dist_poisson" => IrRandomFunc::Poisson,
            "$dist_chi_square" => IrRandomFunc::ChiSquare,
            "$dist_t" => IrRandomFunc::StudentT,
            "$dist_erlang" => IrRandomFunc::Erlang,
            _ => return None,
        })
    }

    fn lower_legacy_random_expr(
        &mut self,
        scope_path: &str,
        kind: crate::sim::ir::IrRandomFunc,
        args: &[NodeId],
    ) -> Result<IrExpr, String> {
        let (seed_node, parameter_nodes) = if kind == crate::sim::ir::IrRandomFunc::Random {
            match args {
                [] => (None, &[][..]),
                [seed] => (Some(*seed), &[][..]),
                _ => {
                    return Err(format!(
                        "$random accepts zero or one seed argument in `{scope_path}`"
                    ))
                }
            }
        } else {
            let Some((seed, parameters)) = args.split_first() else {
                return Err(format!(
                    "legacy random function requires a writable seed in `{scope_path}`"
                ));
            };
            (Some(*seed), parameters)
        };
        if parameter_nodes.len() != kind.arity() {
            return Err(format!(
                "legacy random function has {} parameter(s), got {} in `{scope_path}`",
                kind.arity(),
                parameter_nodes.len()
            ));
        }

        let seed = seed_node
            .map(|node| {
                // Slang inserts an implicit integral cast when adapting the
                // writable seed actual to the legacy system-function formal.
                // The cast is a value-view node, not storage, so peel it
                // before resolving the actual assignment target.
                let mut node = node;
                loop {
                    match self.kind(node) {
                        NodeKind::Expr(ExprKind::Cast { operand, .. }) => node = *operand,
                        NodeKind::Expr(ExprKind::Operation {
                            op: Operation::Assignment,
                            operands,
                            ..
                        }) if !operands.is_empty() => node = operands[0],
                        _ => break,
                    }
                }
                let lhs = self.lower_lhs(scope_path, node)?;
                let Some((width, _signed, _two_state, const_ref)) = self.ref_lhs_type(&lhs) else {
                    return Err(format!(
                        "legacy random seed must be an integral variable in `{scope_path}`"
                    ));
                };
                if width == 0 || const_ref {
                    return Err(format!(
                        "legacy random seed must be a writable integral variable in `{scope_path}`"
                    ));
                }
                Ok(Box::new(lhs))
            })
            .transpose()?;
        let mut parameters = Vec::with_capacity(parameter_nodes.len());
        for node in parameter_nodes {
            let parameter = self.lower_expr(scope_path, *node)?;
            if parameter.is_real() {
                return Err(format!(
                    "legacy random parameters must be integral in `{scope_path}`"
                ));
            }
            parameters.push(parameter);
        }
        Ok(IrExpr::new(
            IrExprKind::SysFunc(IrSysFunc::LegacyRandom {
                kind,
                seed,
                args: parameters,
            }),
            32,
            true,
            None,
        ))
    }
}
