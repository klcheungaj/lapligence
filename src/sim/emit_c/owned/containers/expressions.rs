//! Container calls consume only explicitly prepared operand snapshots.
use super::*;
pub(super) fn render(frame: &mut Frame<'_, '_>, operation: &IrContainerExpr, owners: &mut Vec<Value>, strings: &mut Vec<NativeValue>) -> Result<String, String> {
    let ctx = frame.ctx;
    Ok(match operation {
        IrContainerExpr::Stream {
            container,
            slice,
            direction,
            selector,
        } => {
            let (selector_kind, first, second) = stream_selector(frame, owners, selector.as_ref())?;
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
                operand(frame, owners, index)?.code
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
            operand(frame, owners, index)?.code
        ),
        IrContainerExpr::GetNested { container, indices } => format!(
            "{}(&{}, {}, {})",
            match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_value_get_nested",
                IrContainerKind::Queue { .. } => "llg_queue_value_get_nested",
                IrContainerKind::Associative { .. } => "llg_assoc_value_get_nested_integral",
            },
            name(ctx, *container),
            super::indices(frame, owners, indices)?,
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
            super::indices(frame, owners, indices)?,
            indices.len()
        ),
        IrContainerExpr::GetString { container, key } => format!(
            "llg_owned_assoc_get_string(&{}, {})",
            name(ctx, *container),
            key_operand(frame, strings, key)?
        ),
        IrContainerExpr::GetStringReal { container, key } => format!(
            "llg_owned_assoc_value_get_real(&{}, {})",
            name(ctx, *container),
            key_operand(frame, strings, key)?
        ),
        IrContainerExpr::Exists { container, key } => format!(
            "sv4_from_u64({}(&{}, {}), 32, 1)",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_assoc_exists_integral"
            } else {
                "llg_assoc_value_exists_integral"
            },
            name(ctx, *container),
            operand(frame, owners, key)?.code
        ),
        IrContainerExpr::ExistsString { container, key } => format!(
            "sv4_from_u64({}(&{}, {}), 32, 1)",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_owned_assoc_exists_string"
            } else {
                "llg_owned_assoc_value_exists_string"
            },
            name(ctx, *container),
            key_operand(frame, strings, key)?
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
                frame.address(key_address)?.address
            )
        }
        IrContainerExpr::AssocTraverseString {
            container,
            direction,
            key_object,
        } => format!(
            "sv4_from_u64({}(&{}, {}, {}), 32, 1)",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_owned_assoc_traverse_string"
            } else {
                "llg_owned_assoc_value_traverse_string"
            },
            name(ctx, *container),
            format!("&{}", ctx.model.objects[*key_object].c_name),
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
            "sv4_from_u64({}(&{}, {}, {}), 32, 1)",
            if ctx.model.containers[*container].element.is_packed() {
                "llg_owned_assoc_traverse_string"
            } else {
                "llg_owned_assoc_value_traverse_string"
            },
            name(ctx, *container),
            frame.native_lookup(key_name, NativeKind::String)?.address,
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
