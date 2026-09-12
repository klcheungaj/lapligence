//! C rendering for non-integral objects. String expression results are owned.
use super::constants::c_string_literal;
use super::context::RCtx;
use super::expressions::render_expr_impl;
use crate::sim::ir::*;

pub(super) fn render_indices(ctx: &RCtx<'_>, indices: &[IrExpr]) -> Result<String, String> {
    let values = indices
        .iter()
        .map(|index| render_expr_impl(ctx, index).map(|value| value.code))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    Ok(format!("(const sv4_t[]){{ {values} }}"))
}

pub(super) fn string(ctx: &RCtx<'_>, value: &IrStringExpr) -> Result<String, String> {
    Ok(match value {
        IrStringExpr::Literal(bytes) => {
            let literal = bytes
                .iter()
                .map(|byte| format!("\\{:03o}", byte))
                .collect::<String>();
            format!("llg_string_bytes(\"{literal}\", {})", bytes.len())
        }
        IrStringExpr::RandomState => "llg_process_get_randstate()".to_owned(),
        IrStringExpr::Read(index) => {
            format!("llg_string_clone(&{})", ctx.model.objects[*index].c_name)
        }
        IrStringExpr::LocalRead(name) => format!("llg_string_clone(&{name})"),
        IrStringExpr::FormalRead(index) => {
            format!(
                "llg_string_clone({})",
                if ctx
                    .func
                    .map(|f| f.formals[*index].is_ref())
                    .unwrap_or(false)
                {
                    format!("r{index}")
                } else if ctx
                    .func
                    .map(|f| f.formals[*index].is_out())
                    .unwrap_or(false)
                {
                    format!("o{index}")
                } else {
                    format!("&a{index}")
                }
            )
        }
        IrStringExpr::ContainerGet { container, index } => format!(
            "{}(&{}, {})",
            match ctx.model.containers[*container].kind {
                crate::sim::ir::IrContainerKind::Dynamic => "llg_dyn_value_get_string",
                crate::sim::ir::IrContainerKind::Queue { .. } => "llg_queue_value_get_string",
                crate::sim::ir::IrContainerKind::Associative { .. } => {
                    return Err("string associative read requires a key-aware path".into());
                }
            },
            ctx.model.containers[*container].c_name,
            render_expr_impl(ctx, index)?.code
        ),
        IrStringExpr::ContainerGetNested { container, indices } => format!(
            "{}(&{}, {}, {})",
            match ctx.model.containers[*container].kind {
                crate::sim::ir::IrContainerKind::Dynamic => "llg_dyn_value_get_nested_string",
                crate::sim::ir::IrContainerKind::Queue { .. } =>
                    "llg_queue_value_get_nested_string",
                crate::sim::ir::IrContainerKind::Associative { .. } => {
                    return Err("nested associative string read requires a key-aware path".into());
                }
            },
            ctx.model.containers[*container].c_name,
            render_indices(ctx, indices)?,
            indices.len()
        ),
        IrStringExpr::AssociativeGet { container, key } => format!(
            "llg_model_assoc_value_get_string(&{}, {})",
            ctx.model.containers[*container].c_name,
            string(ctx, key)?
        ),
        IrStringExpr::Call {
            function,
            args,
            depth,
        } => {
            let mut rendered = Vec::with_capacity(args.len() + 1);
            for (arg, formal) in args.iter().zip(&ctx.model.funcs[*function].formals) {
                let code = render_expr_impl(ctx, arg)?.code;
                rendered.push(super::expressions::coerce_two_state(code, formal.two_state));
            }
            rendered.push(depth.code());
            format!(
                "{}({})",
                ctx.model.funcs[*function].c_name,
                rendered.join(", ")
            )
        }
        IrStringExpr::TypedCall {
            function,
            args,
            depth,
        } => render_typed_call(ctx, *function, args, *depth)?,
        IrStringExpr::Concat(parts) => {
            let mut value = "llg_string_bytes(\"\", 0)".to_owned();
            for part in parts {
                value = format!("llg_string_concat({value}, {})", string(ctx, part)?);
            }
            value
        }
        IrStringExpr::Repeat(value, count) => format!(
            "llg_string_repeat({}, {})",
            string(ctx, value)?,
            render_expr_impl(ctx, count)?.code
        ),
        IrStringExpr::FromPacked(value) => format!(
            "llg_string_from_packed({})",
            render_expr_impl(ctx, value)?.code
        ),
        IrStringExpr::EnumName { receiver, members } => {
            let receiver = render_expr_impl(ctx, receiver)?.code;
            let mut code = format!(
                "({{ sv4_t _llg_enum_name_value = {receiver}; \
                 llg_string_t _llg_enum_name_result = llg_string_bytes(\"\", 0); "
            );
            for member in members {
                let value = render_expr_impl(ctx, &member.value)?.code;
                let literal = member
                    .name
                    .iter()
                    .map(|byte| format!("\\{:03o}", byte))
                    .collect::<String>();
                code.push_str(&format!(
                    "if (sv4_to_bool(sv4_case_eq(_llg_enum_name_value, {value}))) {{ \
                     llg_string_destroy(&_llg_enum_name_result); \
                     _llg_enum_name_result = llg_string_bytes(\"{literal}\", {}); }} ",
                    member.name.len()
                ));
            }
            code.push_str("_llg_enum_name_result; })");
            code
        }
        IrStringExpr::Format {
            format,
            args,
            scope,
        } => render_string_format(ctx, format, args, scope)?,
        IrStringExpr::Case(value, upper) => format!(
            "llg_string_case({}, {})",
            string(ctx, value)?,
            u8::from(*upper)
        ),
        IrStringExpr::Substr(value, first, last) => format!(
            "llg_string_substr({}, {}, {})",
            string(ctx, value)?,
            render_expr_impl(ctx, first)?.code,
            render_expr_impl(ctx, last)?.code
        ),
    })
}

