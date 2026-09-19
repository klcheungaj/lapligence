//! C rendering for dynamically sized unpacked containers.

use super::context::RCtx;
use super::expressions::render_expr_impl;
use super::objects::string as render_string;
use crate::sim::ir::{
    IrAssocKey, IrAssocTraversal, IrContainerElement, IrContainerExpr, IrContainerKind,
    IrContainerMethod, IrContainerReduction, IrContainerStmt, IrQueueBound, IrQueueSource,
    IrStreamDirection, IrStreamSelector,
};

fn name<'a>(ctx: &'a RCtx<'_>, index: usize) -> &'a str {
    &ctx.model.containers[index].c_name
}

pub(super) fn stream_selector_code(
    ctx: &RCtx<'_>,
    selector: Option<&IrStreamSelector>,
) -> Result<(i32, String, String), String> {
    match selector {
        None => Ok((
            0,
            "sv4_from_u64(0, 32, 1)".to_owned(),
            "sv4_from_u64(0, 32, 1)".to_owned(),
        )),
        Some(IrStreamSelector::Index(index)) => Ok((
            1,
            render_expr_impl(ctx, index)?.code,
            "sv4_from_u64(0, 32, 1)".to_owned(),
        )),
        Some(IrStreamSelector::Range { left, right }) => Ok((
            2,
            render_expr_impl(ctx, left)?.code,
            render_expr_impl(ctx, right)?.code,
        )),
        Some(IrStreamSelector::Indexed {
            base,
            width,
            negative,
        }) => Ok((
            if *negative { 4 } else { 3 },
            render_expr_impl(ctx, base)?.code,
            render_expr_impl(ctx, width)?.code,
        )),
    }
}

