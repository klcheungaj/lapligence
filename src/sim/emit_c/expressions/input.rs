//! Input.

use super::*;

fn render_plusarg_text(
    ctx: &RCtx<'_>,
    text: &IrPlusArgText,
    name: &str,
) -> Result<(String, String, String), String> {
    match text {
        IrPlusArgText::Literal(text) => Ok((c_string_literal(text), String::new(), String::new())),
        IrPlusArgText::Dynamic(value) => {
            let value = super::super::objects::string(ctx, value)?;
            Ok((
                format!("({name}.data ? {name}.data : \"\")"),
                format!("llg_string_t {name} = {value}; "),
                format!("llg_string_destroy(&{name}); "),
            ))
        }
    }
}

fn render_file_lhs_ref(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    width: u32,
    signed: bool,
    two_state: bool,
) -> Result<(String, String), String> {
    render_file_lhs_ref_with_prefix(ctx, lhs, width, signed, two_state, "_llg_mut_idx")
}

fn render_file_lhs_ref_with_prefix(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    width: u32,
    signed: bool,
    two_state: bool,
    index_prefix: &str,
) -> Result<(String, String), String> {
    let (declarations, lhs) = capture_lhs_indices_with_prefix(ctx, lhs, index_prefix)?;
    let init = match lhs {
        IrLhs::Whole(index) => {
            let signal = ctx.model.signal(index);
            if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                return Err("file input target must be packed variable storage".to_owned());
            }
            format!(
                "&(llg_ref_t){{ .base = &{}, .width = {}, .is_signed = {}, .two_state = {}, .kind = LLG_REF_WHOLE }}",
                signal.c_name, width, signed as u8, two_state as u8
            )
        }
        IrLhs::WholeRef { addr, width: lhs_width, .. } if lhs_width != 0 => format!(
            "&(llg_ref_t){{ .base = {}, .width = {}, .is_signed = {}, .two_state = {}, .kind = LLG_REF_WHOLE }}",
            addr, width, signed as u8, two_state as u8
        ),
        IrLhs::Ref { bit: Some(_), .. } => {
            return Err("file input through a selected ref formal is not supported".to_owned());
        }
        IrLhs::Ref { addr, const_ref, .. } => {
            if const_ref {
                return Err("file input target cannot be a const ref".to_owned());
            }
            addr
        }
        IrLhs::Bit(index, select, _) => {
            let signal = ctx.model.signal(index);
            if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                return Err("file input target must be packed variable storage".to_owned());
            }
            let select = render_expr_impl(ctx, &select)?.code;
            format!(
                "&(llg_ref_t){{ .base = &{}, .width = 1, .is_signed = 0, .two_state = {}, .kind = LLG_REF_BIT, .index = sv4_to_index({select}) }}",
                signal.c_name, two_state as u8
            )
        }
        IrLhs::Part(index, left, right, _) => {
            let signal = ctx.model.signal(index);
            if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                return Err("file input target must be packed variable storage".to_owned());
            }
            format!(
                "&(llg_ref_t){{ .base = &{}, .width = {}, .is_signed = 0, .two_state = {}, .kind = LLG_REF_PART, .left = {}, .right = {} }}",
                signal.c_name, width, two_state as u8, left, right
            )
        }
        IrLhs::IdxPart(index, base, _, selected_width, negative, _) => {
            let signal = ctx.model.signal(index);
            if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                return Err("file input target must be packed variable storage".to_owned());
            }
            let base = render_expr_impl(ctx, &base)?.code;
            format!(
                "&(llg_ref_t){{ .base = &{}, .width = {}, .is_signed = 0, .two_state = {}, .kind = LLG_REF_INDEXED, .index = sv4_to_index({base}), .indexed_width = {}, .indexed_negative = {} }}",
                signal.c_name, selected_width, two_state as u8, selected_width, negative as u8
            )
        }
        IrLhs::ArrayElem {
            arr,
            indices,
            elem_sel: IrElemSel::Whole,
        } => {
            let array = ctx.model.array(arr);
            if array.real {
                return Err("file input target array element must be packed".to_owned());
            }
            let index_codes = indices
                .iter()
                .map(|index| render_expr_impl(ctx, index).map(|value| value.code))
                .collect::<Result<Vec<_>, _>>()?;
            let index = match array_guard(array, &index_codes) {
                Some((decls, condition, linear)) => format!(
                    "({{ {decls} ({condition}) ? (uint64_t)({linear}) : UINT64_MAX; }})"
                ),
                None => "0ULL".to_owned(),
            };
            format!(
                "&(llg_ref_t){{ .base = {}, .width = {}, .is_signed = {}, .two_state = {}, .kind = LLG_REF_ARRAY, .index = {index}, .array_size = {}ULL }}",
                array.c_name, width, signed as u8, two_state as u8, array.total
            )
        }
        IrLhs::ArrayElem { .. } => {
            return Err("file input target does not support an element select".to_owned());
        }
        IrLhs::WholeRef { .. } | IrLhs::Stream { .. } => {
            return Err("file input target requires packed writable storage".to_owned());
        }
    };
    Ok((declarations, init))
}