fn render_string_format(
    ctx: &RCtx<'_>,
    format: &IrStringExpr,
    args: &[IrDisplayArg],
    scope: &str,
) -> Result<String, String> {
    let format = string(ctx, format)?;
    let scope = c_string_literal(scope);
    if args.is_empty() {
        return Ok(format!(
            "llg_string_format_typed({format}, NULL, 0, {scope})"
        ));
    }

    // C does not specify the evaluation order of aggregate initializers.
    // Assign each argument in its own statement so calls with side effects
    // observe the same left-to-right order as the HDL source.
    let mut statements = Vec::with_capacity(args.len() + 3);
    statements.push(format!("llg_string_t _llg_format = {format};"));
    statements.push(format!(
        "llg_fmt_arg_t _llg_format_args[{}] = {{0}};",
        args.len()
    ));
    for (index, arg) in args.iter().enumerate() {
        let assignment = match arg {
            IrDisplayArg::Packed(value) => format!(
                "_llg_format_args[{index}].kind = LLG_FMT_PACKED;\n        _llg_format_args[{index}].value.packed = {};",
                render_expr_impl(ctx, value)?.code
            ),
            IrDisplayArg::Real(value) => format!(
                "_llg_format_args[{index}].kind = LLG_FMT_REAL;\n        _llg_format_args[{index}].value.real = {};",
                render_expr_impl(ctx, value)?.code
            ),
            IrDisplayArg::String(value) => format!(
                "_llg_format_args[{index}].kind = LLG_FMT_STRING;\n        _llg_format_args[{index}].value.string = {};",
                string(ctx, value)?
            ),
        };
        statements.push(assignment);
    }
    statements.push(format!(
        "llg_string_t _llg_format_result = llg_string_format_typed(_llg_format, _llg_format_args, {}, {scope});",
        args.len()
    ));
    statements.push("_llg_format_result;".to_owned());
    Ok(format!("({{ {} }})", statements.join(" ")))
}

