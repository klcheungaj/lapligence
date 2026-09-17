//! Container calls consume only explicitly prepared operand snapshots.
use super::*;
pub(super) fn render(frame: &mut Frame<'_, '_>, operation: &IrContainerStmt, owners: &mut Vec<Value>, strings: &mut Vec<NativeValue>) -> Result<String, String> {
    let ctx = frame.ctx;
    Ok(match operation {
        IrContainerStmt::StreamAssign {
            container,
            source,
            slice,
            direction,
            selector,
        } => {
            let source = operand(frame, owners, source)?;
            let (selector_kind, first, second) = stream_selector(frame, owners, selector.as_ref())?;
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_unstream_assign",
                IrContainerKind::Queue { .. } => "llg_queue_unstream_assign",
                IrContainerKind::Associative { .. } => {
                    return Err("associative arrays are not legal streaming targets".into())
                }
            };
            format!(
                "    {function}(&{}, {}, {slice}, {}, {selector_kind}, {first}, {second});\n",
                name(ctx, *container),
                source.code,
                matches!(direction, IrStreamDirection::RightToLeft) as u8
            )
        }
        IrContainerStmt::DynamicNew {
            container,
            size,
            initializer,
        } => {
            let generic = !ctx.model.containers[*container].element.is_packed();
            let function = if generic {
                "llg_dyn_value_new"
            } else {
                "llg_dyn_new"
            };
            format!(
                "    {function}(&{}, {}, {});\n",
                name(ctx, *container),
                operand(frame, owners, size)?.code,
                initializer
                    .map(|index| format!("&{}", name(ctx, index)))
                    .unwrap_or_else(|| "NULL".to_owned())
            )
        }
        IrContainerStmt::Copy { dst, src } => {
            let generic = !ctx.model.containers[*dst].element.is_packed();
            let function = if generic {
                match ctx.model.containers[*dst].kind {
                    IrContainerKind::Dynamic => "llg_dyn_value_copy",
                    IrContainerKind::Queue { .. } => "llg_queue_value_copy",
                    IrContainerKind::Associative { .. } => "llg_assoc_value_copy",
                }
            } else {
                match ctx.model.containers[*dst].kind {
                    IrContainerKind::Dynamic => "llg_dyn_copy",
                    IrContainerKind::Queue { .. } => "llg_queue_copy",
                    IrContainerKind::Associative { .. } => "llg_assoc_copy",
                }
            };
            format!(
                "    {function}(&{}, &{});\n",
                name(ctx, *dst),
                name(ctx, *src)
            )
        }
        IrContainerStmt::MethodAssign {
            dst,
            src,
            method,
            callback,
        } => {
            let function = match ctx.model.containers[*src].kind {
                IrContainerKind::Dynamic => "llg_dyn_method_assign",
                IrContainerKind::Queue { .. } => "llg_queue_method_assign",
                IrContainerKind::Associative { .. } => "llg_assoc_method_assign",
            };
            format!(
                "    {function}(&{}, &{}, {}, {}, NULL);\n",
                name(ctx, *dst),
                name(ctx, *src),
                method_code(*method),
                callback.as_deref().unwrap_or("NULL")
            )
        }
        IrContainerStmt::Method {
            container,
            method,
            callback,
        } => {
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_method",
                IrContainerKind::Queue { .. } => "llg_queue_method",
                IrContainerKind::Associative { .. } => {
                    return Err("in-place array method cannot target an associative array".into())
                }
            };
            format!(
                "    {function}(&{}, {}, {}, NULL);\n",
                name(ctx, *container),
                method_code(*method),
                callback.as_deref().unwrap_or("NULL")
            )
        }
        IrContainerStmt::AssignValues { container, values } => {
            let data = if values.is_empty() {
                "NULL".to_owned()
            } else {
                let values = values
                    .iter()
                    .map(|value| Ok(operand(frame, owners, value)?.code))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ");
                format!("(const sv4_t[]){{ {values} }}")
            };
            format!(
                "    {}_assign_values(&{}, {}, {});\n",
                prefix(&ctx.model.containers[*container].kind),
                name(ctx, *container),
                data,
                values.len()
            )
        }
        IrContainerStmt::QueueAssign { container, sources } => {
            let generic = !ctx.model.containers[*container].element.is_packed();
            let source_count = sources.len();
            let sources = sources
                .iter()
                .map(|source| {
                    let (queue, left, right, left_unbounded, right_unbounded) = match source {
                        IrQueueSource::Whole(source) => (
                            name(ctx, *source),
                            zero_operand(frame, owners),
                            zero_operand(frame, owners),
                            0,
                            1,
                        ),
                        IrQueueSource::Slice {
                            container: source,
                            left,
                            right,
                        } => {
                            let mut render_bound = |bound: &IrQueueBound| match bound {
                                IrQueueBound::Value(value) => {
                                    operand(frame, owners, value).map(|value| value.code)
                                }
                                IrQueueBound::Unbounded => Ok(
                                    zero_operand(frame, owners),
                                ),
                            };
                            (
                                name(ctx, *source),
                                render_bound(left)?,
                                render_bound(right)?,
                                matches!(left, IrQueueBound::Unbounded) as u8,
                                matches!(right, IrQueueBound::Unbounded) as u8,
                            )
                        }
                    };
                    if generic {
                        Ok(format!(
                            "{{ .queue = NULL, .left = {left}, .right = {right}, .left_unbounded = {left_unbounded}, .right_unbounded = {right_unbounded}, .value_queue = &{queue}, .value_kind = 1 }}"
                        ))
                    } else {
                        Ok(format!(
                            "{{ &{queue}, {left}, {right}, {left_unbounded}, {right_unbounded} }}"
                        ))
                    }
                })
                .collect::<Result<Vec<_>, String>>()?
                .join(", ");
            let data = if sources.is_empty() {
                "NULL".to_owned()
            } else {
                format!("(const llg_queue_source_t[]){{ {sources} }}")
            };
            let function = if generic {
                "llg_queue_value_assign_sources"
            } else {
                "llg_queue_assign_sources"
            };
            format!(
                "    {function}(&{}, {}, {});\n",
                name(ctx, *container),
                data,
                source_count
            )
        }
        IrContainerStmt::AssignRealValues { container, values } => {
            let data = if values.is_empty() {
                "NULL".to_owned()
            } else {
                let values = values
                    .iter()
                    .map(|value| {
                        let rendered = operand(frame, owners, value)?;
                        Ok(if rendered.width == 0 {
                            rendered.code
                        } else {
                            format!("sv4_to_real({})", rendered.code)
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ");
                format!("(const double[]){{ {values} }}")
            };
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_assign_reals",
                IrContainerKind::Queue { .. } => "llg_queue_value_assign_reals",
                IrContainerKind::Associative { .. } => {
                    return Err("real associative positional assignment is unsupported".into())
                }
            };
            format!(
                "    {function}(&{}, {}, {});\n",
                name(ctx, *container),
                data,
                values.len()
            )
        }
        IrContainerStmt::AssignStringValues { container, values } => {
            let data = if values.is_empty() {
                "NULL".to_owned()
            } else {
                let values = values
                    .iter()
                    .map(|value| text_operand(frame, strings, value))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ");
                format!("(llg_string_t[]){{ {values} }}")
            };
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_assign_strings",
                IrContainerKind::Queue { .. } => "llg_queue_value_assign_strings",
                IrContainerKind::Associative { .. } => {
                    return Err("string associative positional assignment is unsupported".into())
                }
            };
            format!(
                "    {function}(&{}, {}, {});\n",
                name(ctx, *container),
                data,
                values.len()
            )
        }
        IrContainerStmt::AssignChandleValues { container, values } => {
            let data = if values.is_empty() {
                "NULL".to_owned()
            } else {
                let values = values
                    .iter()
                    .map(|value| frame.chandle(value))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ");
                format!("(void *[]){{ {values} }}")
            };
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_assign_chandles",
                IrContainerKind::Queue { .. } => "llg_queue_value_assign_chandles",
                IrContainerKind::Associative { .. } => {
                    return Err("chandle associative positional assignment is unsupported".into())
                }
            };
            format!(
                "    {function}(&{}, {}, {});\n",
                name(ctx, *container),
                data,
                values.len()
            )
        }
        IrContainerStmt::Delete(index) => {
            if ctx.model.containers[*index].element.is_packed() {
                format!(
                    "    {}_delete(&{});\n",
                    prefix(&ctx.model.containers[*index].kind),
                    name(ctx, *index)
                )
            } else {
                let function = match ctx.model.containers[*index].kind {
                    IrContainerKind::Dynamic => "llg_dyn_value_delete",
                    IrContainerKind::Queue { .. } => "llg_queue_value_delete",
                    IrContainerKind::Associative { .. } => "llg_assoc_value_delete",
                };
                format!("    {function}(&{});\n", name(ctx, *index))
            }
        }
        IrContainerStmt::Set {
            container,
            index,
            value,
        } => {
            let method = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic
                    if ctx.model.containers[*container].element.is_packed() =>
                {
                    "llg_dyn_set"
                }
                IrContainerKind::Queue { .. }
                    if ctx.model.containers[*container].element.is_packed() =>
                {
                    "llg_queue_set"
                }
                IrContainerKind::Associative {
                    key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard,
                } if ctx.model.containers[*container].element.is_packed() => {
                    "llg_assoc_set_integral"
                }
                IrContainerKind::Queue { .. } => "llg_queue_value_set",
                IrContainerKind::Associative {
                    key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard,
                } => "llg_assoc_value_set_integral",
                IrContainerKind::Dynamic => {
                    return Err(
                        "packed container write used with a non-packed dynamic element".into(),
                    )
                }
                IrContainerKind::Associative {
                    key: IrAssocKey::String,
                } => return Err("packed index used for string-keyed associative array".into()),
            };
            format!(
                "    (void){method}(&{}, {}, {});\n",
                name(ctx, *container),
                operand(frame, owners, index)?.code,
                operand(frame, owners, value)?.code
            )
        }
        IrContainerStmt::SetReal {
            container,
            index,
            value,
        } => {
            let index_code = operand(frame, owners, index)?.code;
            let rendered = operand(frame, owners, value)?;
            let value = if rendered.width == 0 {
                rendered.code
            } else {
                format!("sv4_to_real({})", rendered.code)
            };
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_set_real",
                IrContainerKind::Queue { .. } => "llg_queue_value_set_real",
                IrContainerKind::Associative { .. } => "llg_assoc_value_set_integral_real",
            };
            format!(
                "    (void){function}(&{}, {}, {});\n",
                name(ctx, *container),
                index_code,
                value
            )
        }
        IrContainerStmt::SetStringValue {
            container,
            index,
            value,
        } => {
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_set_string",
                IrContainerKind::Queue { .. } => "llg_queue_value_set_string",
                IrContainerKind::Associative { .. } => "llg_assoc_value_set_integral_string",
            };
            format!(
                "    (void){function}(&{}, {}, {});\n",
                name(ctx, *container),
                operand(frame, owners, index)?.code,
                text_operand(frame, strings, value)?
            )
        }
        IrContainerStmt::SetChandleValue {
            container,
            index,
            value,
        } => {
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_set_chandle",
                IrContainerKind::Queue { .. } => "llg_queue_value_set_chandle",
                IrContainerKind::Associative { .. } => "llg_assoc_value_set_integral_chandle",
            };
            format!(
                "    (void){function}(&{}, {}, {});\n",
                name(ctx, *container),
                operand(frame, owners, index)?.code,
                frame.chandle(value)?
            )
        }
        IrContainerStmt::SetNested {
            container,
            indices,
            value,
        } => format!(
            "    (void){}(&{}, {}, {}, {});\n",
            match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_set_nested",
                IrContainerKind::Queue { .. } => "llg_queue_value_set_nested",
                IrContainerKind::Associative { .. } => "llg_assoc_value_set_nested_integral",
            },
            name(ctx, *container),
            super::indices(frame, owners, indices)?,
            indices.len(),
            operand(frame, owners, value)?.code
        ),
        IrContainerStmt::SetNestedReal {
            container,
            indices,
            value,
        } => {
            let indices_code = super::indices(frame, owners, indices)?;
            let rendered = operand(frame, owners, value)?;
            let value = if rendered.width == 0 {
                rendered.code
            } else {
                format!("sv4_to_real({})", rendered.code)
            };
            format!(
                "    (void){}(&{}, {}, {}, {});\n",
                match ctx.model.containers[*container].kind {
                    IrContainerKind::Dynamic => "llg_dyn_value_set_nested_real",
                    IrContainerKind::Queue { .. } => "llg_queue_value_set_nested_real",
                    IrContainerKind::Associative { .. } =>
                        "llg_assoc_value_set_nested_integral_real",
                },
                name(ctx, *container),
                indices_code,
                indices.len(),
                value
            )
        }
        IrContainerStmt::SetNestedString {
            container,
            indices,
            value,
        } => format!(
            "    (void){}(&{}, {}, {}, {});\n",
            match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_set_nested_string",
                IrContainerKind::Queue { .. } => "llg_queue_value_set_nested_string",
                IrContainerKind::Associative { .. } => "llg_assoc_value_set_nested_integral_string",
            },
            name(ctx, *container),
            super::indices(frame, owners, indices)?,
            indices.len(),
            text_operand(frame, strings, value)?
        ),
        IrContainerStmt::SetNestedChandle {
            container,
            indices,
            value,
        } => format!(
            "    (void){}(&{}, {}, {}, {});\n",
            match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_set_nested_chandle",
                IrContainerKind::Queue { .. } => "llg_queue_value_set_nested_chandle",
                IrContainerKind::Associative { .. } =>
                    "llg_assoc_value_set_nested_integral_chandle",
            },
            name(ctx, *container),
            super::indices(frame, owners, indices)?,
            indices.len(),
            frame.chandle(value)?
        ),
        IrContainerStmt::SetContainer {
            container,
            indices,
            source,
        } => {
            let source_is_packed = ctx.model.containers[*source].element.is_packed();
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => {
                    if source_is_packed {
                        "llg_dyn_value_set_nested_container_from_packed"
                    } else {
                        "llg_dyn_value_set_nested_container"
                    }
                }
                IrContainerKind::Queue { .. } => {
                    if source_is_packed {
                        "llg_queue_value_set_nested_container_from_packed"
                    } else {
                        "llg_queue_value_set_nested_container"
                    }
                }
                IrContainerKind::Associative { .. } => {
                    if source_is_packed {
                        "llg_assoc_value_set_nested_integral_container_from_packed"
                    } else {
                        "llg_assoc_value_set_nested_integral_container"
                    }
                }
            };
            let source = format!("&{}", name(ctx, *source));
            format!(
                "    (void){function}(&{}, {}, {}, {source});\n",
                name(ctx, *container),
                super::indices(frame, owners, indices)?,
                indices.len(),
            )
        }
        IrContainerStmt::SetDefault { container, value } => {
            let function = if ctx.model.containers[*container].element.is_packed() {
                "llg_assoc_set_default"
            } else if ctx.model.containers[*container].element.is_real() {
                "llg_assoc_value_set_default_real"
            } else {
                return Err("packed expression cannot initialize this associative default".into());
            };
            let rendered = operand(frame, owners, value)?;
            let value =
                if ctx.model.containers[*container].element.is_packed() || rendered.width == 0 {
                    rendered.code
                } else {
                    format!("sv4_to_real({})", rendered.code)
                };
            format!("    {function}(&{}, {});\n", name(ctx, *container), value)
        }
        IrContainerStmt::SetDefaultString { container, value } => format!(
            "    llg_assoc_value_set_default_string(&{}, {});\n",
            name(ctx, *container),
            text_operand(frame, strings, value)?
        ),
        IrContainerStmt::SetDefaultChandle { container, value } => format!(
            "    llg_assoc_value_set_default_chandle(&{}, {});\n",
            name(ctx, *container),
            frame.chandle(value)?
        ),
        IrContainerStmt::ResetDefault(container) => {
            let function = if ctx.model.containers[*container].element.is_packed() {
                "llg_assoc_reset_default"
            } else {
                "llg_assoc_value_reset_default"
            };
            format!("    {function}(&{});\n", name(ctx, *container))
        }
        IrContainerStmt::SetString {
            container,
            key,
            value,
        } => format!(
            "    (void)llg_owned_assoc_set_string(&{}, {}, {});\n",
            name(ctx, *container),
            key_operand(frame, strings, key)?,
            operand(frame, owners, value)?.code
        ),
        IrContainerStmt::SetStringReal {
            container,
            key,
            value,
        } => {
            let key_code = key_operand(frame, strings, key)?;
            let rendered = operand(frame, owners, value)?;
            let value = if rendered.width == 0 {
                rendered.code
            } else {
                format!("sv4_to_real({})", rendered.code)
            };
            format!(
                "    (void)llg_owned_assoc_value_set_real(&{}, {}, {});\n",
                name(ctx, *container),
                key_code,
                value
            )
        }
        IrContainerStmt::SetStringString {
            container,
            key,
            value,
        } => format!(
            "    (void)llg_owned_assoc_value_set_string(&{}, {}, {});\n",
            name(ctx, *container),
            key_operand(frame, strings, key)?,
            text_operand(frame, strings, value)?
        ),
        IrContainerStmt::SetStringChandle {
            container,
            key,
            value,
        } => format!(
            "    (void)llg_owned_assoc_value_set_chandle(&{}, {}, {});\n",
            name(ctx, *container),
            key_operand(frame, strings, key)?,
            frame.chandle(value)?
        ),
        IrContainerStmt::QueuePushFront { container, value } => format!(
            "    {}(&{}, {});\n",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_queue_push_front"
            } else {
                "llg_queue_value_push_front_real"
            },
            name(ctx, *container),
            {
                let rendered = operand(frame, owners, value)?;
                if ctx.model.containers[*container].element.is_real() && rendered.width != 0 {
                    format!("sv4_to_real({})", rendered.code)
                } else {
                    rendered.code
                }
            }
        ),
        IrContainerStmt::QueuePushBack { container, value } => format!(
            "    {}(&{}, {});\n",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_queue_push_back"
            } else {
                "llg_queue_value_push_back_real"
            },
            name(ctx, *container),
            {
                let rendered = operand(frame, owners, value)?;
                if ctx.model.containers[*container].element.is_real() && rendered.width != 0 {
                    format!("sv4_to_real({})", rendered.code)
                } else {
                    rendered.code
                }
            }
        ),
        IrContainerStmt::QueuePushFrontString { container, value } => format!(
            "    llg_queue_value_push_front_string(&{}, {});\n",
            name(ctx, *container),
            text_operand(frame, strings, value)?
        ),
        IrContainerStmt::QueuePushBackString { container, value } => format!(
            "    llg_queue_value_push_back_string(&{}, {});\n",
            name(ctx, *container),
            text_operand(frame, strings, value)?
        ),
        IrContainerStmt::QueuePushFrontChandle { container, value } => format!(
            "    llg_queue_value_push_front_chandle(&{}, {});\n",
            name(ctx, *container),
            frame.chandle(value)?
        ),
        IrContainerStmt::QueuePushBackChandle { container, value } => format!(
            "    llg_queue_value_push_back_chandle(&{}, {});\n",
            name(ctx, *container),
            frame.chandle(value)?
        ),
        IrContainerStmt::QueuePushFrontContainer { container, source } => {
            let function = if ctx.model.containers[*source].element.is_packed() {
                "llg_queue_value_push_front_container_from_packed"
            } else {
                "llg_queue_value_push_front_container"
            };
            format!(
                "    {function}(&{}, &{});\n",
                name(ctx, *container),
                name(ctx, *source)
            )
        }
        IrContainerStmt::QueuePushBackContainer { container, source } => {
            let function = if ctx.model.containers[*source].element.is_packed() {
                "llg_queue_value_push_back_container_from_packed"
            } else {
                "llg_queue_value_push_back_container"
            };
            format!(
                "    {function}(&{}, &{});\n",
                name(ctx, *container),
                name(ctx, *source)
            )
        }
        IrContainerStmt::QueueInsert {
            container,
            index,
            value,
        } => format!(
            "    (void){}(&{}, {}, {});\n",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_queue_insert"
            } else {
                "llg_queue_value_insert_real"
            },
            name(ctx, *container),
            operand(frame, owners, index)?.code,
            {
                let rendered = operand(frame, owners, value)?;
                if ctx.model.containers[*container].element.is_real() && rendered.width != 0 {
                    format!("sv4_to_real({})", rendered.code)
                } else {
                    rendered.code
                }
            }
        ),
        IrContainerStmt::QueueInsertString {
            container,
            index,
            value,
        } => format!(
            "    (void)llg_queue_value_insert_string(&{}, {}, {});\n",
            name(ctx, *container),
            operand(frame, owners, index)?.code,
            text_operand(frame, strings, value)?
        ),
        IrContainerStmt::QueueInsertChandle {
            container,
            index,
            value,
        } => format!(
            "    (void)llg_queue_value_insert_chandle(&{}, {}, {});\n",
            name(ctx, *container),
            operand(frame, owners, index)?.code,
            frame.chandle(value)?
        ),
        IrContainerStmt::QueueInsertContainer {
            container,
            index,
            source,
        } => {
            let function = if ctx.model.containers[*source].element.is_packed() {
                "llg_queue_value_insert_container_from_packed"
            } else {
                "llg_queue_value_insert_container"
            };
            format!(
                "    (void){function}(&{}, {}, &{});\n",
                name(ctx, *container),
                operand(frame, owners, index)?.code,
                name(ctx, *source)
            )
        }
        IrContainerStmt::DeleteIndex { container, index } => {
            let method = match ctx.model.containers[*container].kind {
                IrContainerKind::Queue { .. }
                    if ctx.model.containers[*container].element.is_packed() =>
                {
                    "llg_queue_delete_index"
                }
                IrContainerKind::Queue { .. } => "llg_queue_value_delete_index",
                IrContainerKind::Associative { .. }
                    if ctx.model.containers[*container].element.is_packed() =>
                {
                    "llg_assoc_delete_integral"
                }
                IrContainerKind::Associative { .. } => "llg_assoc_value_delete_integral",
                IrContainerKind::Dynamic => {
                    return Err("dynamic-array delete method does not take an index".into())
                }
            };
            format!(
                "    (void){method}(&{}, {});\n",
                name(ctx, *container),
                operand(frame, owners, index)?.code
            )
        }
        IrContainerStmt::DeleteString { container, key } => format!(
            "    (void){}(&{}, {});\n",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_owned_assoc_delete_string"
            } else {
                "llg_owned_assoc_value_delete_string"
            },
            name(ctx, *container),
            key_operand(frame, strings, key)?
        ),
    })
}
