//! File and plusarg calls retain text and selector owners across publication.
use super::native::{NativeKind, NativeValue};
use super::stores::{Selection, Target};
use super::*;

impl Frame<'_, '_> {
    fn input_text(&mut self, text: &IrPlusArgText) -> Result<NativeValue, String> {
        self.scan_text(text, &mut Vec::new())
    }

    /// Owned text of a plusarg/scan string. Packed text records an `int`
    /// local in `unknown` that is set when the value has X/Z bits; a scan
    /// with such a source or format returns EOF (21.3.4.3).
    fn scan_text(
        &mut self,
        text: &IrPlusArgText,
        unknown: &mut Vec<String>,
    ) -> Result<NativeValue, String> {
        match text {
            IrPlusArgText::Literal(text) => {
                self.string(&IrStringExpr::Literal(text.as_bytes().to_vec()))
            }
            IrPlusArgText::Dynamic(text) => self.string(text),
            IrPlusArgText::Packed(value) => {
                let value = self.expression(value)?;
                let flag = self.scalar("int", "0".to_owned());
                let text = self.native_value(
                    NativeKind::String,
                    format!("llg_scan_text_from_packed({}, &{flag})", value.code),
                );
                self.discard(value);
                unknown.push(flag);
                Ok(text)
            }
        }
    }

    /// `call`, or EOF when any packed scan text had unknown bits.
    fn unless_unknown(call: String, unknown: &[String]) -> String {
        if unknown.is_empty() {
            call
        } else {
            format!("(({}) ? -1 : {call})", unknown.join(" || "))
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
            let Some(selection) = &target.selection else {
                return Ok((reference.clone(), target));
            };
            let root_width = target.binding.width;
            let plan = match selection {
                Selection::PackedChain(plan, _) => plan.clone(),
                Selection::Bit(index) => self.scalar(
                    "sv4_select_plan_t",
                    format!("sv4_select_plan_bit({root_width}, {index})"),
                ),
                Selection::Part(left, right) => self.scalar(
                    "sv4_select_plan_t",
                    format!("sv4_select_plan_part({root_width}, {left}LL, {right}LL)"),
                ),
                Selection::Indexed(base, width, negative) => self.scalar(
                    "sv4_select_plan_t",
                    format!(
                        "sv4_select_plan_indexed({root_width}, {}, {width}, {})",
                        base.code,
                        u8::from(*negative)
                    ),
                ),
            };
            // Scanners borrow the view synchronously; its copied numeric plan
            // and canonical parent stay in typed frame storage across setup.
            let view = self.declare(
                "llg_ref_view_t",
                "input_view",
                format!("{{ .parent = (llg_ref_t*){reference}, .plan = {plan}, .tag_check_count = 0, .tag_checks = NULL, .location = NULL }}"),
            );
            let reference = self.declare(
                "llg_ref_t",
                "input_reference",
                format!("{{ .kind = LLG_REF_VIEW, .width = {width}, .is_signed = {}, .two_state = {}, .retained = &{view} }}",
                    u8::from(signed), u8::from(two_state)),
            );
            return Ok((format!("&{reference}"), target));
        }
        let selection = match &target.selection {
            None => ".kind = LLG_REF_WHOLE".to_owned(),
            Some(Selection::PackedChain(plan, _)) => format!(".kind = LLG_REF_PACKED_PLAN, .retained = &{plan}"),
            Some(Selection::Bit(index)) => format!(".kind = LLG_REF_BIT, .index = {index}"),
            Some(Selection::Part(left, right)) => format!(".kind = LLG_REF_PART, .left = {left}LL, .right = {right}LL"),
            Some(Selection::Indexed(base, width, negative)) => format!(".kind = LLG_REF_INDEXED, .index = sv4_to_index({}), .indexed_width = {width}, .indexed_negative = {}", base.code, u8::from(*negative)),
        };
        let reference = self.declare(
            "llg_ref_t",
            "input_reference",
            format!("{{ .base = ({} ? {} : NULL), .width = {width}, .is_signed = {}, .two_state = {}, {selection} }}",
                target.valid, target.binding.address, u8::from(signed), u8::from(two_state)),
        );
        Ok((format!("&{reference}"), target))
    }