pub(super) fn chandle(ctx: &RCtx<'_>, value: &IrChandleExpr) -> Result<String, String> {
    Ok(match value {
        IrChandleExpr::Null => "NULL".to_owned(),
        IrChandleExpr::Read(index) => ctx.model.objects[*index].c_name.clone(),
        IrChandleExpr::LocalRead(name) => name.clone(),
        IrChandleExpr::FormalRead(index) => {
            let Some(function) = ctx.func else {
                return Ok(format!("a{index}"));
            };
            let Some(formal) = function.formals.get(*index) else {
                return Ok(format!("a{index}"));
            };
            if formal.is_ref() {
                format!("*r{index}")
            } else if formal.is_out {
                format!("*o{index}")
            } else {
                format!("a{index}")
            }
        }
        IrChandleExpr::ContainerGet { container, index } => format!(
            "{}(&{}, {})",
            match ctx.model.containers[*container].kind {
                crate::sim::ir::IrContainerKind::Dynamic => "llg_dyn_value_get_chandle",
                crate::sim::ir::IrContainerKind::Queue { .. } => "llg_queue_value_get_chandle",
                crate::sim::ir::IrContainerKind::Associative { .. } => {
                    return Err("chandle associative read requires a key-aware path".into());
                }
            },
            ctx.model.containers[*container].c_name,
            render_expr_impl(ctx, index)?.code
        ),
        IrChandleExpr::ContainerGetNested { container, indices } => format!(
            "{}(&{}, {}, {})",
            match ctx.model.containers[*container].kind {
                crate::sim::ir::IrContainerKind::Dynamic => "llg_dyn_value_get_nested_chandle",
                crate::sim::ir::IrContainerKind::Queue { .. } =>
                    "llg_queue_value_get_nested_chandle",
                crate::sim::ir::IrContainerKind::Associative { .. } => {
                    return Err("nested associative chandle read requires a key-aware path".into());
                }
            },
            ctx.model.containers[*container].c_name,
            render_indices(ctx, indices)?,
            indices.len()
        ),
        IrChandleExpr::AssociativeGet { container, key } => format!(
            "llg_model_assoc_value_get_chandle(&{}, {})",
            ctx.model.containers[*container].c_name,
            string(ctx, key)?
        ),
        IrChandleExpr::Call {
            function,
            args,
            depth,
        } => {
            let mut args = args
                .iter()
                .map(|arg| match arg {
                    IrCallArg::ChandleVal(value) => chandle(ctx, value),
                    IrCallArg::Val(value) => render_expr_impl(ctx, value).map(|value| value.code),
                    IrCallArg::ChandleAddr(addr)
                    | IrCallArg::ChandleRefAddr(addr)
                    | IrCallArg::StringOutAddr(addr)
                    | IrCallArg::StringRefAddr { addr, .. }
                    | IrCallArg::OutAddr(addr) => Ok(addr.clone()),
                    IrCallArg::RefAddr { addr, .. } => Ok(addr.clone()),
                    IrCallArg::OutTemp { name, .. } => Ok(format!("&{name}")),
                    IrCallArg::StringVal(_) | IrCallArg::StringOutTemp { .. } => {
                        Err("string argument is invalid in a chandle call".to_owned())
                    }
                })
                .collect::<Result<Vec<_>, String>>()?;
            args.push(depth.code());
            format!("{}({})", ctx.model.funcs[*function].c_name, args.join(", "))
        }
    })
}

