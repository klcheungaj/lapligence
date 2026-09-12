//! C rendering for dynamically sized unpacked containers.

use super::context::RCtx;
use super::expressions::render_expr_impl;
use super::objects::string as render_string;
use crate::sim::ir::{
    IrAssocKey, IrAssocTraversal, IrContainerExpr, IrContainerKind, IrContainerReduction,
    IrContainerStmt, IrType,
};

fn name<'a>(ctx: &'a RCtx<'_>, index: usize) -> &'a str {
    &ctx.model.containers[index].c_name
}

pub(super) fn expression(ctx: &RCtx<'_>, operation: &IrContainerExpr) -> Result<String, String> {
    Ok(match operation {
        IrContainerExpr::Size(index) => {
            let method = match ctx.model.containers[*index].kind {
                IrContainerKind::Dynamic => "llg_dyn_size",
                IrContainerKind::Queue { .. } => "llg_queue_size",
                IrContainerKind::Associative { .. } => "llg_assoc_count",
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
        IrContainerExpr::Get { container, index } => {
            let method = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_get",
                IrContainerKind::Queue { .. } => "llg_queue_get",
                IrContainerKind::Associative {
                    key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard,
                } => "llg_assoc_get_integral",
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
        IrContainerExpr::GetString { container, key } => format!(
            "llg_model_assoc_get_string(&{}, {})",
            name(ctx, *container),
            render_string(ctx, key)?
        ),
        IrContainerExpr::Exists { container, key } => format!(
            "sv4_from_u64(llg_assoc_exists_integral(&{}, {}), 32, 1)",
            name(ctx, *container),
            render_expr_impl(ctx, key)?.code
        ),
        IrContainerExpr::ExistsString { container, key } => format!(
            "sv4_from_u64(llg_model_assoc_exists_string(&{}, {}), 32, 1)",
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
            "sv4_from_u64(llg_model_assoc_traverse_string(&{}, &{}, {}), 32, 1)",
            name(ctx, *container),
            ctx.model.objects[*key_object].c_name,
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
        IrContainerStmt::DynamicNew {
            container,
            size,
            initializer,
        } => format!(
            "    llg_dyn_new(&{}, {}, {});\n",
            name(ctx, *container),
            render_expr_impl(ctx, size)?.code,
            initializer
                .map(|index| format!("&{}", name(ctx, index)))
                .unwrap_or_else(|| "NULL".to_owned())
        ),
        IrContainerStmt::Copy { dst, src } => format!(
            "    {}_copy(&{}, &{});\n",
            prefix(&ctx.model.containers[*dst].kind),
            name(ctx, *dst),
            name(ctx, *src)
        ),
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
        IrContainerStmt::Delete(index) => format!(
            "    {}_delete(&{});\n",
            prefix(&ctx.model.containers[*index].kind),
            name(ctx, *index)
        ),
        IrContainerStmt::Set {
            container,
            index,
            value,
        } => {
            let method = match ctx.model.containers[*container].kind {
                IrContainerKind::Dynamic => "llg_dyn_set",
                IrContainerKind::Queue { .. } => "llg_queue_set",
                IrContainerKind::Associative {
                    key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard,
                } => "llg_assoc_set_integral",
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
        IrContainerStmt::SetDefault { container, value } => format!(
            "    llg_assoc_set_default(&{}, {});\n",
            name(ctx, *container),
            render_expr_impl(ctx, value)?.code
        ),
        IrContainerStmt::ResetDefault(container) => format!(
            "    llg_assoc_reset_default(&{});\n",
            name(ctx, *container)
        ),
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
        IrContainerStmt::QueuePushFront { container, value } => format!(
            "    llg_queue_push_front(&{}, {});\n",
            name(ctx, *container),
            render_expr_impl(ctx, value)?.code
        ),
        IrContainerStmt::QueuePushBack { container, value } => format!(
            "    llg_queue_push_back(&{}, {});\n",
            name(ctx, *container),
            render_expr_impl(ctx, value)?.code
        ),
        IrContainerStmt::QueueInsert {
            container,
            index,
            value,
        } => format!(
            "    (void)llg_queue_insert(&{}, {}, {});\n",
            name(ctx, *container),
            render_expr_impl(ctx, index)?.code,
            render_expr_impl(ctx, value)?.code
        ),
        IrContainerStmt::DeleteIndex { container, index } => {
            let method = match ctx.model.containers[*container].kind {
                IrContainerKind::Queue { .. } => "llg_queue_delete_index",
                IrContainerKind::Associative { .. } => "llg_assoc_delete_integral",
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
            "    (void)llg_model_assoc_delete_string(&{}, {});\n",
            name(ctx, *container),
            render_string(ctx, key)?
        ),
    })
}

/// Adapters consume owned string-expression results while the container runtime
/// remains independent of the string runtime's representation.
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
     }\n\n"
}

pub(super) fn declaration_and_init(
    container: &crate::sim::ir::IrContainer,
) -> Result<(String, String), String> {
    let IrType::Packed {
        width,
        signed,
        two_state,
    } = container.element
    else {
        return Err("container element must be packed".into());
    };
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

fn c_type(kind: &IrContainerKind) -> &'static str {
    match kind {
        IrContainerKind::Dynamic => "llg_dyn_array_t",
        IrContainerKind::Queue { .. } => "llg_queue_t",
        IrContainerKind::Associative { .. } => "llg_assoc_t",
    }
}