pub(super) fn expression(ctx: &RCtx<'_>, operation: &IrContainerExpr) -> Result<String, String> {
    Ok(match operation {
        IrContainerExpr::Stream {
            container,
            slice,
            direction,
            selector,
        } => {
            let (selector_kind, first, second) = stream_selector_code(ctx, selector.as_ref())?;
            let function = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_stream",
                IrContainerKind::Queue { .. } => "llg_queue_stream",
                IrContainerKind::Associative { .. } => {
                    return Err("associative arrays are not legal streaming operands".into())
                }
            };
            format!(
                "{function}(&{}, {slice}, {}, {selector_kind}, {first}, {second})",
                name(ctx, *container),
                matches!(direction, IrStreamDirection::RightToLeft) as u8
            )
        }
        IrContainerExpr::Size(index) => {
            let method = match ctx.model.containers[*index].kind {
                IrContainerKind::Dynamic => {
                    if ctx.model.containers[*index].element.is_packed() {
                        "llg_dyn_size"
                    } else {
                        "llg_dyn_value_size"
                    }
                }
                IrContainerKind::Queue { .. } => {
                    if ctx.model.containers[*index].element.is_packed() {
                        "llg_queue_size"
                    } else {
                        "llg_queue_value_size"
                    }
                }
                IrContainerKind::Associative { .. } => {
                    if ctx.model.containers[*index].element.is_packed() {
                        "llg_assoc_count"
                    } else {
                        "llg_assoc_value_count"
                    }
                }
            };
            format!(
                "sv4_from_u64((uint64_t){method}(&{}), 32, 1)",
                name(ctx, *index)
            )
        }
        IrContainerExpr::Reduce {
            container,
            operation,
        } => format!(
            "{}_reduce(&{}, {})",
            prefix(&ctx.model.containers[*container].kind),
            name(ctx, *container),
            match operation {
                IrContainerReduction::Sum => "LLG_CONTAINER_REDUCE_SUM",
                IrContainerReduction::Product => "LLG_CONTAINER_REDUCE_PRODUCT",
                IrContainerReduction::BitAnd => "LLG_CONTAINER_REDUCE_AND",
                IrContainerReduction::BitOr => "LLG_CONTAINER_REDUCE_OR",
                IrContainerReduction::BitXor => "LLG_CONTAINER_REDUCE_XOR",
            }
        ),
        IrContainerExpr::ReduceWith {
            container,
            operation,
            callback,
            result_width,
            result_signed,
            result_two_state,
        } => format!(
            "{}_reduce_with(&{}, {}, {}, {}, {}, {}, NULL)",
            prefix(&ctx.model.containers[*container].kind),
            name(ctx, *container),
            match operation {
                IrContainerReduction::Sum => "LLG_CONTAINER_REDUCE_SUM",
                IrContainerReduction::Product => "LLG_CONTAINER_REDUCE_PRODUCT",
                IrContainerReduction::BitAnd => "LLG_CONTAINER_REDUCE_AND",
                IrContainerReduction::BitOr => "LLG_CONTAINER_REDUCE_OR",
                IrContainerReduction::BitXor => "LLG_CONTAINER_REDUCE_XOR",
            },
            result_width,
            *result_signed as u8,
            *result_two_state as u8,
            callback,
        ),
        IrContainerExpr::Get { container, index } => {
            let method = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_get",
                IrContainerKind::Queue { .. }
                    if ctx.model.containers[*container].element.is_packed() =>
                {
                    "llg_queue_get"
                }
                IrContainerKind::Queue { .. } => "llg_queue_value_get",
                IrContainerKind::Associative {
                    key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard,
                } if ctx.model.containers[*container].element.is_packed() => {
                    "llg_assoc_get_integral"
                }
                IrContainerKind::Associative {
                    key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard,
                } => "llg_assoc_value_get_integral",
                IrContainerKind::Associative {
                    key: IrAssocKey::String,
                } => return Err("packed index used for string-keyed associative array".into()),
            };
            format!(
                "{method}(&{}, {})",
                name(ctx, *container),
                render_expr_impl(ctx, index)?.code
            )
        }
        IrContainerExpr::GetReal { container, index } => format!(
            "{}(&{}, {})",
            match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_get_real",
                IrContainerKind::Queue { .. } => "llg_queue_value_get_real",
                IrContainerKind::Associative { .. } => "llg_assoc_value_get_integral_real",
            },
            name(ctx, *container),
            render_expr_impl(ctx, index)?.code
        ),
        IrContainerExpr::GetNested { container, indices } => format!(
            "{}(&{}, {}, {})",
            match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_get_nested",
                IrContainerKind::Queue { .. } => "llg_queue_value_get_nested",
                IrContainerKind::Associative { .. } => "llg_assoc_value_get_nested_integral",
            },
            name(ctx, *container),
            super::objects::render_indices(ctx, indices)?,
            indices.len()
        ),
        IrContainerExpr::GetNestedReal { container, indices } => format!(
            "{}(&{}, {}, {})",
            match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_get_nested_real",
                IrContainerKind::Queue { .. } => "llg_queue_value_get_nested_real",
                IrContainerKind::Associative { .. } => "llg_assoc_value_get_nested_integral_real",
            },
            name(ctx, *container),
            super::objects::render_indices(ctx, indices)?,
            indices.len()
        ),
        IrContainerExpr::GetString { container, key } => format!(
            "llg_model_assoc_get_string(&{}, {})",
            name(ctx, *container),
            render_string(ctx, key)?
        ),
        IrContainerExpr::GetStringReal { container, key } => format!(
            "llg_model_assoc_value_get_real(&{}, {})",
            name(ctx, *container),
            render_string(ctx, key)?
        ),
        IrContainerExpr::Exists { container, key } => format!(
            "sv4_from_u64({}(&{}, {}), 32, 1)",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_assoc_exists_integral"
            } else {
                "llg_assoc_value_exists_integral"
            },
            name(ctx, *container),
            render_expr_impl(ctx, key)?.code
        ),
        IrContainerExpr::ExistsString { container, key } => format!(
            "sv4_from_u64({}(&{}, {}), 32, 1)",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_model_assoc_exists_string"
            } else {
                "llg_model_assoc_value_exists_string"
            },
            name(ctx, *container),
            render_string(ctx, key)?
        ),
        IrContainerExpr::AssocTraverse {
            container,
            direction,
            key_address,
            ..
        } => {
            let method = match direction {
                IrAssocTraversal::First => "llg_assoc_first_integral",
                IrAssocTraversal::Last => "llg_assoc_last_integral",
                IrAssocTraversal::Next => "llg_assoc_next_integral",
                IrAssocTraversal::Prev => "llg_assoc_prev_integral",
            };
            let method = if ctx.model.containers[*container].element.is_packed() {
                method
            } else {
                match direction {
                    IrAssocTraversal::First => "llg_assoc_value_first_integral",
                    IrAssocTraversal::Last => "llg_assoc_value_last_integral",
                    IrAssocTraversal::Next => "llg_assoc_value_next_integral",
                    IrAssocTraversal::Prev => "llg_assoc_value_prev_integral",
                }
            };
            format!(
                "sv4_from_u64({method}(&{}, {}), 32, 1)",
                name(ctx, *container),
                key_address
            )
        }
        IrContainerExpr::AssocTraverseString {
            container,
            direction,
            key_object,
        } => format!(
            "sv4_from_u64({}(&{}, &{}, {}), 32, 1)",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_model_assoc_traverse_string"
            } else {
                "llg_model_assoc_value_traverse_string"
            },
            name(ctx, *container),
            ctx.model.objects[*key_object].c_name,
            match direction {
                IrAssocTraversal::First => 0,
                IrAssocTraversal::Last => 1,
                IrAssocTraversal::Next => 2,
                IrAssocTraversal::Prev => 3,
            }
        ),
        IrContainerExpr::AssocTraverseStringLocal {
            container,
            direction,
            key_name,
        } => format!(
            "sv4_from_u64({}(&{}, &{}, {}), 32, 1)",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_model_assoc_traverse_string"
            } else {
                "llg_model_assoc_value_traverse_string"
            },
            name(ctx, *container),
            key_name,
            match direction {
                IrAssocTraversal::First => 0,
                IrAssocTraversal::Last => 1,
                IrAssocTraversal::Next => 2,
                IrAssocTraversal::Prev => 3,
            }
        ),
        IrContainerExpr::QueueFront(index) => {
            format!("llg_queue_front(&{})", name(ctx, *index))
        }
        IrContainerExpr::QueueBack(index) => {
            format!("llg_queue_back(&{})", name(ctx, *index))
        }
        IrContainerExpr::QueuePopFront(index) => {
            format!("llg_queue_pop_front(&{})", name(ctx, *index))
        }
        IrContainerExpr::QueuePopBack(index) => {
            format!("llg_queue_pop_back(&{})", name(ctx, *index))
        }
    })
}

