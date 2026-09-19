//! Numeric runtime calls with explicitly borrowed operands.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn system_expression(
        &mut self,
        function: &IrSysFunc,
        expr: &IrExpr,
    ) -> Result<Value, String> {
        if self.read_only_callback
            && matches!(
                function,
                IrSysFunc::VpiCall { .. }
                    | IrSysFunc::LegacyRandom { .. }
                    | IrSysFunc::QFull { .. }
                    | IrSysFunc::Urandom { .. }
                    | IrSysFunc::UrandomRange { .. }
            )
        {
            return Err(pending(
                "side-effecting system calls in read-only callbacks",
            ));
        }
        let result = match function {
            IrSysFunc::VpiCall { site, name, args } => self
                .vpi_call(*site, name, args, Some((expr.width, expr.signed)))?
                .ok_or_else(|| "VPI function returned no value".to_owned())?,
            IrSysFunc::LegacyRandom { kind, seed, args } => {
                self.legacy_random(*kind, seed.as_deref(), args)?
            }
            IrSysFunc::QFull { q_id, status } => self.queue_full(q_id, status)?,
            IrSysFunc::System(command) => self.system_command(command.as_ref())?,
            IrSysFunc::Sampled(call) => match call.kind {
                IrSampledFunc::Sampled => {
                    let previous = self.sampled_reads;
                    self.sampled_reads = true;
                    let value = self.expression(&call.argument);
                    self.sampled_reads = previous;
                    value?
                }
                IrSampledFunc::Past => {
                    let domain = call
                        .domain
                        .ok_or_else(|| "sampled past call has no domain".to_owned())?;
                    self.value(
                        format!("llg_sampled_domain_past({domain}, {}ULL)", call.ticks),
                        expr.width,
                        expr.signed,
                    )
                }
                kind => {
                    let domain = call
                        .domain
                        .ok_or_else(|| "sampled status call has no domain".to_owned())?;
                    let status = match kind {
                        IrSampledFunc::Rose => 0,
                        IrSampledFunc::Fell => 1,
                        IrSampledFunc::Stable => 2,
                        IrSampledFunc::Changed => 3,
                        _ => unreachable!("handled above"),
                    };
                    self.value(
                        format!(
                            "sv4_from_u64(llg_sampled_domain_status({domain}, {status}), 1, 0)"
                        ),
                        1,
                        false,
                    )
                }
            },
            IrSysFunc::Bits(arg) => {
                self.value(format!("sv4_from_u64({}, 32, 1)", arg.width), 32, true)
            }
            IrSysFunc::Time {
                precision_fs,
                unit_fs,
                kind,
            } => self.value(
                format!(
                    "sv4_from_u64(llg_time_scaled({precision_fs}ULL, {unit_fs}ULL), {}, 0)",
                    kind.width()
                ),
                kind.width(),
                false,
            ),
            IrSysFunc::Realtime {
                precision_fs,
                unit_fs,
            } => self.value(
                format!("((double)llg_time() * {precision_fs}.0 / {unit_fs}.0)"),
                0,
                true,
            ),
            IrSysFunc::Urandom { seed } => {
                if let Some(seed) = seed {
                    let seed = self.expression(seed)?;
                    let code = format!("llg_urandom_seed({})", seed.code);
                    self.replace(seed, code, 32, false)
                } else {
                    self.value("llg_urandom()".to_owned(), 32, false)
                }
            }
            IrSysFunc::UrandomRange { max, min } => {
                let max = self.expression(max)?;
                let minimum = if let Some(min) = min {
                    self.expression(min)?
                } else {
                    self.value("sv4_from_u64(0, 32, 0)".to_owned(), 32, false)
                };
                let code = format!(
                    "llg_urandom_range({}, {}, {})",
                    max.code,
                    minimum.code,
                    u8::from(min.is_some())
                );
                let result = self.replace(max, code, 32, false);
                self.discard(minimum);
                result
            }
            IrSysFunc::Clog2(arg)
            | IrSysFunc::Rtoi(arg)
            | IrSysFunc::Itor(arg)
            | IrSysFunc::RealToBits(arg)
            | IrSysFunc::BitsToReal(arg)
            | IrSysFunc::ShortRealToBits(arg)
            | IrSysFunc::BitsToShortReal(arg) => {
                let arg = self.expression(arg)?;
                let (name, parameter) = match function {
                    IrSysFunc::Clog2(_) => ("sv4_clog2", arg.code.clone()),
                    IrSysFunc::Rtoi(_) => ("sv4_rtoi", arg.real()),
                    IrSysFunc::Itor(_) => ("sv4_to_real", arg.code.clone()),
                    IrSysFunc::RealToBits(_) => ("sv4_realtobits", arg.real()),
                    IrSysFunc::BitsToReal(_) => ("sv4_bitstoreal", arg.code.clone()),
                    IrSysFunc::ShortRealToBits(_) => ("sv4_shortrealtobits", arg.real()),
                    _ => ("sv4_bitstoshortreal", arg.code.clone()),
                };
                self.replace(arg, format!("{name}({parameter})"), expr.width, expr.signed)
            }
            IrSysFunc::BitQuery { kind, arg } => {
                let value = self.expression(arg)?;
                let code = match kind {
                    IrBitQuery::CountOnes => format!("sv4_countones({})", value.code),
                    IrBitQuery::OneHot => format!("sv4_onehot({}, 0)", value.code),
                    IrBitQuery::OneHot0 => format!("sv4_onehot({}, 1)", value.code),
                    IrBitQuery::IsUnknown => {
                        format!("sv4_from_u64(sv4_is_unknown({}), 1, 0)", value.code)
                    }
                };
                self.replace(value, code, expr.width, expr.signed)
            }
            IrSysFunc::Math { kind, args } => {
                let name = match kind {
                    IrMathFunc::Ln => "log",
                    IrMathFunc::Log10 => "log10",
                    IrMathFunc::Exp => "exp",
                    IrMathFunc::Sqrt => "sqrt",
                    IrMathFunc::Pow => "pow",
                    IrMathFunc::Floor => "floor",
                    IrMathFunc::Ceil => "ceil",
                    IrMathFunc::Sin => "sin",
                    IrMathFunc::Cos => "cos",
                    IrMathFunc::Tan => "tan",
                    IrMathFunc::Asin => "asin",
                    IrMathFunc::Acos => "acos",
                    IrMathFunc::Atan => "atan",
                    IrMathFunc::Atan2 => "atan2",
                    IrMathFunc::Hypot => "hypot",
                    IrMathFunc::Sinh => "sinh",
                    IrMathFunc::Cosh => "cosh",
                    IrMathFunc::Tanh => "tanh",
                    IrMathFunc::Asinh => "asinh",
                    IrMathFunc::Acosh => "acosh",
                    IrMathFunc::Atanh => "atanh",
                };
                let mut values = Vec::new();
                for arg in args {
                    values.push(self.expression(arg)?);
                }
                let arguments = values
                    .iter()
                    .map(Value::real)
                    .collect::<Vec<_>>()
                    .join(", ");
                let result = self.value(format!("{name}({arguments})"), 0, true);
                for value in values {
                    self.discard(value);
                }
                result
            }
            IrSysFunc::TestPlusArgs { pattern } => self.test_plusargs(pattern)?,
            IrSysFunc::ValuePlusArgs { format, target } => self.value_plusargs(format, target)?,
            IrSysFunc::FileInput(input) => self.file_input(input)?,
            IrSysFunc::FileOpen { .. }
            | IrSysFunc::FileTell(_)
            | IrSysFunc::FileSeek { .. }
            | IrSysFunc::FileError { .. }
            | IrSysFunc::FileEof(_) => self.file_expression(function, expr)?,
        };
        Ok(result)
    }
}