fn string_inside(
    ctx: &RCtx<'_>,
    value: &IrStringExpr,
    items: &[IrStringInsideItem],
) -> Result<String, String> {
    let selector = string(ctx, value)?;
    let zero = "SV4_C(0, 32)";
    let mut code = format!(
        "({{ llg_string_t _inside_string_value = {selector}; \
         sv4_t _inside_string_result = SV4_C(0, 1); "
    );
    for (index, item) in items.iter().enumerate() {
        match item {
            IrStringInsideItem::Value(item) => {
                let item_code = string(ctx, item)?;
                code.push_str(&format!(
                    "llg_string_t _inside_string_item_{index} = {item_code}; \
                     sv4_t _inside_string_cmp_{index} = llg_string_compare( \
                     llg_string_clone(&_inside_string_value), _inside_string_item_{index}, 0); \
                     _inside_string_result = sv4_logor(_inside_string_result, \
                     sv4_eq(_inside_string_cmp_{index}, {zero})); "
                ));
            }
            IrStringInsideItem::Range { low, high } => {
                let low_code = string(ctx, low)?;
                let high_code = string(ctx, high)?;
                code.push_str(&format!(
                    "llg_string_t _inside_string_low_{index} = {low_code}; \
                     llg_string_t _inside_string_high_{index} = {high_code}; \
                     sv4_t _inside_string_low_cmp_{index} = llg_string_compare( \
                     llg_string_clone(&_inside_string_value), _inside_string_low_{index}, 0); \
                     sv4_t _inside_string_high_cmp_{index} = llg_string_compare( \
                     llg_string_clone(&_inside_string_value), _inside_string_high_{index}, 0); \
                     _inside_string_result = sv4_logor(_inside_string_result, \
                     sv4_logand(sv4_ge(_inside_string_low_cmp_{index}, {zero}), \
                     sv4_le(_inside_string_high_cmp_{index}, {zero}))); "
                ));
            }
        }
    }
    code.push_str("llg_string_destroy(&_inside_string_value); _inside_string_result; })");
    Ok(code)
}

pub(super) fn query(
    ctx: &RCtx<'_>,
    query: &IrObjectQuery,
    width: u32,
    signed: bool,
) -> Result<String, String> {
    Ok(match query {
        IrObjectQuery::StringLen(value) => format!("llg_string_len({})", string(ctx, value)?),
        IrObjectQuery::StringGetc(value, index) => format!(
            "llg_string_getc({}, {})",
            string(ctx, value)?,
            render_expr_impl(ctx, index)?.code
        ),
        IrObjectQuery::StringCompare(a, b, ignore) => format!(
            "llg_string_compare({}, {}, {})",
            string(ctx, a)?,
            string(ctx, b)?,
            u8::from(*ignore)
        ),
        IrObjectQuery::StringInside { value, items } => string_inside(ctx, value, items)?,
        IrObjectQuery::StringAtoi(value, base) => {
            format!("llg_string_atoi({}, {base})", string(ctx, value)?)
        }
        IrObjectQuery::StringAtoreal(value) => {
            format!("llg_string_atoreal({})", string(ctx, value)?)
        }
        IrObjectQuery::StringPacked(value) => format!(
            "llg_string_to_packed({}, {width}, {})",
            string(ctx, value)?,
            u8::from(signed)
        ),
        IrObjectQuery::ChandleEq(a, b) => format!(
            "sv4_from_u64({} == {}, 1, 0)",
            chandle(ctx, a)?,
            chandle(ctx, b)?
        ),
        IrObjectQuery::ArrayQuery(query) => array_query(ctx, query, width, signed)?,
    })
}

fn static_query_value(kind: IrArrayQueryKind, dimension: IrArrayDimension) -> Option<i128> {
    let (Some(left), Some(right)) = (dimension.left, dimension.right) else {
        return None;
    };
    match kind {
        IrArrayQueryKind::Left => Some(left),
        IrArrayQueryKind::Right => Some(right),
        IrArrayQueryKind::Low => Some(left.min(right)),
        IrArrayQueryKind::High => Some(left.max(right)),
        IrArrayQueryKind::Increment => Some(if left >= right { 1 } else { -1 }),
        IrArrayQueryKind::Size => left
            .checked_sub(right)
            .and_then(|extent| extent.unsigned_abs().checked_add(1))
            .and_then(|size| i128::try_from(size).ok()),
    }
}