    /// Bind a retained element cell for a container element destination and
    /// return its reference address and the statement that releases the cell
    /// after the synchronous input call (SIM-008).
    fn element_reference(
        &mut self,
        read: &IrExpr,
        width: u32,
        signed: bool,
        two_state: bool,
    ) -> Result<(String, String), String> {
        let IrExprKind::Container(operation) = read.kind() else {
            return Err("file input element target is not a container read".to_owned());
        };
        let (acquire, read_fn, write_fn, release) = match operation.as_ref() {
            IrContainerExpr::Get { container, index } => {
                let name = self.container_name(*container)?;
                let index = self.expression(index)?;
                let acquire = match self.ctx.model.containers[*container].kind {
                    IrContainerKind::Queue { .. } => format!(
                        "llg_queue_ref_acquire(&{name}, sv4_to_index({}))",
                        index.code
                    ),
                    IrContainerKind::Dynamic => {
                        format!("llg_dyn_ref_acquire(&{name}, {})", index.code)
                    }
                    IrContainerKind::Associative { .. } => {
                        format!("llg_assoc_ref_acquire_integral(&{name}, {})", index.code)
                    }
                };
                let queue = matches!(
                    self.ctx.model.containers[*container].kind,
                    IrContainerKind::Queue { .. }
                );
                let cell = self.declare("void*", "input_cell", acquire);
                self.discard(index);
                if queue {
                    (
                        cell,
                        "llg_queue_cell_read",
                        "llg_queue_cell_write",
                        "llg_queue_ref_release",
                    )
                } else {
                    (
                        cell,
                        "llg_element_cell_read",
                        "llg_element_cell_write",
                        "llg_element_ref_release",
                    )
                }
            }
            IrContainerExpr::GetString { container, key } => {
                let name = self.container_name(*container)?;
                let key = self.string(key)?;
                let code = key.code();
                let cell = self.declare(
                    "void*",
                    "input_cell",
                    format!("llg_assoc_ref_acquire_string(&{name}, ({code}).data, ({code}).len)"),
                );
                self.native_discard(key);
                (
                    cell,
                    "llg_element_cell_read",
                    "llg_element_cell_write",
                    "llg_element_ref_release",
                )
            }
            _ => return Err("file input element target is not an element read".to_owned()),
        };
        let reference = self.declare(
            "llg_ref_t",
            "input_reference",
            format!(
                "{{ .kind = LLG_REF_QUEUE, .width = {width}, .is_signed = {}, .two_state = {}, .retained = {acquire}, .retained_read = {read_fn}, .retained_write = {write_fn} }}",
                u8::from(signed),
                u8::from(two_state)
            ),
        );
        Ok((format!("&{reference}"), format!("{release}({acquire});")))
    }