pub(super) fn statement(ctx: &RCtx<'_>, operation: &IrContainerStmt) -> Result<String, String> {
    Ok(match operation {
        IrContainerStmt::StreamAssign {
            container,
            source,
            slice,
            direction,
            selector,
        } => {
            let source = render_expr_impl(ctx, source)?;
            let (selector_kind, first, second) = stream_selector_code(ctx, selector.as_ref())?;
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
                render_expr_impl(ctx, size)?.code,
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
                    .map(|value| Ok(render_expr_impl(ctx, value)?.code))
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
                            "sv4_from_u64(0, 32, 1)".to_owned(),
                            "sv4_from_u64(0, 32, 1)".to_owned(),
                            0,
                            1,
                        ),
                        IrQueueSource::Slice {
                            container: source,
                            left,
                            right,
                        } => {
                            let render_bound = |bound: &IrQueueBound| match bound {
                                IrQueueBound::Value(value) => {
                                    render_expr_impl(ctx, value).map(|value| value.code)
                                }
                                IrQueueBound::Unbounded => Ok(
                                    "sv4_from_u64(0, 32, 1)".to_owned(),
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
                        let rendered = render_expr_impl(ctx, value)?;
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
                    .map(|value| super::objects::string(ctx, value))
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
                    .map(|value| super::objects::chandle(ctx, value))
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
                render_expr_impl(ctx, index)?.code,
                render_expr_impl(ctx, value)?.code
            )
        }
        IrContainerStmt::SetReal {
            container,
            index,
            value,
        } => {
            let rendered = render_expr_impl(ctx, value)?;
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
                render_expr_impl(ctx, index)?.code,
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
                render_expr_impl(ctx, index)?.code,
                super::objects::string(ctx, value)?
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
                render_expr_impl(ctx, index)?.code,
                super::objects::chandle(ctx, value)?
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
            super::objects::render_indices(ctx, indices)?,
            indices.len(),
            render_expr_impl(ctx, value)?.code
        ),
        IrContainerStmt::SetNestedReal {
            container,
            indices,
            value,
        } => {
            let rendered = render_expr_impl(ctx, value)?;
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
                super::objects::render_indices(ctx, indices)?,
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
            super::objects::render_indices(ctx, indices)?,
            indices.len(),
            super::objects::string(ctx, value)?
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
            super::objects::render_indices(ctx, indices)?,
            indices.len(),
            super::objects::chandle(ctx, value)?
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
                super::objects::render_indices(ctx, indices)?,
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
            let rendered = render_expr_impl(ctx, value)?;
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
            super::objects::string(ctx, value)?
        ),
        IrContainerStmt::SetDefaultChandle { container, value } => format!(
            "    llg_assoc_value_set_default_chandle(&{}, {});\n",
            name(ctx, *container),
            super::objects::chandle(ctx, value)?
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
            "    (void)llg_model_assoc_set_string(&{}, {}, {});\n",
            name(ctx, *container),
            render_string(ctx, key)?,
            render_expr_impl(ctx, value)?.code
        ),
        IrContainerStmt::SetStringReal {
            container,
            key,
            value,
        } => {
            let rendered = render_expr_impl(ctx, value)?;
            let value = if rendered.width == 0 {
                rendered.code
            } else {
                format!("sv4_to_real({})", rendered.code)
            };
            format!(
                "    (void)llg_model_assoc_value_set_real(&{}, {}, {});\n",
                name(ctx, *container),
                render_string(ctx, key)?,
                value
            )
        }
        IrContainerStmt::SetStringString {
            container,
            key,
            value,
        } => format!(
            "    (void)llg_model_assoc_value_set_string(&{}, {}, {});\n",
            name(ctx, *container),
            render_string(ctx, key)?,
            super::objects::string(ctx, value)?
        ),
        IrContainerStmt::SetStringChandle {
            container,
            key,
            value,
        } => format!(
            "    (void)llg_model_assoc_value_set_chandle(&{}, {}, {});\n",
            name(ctx, *container),
            render_string(ctx, key)?,
            super::objects::chandle(ctx, value)?
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
                let rendered = render_expr_impl(ctx, value)?;
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
                let rendered = render_expr_impl(ctx, value)?;
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
            super::objects::string(ctx, value)?
        ),
        IrContainerStmt::QueuePushBackString { container, value } => format!(
            "    llg_queue_value_push_back_string(&{}, {});\n",
            name(ctx, *container),
            super::objects::string(ctx, value)?
        ),
        IrContainerStmt::QueuePushFrontChandle { container, value } => format!(
            "    llg_queue_value_push_front_chandle(&{}, {});\n",
            name(ctx, *container),
            super::objects::chandle(ctx, value)?
        ),
        IrContainerStmt::QueuePushBackChandle { container, value } => format!(
            "    llg_queue_value_push_back_chandle(&{}, {});\n",
            name(ctx, *container),
            super::objects::chandle(ctx, value)?
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
            render_expr_impl(ctx, index)?.code,
            {
                let rendered = render_expr_impl(ctx, value)?;
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
            render_expr_impl(ctx, index)?.code,
            super::objects::string(ctx, value)?
        ),
        IrContainerStmt::QueueInsertChandle {
            container,
            index,
            value,
        } => format!(
            "    (void)llg_queue_value_insert_chandle(&{}, {}, {});\n",
            name(ctx, *container),
            render_expr_impl(ctx, index)?.code,
            super::objects::chandle(ctx, value)?
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
                render_expr_impl(ctx, index)?.code,
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
                render_expr_impl(ctx, index)?.code
            )
        }
        IrContainerStmt::DeleteString { container, key } => format!(
            "    (void){}(&{}, {});\n",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_model_assoc_delete_string"
            } else {
                "llg_model_assoc_value_delete_string"
            },
            name(ctx, *container),
            render_string(ctx, key)?
        ),
    })
}