fn static_query_code(
    kind: IrArrayQueryKind,
    dimension: IrArrayDimension,
    width: u32,
    signed: bool,
) -> Option<String> {
    static_query_value(kind, dimension).map(|value| {
        format!(
            "sv4_from_u64((uint64_t)({value}), {width}, {})",
            u8::from(signed)
        )
    })
}

fn dynamic_query_code(
    ctx: &RCtx<'_>,
    kind: IrArrayQueryKind,
    target: &IrArrayQueryTarget,
) -> Result<String, String> {
    Ok(match target {
        IrArrayQueryTarget::Container { container, .. } => {
            let container_model = ctx
                .model
                .containers
                .get(*container)
                .ok_or_else(|| "array query container index is out of bounds".to_owned())?;
            let name = &container_model.c_name;
            match &container_model.kind {
                IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => {
                    let size_fn = match &container_model.kind {
                        IrContainerKind::Dynamic => {
                            if container_model.element.is_packed() {
                                "llg_dyn_size"
                            } else {
                                "llg_dyn_value_size"
                            }
                        }
                        IrContainerKind::Queue { .. } => "llg_queue_size",
                        IrContainerKind::Associative { .. } => unreachable!(),
                    };
                    let size = format!("sv4_from_u64((uint64_t){size_fn}(&{name}), 32, 1)");
                    match kind {
                        IrArrayQueryKind::Left | IrArrayQueryKind::Low => {
                            "sv4_from_u64(0, 32, 1)".to_owned()
                        }
                        IrArrayQueryKind::Right | IrArrayQueryKind::High => {
                            format!("sv4_sub({size}, sv4_from_u64(1, 32, 1))")
                        }
                        IrArrayQueryKind::Size => size,
                        IrArrayQueryKind::Increment => {
                            "sv4_from_u64((uint64_t)-1, 32, 1)".to_owned()
                        }
                    }
                }
                IrContainerKind::Associative {
                    key:
                        IrAssocKey::Integral {
                            width: key_width,
                            signed: key_signed,
                            ..
                        },
                } => {
                    let zero = format!("sv4_from_u64(0, {key_width}, {})", u8::from(*key_signed));
                    match kind {
                        IrArrayQueryKind::Left => zero,
                        IrArrayQueryKind::Right => {
                            format!("sv4_fill(1, {key_width}, {})", u8::from(*key_signed))
                        }
                        IrArrayQueryKind::Low | IrArrayQueryKind::High => {
                            let traversal = if kind == IrArrayQueryKind::Low {
                                "llg_assoc_first_integral"
                            } else {
                                "llg_assoc_last_integral"
                            };
                            format!(
                                "({{ sv4_t _llg_qkey = {zero}; {traversal}(&{name}, &_llg_qkey) ? _llg_qkey : sv4_fill(2, {key_width}, {}) ; }})",
                                u8::from(*key_signed)
                            )
                        }
                        IrArrayQueryKind::Size => format!(
                            "sv4_from_u64((uint64_t)llg_assoc_count(&{name}), {key_width}, {})",
                            u8::from(*key_signed)
                        ),
                        IrArrayQueryKind::Increment => {
                            "sv4_from_u64((uint64_t)-1, 32, 1)".to_owned()
                        }
                    }
                }
                IrContainerKind::Associative {
                    key: IrAssocKey::String | IrAssocKey::Wildcard,
                } => return Err(
                    "array query on a string-keyed or wildcard associative array is unsupported"
                        .to_owned(),
                ),
            }
        }
        IrArrayQueryTarget::String { value, .. } => {
            let len = format!("llg_string_len({})", string(ctx, value)?);
            match kind {
                IrArrayQueryKind::Left | IrArrayQueryKind::Low => {
                    "sv4_from_u64(0, 32, 1)".to_owned()
                }
                IrArrayQueryKind::Right | IrArrayQueryKind::High => {
                    format!("sv4_sub({len}, sv4_from_u64(1, 32, 1))")
                }
                IrArrayQueryKind::Size => len,
                IrArrayQueryKind::Increment => "sv4_from_u64((uint64_t)-1, 32, 1)".to_owned(),
            }
        }
        IrArrayQueryTarget::Static { .. } => {
            return Err("runtime query selected a fixed dimension without bounds".to_owned())
        }
    })
}

