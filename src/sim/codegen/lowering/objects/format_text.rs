//! Display-family argument lists and literal format text (SV 21.2.1).
//!
//! Every string-literal argument of a display-family task is a format
//! segment whose conversions consume the arguments that follow it; any other
//! argument is formatted with the task's default conversion. Literal formats
//! are checked against typed arguments here, so the runtime formatter only
//! interprets validated text. Dynamic `$sformat`/`$sformatf` formats keep the
//! same runtime formatter but are interpreted only at run time.
use super::*;
use crate::sim::ir::IrDisplayRadix;

impl Codegen<'_> {
    /// Lower a display-family argument list into normalized format text and
    /// the typed values it consumes, in source order. `library` is the
    /// calling scope's `%l` text, when known.
    pub(in super::super) fn lower_format_segments(
        &mut self,
        path: &str,
        library: Option<&str>,
        name: &str,
        args: &[NodeId],
        default_radix: IrDisplayRadix,
    ) -> Result<(Vec<u8>, Vec<IrDisplayArg>), String> {
        let mut format = Vec::new();
        let mut values = Vec::new();
        let mut index = 0usize;
        while index < args.len() {
            let node = args[index];
            if self.is_empty_format_argument(node) {
                // An empty argument displays one space (SV 21.2.1).
                format.push(b' ');
                index += 1;
                continue;
            }
            if let Some(text) = self.format_literal_bytes(node, name)? {
                let consumed = self.parse_format_text(
                    path,
                    library,
                    name,
                    &text,
                    &args[index + 1..],
                    &mut format,
                    &mut values,
                )?;
                index += consumed + 1;
                continue;
            }
            self.reject_unformattable(path, name, default_radix.specifier() as u8, node)?;
            let value = self.lower_format_arg(path, node)?;
            push_default_conversion(&mut format, &value, default_radix);
            values.push(value);
            index += 1;
        }
        Ok((format, values))
    }

    /// Lower `$sformat`/`$sformatf`: the explicit format argument is the
    /// only format. A literal format is validated here and any argument it
    /// does not consume gets the default decimal conversion; a dynamic
    /// format is interpreted by the runtime formatter.
    pub(in super::super) fn lower_explicit_format(
        &mut self,
        path: &str,
        library: Option<&str>,
        name: &str,
        format_node: NodeId,
        args: &[NodeId],
    ) -> Result<IrStringExpr, String> {
        let scope = self.format_scope(path, format_node);
        if let Some(text) = self.format_literal_bytes(format_node, name)? {
            let mut format = Vec::new();
            let mut values = Vec::new();
            let consumed =
                self.parse_format_text(path, library, name, &text, args, &mut format, &mut values)?;
            for node in &args[consumed..] {
                if self.is_empty_format_argument(*node) {
                    format.push(b' ');
                    continue;
                }
                self.reject_unformattable(path, name, b'd', *node)?;
                let value = self.lower_format_arg(path, *node)?;
                push_default_conversion(&mut format, &value, IrDisplayRadix::Decimal);
                values.push(value);
            }
            return Ok(IrStringExpr::Format {
                format: Box::new(IrStringExpr::Literal(format)),
                args: values,
                scope,
            });
        }
        let format = self.lower_string(path, format_node)?;
        let values = args
            .iter()
            .map(|value| self.lower_dynamic_format_arg(path, *value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(IrStringExpr::Format {
            format: Box::new(format),
            args: values,
            scope,
        })
    }

    /// Parse one literal format segment, appending its normalized text to
    /// `format` and the values its conversions consume to `values`. Returns
    /// the number of following arguments consumed.
    #[allow(clippy::too_many_arguments)]
    fn parse_format_text(
        &mut self,
        path: &str,
        library: Option<&str>,
        name: &str,
        text: &[u8],
        following: &[NodeId],
        format: &mut Vec<u8>,
        values: &mut Vec<IrDisplayArg>,
    ) -> Result<usize, String> {
        let mut consumed = 0usize;
        let mut index = 0usize;
        while index < text.len() {
            let byte = text[index];
            index += 1;
            if byte != b'%' {
                format.push(byte);
                continue;
            }
            let start = index - 1;
            // Slang's format grammar admits the left-justify and zero flags,
            // a width and a precision; other C flags are frontend errors.
            while index < text.len() && matches!(text[index], b'-' | b'0'..=b'9' | b'.') {
                index += 1;
            }
            let Some(&conversion) = text.get(index) else {
                return Err(format!("incomplete {name} format at end of `{path}`"));
            };
            index += 1;
            let spec = &text[start..index];
            let lower = conversion.to_ascii_lowercase();
            match lower {
                b'%' | b'm' => {
                    format.extend_from_slice(spec);
                    continue;
                }
                b'l' => {
                    // The library binding is static per scope; without a
                    // known scope the runtime prints the calling scope.
                    match library {
                        Some(library) => {
                            format.extend_from_slice(library.replace('%', "%%").as_bytes())
                        }
                        None => format.extend_from_slice(spec),
                    }
                    continue;
                }
                b'd' | b'h' | b'x' | b'b' | b'o' | b'c' | b'u' | b'z' | b'v' | b't' | b's'
                | b'e' | b'f' | b'g' | b'p' => {}
                other => {
                    return Err(format!(
                        "unsupported {name} format specifier `%{}` in `{path}`",
                        other as char
                    ));
                }
            }
            let Some(&node) = following.get(consumed) else {
                return Err(format!(
                    "{name} format `%{}` in `{path}` has no argument",
                    conversion as char
                ));
            };
            if self.is_empty_format_argument(node) {
                return Err(format!(
                    "{name} format `%{}` in `{path}` has an empty argument",
                    conversion as char
                ));
            }
            consumed += 1;
            if lower != b'p' {
                self.reject_unformattable(path, name, conversion, node)?;
            }
            let value = if lower == b'p' {
                self.lower_pattern_arg(path, node, spec_is_abbreviated(spec))?
            } else {
                self.lower_format_arg(path, node)?
            };
            let value = self.check_format_conversion(path, name, conversion, value, node)?;
            values.push(value);
            format.extend_from_slice(spec);
        }
        Ok(consumed)
    }

    /// Only `%p` formats unpacked aggregates and handles (SV 21.2.1.2):
    /// other conversions need an integral, real or string value.
    fn reject_unformattable(
        &self,
        path: &str,
        name: &str,
        conversion: u8,
        node: NodeId,
    ) -> Result<(), String> {
        let Some(descriptor) = self.query_descriptor(node) else {
            return Ok(());
        };
        let kind = match &descriptor.shape {
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion
                ) =>
            {
                "an unpacked structure or union"
            }
            TypeShape::FixedArray { .. } => "an unpacked array",
            TypeShape::Container { .. } => "a queue, dynamic or associative array",
            TypeShape::Opaque { kind } if kind != "Void" => "a handle",
            _ => return Ok(()),
        };
        Err(format!(
            "{name} format `%{}` cannot format {kind} of type `{}` in `{path}`; only `%p` formats it (SV 21.2.1.2)",
            conversion as char, descriptor.name
        ))
    }

    /// Lower an argument whose conversion is known only at run time: an
    /// aggregate or handle becomes its full `%p` text.
    fn lower_dynamic_format_arg(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrDisplayArg, String> {
        let pattern = self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(
                &descriptor.shape,
                TypeShape::Aggregate(layout) if !matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                )
            ) || matches!(
                &descriptor.shape,
                TypeShape::FixedArray { .. } | TypeShape::Container { .. }
            ) || matches!(&descriptor.shape, TypeShape::Opaque { kind } if kind != "Void")
        });
        if pattern {
            return self.lower_pattern_arg(path, node, false);
        }
        self.lower_format_arg(path, node)
    }

    /// Check that `value` is legal for `conversion` and select a strength
    /// view for `%v` of a resolved net.
    fn check_format_conversion(
        &mut self,
        path: &str,
        name: &str,
        conversion: u8,
        value: IrDisplayArg,
        node: NodeId,
    ) -> Result<IrDisplayArg, String> {
        let lower = conversion.to_ascii_lowercase();
        let legal = match (&value, lower) {
            // Integral conversions accept packed and string values; a real
            // value converts to a 64-bit integer as in Slang's evaluator.
            (
                IrDisplayArg::Packed(_) | IrDisplayArg::String(_) | IrDisplayArg::Real(_),
                b'd' | b'h' | b'x' | b'b' | b'o' | b'c',
            ) => true,
            (IrDisplayArg::Packed(_), b'u' | b'z' | b'v') => true,
            (IrDisplayArg::Packed(_) | IrDisplayArg::Real(_), b't' | b'e' | b'f' | b'g') => true,
            (IrDisplayArg::Packed(_) | IrDisplayArg::String(_), b's') => true,
            (_, b'p') => true,
            _ => false,
        };
        if !legal {
            return Err(format!(
                "{name} format `%{}` has an incompatible argument in `{path}`",
                conversion as char
            ));
        }
        if lower == b'v' {
            if let Some(view) = self.strength_display_arg(node)? {
                return Ok(view);
            }
        }
        Ok(value)
    }

    /// A source string literal used as display format text, recovered
    /// through the implicit string cast Slang may insert at a system-task
    /// argument boundary.
    pub(in super::super) fn format_literal_bytes(
        &self,
        node: NodeId,
        context: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let _ = context;
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                value,
                ..
            }) => decoded_string_bytes(value).map(Some),
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => {
                self.format_literal_bytes(*operand, context)
            }
            _ => Ok(None),
        }
    }

    fn is_empty_format_argument(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::Expr(ExprKind::Other))
            && self.db.semantic_detail(node) == Some("EmptyArgument")
    }
}