fn render_file_real_target(ctx: &RCtx<'_>, lhs: &IrLhs) -> Result<String, String> {
    match lhs {
        IrLhs::Whole(index) => {
            let signal = ctx.model.signal(*index);
            if matches!(signal.ty, IrType::Real { .. }) {
                Ok(format!("&{}", signal.c_name))
            } else {
                Err("file input real target is not real storage".to_owned())
            }
        }
        IrLhs::WholeRef { addr, width: 0, .. } => Ok(addr.clone()),
        _ => Err("file input real target requires whole real storage".to_owned()),
    }
}

fn render_file_input_target_with_prefix(
    ctx: &RCtx<'_>,
    target: &IrFileInputTarget,
    index_prefix: &str,
) -> Result<(String, String), String> {
    match target {
        IrFileInputTarget::Packed {
            lhs,
            width,
            signed,
            two_state,
        } => {
            let (declarations, descriptor) = render_file_lhs_ref_with_prefix(
                ctx,
                lhs,
                *width,
                *signed,
                *two_state,
                index_prefix,
            )?;
            Ok((
                declarations,
                format!("{{ .kind = LLG_FILE_INPUT_PACKED, .packed = {descriptor} }}"),
            ))
        }
        IrFileInputTarget::Real { lhs, .. } => Ok((
            String::new(),
            format!(
                "{{ .kind = LLG_FILE_INPUT_REAL, .real = {}, .shortreal = {} }}",
                render_file_real_target(ctx, lhs)?,
                matches!(
                    target,
                    IrFileInputTarget::Real {
                        shortreal: true,
                        ..
                    }
                ) as u8
            ),
        )),
        IrFileInputTarget::String { address } => Ok((
            String::new(),
            format!("{{ .kind = LLG_FILE_INPUT_STRING, .string = {address} }}"),
        )),
    }
}

fn render_file_input_targets(
    ctx: &RCtx<'_>,
    targets: &[IrFileInputTarget],
) -> Result<(String, String), String> {
    let mut declarations = String::new();
    let mut rendered = Vec::with_capacity(targets.len());
    for (index, target) in targets.iter().enumerate() {
        let index_prefix = format!("_llg_file_input_idx{index}_");
        let (setup, target) = render_file_input_target_with_prefix(ctx, target, &index_prefix)?;
        declarations.push_str(&setup);
        rendered.push(target);
    }
    let array = if rendered.is_empty() {
        "NULL".to_owned()
    } else {
        format!(
            "(const llg_file_input_target_t[]){{ {} }}",
            rendered.join(", ")
        )
    };
    Ok((declarations, array))
}

