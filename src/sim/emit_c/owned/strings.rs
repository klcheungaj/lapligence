//! Owned byte strings, evaluated before their consuming C boundary.
use super::native::{NativeKind, NativeValue};
use super::*;

impl Frame<'_, '_> {
    pub(super) fn string(&mut self, expression: &IrStringExpr) -> Result<NativeValue, String> {
        use IrStringExpr::*;
        Ok(match expression {
            Conditional {
                predicate,
                then,
                otherwise,
            } => {
                let selector = self.expression(predicate)?;
                let result = self.native_value(NativeKind::String, "(llg_string_t){0}".to_owned());
                self.line(format!("if ({}) {{", selector.truth()));
                self.string_arm(&result, then)?;
                if selector.width == 0 {
                    self.line("} else {");
                    self.string_arm(&result, otherwise)?;
                } else {
                    self.line(format!("}} else if (!{}) {{", selector.unknown_truth()));
                    self.string_arm(&result, otherwise)?;
                    self.line("} else {");
                    // SV 11.4.11: an ambiguous predicate evaluates both arms;
                    // unequal strings yield the empty default.
                    let left = self.string(then)?;
                    let right = self.string(otherwise)?;
                    self.line(format!(
                        "*({}) = llg_string_conditional_merge({}, {});",
                        result.address,
                        left.take_string(),
                        right.take_string()
                    ));
                    self.native_discard(right);
                    self.native_discard(left);
                }
                self.line("}");
                self.discard(selector);
                result
            }
            Literal(bytes) => {
                let literal = bytes
                    .iter()
                    .map(|byte| format!("\\{byte:03o}"))
                    .collect::<String>();
                self.native_value(
                    NativeKind::String,
                    format!("llg_string_bytes(\"{literal}\", {})", bytes.len()),
                )
            }
            RandomState => {
                self.native_value(NativeKind::String, "llg_process_get_randstate()".to_owned())
            }
            ProcessRandState(target) => {
                let target = self.process_value(target)?;
                let value = self.native_value(
                    NativeKind::String,
                    format!("llg_process_handle_get_randstate({})", target.code()),
                );
                self.native_discard(target);
                value
            }
            Read(index) => self.native_value(
                NativeKind::String,
                format!(
                    "llg_string_clone(&{})",
                    self.ctx.model.objects[*index].c_name
                ),
            ),
            LocalRead(name) => {
                let binding = self.native_lookup(name, NativeKind::String)?;
                self.native_value(
                    NativeKind::String,
                    format!("llg_string_clone({})", binding.address),
                )
            }
            FormalRead(index) => {
                let function = self
                    .ctx
                    .func
                    .ok_or_else(|| "string formal outside a function".to_owned())?;
                let formal = function
                    .formals
                    .get(*index)
                    .ok_or_else(|| "invalid string formal".to_owned())?;
                let address = if formal.is_ref() {
                    format!("r{index}")
                } else if formal.is_out {
                    format!("o{index}")
                } else {
                    self.native_lookup(&format!("a{index}"), NativeKind::String)?
                        .address
                };
                self.native_value(NativeKind::String, format!("llg_string_clone({address})"))
            }
            Concat(parts) => {
                let mut result =
                    self.native_value(NativeKind::String, "(llg_string_t){0}".to_owned());
                for part in parts {
                    let part = self.string(part)?;
                    let next = self.native_value(
                        NativeKind::String,
                        format!(
                            "llg_string_concat({}, {})",
                            result.take_string(),
                            part.take_string()
                        ),
                    );
                    self.native_discard(result);
                    self.native_discard(part);
                    result = next;
                }
                result
            }
            Repeat(text, count) => {
                let text = self.string(text)?;
                let count = self.expression(count)?;
                let result = self.native_value(
                    NativeKind::String,
                    format!("llg_string_repeat({}, {})", text.take_string(), count.code),
                );
                self.native_discard(text);
                self.discard(count);
                result
            }
            FromPacked(value) => {
                let value = self.expression(value)?;
                let result = self.native_value(
                    NativeKind::String,
                    format!("llg_string_from_packed({})", value.code),
                );
                self.discard(value);
                result
            }
            Case(text, upper) => {
                let text = self.string(text)?;
                let result = self.native_value(
                    NativeKind::String,
                    format!(
                        "llg_string_case({}, {})",
                        text.take_string(),
                        u8::from(*upper)
                    ),
                );
                self.native_discard(text);
                result
            }
            Substr(text, first, last) => {
                let text = self.string(text)?;
                let first = self.expression(first)?;
                let last = self.expression(last)?;
                let result = self.native_value(
                    NativeKind::String,
                    format!(
                        "llg_string_substr({}, {}, {})",
                        text.take_string(),
                        first.code,
                        last.code
                    ),
                );
                self.native_discard(text);
                self.discard(first);
                self.discard(last);
                result
            }
            Format {
                format,
                args,
                scope,
            } => {
                let format = self.string(format)?;
                // Reserve before publishing formatter arguments: this is the
                // last allocation before their consuming, non-yielding call.
                let result = self.native_reserve(NativeKind::String);
                let arguments = self.formatted_arguments(args, self.ctx.model.precision_fs)?;
                self.line(super::super::destinations::assign_string(
                    &result.address,
                    &format!(
                        "llg_string_format_typed({}, {arguments}, {}, {})",
                        format.take_string(),
                        args.len(),
                        c_string_literal(scope)
                    ),
                ));
                self.native_discard(format);
                result
            }
            EnumName { receiver, members } => {
                let receiver = self.expression(receiver)?;
                let result = self.native_value(NativeKind::String, "(llg_string_t){0}".to_owned());
                for member in members {
                    let value = self.expression(&member.value)?;
                    let cmp = self.value(
                        format!("sv4_case_eq({}, {})", receiver.code, value.code),
                        1,
                        false,
                    );
                    self.line(format!("if ({}) {{", cmp.truth()));
                    let literal = member
                        .name
                        .iter()
                        .map(|byte| format!("\\{byte:03o}"))
                        .collect::<String>();
                    // `result` is an expression owner, not change-tracked storage.
                    self.line(super::super::destinations::assign_string(
                        &result.address,
                        &format!("llg_string_bytes(\"{literal}\", {})", member.name.len()),
                    ));
                    self.line("}");
                    self.discard(cmp);
                    self.discard(value);
                }
                self.discard(receiver);
                result
            }
            ContainerGet { container, index } => {
                let container_name = self.container_name(*container)?;
                let mut container = self.ctx.model.containers[*container].clone();
                container.c_name = container_name;
                let index = self.expression(index)?;
                let function = match container.kind {
                    IrContainerKind::Dynamic => "llg_dyn_value_get_string",
                    IrContainerKind::Queue { .. } => "llg_queue_value_get_string",
                    IrContainerKind::Associative { .. } => "llg_assoc_value_get_integral_string",
                };
                let result = self.native_value(
                    NativeKind::String,
                    format!("{function}(&{}, {})", container.c_name, index.code),
                );
                self.discard(index);
                result
            }
            QueuePop { container, back } => {
                if self.read_only_callback {
                    return Err(pending("mutating container query in a read-only callback"));
                }
                // The removed string moves into a registered result owner
                // before the queue notifies its readers.
                let name = self.container_name(*container)?;
                let result = self.native_reserve(NativeKind::String);
                self.line(format!(
                    "llg_queue_value_pop_string_to({}, &{name}, {});",
                    result.address,
                    i32::from(*back)
                ));
                result
            }
            ContainerGetNested { container, indices } => {
                let container_name = self.container_name(*container)?;
                let mut container = self.ctx.model.containers[*container].clone();
                container.c_name = container_name;
                let (list, values) = self.container_indices(indices)?;
                let function = match container.kind {
                    IrContainerKind::Dynamic => "llg_dyn_value_get_nested_string",
                    IrContainerKind::Queue { .. } => "llg_queue_value_get_nested_string",
                    IrContainerKind::Associative { .. } => {
                        "llg_assoc_value_get_nested_integral_string"
                    }
                };
                let result = self.native_value(
                    NativeKind::String,
                    format!(
                        "{function}(&{}, {list}, {})",
                        container.c_name,
                        indices.len()
                    ),
                );
                for value in values {
                    self.discard(value);
                }
                result
            }
            AssociativeGet { container, key } => {
                let name = self.container_name(*container)?;
                let key = self.string(key)?;
                let result = self.native_value(
                    NativeKind::String,
                    format!(
                        "llg_assoc_value_get_string_string(&{name}, ({})->data, ({})->len)",
                        key.address, key.address
                    ),
                );
                self.native_discard(key);
                result
            }
            Call {
                function,
                args,
                depth,
                receiver,
                virtual_dispatch,
            } => {
                let args = args.iter().cloned().map(IrCallArg::Val).collect::<Vec<_>>();
                self.native_method_call(
                    *function,
                    &args,
                    *depth,
                    NativeKind::String,
                    receiver.as_deref(),
                    *virtual_dispatch,
                )?
            }
            TypedCall {
                function,
                args,
                depth,
                receiver,
                virtual_dispatch,
            } => self.native_method_call(
                *function,
                args,
                *depth,
                NativeKind::String,
                receiver.as_deref(),
                *virtual_dispatch,
            )?,
        })
    }