    fn input_targets(
        &mut self,
        targets: &[IrFileInputTarget],
        releases: &mut Vec<String>,
    ) -> Result<(String, Vec<Target>), String> {
        let mut owners = Vec::new();
        let mut entries = Vec::new();
        for target in targets {
            entries.push(match target {
                IrFileInputTarget::Element {
                    read,
                    width,
                    signed,
                    two_state,
                } => {
                    let (reference, release) =
                        self.element_reference(read, *width, *signed, *two_state)?;
                    releases.push(release);
                    format!("{{ .kind = LLG_FILE_INPUT_PACKED, .packed = {reference} }}")
                }
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
            self.declare_array_init(
                "llg_file_input_target_t",
                "input_targets",
                entries.len(),
                &entries.join(", "),
            )
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
        let mut cells_to_release = Vec::new();
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
                    IrFileInputTarget::Element {
                        read,
                        width,
                        signed,
                        two_state,
                    } => {
                        let (address, release) =
                            self.element_reference(read, *width, *signed, *two_state)?;
                        cells_to_release.push(release);
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
                scope,
            } => {
                let descriptor = self.descriptor(descriptor)?;
                let mut unknown = Vec::new();
                let format = self.scan_text(format, &mut unknown)?;
                let (array, owners) = self.input_targets(targets, &mut cells_to_release)?;
                targets_to_release.extend(owners);
                let call = format!(
                    "llg_file_scanf_scoped({descriptor}, {}, {array}, {}, {}, {}ULL)",
                    Self::text_pointer(&format),
                    targets.len(),
                    c_string_literal(&scope.name),
                    scope.time_unit_fs
                );
                text.push(format);
                Self::unless_unknown(call, &unknown)
            }
            IrFileInput::ScanString {
                source,
                format,
                targets,
                scope,
            } => {
                let mut unknown = Vec::new();
                let source = self.scan_text(source, &mut unknown)?;
                let format = self.scan_text(format, &mut unknown)?;
                let (array, owners) = self.input_targets(targets, &mut cells_to_release)?;
                targets_to_release.extend(owners);
                let call = format!(
                    "llg_string_scanf_scoped({}, ({})->len, {}, {array}, {}, {}, {}ULL)",
                    Self::text_pointer(&source),
                    source.address,
                    Self::text_pointer(&format),
                    targets.len(),
                    c_string_literal(&scope.name),
                    scope.time_unit_fs
                );
                text.push(source);
                text.push(format);
                Self::unless_unknown(call, &unknown)
            }
            IrFileInput::Read {
                descriptor,
                target,
                start,
                count,
            } => {
                let descriptor = self.descriptor(descriptor)?;
                let reference = match target {
                    IrFileReadTarget::Packed {
                        lhs,
                        width,
                        signed,
                        two_state,
                    } => {
                        let (address, target) =
                            self.file_reference(lhs, *width, *signed, *two_state)?;
                        targets_to_release.push(target);
                        Some(address)
                    }
                    IrFileReadTarget::Element {
                        read,
                        width,
                        signed,
                        two_state,
                    } => {
                        let (address, release) =
                            self.element_reference(read, *width, *signed, *two_state)?;
                        cells_to_release.push(release);
                        Some(address)
                    }
                    IrFileReadTarget::Array { .. } | IrFileReadTarget::Container { .. } => None,
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
                } else if let IrFileReadTarget::Container { container } = target {
                    let runtime = match self.ctx.model.containers[*container].kind {
                        IrContainerKind::Queue { .. } => "llg_queue_file_read",
                        _ => "llg_dyn_file_read",
                    };
                    format!(
                        "{runtime}({descriptor}, &{}, {}, {first}, {}, {count_code})",
                        self.container_name(*container)?,
                        u8::from(start.is_some()),
                        u8::from(count.is_some())
                    )
                } else if let IrFileReadTarget::Array { array } = target {
                    let array = self.ctx.model.array(*array);
                    let dimensions = array
                        .dims
                        .iter()
                        .map(|(l, r)| format!("{l}, {r}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let runtime = if array.sparse() {
                        "llg_fixed_file_read_array"
                    } else {
                        "llg_file_read_array"
                    };
                    let memory = if array.sparse() {
                        self.fixed_array_address(match target {
                            IrFileReadTarget::Array { array } => *array,
                            _ => unreachable!(),
                        })?
                    } else {
                        array.c_name.clone()
                    };
                    format!("{runtime}({descriptor}, {}, {}, {}, {}, {}ULL, (const int32_t[]){{ {dimensions} }}, {}, {}, {first}, {}, {count_code})",
                            memory, array.elem_width, u8::from(array.signed), u8::from(array.two_state), array.total, array.dims.len(), u8::from(start.is_some()), u8::from(count.is_some()))
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
        for release in cells_to_release {
            self.line(release);
        }
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
            IrPlusArgTarget::Element {
                read,
                width,
                signed,
                two_state,
            } => {
                let (reference, release) =
                    self.element_reference(read, *width, *signed, *two_state)?;
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
                self.line(format!(
                    "if ({status}) llg_ref_write({reference}, {});",
                    value.code
                ));
                self.line(release);
                self.discard(value);
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
