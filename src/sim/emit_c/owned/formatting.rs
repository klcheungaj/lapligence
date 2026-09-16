//! Transfer packed owners to the formatter only after all arguments evaluate.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn formatted_arguments(&mut self, args: &[IrDisplayArg], time_unit: u64) -> Result<String, String> {
        let mut values = Vec::new();
        for arg in args {
            values.push(match arg {
                IrDisplayArg::Packed(expr) | IrDisplayArg::Real(expr) => Some(self.expression(expr)?),
                IrDisplayArg::String(IrStringExpr::Literal(_)) => None,
                _ => return Err(pending("nonliteral native-string format arguments")),
            });
        }
        if args.is_empty() { return Ok("NULL".to_owned()); }
        let array = self.name("format_args");
        self.line(format!("llg_fmt_arg_t {array}[{}] = {{0}};", args.len()));
        // From here through the consuming formatter there are no calls which
        // can yield. Stack descriptors therefore never hide canceled owners.
        for (index, (arg, value)) in args.iter().zip(values).enumerate() {
            self.line(format!("{array}[{index}].time_unit_fs = {time_unit}ULL;"));
            match (arg, value) {
                (IrDisplayArg::Packed(_), Some(value)) => {
                    self.line(format!("{array}[{index}].kind = LLG_FMT_PACKED;"));
                    self.line(format!("sv4_move(&{array}[{index}].value.packed, &{});", value.code));
                    self.discard(value);
                }
                (IrDisplayArg::Real(_), Some(value)) => {
                    self.line(format!("{array}[{index}].kind = LLG_FMT_REAL;"));
                    self.line(format!("{array}[{index}].value.real = {};", value.real()));
                    self.discard(value);
                }
                (IrDisplayArg::String(IrStringExpr::Literal(bytes)), None) => {
                    let literal = bytes.iter().map(|byte| format!("\\{byte:03o}")).collect::<String>();
                    self.line(format!("{array}[{index}].kind = LLG_FMT_STRING;"));
                    self.line(format!("{array}[{index}].value.string = llg_string_bytes(\"{literal}\", {});", bytes.len()));
                }
                _ => unreachable!("format argument classification"),
            }
        }
        Ok(array)
    }

    pub(super) fn display(&mut self, fmt: &str, args: &[IrDisplayArg], scope: &str,
        newline: bool, descriptor: Option<&IrExpr>, time_unit: u64) -> Result<(), String> {
        let descriptor = if let Some(expr) = descriptor {
            let value = self.expression(expr)?;
            let number = self.scalar("uint32_t", format!("llg_file_descriptor({})", value.code));
            self.discard(value);
            Some(number)
        } else { None };
        let array = self.formatted_arguments(args, time_unit)?;
        let scope = c_string_literal(scope);
        self.line(if let Some(descriptor) = descriptor {
            format!("llg_file_display_typed({descriptor}, {fmt}, {array}, {}, {scope}, {});", args.len(), u8::from(newline))
        } else { format!("{}({fmt}, {array}, {}, {scope});", if newline { "llg_display_typed" } else { "llg_write_typed" }, args.len()) });
        Ok(())
    }
}
