//! System tasks.

use super::super::constants::emit_const;
use super::*;
use crate::sim::ir::IrConst;

pub(super) fn render_memory(
    ctx: &RCtx<'_>,
    write: bool,
    path: &crate::sim::ir::IrStringExpr,
    array: usize,
    radix: IrMemoryRadix,
    addressing: IrMemoryAddressingPolicy,
    enum_values: Option<&[IrConst]>,
    start: Option<&IrExpr>,
    finish: Option<&IrExpr>,
) -> Result<String, String> {
    let array_info = ctx
        .model
        .arrays
        .get(array)
        .ok_or_else(|| "memory task array index is out of bounds".to_owned())?;
    if array_info.real {
        return Err("memory task does not support real arrays".to_owned());
    }
    if array_info.dims.len() != 1 {
        return Err("memory task requires a one-dimensional array".to_owned());
    }
    let path = super::super::objects::string(ctx, path)?;
    let start_code = start
        .map(|value| render_expr(ctx, value).map(|rendered| rendered.code))
        .transpose()?;
    let finish_code = finish
        .map(|value| render_expr(ctx, value).map(|rendered| rendered.code))
        .transpose()?;
    let start_value = start_code.as_deref().unwrap_or("SV4_C(0, 1)");
    let finish_value = finish_code.as_deref().unwrap_or("SV4_C(0, 1)");
    let (left, right) = array_info.dims[0];
    let runtime = if write {
        "llg_memory_write"
    } else {
        "llg_memory_read"
    };
    let radix = match radix {
        IrMemoryRadix::Binary => 2,
        IrMemoryRadix::Hex => 16,
    };
    let addressing = match addressing {
        IrMemoryAddressingPolicy::Verilog2001 => 0,
        IrMemoryAddressingPolicy::SystemVerilog2009 => 1,
    };
    let enum_values = enum_values.filter(|values| !values.is_empty());
    let enum_count = enum_values.map_or(0, <[_]>::len);
    let enum_pointer = if enum_values.is_some() {
        "_llg_memory_enum_values"
    } else {
        "NULL"
    };
    let enum_declaration = enum_values
        .map(|values| {
            let values = values.iter().map(emit_const).collect::<Vec<_>>().join(", ");
            format!(
                "             sv4_t _llg_memory_enum_values[{}] = {{ {} }};\n",
                values.len(),
                values
            )
        })
        .unwrap_or_default();
    let enum_cleanup = if enum_values.is_some() {
        format!("             sv4_destroy_array(_llg_memory_enum_values, {enum_count});\n")
    } else {
        String::new()
    };
    Ok(format!(
        "{{\n\
             llg_string_t _llg_memory_path = {path};\n\
             sv4_t _llg_memory_start = {start_value};\n\
             sv4_t _llg_memory_finish = {finish_value};\n\
             {enum_declaration}\
             {runtime}(_llg_memory_path, {name}, {total}ULL, {width}u, {signed}, {two_state},\n\
                       (const int32_t[]){{ {left}, {right} }}, 1,\n\
                       _llg_memory_start, _llg_memory_finish, {has_start}, {has_finish}, {addressing}, {enum_pointer}, {enum_count}, {radix});\n\
             {enum_cleanup}\
         }}\n",
        name = array_info.c_name,
        total = array_info.total,
        width = array_info.elem_width,
        signed = array_info.signed as u8,
        two_state = array_info.two_state as u8,
        has_start = start.is_some() as u8,
        has_finish = finish.is_some() as u8,
        enum_declaration = enum_declaration,
        enum_pointer = enum_pointer,
        enum_count = enum_count,
        enum_cleanup = enum_cleanup,
    ))
}

pub(super) fn render_vpi_call(
    ctx: &RCtx<'_>,
    site: usize,
    name: &str,
    args: &[IrExpr],
) -> Result<String, String> {
    let mut declarations = String::new();
    let mut values = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        let rendered = render_expr(ctx, arg)?;
        if rendered.width == 0 {
            let value_name = format!("_llg_vpi_arg{index}_real");
            declarations.push_str(&format!("double {value_name} = {}; ", rendered.code));
            values.push(format!(
                "{{ LLG_FMT_REAL, 0, {}, 1, sv4_x(1, 0), {value_name} }}",
                rendered.signed as u8
            ));
        } else {
            let value_name = format!("_llg_vpi_arg{index}_packed");
            declarations.push_str(&format!("sv4_t {value_name} = {}; ", rendered.code));
            values.push(format!(
                "{{ LLG_FMT_PACKED, {}, {}, 0, {value_name}, 0.0 }}",
                rendered.width, rendered.signed as u8
            ));
        }
    }
    let array_len = args.len().max(1);
    let initializers = if values.is_empty() {
        "{ 0 }".to_owned()
    } else {
        values.join(", ")
    };
    Ok(format!(
        "{{ {declarations} llg_vpi_arg_t _llg_vpi_args[{array_len}] = {{ {initializers} }}; (void)llg_vpi_call_task_site({site}ULL, {}, _llg_vpi_args, {}); }}\n",
        c_string_literal(name),
        args.len()
    ))
}
