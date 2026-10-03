//! System tasks.

use super::super::constants::emit_const;
use super::*;
use crate::sim::ir::IrConst;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_memory(
    ctx: &RCtx<'_>,
    write: bool,
    path: &crate::sim::ir::IrStringExpr,
    view: &crate::sim::ir::IrMemoryView,
    radix: IrMemoryRadix,
    addressing: IrMemoryAddressingPolicy,
    enum_values: Option<&[IrConst]>,
    start: Option<&IrExpr>,
    finish: Option<&IrExpr>,
) -> Result<String, String> {
    let array_info = ctx
        .model
        .arrays
        .get(view.array)
        .ok_or_else(|| "memory task array index is out of bounds".to_owned())?;
    if array_info.real {
        return Err("memory task does not support real arrays".to_owned());
    }
    if view.dims.is_empty() || view.dims.len() != view.strides.len() {
        return Err("memory task requires a fixed packed memory view".to_owned());
    }
    let path = super::super::objects::string(ctx, path)?;
    let mut selector_declarations = String::new();
    let mut selector_cleanups = String::new();
    let origin = if view.selectors.is_empty() {
        format!("{}ULL", view.origin)
    } else {
        selector_declarations.push_str(&format!(
            "             uint64_t _llg_memory_origin = {}ULL;\n",
            view.origin
        ));
        for (index, selector) in view.selectors.iter().enumerate() {
            let rendered = render_expr(ctx, &selector.value)?;
            let selector_name = format!("_llg_memory_selector_{index}");
            let index_name = format!("_llg_memory_selector_index_{index}");
            let offset_name = format!("_llg_memory_selector_offset_{index}");
            let (left, right) = (selector.left, selector.right);
            let minimum = left.min(right);
            let maximum = left.max(right);
            let offset = if left >= right {
                format!("(uint64_t)((int64_t){left} - {index_name})")
            } else {
                format!("(uint64_t)({index_name} - (int64_t){left})")
            };
            selector_declarations.push_str(&format!(
                "             sv4_t {selector_name} = {};\n\
                 int64_t {index_name} = 0;\n\
                 if (_llg_memory_origin != UINT64_MAX) {{ if (!sv4_to_index_i64({selector_name}, &{index_name}) || {index_name} < {minimum} || {index_name} > {maximum}) {{ _llg_memory_origin = UINT64_MAX; }} else {{ uint64_t {offset_name} = {offset}; if ({offset_name} > (UINT64_MAX - _llg_memory_origin) / {}ULL) {{ _llg_memory_origin = UINT64_MAX; }} else {{ _llg_memory_origin += {offset_name} * {}ULL; }} }} }}\n",
                rendered.code,
                selector.stride,
                selector.stride
            ));
            selector_cleanups.push_str(&format!("             sv4_destroy(&{selector_name});\n"));
        }
        "_llg_memory_origin".to_owned()
    };
    let start_code = start
        .map(|value| render_expr(ctx, value).map(|rendered| rendered.code))
        .transpose()?;
    let finish_code = finish
        .map(|value| render_expr(ctx, value).map(|rendered| rendered.code))
        .transpose()?;
    let start_value = start_code.as_deref().unwrap_or("SV4_C(0, 1)");
    let finish_value = finish_code.as_deref().unwrap_or("SV4_C(0, 1)");
    let runtime = if write {
        "llg_memory_write_view"
    } else {
        "llg_memory_read_view"
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
            let entry_count = values.len();
            let rendered_values = values.iter().map(emit_const).collect::<Vec<_>>().join(", ");
            format!(
                "             sv4_t _llg_memory_enum_values[{}] = {{ {} }};\n",
                entry_count, rendered_values
            )
        })
        .unwrap_or_default();
    let enum_cleanup = if enum_values.is_some() {
        format!("             sv4_destroy_array(_llg_memory_enum_values, {enum_count});\n")
    } else {
        String::new()
    };
    let dimensions = view
        .dims
        .iter()
        .flat_map(|(left, right)| [left.to_string(), right.to_string()])
        .collect::<Vec<_>>()
        .join(", ");
    let strides = view
        .strides
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "{{\n\
             llg_string_t _llg_memory_path = {path};\n\
             {selector_declarations}\
             sv4_t _llg_memory_start = {start_value};\n\
             sv4_t _llg_memory_finish = {finish_value};\n\
             {enum_declaration}\
             {runtime}(_llg_memory_path, {name}, {total}ULL, {width}u, {signed}, {two_state},\n\
                       (const int32_t[]){{ {dimensions} }}, {n_dims},\n\
                       (const uint64_t[]){{ {strides} }}, {origin}, {view_total}ULL,\n\
                       _llg_memory_start, _llg_memory_finish, {has_start}, {has_finish}, {addressing}, {enum_pointer}, {enum_count}, {radix});\n\
             {selector_cleanups}\
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
        selector_declarations = selector_declarations,
        selector_cleanups = selector_cleanups,
        origin = origin,
        dimensions = dimensions,
        n_dims = view.dims.len(),
        strides = strides,
        view_total = view.total,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ir::{IrArray, IrConst, IrMemoryView, IrModel, IrModelParts, IrStringExpr};

    fn wide_enum_values() -> Vec<IrConst> {
        (1..=3)
            .map(|value| {
                IrConst::packed(
                    vec![value, value << 8, value],
                    vec![0, 0, 0],
                    vec![0, 0, 0],
                    130,
                    false,
                    None,
                )
                .expect("wide enum value")
            })
            .collect()
    }

    #[test]
    fn legacy_memory_emitter_uses_enum_entry_count_for_c_array_bound() {
        let array = IrArray::new(
            "G_memory".to_owned(),
            "tb.memory".to_owned(),
            130,
            false,
            vec![(0, 2)],
        )
        .expect("memory array");
        let model = IrModel::from_parts(
            "enum_memory".to_owned(),
            1,
            IrModelParts {
                arrays: vec![array],
                ..Default::default()
            },
        )
        .expect("valid model");
        let ctx = RCtx {
            value_backend: crate::sim::value_backend::ValueBackend::Legacy,
            model: &model,
            func: None,
            sampled: false,
            activation_label: None,
            constants: None,
        };
        let values = wide_enum_values();
        let rendered = render_memory(
            &ctx,
            false,
            &IrStringExpr::Literal(b"enum.mem".to_vec()),
            &IrMemoryView {
                array: 0,
                origin: 0,
                selectors: Vec::new(),
                sliced: false,
                dims: vec![(0, 2)],
                strides: vec![1],
                total: 3,
            },
            IrMemoryRadix::Hex,
            IrMemoryAddressingPolicy::SystemVerilog2009,
            Some(&values),
            None,
            None,
        )
        .expect("legacy memory rendering");

        let declaration = rendered
            .lines()
            .find(|line| line.contains("sv4_t _llg_memory_enum_values"))
            .expect("enum declaration");
        assert!(declaration.contains("[3]"), "{declaration}");
        assert!(rendered.contains(", 3, 16);"), "{rendered}");
    }
}