pub(super) fn render_file_input(
    ctx: &RCtx<'_>,
    input: &IrFileInput,
) -> Result<RenderedExpr, String> {
    let result = |code: String| RenderedExpr {
        code,
        width: 32,
        signed: true,
        fill: None,
    };
    let code = match input {
        IrFileInput::Getc { descriptor } => {
            let descriptor = render_expr_impl(ctx, descriptor)?;
            format!(
                "sv4_from_i64((int64_t)llg_file_getc(llg_file_descriptor({})), 32)",
                descriptor.code
            )
        }
        IrFileInput::Ungetc {
            character,
            descriptor,
        } => {
            let character = render_expr_impl(ctx, character)?;
            let descriptor = render_expr_impl(ctx, descriptor)?;
            format!(
                "sv4_from_i64((int64_t)llg_file_ungetc(llg_file_descriptor({}), {}), 32)",
                descriptor.code, character.code
            )
        }
        IrFileInput::Gets { descriptor, target } => {
            let descriptor = render_expr_impl(ctx, descriptor)?;
            match target {
                IrFileInputTarget::String { address } => format!(
                    "sv4_from_i64((int64_t)llg_file_gets(llg_file_descriptor({}), {address}), 32)",
                    descriptor.code
                ),
                IrFileInputTarget::Packed {
                    lhs,
                    width,
                    signed,
                    two_state,
                } => {
                    let (setup, target) =
                        render_file_lhs_ref(ctx, lhs, *width, *signed, *two_state)?;
                    format!(
                        "({{ {setup} sv4_from_i64((int64_t)llg_file_gets_packed(llg_file_descriptor({}), {target}), 32); }})",
                        descriptor.code
                    )
                }
                IrFileInputTarget::Real { .. } => {
                    return Err("file line input target cannot be real storage".to_owned());
                }
            }
        }
        IrFileInput::ScanFile {
            descriptor,
            format,
            targets,
        } => {
            let descriptor = render_expr_impl(ctx, descriptor)?;
            let (format, setup, cleanup) =
                render_plusarg_text(ctx, format, "_llg_file_input_format")?;
            let (target_setup, target_array) = render_file_input_targets(ctx, targets)?;
            format!(
                "({{ {setup}{target_setup} int _llg_file_input_result = llg_file_scanf(llg_file_descriptor({}), {format}, {target_array}, {}); {cleanup} sv4_from_i64((int64_t)_llg_file_input_result, 32); }})",
                descriptor.code,
                targets.len()
            )
        }
        IrFileInput::ScanString {
            source,
            format,
            targets,
        } => {
            let source_code = super::super::objects::string(ctx, source)?;
            let (format, setup, cleanup) =
                render_plusarg_text(ctx, format, "_llg_string_input_format")?;
            let (target_setup, target_array) = render_file_input_targets(ctx, targets)?;
            format!(
                "({{ llg_string_t _llg_string_input_source = {source_code}; {setup}{target_setup} int _llg_file_input_result = llg_string_scanf(_llg_string_input_source.data, _llg_string_input_source.len, {format}, {target_array}, {}); {cleanup} llg_string_destroy(&_llg_string_input_source); sv4_from_i64((int64_t)_llg_file_input_result, 32); }})",
                targets.len()
            )
        }
        IrFileInput::Read {
            descriptor,
            target,
            start,
            count,
        } => {
            let descriptor = render_expr_impl(ctx, descriptor)?;
            let start_code = start
                .as_ref()
                .map(|value| render_expr_impl(ctx, value).map(|value| value.code))
                .transpose()?;
            let count_code = count
                .as_ref()
                .map(|value| render_expr_impl(ctx, value).map(|value| value.code))
                .transpose()?;
            let has_start = start_code.is_some();
            let has_count = count_code.is_some();
            let start = start_code.unwrap_or_else(|| "sv4_from_u64(0, 1, 0)".to_owned());
            let count = count_code.unwrap_or_else(|| "sv4_from_u64(0, 1, 0)".to_owned());
            match target {
                IrFileReadTarget::Packed {
                    lhs,
                    width,
                    signed,
                    two_state,
                } => {
                    let (setup, descriptor_code) =
                        render_file_lhs_ref(ctx, lhs, *width, *signed, *two_state)?;
                    format!(
                        "({{ {setup} int _llg_file_input_result = llg_file_read_packed(llg_file_descriptor({}), {descriptor_code}); (void)({start}); (void)({count}); sv4_from_i64((int64_t)_llg_file_input_result, 32); }})",
                        descriptor.code
                    )
                }
                IrFileReadTarget::Array { array } => {
                    let array = ctx.model.array(*array);
                    let dims = array
                        .dims
                        .iter()
                        .map(|(left, right)| format!("{left}, {right}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "sv4_from_i64((int64_t)llg_file_read_array(llg_file_descriptor({}), {}, {}, {}, {}, {}ULL, (const int32_t[]){{ {dims} }}, {}, {}, {start}, {}, {count}), 32)",
                        descriptor.code,
                        array.c_name,
                        array.elem_width,
                        array.signed as u8,
                        array.two_state as u8,
                        array.total,
                        array.dims.len(),
                        has_start as u8,
                        has_count as u8,
                    )
                }
            }
        }
    };
    Ok(result(code))
}

pub(super) fn render_test_plusargs(
    ctx: &RCtx<'_>,
    pattern: &IrPlusArgText,
) -> Result<RenderedExpr, String> {
    let (pattern, setup, cleanup) = render_plusarg_text(ctx, pattern, "_llg_plusarg_pattern")?;
    let code = if setup.is_empty() {
        format!("sv4_from_u64((uint64_t)llg_test_plusargs({pattern}), 32, 1)")
    } else {
        format!(
            "({{ {setup} int _llg_plusarg_result = llg_test_plusargs({pattern}); {cleanup} sv4_from_u64((uint64_t)_llg_plusarg_result, 32, 1); }})"
        )
    };
    Ok(RenderedExpr {
        code,
        width: 32,
        signed: true,
        fill: None,
    })
}

pub(super) fn render_value_plusargs(
    ctx: &RCtx<'_>,
    format: &IrPlusArgText,
    target: &crate::sim::ir::IrPlusArgTarget,
    result_width: u32,
    result_signed: bool,
) -> Result<RenderedExpr, String> {
    let (format, setup, cleanup) = render_plusarg_text(ctx, format, "_llg_plusarg_format")?;
    let mut code = String::from("({ int _llg_plusarg_ok = 0; ");
    code.push_str(&setup);
    match target {
        crate::sim::ir::IrPlusArgTarget::Packed {
            lhs,
            width,
            signed,
            two_state,
        } => {
            code.push_str(&format!(
                "sv4_t _llg_plusarg_value = sv4_x({width}, {}); ",
                *signed as u8
            ));
            code.push_str(&format!(
                "if (llg_value_plusargs_packed({format}, &_llg_plusarg_value, {width}, {}, {})) {{ ",
                *signed as u8,
                *two_state as u8
            ));
            let value = IrExpr::new(
                IrExprKind::LocalRead("_llg_plusarg_value".to_owned()),
                *width,
                *signed,
                None,
            );
            code.push_str(&render_assign(ctx, lhs, &value, false)?);
            code.push_str(" _llg_plusarg_ok = 1; }");
        }
        crate::sim::ir::IrPlusArgTarget::Real { lhs, shortreal } => {
            code.push_str("double _llg_plusarg_value = 0.0; ");
            code.push_str(&format!(
                "if (llg_value_plusargs_real({format}, &_llg_plusarg_value)) {{ "
            ));
            let value = IrExpr::new(
                IrExprKind::LocalRead("_llg_plusarg_value".to_owned()),
                0,
                true,
                None,
            );
            // Keep the target's shortreal rounding in the ordinary assignment
            // path rather than duplicating it in the plusarg runtime API.
            let _ = shortreal;
            code.push_str(&render_assign(ctx, lhs, &value, false)?);
            code.push_str(" _llg_plusarg_ok = 1; }");
        }
        crate::sim::ir::IrPlusArgTarget::String { address } => {
            code.push_str("llg_string_t _llg_plusarg_value = (llg_string_t){0}; ");
            code.push_str(&format!(
                "if (llg_value_plusargs_string({format}, &_llg_plusarg_value)) {{ llg_string_move({address}, _llg_plusarg_value); _llg_plusarg_ok = 1; }} else {{ llg_string_destroy(&_llg_plusarg_value); }}"
            ));
        }
    }
    code.push_str(&cleanup);
    code.push_str(&format!(
        " sv4_from_u64((uint64_t)_llg_plusarg_ok, {result_width}, {}) ; }})",
        result_signed as u8
    ));
    Ok(RenderedExpr {
        code,
        width: result_width,
        signed: result_signed,
        fill: None,
    })
}