/// Adapters consume owned string-expression results while the container runtime
/// remains independent of the string runtime's representation.
#[allow(dead_code)] // legacy emitter helper retained until the owned-emission migration removes it
pub(super) fn string_adapters() -> &'static str {
    "static sv4_t llg_model_assoc_get_string(const llg_assoc_t *array, llg_string_t key) {\n\
     \x20   sv4_t result = llg_assoc_get_string(array, key.data, key.len);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_exists_string(const llg_assoc_t *array, llg_string_t key) {\n\
     \x20   int result = llg_assoc_exists_string(array, key.data, key.len);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_set_string(llg_assoc_t *array, llg_string_t key, sv4_t value) {\n\
     \x20   int result = llg_assoc_set_string(array, key.data, key.len, value);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_delete_string(llg_assoc_t *array, llg_string_t key) {\n\
     \x20   int result = llg_assoc_delete_string(array, key.data, key.len);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_traverse_string(const llg_assoc_t *array, llg_string_t *current, int direction) {\n\
     \x20   const unsigned char *bytes = NULL;\n\
     \x20   size_t length = 0;\n\
     \x20   int result;\n\
     \x20   switch (direction) {\n\
     \x20   case 0: result = llg_assoc_first_string(array, &bytes, &length); break;\n\
     \x20   case 1: result = llg_assoc_last_string(array, &bytes, &length); break;\n\
     \x20   case 2: result = llg_assoc_next_string(array, current->data, current->len, &bytes, &length); break;\n\
     \x20   default: result = llg_assoc_prev_string(array, current->data, current->len, &bytes, &length); break;\n\
     \x20   }\n\
     \x20   if (result) {\n\
     \x20       llg_string_t replacement = llg_string_bytes((const char *)bytes, length);\n\
     \x20       llg_string_move(current, replacement);\n\
     \x20   }\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_value_exists_string(const llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   int result = llg_assoc_value_exists_string(array, key.data, key.len);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static llg_string_t llg_model_assoc_value_get_string(const llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   llg_string_t result = llg_assoc_value_get_string(array, key.data, key.len);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static double llg_model_assoc_value_get_real(const llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   double result = llg_assoc_value_get_string_real(array, key.data, key.len);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static void *llg_model_assoc_value_get_chandle(const llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   void *result = llg_assoc_value_get_string_chandle(array, key.data, key.len);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_value_set_real(llg_assoc_value_t *array, llg_string_t key, double value) {\n\
     \x20   int result = llg_assoc_value_set_string_real(array, key.data, key.len, value);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_value_set_string(llg_assoc_value_t *array, llg_string_t key, llg_string_t value) {\n\
     \x20   int result = llg_assoc_value_set_string_string(array, key.data, key.len, value);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_value_set_chandle(llg_assoc_value_t *array, llg_string_t key, void *value) {\n\
     \x20   int result = llg_assoc_value_set_string_chandle(array, key.data, key.len, value);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_value_delete_string(llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   int result = llg_assoc_value_delete_string(array, key.data, key.len);\n\
     \x20   llg_string_destroy(&key);\n\
     \x20   return result;\n\
     }\n\
     static int llg_model_assoc_value_traverse_string(const llg_assoc_value_t *array, llg_string_t *current, int direction) {\n\
     \x20   const unsigned char *bytes = NULL;\n\
     \x20   size_t length = 0;\n\
     \x20   int result;\n\
     \x20   switch (direction) {\n\
     \x20   case 0: result = llg_assoc_value_first_string(array, &bytes, &length); break;\n\
     \x20   case 1: result = llg_assoc_value_last_string(array, &bytes, &length); break;\n\
     \x20   case 2: result = llg_assoc_value_next_string(array, current->data, current->len, &bytes, &length); break;\n\
     \x20   default: result = llg_assoc_value_prev_string(array, current->data, current->len, &bytes, &length); break;\n\
     \x20   }\n\
     \x20   if (result) {\n\
     \x20       llg_string_t replacement = llg_string_bytes((const char *)bytes, length);\n\
     \x20       llg_string_move(current, replacement);\n\
     \x20   }\n\
     \x20   return result;\n\
     }\n\n"
}

