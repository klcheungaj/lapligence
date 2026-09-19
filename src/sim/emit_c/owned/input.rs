//! File and plusarg calls retain text and selector owners across publication.
use super::native::{NativeKind, NativeValue};
use super::stores::{Selection, Target};
use super::*;

impl Frame<'_, '_> {
    fn input_text(&mut self, text: &IrPlusArgText) -> Result<NativeValue, String> {
        match text {
            IrPlusArgText::Literal(text) => {
                self.string(&IrStringExpr::Literal(text.as_bytes().to_vec()))
            }
            IrPlusArgText::Dynamic(text) => self.string(text),
        }
    }

    fn text_pointer(value: &NativeValue) -> String {
        format!(
            "(({})->data ? ({})->data : \"\")",
            value.address, value.address
        )
    }

    fn file_reference(
        &mut self,
        lhs: &IrLhs,
        width: u32,
        signed: bool,
        two_state: bool,
    ) -> Result<(String, Target), String> {
        let target = self.target(lhs)?;
        if target.width == 0 || target.net.is_some() {
            return Err("file input requires packed variable storage".to_owned());
        }
        if target.width != width {
            return Err("file input selected width disagrees with its type".to_owned());
        }
        if let Some(reference) = &target.reference {
            if target.selection.is_some() {
                return Err(pending("selected reference file destinations"));
            }
            return Ok((reference.clone(), target));
        }
        let selection = match &target.selection {
            None => ".kind = LLG_REF_WHOLE".to_owned(),
            Some(Selection::PackedChain(plan, _)) => format!(".kind = LLG_REF_PACKED_PLAN, .retained = &{plan}"),
            Some(Selection::Bit(index)) => format!(".kind = LLG_REF_BIT, .index = {index}"),
            Some(Selection::Part(left, right)) => format!(".kind = LLG_REF_PART, .left = {left}LL, .right = {right}LL"),
            Some(Selection::Indexed(base, width, negative)) => format!(".kind = LLG_REF_INDEXED, .index = sv4_to_index({}), .indexed_width = {width}, .indexed_negative = {}", base.code, u8::from(*negative)),
        };
        let reference = self.name("input_reference");
        self.line(format!("llg_ref_t {reference} = {{ .base = ({} ? {} : NULL), .width = {width}, .is_signed = {}, .two_state = {}, {selection} }};",
            target.valid, target.binding.address, u8::from(signed), u8::from(two_state)));
        Ok((format!("&{reference}"), target))
    }

    fn input_targets(
        &mut self,
        targets: &[IrFileInputTarget],
    ) -> Result<(String, Vec<Target>), String> {
        let mut owners = Vec::new();
        let mut entries = Vec::new();
        for target in targets {
            entries.push(match target {
                IrFileInputTarget::Packed {
                    lhs,
                    width,
                    signed,
                    two_state,
                } => {
                    let (reference, owner) =
                        self.file_reference(lhs, *width, *signed, *two_state)?;
                    owners.push(owner);
                    format!("{{ .kind = LLG_FILE_INPUT_PACKED, .packed = {reference} }}")
                }
                IrFileInputTarget::Real { lhs, shortreal } => {
                    let owner = self.target(lhs)?;
                    if owner.width != 0 || owner.selection.is_some() {
                        return Err("file input real target must be whole real storage".to_owned());
                    }
                    let entry = format!(
                        "{{ .kind = LLG_FILE_INPUT_REAL, .real = {}, .shortreal = {} }}",
                        owner.binding.address,
                        u8::from(*shortreal)
                    );
                    owners.push(owner);
                    entry
                }
                IrFileInputTarget::String { address } => format!(
                    "{{ .kind = LLG_FILE_INPUT_STRING, .string = {} }}",
                    self.native_address(address, NativeKind::String)?.address
                ),
            });
        }
        let array = if entries.is_empty() {
            "NULL".to_owned()
        } else {
            let array = self.name("input_targets");
            self.line(format!(
                "const llg_file_input_target_t {array}[] = {{ {} }};",
                entries.join(", ")
            ));
            array
        };
        Ok((array, owners))
    }

    fn integer_result(&mut self, code: String) -> Value {
        self.value(format!("sv4_from_i64((int64_t)({code}), 32)"), 32, true)
    }