fn array_query(
    ctx: &RCtx<'_>,
    query: &IrArrayQuery,
    width: u32,
    signed: bool,
) -> Result<String, String> {
    let dimensions = match &query.target {
        IrArrayQueryTarget::Static { dimensions }
        | IrArrayQueryTarget::Container { dimensions, .. }
        | IrArrayQueryTarget::String { dimensions, .. } => dimensions,
    };
    if dimensions.is_empty() {
        return Err("array query target has no dimensions".to_owned());
    }
    let selected = |index: usize| {
        static_query_code(query.kind, dimensions[index], width, signed)
            .or_else(|| dynamic_query_code(ctx, query.kind, &query.target).ok())
    };
    if let Some(dimension) = &query.dimension {
        let dimension = render_expr_impl(ctx, dimension)?.code;
        let unknown = format!("sv4_fill(2, {width}, {})", u8::from(signed));
        let mut choices = unknown.clone();
        for index in (0..dimensions.len()).rev() {
            let value = selected(index)
                .ok_or_else(|| "array query dimension has no representable result".to_owned())?;
            choices = format!("(_llg_qindex == {} ? {value} : {choices})", index + 1);
        }
        return Ok(format!(
            "({{ sv4_t _llg_qdim = {dimension}; int64_t _llg_qindex = 0; int _llg_qvalid = sv4_to_index_i64(_llg_qdim, &_llg_qindex); (!_llg_qvalid || _llg_qindex < 1 || _llg_qindex > {}) ? {unknown} : {choices}; }})",
            dimensions.len()
        ));
    }
    static_query_code(query.kind, dimensions[0], width, signed)
        .or_else(|| dynamic_query_code(ctx, query.kind, &query.target).ok())
        .ok_or_else(|| "array query dimension has no representable result".to_owned())
}

pub(super) fn statement(ctx: &RCtx<'_>, operation: &IrObjectStmt) -> Result<String, String> {
    Ok(match operation {
        IrObjectStmt::StringPrint(value) => {
            format!("    llg_string_print({});\n", string(ctx, value)?)
        }
        IrObjectStmt::StringAssign(index, value) => format!(
            "    llg_string_move(&{}, {});\n",
            ctx.model.objects[*index].c_name,
            string(ctx, value)?
        ),
        IrObjectStmt::StringAssignLocal(target, value) => {
            format!("    llg_string_move(&{target}, {});\n", string(ctx, value)?)
        }
        IrObjectStmt::StringPutc(index, position, value) => format!(
            "    llg_string_putc(&{}, {}, {});\n",
            ctx.model.objects[*index].c_name,
            render_expr_impl(ctx, position)?.code,
            render_expr_impl(ctx, value)?.code
        ),
        IrObjectStmt::StringItoa(index, value, base) => format!(
            "    llg_string_itoa(&{}, {}, {base});\n",
            ctx.model.objects[*index].c_name,
            render_expr_impl(ctx, value)?.code
        ),
        IrObjectStmt::StringRealtoa(index, value) => format!(
            "    llg_string_realtoa(&{}, {});\n",
            ctx.model.objects[*index].c_name,
            render_expr_impl(ctx, value)?.code
        ),
        IrObjectStmt::StringPutcLocal(target, position, value) => format!(
            "    llg_string_putc(&{target}, {}, {});\n",
            render_expr_impl(ctx, position)?.code,
            render_expr_impl(ctx, value)?.code
        ),
        IrObjectStmt::StringItoaLocal(target, value, base) => format!(
            "    llg_string_itoa(&{target}, {}, {base});\n",
            render_expr_impl(ctx, value)?.code
        ),
        IrObjectStmt::StringRealtoaLocal(target, value) => format!(
            "    llg_string_realtoa(&{target}, {});\n",
            render_expr_impl(ctx, value)?.code
        ),
        IrObjectStmt::ChandleDeclareLocal(name, value) => format!(
            "    void *{name} = {};\n",
            value
                .as_ref()
                .map(|value| chandle(ctx, value))
                .transpose()?
                .unwrap_or_else(|| "NULL".to_owned())
        ),
        IrObjectStmt::ChandleAssign(index, value) => format!(
            "    {} = {};\n",
            ctx.model.objects[*index].c_name,
            chandle(ctx, value)?
        ),
        IrObjectStmt::ChandleAssignLocal(target, value) => {
            format!("    {target} = {};\n", chandle(ctx, value)?)
        }
    })
}