#[derive(Clone)]
struct ValueDescriptorNode {
    element: IrContainerElement,
    child: Option<usize>,
    members: Vec<usize>,
}

fn collect_value_descriptor(
    element: &IrContainerElement,
    nodes: &mut Vec<ValueDescriptorNode>,
) -> usize {
    let index = nodes.len();
    nodes.push(ValueDescriptorNode {
        element: element.clone(),
        child: None,
        members: Vec::new(),
    });
    match element {
        IrContainerElement::FixedArray { element, .. }
        | IrContainerElement::Container { element, .. } => {
            nodes[index].child = Some(collect_value_descriptor(element, nodes));
        }
        IrContainerElement::Aggregate { members, .. } => {
            let member_indices = members
                .iter()
                .map(|member| collect_value_descriptor(&member.element, nodes))
                .collect();
            nodes[index].members = member_indices;
        }
        _ => {}
    }
    index
}

fn value_descriptor(container: &crate::sim::ir::IrContainer) -> Result<(String, String), String> {
    let mut nodes = Vec::new();
    let root = collect_value_descriptor(&container.element, &mut nodes);
    let prefix = format!("{}_llg_value", container.c_name);
    let names = (0..nodes.len())
        .map(|index| format!("{prefix}_desc_{index}"))
        .collect::<Vec<_>>();
    let member_names = (0..nodes.len())
        .map(|index| format!("{prefix}_members_{index}"))
        .collect::<Vec<_>>();
    let mut out = String::new();
    for name in &names {
        out.push_str(&format!("static const llg_value_desc_t {name};\n"));
    }
    for (index, node) in nodes.iter().enumerate() {
        if !node.members.is_empty() {
            let entries = node
                .members
                .iter()
                .map(|member| format!("{{ &{} }}", names[*member]))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "static const llg_value_member_desc_t {}[] = {{ {} }};\n",
                member_names[index], entries
            ));
        }
    }
    for (index, node) in nodes.iter().enumerate() {
        let (kind, type_id, width, signed, two_state, real_short, count) = match &node.element {
            IrContainerElement::Packed {
                width,
                signed,
                two_state,
            } => (
                "LLG_VALUE_PACKED",
                0,
                *width,
                *signed as u8,
                *two_state as u8,
                0,
                0,
            ),
            IrContainerElement::Real { shortreal } => {
                ("LLG_VALUE_REAL", 0, 0, 0, 0, *shortreal as u8, 0)
            }
            IrContainerElement::String => ("LLG_VALUE_STRING", 0, 0, 0, 0, 0, 0),
            IrContainerElement::Chandle => ("LLG_VALUE_CHANDLE", 0, 0, 0, 0, 0, 0),
            IrContainerElement::Event => ("LLG_VALUE_EVENT", 0, 0, 0, 0, 0, 0),
            IrContainerElement::Union { .. } => {
                return Err(
                    "unpacked unions in resizable containers require overlay storage".into(),
                );
            }
            IrContainerElement::Aggregate { type_id, members } => {
                ("LLG_VALUE_AGGREGATE", *type_id, 0, 0, 0, 0, members.len())
            }
            IrContainerElement::FixedArray { dimensions, .. } => {
                let count = dimensions
                    .iter()
                    .try_fold(1u128, |total, (left, right)| {
                        let extent = (i64::from(*left) - i64::from(*right))
                            .unsigned_abs()
                            .checked_add(1)?;
                        total.checked_mul(u128::from(extent))
                    })
                    .ok_or_else(|| {
                        format!(
                            "container {} fixed element count overflows C size_t",
                            container.c_name
                        )
                    })?;
                let count = usize::try_from(count).map_err(|_| {
                    format!(
                        "container {} fixed element count is not host-representable",
                        container.c_name
                    )
                })?;
                ("LLG_VALUE_FIXED_ARRAY", 0, 0, 0, 0, 0, count)
            }
            IrContainerElement::Container { type_id, .. } => {
                ("LLG_VALUE_CONTAINER", *type_id, 0, 0, 0, 0, 0)
            }
            IrContainerElement::Opaque { type_id, .. } => {
                ("LLG_VALUE_OPAQUE", *type_id, 0, 0, 0, 0, 0)
            }
        };
        let child = node
            .child
            .map(|child| format!("&{}", names[child]))
            .unwrap_or_else(|| "NULL".to_owned());
        let members = if node.members.is_empty() {
            "NULL".to_owned()
        } else {
            format!("&{}", member_names[index])
        };
        out.push_str(&format!(
            "static const llg_value_desc_t {} = {{ {}, UINT64_C({}), {}, {}, {}, {}, {}, {}, {}, {} }};\n",
            names[index],
            kind,
            type_id,
            width,
            signed,
            two_state,
            real_short,
            count,
            child,
            members,
            node.members.len()
        ));
    }
    Ok((out, names[root].clone()))
}