/// `%0p` (a zero flag without width digits) selects the abbreviated form.
fn spec_is_abbreviated(spec: &[u8]) -> bool {
    let flags = &spec[1..spec.len() - 1];
    flags.first() == Some(&b'0') && !flags[1..].iter().any(u8::is_ascii_digit)
}

/// Append the default conversion of an argument without a format: `%s` for
/// strings, `%f` for reals and the task radix for packed values.
fn push_default_conversion(format: &mut Vec<u8>, value: &IrDisplayArg, radix: IrDisplayRadix) {
    format.push(b'%');
    format.push(match value {
        IrDisplayArg::Real(_) => b'f',
        IrDisplayArg::String(_) | IrDisplayArg::Text(_) => b's',
        IrDisplayArg::Packed(_) | IrDisplayArg::Strength(_) => radix.specifier() as u8,
    });
}

/// C string literal text for normalized format bytes. Bytes outside
/// printable ASCII use octal escapes, so arbitrary format bytes survive.
pub(in crate::sim::codegen) fn c_format_literal(format: &[u8]) -> String {
    let mut out = String::with_capacity(format.len() + 2);
    out.push('"');
    for &byte in format {
        match byte {
            b'"' => out.push_str("\\\""),
            // Never form a C trigraph.
            b'?' => out.push_str("\\?"),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            b' '..=b'~' => out.push(byte as char),
            _ => out.push_str(&format!("\\{byte:03o}")),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_format_literal_escapes_every_non_printing_byte() {
        assert_eq!(
            c_format_literal(b"a\"?\\\n\t\r\x01\xff7"),
            "\"a\\\"\\?\\\\\\n\\t\\r\\001\\3777\""
        );
        assert_eq!(c_format_literal(b""), "\"\"");
    }

    #[test]
    fn only_a_bare_zero_flag_abbreviates_patterns() {
        assert!(spec_is_abbreviated(b"%0p"));
        assert!(!spec_is_abbreviated(b"%p"));
        assert!(!spec_is_abbreviated(b"%05p"));
        assert!(!spec_is_abbreviated(b"%-0p"));
    }
}