    pub(super) fn file_input(&mut self, input: &IrFileInput) -> Result<Value, String> {
        if self.read_only_callback {
            return Err(pending("file input in read-only callbacks"));
        }
        let mut targets_to_release = Vec::new();
        let mut numeric = Vec::new();
        let mut text = Vec::new();
        let call = match input {
            IrFileInput::Getc { descriptor } => {
                let descriptor = self.descriptor(descriptor)?;
                format!("llg_file_getc({descriptor})")
            }
            IrFileInput::Ungetc {
                character,
                descriptor,
            } => {
                let character = self.expression(character)?;
                let descriptor = self.descriptor(descriptor)?;
                let call = format!("llg_file_ungetc({descriptor}, {})", character.code);
                numeric.push(character);
                call
            }
            IrFileInput::Gets { descriptor, target } => {
                // $fgets(target, fd): capture target selectors before fd.
                let (address, packed) = match target {
                    IrFileInputTarget::String { address } => (
                        self.native_address(address, NativeKind::String)?.address,
                        false,
                    ),
                    IrFileInputTarget::Packed {
                        lhs,
                        width,
                        signed,
                        two_state,
                    } => {
                        let (address, target) =
                            self.file_reference(lhs, *width, *signed, *two_state)?;
                        targets_to_release.push(target);
                        (address, true)
                    }
                    IrFileInputTarget::Real { .. } => {
                        return Err("line input target cannot be real".to_owned())
                    }
                };
                let descriptor = self.descriptor(descriptor)?;
                format!(
                    "{}({descriptor}, {address})",
                    if packed {
                        "llg_file_gets_packed"
                    } else {
                        "llg_file_gets"
                    }
                )
            }
            IrFileInput::ScanFile {
                descriptor,
                format,
                targets,
            } => {
                let descriptor = self.descriptor(descriptor)?;
                let format = self.input_text(format)?;
                let (array, owners) = self.input_targets(targets)?;
                targets_to_release.extend(owners);
                let call = format!(
                    "llg_file_scanf({descriptor}, {}, {array}, {})",
                    Self::text_pointer(&format),
                    targets.len()
                );
                text.push(format);
                call
            }
            IrFileInput::ScanString {
                source,
                format,
                targets,
            } => {
                let source = self.string(source)?;
                let format = self.input_text(format)?;
                let (array, owners) = self.input_targets(targets)?;
                targets_to_release.extend(owners);
                let call = format!(
                    "llg_string_scanf({}, ({})->len, {}, {array}, {})",
                    Self::text_pointer(&source),
                    source.address,
                    Self::text_pointer(&format),
                    targets.len()
                );
                text.push(source);
                text.push(format);
                call
            }
            IrFileInput::Read {
                descriptor,
                target,
                start,
                count,
            } => {
                let descriptor = self.descriptor(descriptor)?;
                let reference = if let IrFileReadTarget::Packed {
                    lhs,
                    width,
                    signed,
                    two_state,
                } = target
                {
                    let (address, target) =
                        self.file_reference(lhs, *width, *signed, *two_state)?;
                    targets_to_release.push(target);
                    Some(address)
                } else {
                    None
                };
                let start_value = start
                    .as_ref()
                    .map(|value| self.expression(value))
                    .transpose()?;
                let count_value = count
                    .as_ref()
                    .map(|value| self.expression(value))
                    .transpose()?;
                let first = start_value
                    .as_ref()
                    .map(|v| v.code.as_str())
                    .unwrap_or("(sv4_t)SV4_EMPTY");
                let count_code = count_value
                    .as_ref()
                    .map(|v| v.code.as_str())
                    .unwrap_or("(sv4_t)SV4_EMPTY");
                let call = if let Some(reference) = reference {
                    format!("llg_file_read_packed({descriptor}, {reference})")
                } else if let IrFileReadTarget::Array { array } = target {
                    let array = self.ctx.model.array(*array);
                    let dimensions = array
                        .dims
                        .iter()
                        .map(|(l, r)| format!("{l}, {r}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("llg_file_read_array({descriptor}, {}, {}, {}, {}, {}ULL, (const int32_t[]){{ {dimensions} }}, {}, {}, {first}, {}, {count_code})",
                            array.c_name, array.elem_width, u8::from(array.signed), u8::from(array.two_state), array.total, array.dims.len(), u8::from(start.is_some()), u8::from(count.is_some()))
                } else {
                    unreachable!("packed target has a reference")
                };
                if let Some(value) = start_value {
                    numeric.push(value);
                }
                if let Some(value) = count_value {
                    numeric.push(value);
                }
                call
            }
        };
        let result = self.integer_result(call);
        for target in targets_to_release {
            self.release_target(target);
        }
        for value in numeric {
            self.discard(value);
        }
        for value in text {
            self.native_discard(value);
        }
        Ok(result)
    }

    pub(super) fn file_expression(
        &mut self,
        function: &IrSysFunc,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        if self.read_only_callback {
            return Err(pending("file operations in read-only callbacks"));
        }
        Ok(match function {
            IrSysFunc::FileOpen { path, mode } => {
                let path = self.string(path)?;
                let mode_value = if let Some(mode) = mode {
                    self.string(mode)?
                } else {
                    self.native_value(NativeKind::String, "(llg_string_t){0}".to_owned())
                };
                let result = self.value(
                    format!(
                        "sv4_from_u64(llg_file_open({}, {}, {}), {}, {})",
                        path.take_string(),
                        mode_value.take_string(),
                        u8::from(mode.is_some()),
                        expression.width,
                        u8::from(expression.signed)
                    ),
                    expression.width,
                    expression.signed,
                );
                self.native_discard(path);
                self.native_discard(mode_value);
                result
            }
            IrSysFunc::FileTell(descriptor) | IrSysFunc::FileEof(descriptor) => {
                let descriptor = self.descriptor(descriptor)?;
                let function = if matches!(function, IrSysFunc::FileTell(_)) {
                    "llg_file_tell"
                } else {
                    "llg_file_eof"
                };
                self.value(
                    format!(
                        "sv4_from_i64((int64_t){function}({descriptor}), {})",
                        expression.width
                    ),
                    expression.width,
                    expression.signed,
                )
            }
            IrSysFunc::FileSeek {
                descriptor,
                offset,
                operation,
            } => {
                let descriptor = self.descriptor(descriptor)?;
                let offset = self.expression(offset)?;
                let operation = self.expression(operation)?;
                let result = self.integer_result(format!(
                    "llg_file_seek({descriptor}, {}, {})",
                    offset.code, operation.code
                ));
                self.discard(offset);
                self.discard(operation);
                result
            }
            IrSysFunc::FileError {
                descriptor,
                message,
            } => {
                let descriptor = self.descriptor(descriptor)?;
                let address = if let Some(message) = message {
                    self.native_address(message, NativeKind::String)?.address
                } else {
                    "NULL".to_owned()
                };
                self.integer_result(format!("llg_file_error({descriptor}, {address})"))
            }
            _ => return Err("not a file expression".to_owned()),
        })
    }

    pub(super) fn test_plusargs(&mut self, pattern: &IrPlusArgText) -> Result<Value, String> {
        let pattern = self.input_text(pattern)?;
        let result = self.integer_result(format!(
            "llg_test_plusargs({})",
            Self::text_pointer(&pattern)
        ));
        self.native_discard(pattern);
        Ok(result)
    }

    pub(super) fn value_plusargs(
        &mut self,
        format: &IrPlusArgText,
        destination: &IrPlusArgTarget,
    ) -> Result<Value, String> {
        if self.read_only_callback {
            return Err(pending("plusarg writes in read-only callbacks"));
        }
        let format = self.input_text(format)?;
        let code = Self::text_pointer(&format);
        let status = match destination {
            IrPlusArgTarget::String { address } => {
                let address = self.native_address(address, NativeKind::String)?.address;
                // The runtime stages conversion and changes the target only
                // on a successful match; the format remains an owned snapshot.
                self.scalar(
                    "int",
                    format!("llg_value_plusargs_string({code}, {address})"),
                )
            }
            IrPlusArgTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            } => {
                let target = self.target(lhs)?;
                let value = self.value(
                    format!("sv4_x({width}, {})", u8::from(*signed)),
                    *width,
                    *signed,
                );
                let status = self.scalar(
                    "int",
                    format!(
                        "llg_value_plusargs_packed({code}, &{}, {width}, {}, {})",
                        value.code,
                        u8::from(*signed),
                        u8::from(*two_state)
                    ),
                );
                self.line(format!("if ({status}) {{"));
                let copy = self.value(
                    format!("sv4_clone(&{})", value.code),
                    value.width,
                    value.signed,
                );
                self.store(&target, copy, false, "0")?;
                self.line("}");
                self.discard(value);
                self.release_target(target);
                status
            }
            IrPlusArgTarget::Real { lhs, shortreal } => {
                let target = self.target(lhs)?;
                let value = self.value("0.0".to_owned(), 0, true);
                let status = self.scalar(
                    "int",
                    format!("llg_value_plusargs_real({code}, &{})", value.code),
                );
                self.line(format!("if ({status}) {{"));
                let copy = self.value(round_shortreal(value.code.clone(), *shortreal), 0, true);
                self.store(&target, copy, false, "0")?;
                self.line("}");
                self.discard(value);
                self.release_target(target);
                status
            }
        };
        self.native_discard(format);
        Ok(self.value(format!("sv4_from_u64({status}, 32, 1)"), 32, true))
    }
}