pub(super) fn declaration_and_init(
    container: &crate::sim::ir::IrContainer,
) -> Result<(String, String), String> {
    if !container.element.is_packed() {
        let (descriptor, root) = value_descriptor(container)?;
        let (declaration, init) = match &container.kind {
            IrContainerKind::Dynamic => (
                format!(
                    "{descriptor}static llg_dyn_value_array_t {};\n",
                    container.c_name
                ),
                format!(
                    "    llg_dyn_value_init(&{}, &{});\n",
                    container.c_name, root
                ),
            ),
            IrContainerKind::Queue { maximum_elements } => (
                format!(
                    "{descriptor}static llg_queue_value_array_t {};\n",
                    container.c_name
                ),
                format!(
                    "    llg_queue_value_init(&{}, &{}, {});\n",
                    container.c_name,
                    root,
                    maximum_elements
                        .map(|value| format!("{value}ULL"))
                        .unwrap_or_else(|| "UINT64_MAX".to_owned())
                ),
            ),
            IrContainerKind::Associative { key } => {
                let init = match key {
                    IrAssocKey::Wildcard => format!(
                        "    llg_assoc_value_init_integral(&{}, &{}, 0, 0, 0);\n",
                        container.c_name, root
                    ),
                    IrAssocKey::Integral {
                        width,
                        signed,
                        two_state,
                    } => format!(
                        "    llg_assoc_value_init_integral(&{}, &{}, {}, {}, {});\n",
                        container.c_name, root, width, *signed as u8, *two_state as u8
                    ),
                    IrAssocKey::String => format!(
                        "    llg_assoc_value_init_string(&{}, &{});\n",
                        container.c_name, root
                    ),
                };
                (
                    format!(
                        "{descriptor}static llg_assoc_value_t {};\n",
                        container.c_name
                    ),
                    init,
                )
            }
        };
        let init = format!(
            "{init}    {}.contents_dependency = &{}_llg_contents_dep;\n\
             {}.shape_dependency = &{}_llg_shape_dep;\n\
             {}.notify = llg_dependency_notify;\n",
            container.c_name,
            container.c_name,
            container.c_name,
            container.c_name,
            container.c_name
        );
        return Ok((declaration, init));
    }
    let (width, signed, two_state) = container.element.packed().unwrap();
    let declaration = format!("static {} {};\n", c_type(&container.kind), container.c_name);
    let init = match &container.kind {
        IrContainerKind::Dynamic => format!(
            "    llg_dyn_init(&{}, {width}, {}, {});\n",
            container.c_name, signed as u8, two_state as u8
        ),
        IrContainerKind::Queue { maximum_elements } => format!(
            "    llg_queue_init(&{}, {width}, {}, {}, {});\n",
            container.c_name,
            signed as u8,
            two_state as u8,
            maximum_elements
                .map(|value| format!("{value}ULL"))
                .unwrap_or_else(|| "UINT64_MAX".to_owned())
        ),
        IrContainerKind::Associative { key } => match key {
            IrAssocKey::Wildcard => format!(
                "    llg_assoc_init_integral(&{}, {width}, {}, {}, 0, 0, 0);\n",
                container.c_name, signed as u8, two_state as u8
            ),
            IrAssocKey::Integral {
                width: key_width,
                signed: key_signed,
                two_state: key_two_state,
            } => format!(
                "    llg_assoc_init_integral(&{}, {width}, {}, {}, {key_width}, {}, {});\n",
                container.c_name,
                signed as u8,
                two_state as u8,
                *key_signed as u8,
                *key_two_state as u8
            ),
            IrAssocKey::String => format!(
                "    llg_assoc_init_string(&{}, {width}, {}, {});\n",
                container.c_name, signed as u8, two_state as u8
            ),
        },
    };
    let init = format!(
        "{init}    {}.contents_dependency = &{}_llg_contents_dep;\n\
             {}.shape_dependency = &{}_llg_shape_dep;\n\
             {}.notify = llg_dependency_notify;\n",
        container.c_name, container.c_name, container.c_name, container.c_name, container.c_name
    );
    Ok((declaration, init))
}