    pub(super) fn string_assign(
        &mut self,
        address: &str,
        expression: &IrStringExpr,
    ) -> Result<(), String> {
        let value = self.string(expression)?;
        self.line(super::super::destinations::move_string(
            address,
            &value.take_string(),
        ));
        self.native_discard(value);
        Ok(())
    }

    pub(super) fn string_inside(
        &mut self,
        source: &IrStringExpr,
        items: &[IrStringInsideItem],
    ) -> Result<Value, String> {
        let source = self.string(source)?;
        let result = self.value("sv4_from_u64(0, 1, 0)".to_owned(), 1, false);
        for item in items {
            self.line(format!("if (!{}) {{", result.truth()));
            let hit = match item {
                IrStringInsideItem::Value(item) => {
                    let item = self.string(item)?;
                    let cmp = self.value(
                        format!(
                            "llg_string_compare(llg_string_clone({}), {}, 0)",
                            source.address,
                            item.take_string()
                        ),
                        32,
                        true,
                    );
                    let hit = self.scalar("int", format!("sv4_to_i64({}) == 0", cmp.code));
                    self.discard(cmp);
                    self.native_discard(item);
                    hit
                }
                IrStringInsideItem::Range { low, high } => {
                    let low = self.string(low)?;
                    let high = self.string(high)?;
                    let lower = self.value(
                        format!(
                            "llg_string_compare(llg_string_clone({}), {}, 0)",
                            source.address,
                            low.take_string()
                        ),
                        32,
                        true,
                    );
                    let upper = self.value(
                        format!(
                            "llg_string_compare(llg_string_clone({}), {}, 0)",
                            source.address,
                            high.take_string()
                        ),
                        32,
                        true,
                    );
                    let hit = self.scalar(
                        "int",
                        format!(
                            "sv4_to_i64({}) >= 0 && sv4_to_i64({}) <= 0",
                            lower.code, upper.code
                        ),
                    );
                    self.discard(lower);
                    self.discard(upper);
                    self.native_discard(low);
                    self.native_discard(high);
                    hit
                }
            };
            self.assign(
                &format!("&{}", result.code),
                &format!("sv4_from_u64({hit}, 1, 0)"),
            );
            self.line("}");
        }
        self.native_discard(source);
        Ok(result)
    }

    /// Move one evaluated conditional arm into the reserved empty result.
    fn string_arm(&mut self, result: &NativeValue, arm: &IrStringExpr) -> Result<(), String> {
        let value = self.string(arm)?;
        self.line(format!("*({}) = {};", result.address, value.take_string()));
        self.native_discard(value);
        Ok(())
    }
}