fn render_typed_call(
    ctx: &RCtx<'_>,
    function: usize,
    args: &[IrCallArg],
    depth: IrDepth,
) -> Result<String, String> {
    let f = ctx.model.func(function);
    let order = f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
        .chain(
            f.formals
                .iter()
                .enumerate()
                .filter(|(_, formal)| !formal.is_address()),
        );
    let mut rendered = Vec::new();
    let mut temps = Vec::new();
    for ((idx, _formal), arg) in order.zip(args) {
        let value = match arg {
            IrCallArg::StringVal(value) => string(ctx, value)?,
            IrCallArg::Val(value) => render_expr_impl(ctx, value)?.code,
            IrCallArg::StringOutAddr(addr) | IrCallArg::StringRefAddr { addr, .. } => addr.clone(),
            IrCallArg::OutAddr(addr) | IrCallArg::RefAddr { addr, .. } => addr.clone(),
            IrCallArg::StringOutTemp {
                name,
                init,
                writeback,
                storage_addr,
                storage_read,
            } => {
                temps.push((
                    name,
                    init.as_deref(),
                    writeback,
                    storage_addr.as_deref(),
                    storage_read.as_deref(),
                ));
                storage_addr.clone().unwrap_or_else(|| format!("&{name}"))
            }
            IrCallArg::OutTemp {
                name, storage_addr, ..
            } => storage_addr.clone().unwrap_or_else(|| format!("&{name}")),
            IrCallArg::ChandleVal(value) => super::objects::chandle(ctx, value)?,
            IrCallArg::ChandleAddr(addr) | IrCallArg::ChandleRefAddr(addr) => addr.clone(),
        };
        rendered.push(value);
        let _ = idx;
    }
    rendered.push(depth.code());
    let call = format!("{}({})", f.c_name, rendered.join(", "));
    if temps.is_empty() {
        return Ok(call);
    }
    let mut parts = Vec::new();
    for (name, init, _, storage_addr, _) in &temps {
        let init = init
            .map(|value| string(ctx, value))
            .transpose()?
            .unwrap_or_else(|| "(llg_string_t){0}".to_owned());
        parts.push(format!("llg_string_t {name} = {init}"));
        if let Some(storage_addr) = storage_addr {
            parts.push(format!(
                "llg_string_move({storage_addr}, llg_string_clone(&{name}))"
            ));
        }
    }
    parts.push(format!("llg_string_t _llg_string_ret = {call}"));
    for (name, _, writeback, storage_addr, storage_read) in &temps {
        if let Some(read) = storage_read {
            let value = string(ctx, read)?;
            parts.push(format!("llg_string_move(&{name}, {value})"));
            parts.push(format!(
                "llg_string_move({writeback}, llg_string_clone(&{name}))"
            ));
            parts.push(format!("llg_string_destroy(&{name})"));
        } else {
            let _ = storage_addr;
            parts.push(format!("llg_string_move({writeback}, {name})"));
        }
    }
    parts.push("_llg_string_ret".to_owned());
    Ok(format!("({{ {}; }})", parts.join("; ")))
}