pub(super) fn destroy(container: &crate::sim::ir::IrContainer) -> String {
    if !container.element.is_packed() {
        let function = match &container.kind {
            IrContainerKind::Dynamic => "llg_dyn_value_destroy",
            IrContainerKind::Queue { .. } => "llg_queue_value_destroy",
            IrContainerKind::Associative { .. } => "llg_assoc_value_destroy",
        };
        return format!("    {function}(&{});\n", container.c_name);
    }
    format!(
        "    {}_destroy(&{});\n",
        prefix(&container.kind),
        container.c_name
    )
}

fn prefix(kind: &IrContainerKind) -> &'static str {
    match kind {
        IrContainerKind::Dynamic => "llg_dyn",
        IrContainerKind::Queue { .. } => "llg_queue",
        IrContainerKind::Associative { .. } => "llg_assoc",
    }
}

fn method_code(method: IrContainerMethod) -> &'static str {
    match method {
        IrContainerMethod::Find => "LLG_CONTAINER_METHOD_FIND",
        IrContainerMethod::FindIndex => "LLG_CONTAINER_METHOD_FIND_INDEX",
        IrContainerMethod::FindFirst => "LLG_CONTAINER_METHOD_FIND_FIRST",
        IrContainerMethod::FindFirstIndex => "LLG_CONTAINER_METHOD_FIND_FIRST_INDEX",
        IrContainerMethod::FindLast => "LLG_CONTAINER_METHOD_FIND_LAST",
        IrContainerMethod::FindLastIndex => "LLG_CONTAINER_METHOD_FIND_LAST_INDEX",
        IrContainerMethod::Min => "LLG_CONTAINER_METHOD_MIN",
        IrContainerMethod::Max => "LLG_CONTAINER_METHOD_MAX",
        IrContainerMethod::Unique => "LLG_CONTAINER_METHOD_UNIQUE",
        IrContainerMethod::UniqueIndex => "LLG_CONTAINER_METHOD_UNIQUE_INDEX",
        IrContainerMethod::Sort => "LLG_CONTAINER_METHOD_SORT",
        IrContainerMethod::RSort => "LLG_CONTAINER_METHOD_RSORT",
        IrContainerMethod::Reverse => "LLG_CONTAINER_METHOD_REVERSE",
        IrContainerMethod::Shuffle => "LLG_CONTAINER_METHOD_SHUFFLE",
    }
}

fn c_type(kind: &IrContainerKind) -> &'static str {
    match kind {
        IrContainerKind::Dynamic => "llg_dyn_array_t",
        IrContainerKind::Queue { .. } => "llg_queue_t",
        IrContainerKind::Associative { .. } => "llg_assoc_t",
    }
}
