//! DPI-C exports and context scopes (SV 35.5.3, 35.7-35.9, Annex H.8-H.9).
//!
//! Every instantiated scope holding DPI-C declarations is one entry of the
//! sorted `llg_dpi_scopes` table, which the runtime searches and validates
//! `svScope` values against. An exported subroutine gets, per instance, a
//! static thunk with its Annex H prototype that converts the foreign
//! arguments into model values the thunk owns, calls the subroutine with
//! the internal calling convention, writes outputs and the result back in
//! declaration order, and restores the call-chain state. Each exported C
//! identifier gets one external entry point that selects the thunk of the
//! current context scope (35.5.3) and forwards its arguments unchanged.
//!
//! Strings handed to foreign code by an export (outputs, changed inouts and
//! results) stay valid until the same thunk is called again, which retains
//! one model-owned copy per such argument (H.8.10 leaves the lifetime open).

use super::dpi::{
    chunks, dpi_packed_result, dpi_param_type, int_c_type, is_aggregate, small_c_type, LayoutTables,
};
use super::*;
use crate::sim::ir::{IrDpiExport, IrDpiType, IrFormalMode, IrModel};

/// Exported C identifiers in ascending order; the position is the id the
/// scope tables and entry points use.
fn export_ids(model: &IrModel) -> Vec<&str> {
    model
        .funcs
        .iter()
        .filter_map(|function| function.dpi_export().map(IrDpiExport::c_name))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn export_of(f: &IrFunc) -> Result<&IrDpiExport, String> {
    f.dpi_export()
        .ok_or_else(|| "DPI export thunk requested for an unexported subroutine".to_owned())
}

fn thunk_name(f: &IrFunc) -> String {
    format!("llg_dpix_{}", f.c_name)
}

/// C return type of an exported subroutine: an exported task returns the
/// `int` of the disable protocol (35.8, 35.9).
fn foreign_return_type(f: &IrFunc, export: &IrDpiExport) -> Result<&'static str, String> {
    if f.is_task {
        return Ok("int");
    }
    export.ret().map_or(Ok("void"), small_c_type)
}

/// `(type, name)` of each foreign parameter, as an import of the same
/// signature would declare them (H.8.2).
fn foreign_params(f: &IrFunc, export: &IrDpiExport) -> Result<Vec<(String, String)>, String> {
    let declared = export.formals();
    if declared.len() != f.formals.len() {
        return Err(format!(
            "DPI-C export `{}` has {} foreign formals for {} formals",
            export.c_name(),
            declared.len(),
            f.formals.len()
        ));
    }
    declared
        .iter()
        .zip(&f.formals)
        .enumerate()
        .map(|(index, (ty, formal))| Ok((dpi_param_type(ty, formal)?, format!("p{index}"))))
        .collect()
}

fn param_list(params: &[(String, String)]) -> String {
    if params.is_empty() {
        return "void".to_owned();
    }
    params
        .iter()
        .map(|(ty, name)| {
            if ty.ends_with('*') {
                format!("{ty}{name}")
            } else {
                format!("{ty} {name}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A value of foreign return type `ty` for an entry point that found no
/// thunk; the fatal error already requested completion.
fn default_return(ty: &str) -> String {
    match ty {
        "void" => String::new(),
        "const char *" => " \"\"".to_owned(),
        other => format!(" ({other})0"),
    }
}

/// Declarations that precede every subroutine body: thunk prototypes, the
/// retained strings, each scope's export table, the scope table and the
/// call-site table.
pub(super) fn render_dpi_declarations(model: &IrModel) -> Result<String, String> {
    let mut out = String::new();
    if model.dpi_scopes.is_empty() && model.dpi_sites.is_empty() {
        return Ok(out);
    }
    let ids = export_ids(model);
    let mut slots: Vec<Vec<(usize, String)>> = vec![Vec::new(); model.dpi_scopes.len()];
    out.push_str("\n/* DPI-C context scopes and exports (SV 35.5.3, Annex H.9) */\n");
    for f in model.funcs.iter().filter(|f| f.dpi_export().is_some()) {
        let export = export_of(f)?;
        let params = foreign_params(f, export)?;
        out.push_str(&format!(
            "static {} {}({});\n",
            foreign_return_type(f, export)?,
            thunk_name(f),
            param_list(&params)
        ));
        let id = ids
            .binary_search(&export.c_name())
            .map_err(|_| "exported C identifier without an id".to_owned())?;
        slots
            .get_mut(export.scope())
            .ok_or("DPI export scope is out of range")?
            .push((id, thunk_name(f)));
    }
    for (index, scope_slots) in slots.iter_mut().enumerate() {
        if scope_slots.is_empty() {
            continue;
        }
        scope_slots.sort();
        let entries = scope_slots
            .iter()
            .map(|(id, thunk)| format!("{{ {id}u, (llg_dpi_fn_t){thunk} }}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "static const llg_dpi_export_slot_t llg_dpi_exports_{index}[] = {{ {entries} }};\n"
        ));
    }
    if !model.dpi_scopes.is_empty() {
        out.push_str(&format!(
            "static llg_dpi_scope_t llg_dpi_scopes[{}] = {{\n",
            model.dpi_scopes.len()
        ));
        for (index, scope) in model.dpi_scopes.iter().enumerate() {
            let exports = if slots[index].is_empty() {
                "NULL, 0u".to_owned()
            } else {
                format!("llg_dpi_exports_{index}, {}u", slots[index].len())
            };
            out.push_str(&format!(
                "    {{ {}, {exports}, NULL }},\n",
                c_string_literal(scope.name())
            ));
        }
        out.push_str("};\n");
    }
    if !model.dpi_sites.is_empty() {
        out.push_str(&format!(
            "static const llg_dpi_site_t llg_dpi_sites[{}] = {{\n",
            model.dpi_sites.len()
        ));
        for site in &model.dpi_sites {
            out.push_str(&format!(
                "    {{ {}, {} }},\n",
                c_string_literal(&site.file),
                i32::try_from(site.line).unwrap_or(i32::MAX)
            ));
        }
        out.push_str("};\n");
    }
    Ok(out)
}

/// The export thunks and the external entry point of every exported C
/// identifier, after every subroutine body. `coroutine_frame` gives the
/// final frame type of each subroutine rendered as a coroutine: an exported
/// task that can consume time (ADV-018).
pub(super) fn render_dpi_export_bodies(
    model: &IrModel,
    coroutine_frame: impl Fn(usize) -> Option<String>,
) -> Result<String, String> {
    let mut out = String::new();
    let ids = export_ids(model);
    if ids.is_empty() {
        return Ok(out);
    }
    for (index, f) in model
        .funcs
        .iter()
        .enumerate()
        .filter(|(_, f)| f.dpi_export().is_some())
    {
        let frame = coroutine_frame(index).filter(|_| f.is_task);
        out.push_str(&render_export_thunk(f, frame.as_deref())?);
    }
    for (id, c_name) in ids.iter().enumerate() {
        // Every scope exporting one C identifier has the same signature
        // (35.4), so the first subroutine describes the entry point.
        let f = model
            .funcs
            .iter()
            .find(|f| {
                f.dpi_export()
                    .is_some_and(|export| export.c_name() == *c_name)
            })
            .ok_or("exported C identifier without a subroutine")?;
        let export = export_of(f)?;
        let params = foreign_params(f, export)?;
        let ret = foreign_return_type(f, export)?;
        let list = param_list(&params);
        let types = if params.is_empty() {
            "void".to_owned()
        } else {
            params
                .iter()
                .map(|(ty, _)| ty.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let names = params
            .iter()
            .map(|(_, name)| name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let invoke = format!("(({ret} (*)({types}))_dpi_fn)({names})");
        out.push_str(&format!(
            "/* llg DPI-C export: c_name={c_name} */\n{ret} {c_name}({list});\n{ret} {c_name}({list}) {{\n    llg_dpi_fn_t _dpi_fn = llg_dpi_export_target({id}u, {}, {});\n    if (!_dpi_fn) return{};\n    {}{invoke};\n{}}}\n\n",
            c_string_literal(c_name),
            u8::from(f.is_task),
            default_return(ret),
            if ret == "void" { "" } else { "return " },
            if ret == "void" { "    return;\n" } else { "" },
        ));
    }
    Ok(out)
}

/// One instance's export thunk: C arguments in, SystemVerilog call, C
/// outputs and result out (H.8, 35.6.1 copy-in/copy-out). The thunk of a
/// task that can consume time (a coroutine whose frame type is
/// `coroutine_frame`) runs on a foreign call context:
/// it fills the task's coroutine frame on the calling chain's arena and
/// waits while the scheduler thread runs it (ADV-018); the outputs live in
/// the thunk's value scopes and C locals until it returns.
fn render_export_thunk(f: &IrFunc, coroutine_frame: Option<&str>) -> Result<String, String> {
    let export = export_of(f)?;
    let params = foreign_params(f, export)?;
    let ret = foreign_return_type(f, export)?;
    let name = thunk_name(f);
    let mut tables = LayoutTables {
        prefix: format!("llg_dpix_{}_", f.c_name),
        count: 0,
        out: String::new(),
    };
    let mut layouts = Vec::with_capacity(f.formals.len());
    for ty in export.formals() {
        layouts.push(if is_aggregate(ty) {
            Some(tables.emit(ty)?)
        } else {
            None
        });
    }
    let mut out = tables.out;
    let mut keep = Vec::new();
    let mut body = String::new();
    let result_slot = f.formals.len();
    body.push_str(&format!(
        "static {ret} {name}({}) {{\n    llg_dpi_call_t _dpi_call;\n    int depth = llg_dpi_export_enter(&_dpi_call);\n    llg_value_scope_t* _dpi_mark = llg_value_scope_mark();\n    llg_value_scope_t* _dpi_scope = llg_value_scope_begin({});\n    sv4_t* _dpi_values = llg_value_scope_values(_dpi_scope);\n    (void)_dpi_values;\n",
        param_list(&params),
        f.formals.len() + 1
    ));
    let mut outputs = Vec::new();
    let mut inputs = Vec::new();
    let mut copy_back = String::new();
    for (index, (ty, formal)) in export.formals().iter().zip(&f.formals).enumerate() {
        let parameter = format!("p{index}");
        let inout = formal.mode() == IrFormalMode::Inout;
        let address = formal.is_address();
        // The foreign value an input or inout carries in.
        let source = if address {
            format!("*{parameter}")
        } else {
            parameter.clone()
        };
        let slot = format!("&_dpi_values[{index}]");
        match ty {
            IrDpiType::String => {
                let owner = format!("_dpi_s{index}");
                let initial = if address && !inout {
                    "(llg_string_t){0}".to_owned()
                } else {
                    format!("{source} ? llg_string_bytes({source}, strlen({source})) : (llg_string_t){{0}}")
                };
                body.push_str(&format!(
                    "    llg_value_scope_t* {owner}_owner = llg_value_scope_begin_object(sizeof(llg_string_t), llg_owned_string_drop);\n    llg_string_t* {owner} = (llg_string_t*)llg_value_scope_object({owner}_owner);\n    *{owner} = {initial};\n"
                ));
                if !address {
                    inputs.push(format!("*{owner}"));
                    continue;
                }
                // A static subroutine keeps the formal in persistent storage
                // (which outputs do not reset); the result is copied from it.
                let value = match export.storage(index) {
                    Some(storage) => {
                        if inout {
                            body.push_str(&format!(
                                "    llg_string_assign(&{storage}, {owner});\n"
                            ));
                        }
                        format!("(&{storage})")
                    }
                    None => owner.clone(),
                };
                outputs.push(value.clone());
                let retained = format!("{name}_keep{index}");
                keep.push(retained.clone());
                let publish = format!(
                    "llg_string_destroy(&{retained}); {retained} = llg_string_clone({value}); *{parameter} = {retained}.data ? {retained}.data : \"\";"
                );
                if inout {
                    // SystemVerilog writes a new address only when it
                    // changed the string (H.8.10).
                    body.push_str(&format!("    const char* _dpi_in{index} = *{parameter};\n"));
                    copy_back.push_str(&format!(
                        "    if (!_dpi_in{index} || strcmp(_dpi_in{index}, {value}->data ? {value}->data : \"\") != 0) {{ {publish} }}\n"
                    ));
                } else {
                    copy_back.push_str(&format!("    {publish}\n"));
                }
            }
            IrDpiType::Chandle => {
                if !address {
                    inputs.push(parameter);
                    continue;
                }
                let cell = export
                    .storage(index)
                    .map_or_else(|| format!("_dpi_h{index}"), str::to_owned);
                if export.storage(index).is_none() {
                    body.push_str(&format!(
                        "    void* {cell} = {};\n",
                        if inout { source } else { "NULL".to_owned() }
                    ));
                } else if inout {
                    body.push_str(&format!("    {cell} = {source};\n"));
                }
                outputs.push(format!("&{cell}"));
                copy_back.push_str(&format!("    *{parameter} = {cell};\n"));
            }
            IrDpiType::Real | IrDpiType::ShortReal => {
                if !address {
                    inputs.push(format!("(double){parameter}"));
                    continue;
                }
                let cell = export
                    .storage(index)
                    .map_or_else(|| format!("_dpi_r{index}"), str::to_owned);
                if export.storage(index).is_none() {
                    body.push_str(&format!(
                        "    double {cell} = {};\n",
                        if inout {
                            format!("(double){source}")
                        } else {
                            "0.0".to_owned()
                        }
                    ));
                } else if inout {
                    body.push_str(&format!("    llg_ba_d(&{cell}, (double){source});\n"));
                }
                outputs.push(format!("&{cell}"));
                let c_type = if matches!(ty, IrDpiType::ShortReal) {
                    "float"
                } else {
                    "double"
                };
                copy_back.push_str(&format!("    *{parameter} = ({c_type}){cell};\n"));
            }
            _ => {
                let signed = u8::from(formal.signed);
                let width = formal.width;
                if address && !inout {
                    // The output's initial value is the type's default
                    // (35.6.1: nothing is copied in).
                    body.push_str(&format!(
                        "    {}\n",
                        if formal.two_state {
                            format!("sv4_from_u64_to({slot}, 0, {width}, {signed});")
                        } else {
                            format!("sv4_x_to({slot}, {width}, {signed});")
                        }
                    ));
                } else {
                    match ty {
                        IrDpiType::Vector { width, logic } => body.push_str(&format!(
                            "    sv4_x_to({slot}, {width}, {signed});\n    llg_dpi_import_{}({slot}, {parameter}, {}u);\n",
                            if *logic { "logic" } else { "bits" },
                            chunks(*width)
                        )),
                        _ if is_aggregate(ty) => {
                            let (_, table) = layouts[index]
                                .as_ref()
                                .ok_or("DPI aggregate formal without a layout table")?;
                            body.push_str(&format!(
                                "    sv4_x_to({slot}, {width}, {signed});\n    llg_dpi_from_c(&{table}, {parameter}, {slot});\n"
                            ));
                        }
                        scalar => {
                            let value = dpi_packed_result(scalar, &source, formal.signed)?;
                            body.push_str(&format!(
                                "    {}\n",
                                super::super::destinations::assign(&slot, &value)
                            ));
                        }
                    }
                }
                if !address {
                    inputs.push(slot);
                    continue;
                }
                // A static subroutine reads an inout from, and writes an
                // output to, its persistent formal storage.
                let slot = match export.storage(index) {
                    Some(storage) => {
                        if inout {
                            body.push_str(&format!("    llg_ba_from(&{storage}, {slot});\n"));
                        }
                        format!("&{storage}")
                    }
                    None => slot,
                };
                outputs.push(slot.clone());
                let value = format!("(*{slot})");
                copy_back.push_str(&match ty {
                    IrDpiType::Bit => {
                        format!("    *{parameter} = llg_dpi_bit_from_sv4({value});\n")
                    }
                    IrDpiType::Logic => {
                        format!("    *{parameter} = llg_dpi_logic_from_sv4({value});\n")
                    }
                    IrDpiType::Int { bytes, signed } => format!(
                        "    *{parameter} = ({})sv4_to_{}({value});\n",
                        int_c_type(*bytes, *signed)?,
                        if *signed { "i64" } else { "u64" }
                    ),
                    IrDpiType::Vector { width, logic } => format!(
                        "    llg_dpi_export_{}({slot}, {parameter}, {}u);\n",
                        if *logic { "logic" } else { "bits" },
                        chunks(*width)
                    ),
                    _ => {
                        let (_, table) = layouts[index]
                            .as_ref()
                            .ok_or("DPI aggregate formal without a layout table")?;
                        format!("    llg_dpi_to_c(&{table}, {slot}, {parameter});\n")
                    }
                });
            }
        }
    }
    let mut arguments = outputs;
    arguments.extend(inputs);
    arguments.push("depth".to_owned());
    let packed = super::functions::packed_result(f);
    if packed {
        let ty = f.ret.ok_or("packed result without a type")?;
        body.push_str(&format!(
            "    sv4_x_to(&_dpi_values[{result_slot}], {}, {});\n",
            ty.width(),
            u8::from(ty.signed())
        ));
        arguments.push(format!("&_dpi_values[{result_slot}]"));
    }
    let call = format!("{}({})", f.c_name, arguments.join(", "));
    let result = match (f.is_task, export.ret()) {
        (true, _) if coroutine_frame.is_some() => {
            body.push_str(&suspending_call(
                f,
                coroutine_frame.unwrap_or_default(),
                &arguments,
            )?);
            None
        }
        (true, _) | (false, None) => {
            body.push_str(&format!("    {call};\n"));
            None
        }
        (false, Some(IrDpiType::String)) => {
            let retained = format!("{name}_keep");
            keep.push(retained.clone());
            body.push_str(&format!(
                "    llg_string_t _dpi_string = {call};\n    llg_string_destroy(&{retained});\n    {retained} = _dpi_string;\n"
            ));
            Some(format!("{retained}.data ? {retained}.data : \"\""))
        }
        (false, Some(IrDpiType::Chandle)) => {
            body.push_str(&format!("    void* _dpi_handle = {call};\n"));
            Some("_dpi_handle".to_owned())
        }
        (false, Some(IrDpiType::Real)) => {
            body.push_str(&format!("    double _dpi_real = {call};\n"));
            Some("_dpi_real".to_owned())
        }
        (false, Some(IrDpiType::ShortReal)) => {
            body.push_str(&format!("    double _dpi_real = {call};\n"));
            Some("(float)_dpi_real".to_owned())
        }
        (false, Some(scalar)) => {
            if !packed {
                return Err("small DPI-C export result without a packed slot".to_owned());
            }
            body.push_str(&format!("    {call};\n"));
            let value = format!("_dpi_values[{result_slot}]");
            Some(match scalar {
                IrDpiType::Bit => format!("llg_dpi_bit_from_sv4({value})"),
                IrDpiType::Logic => format!("llg_dpi_logic_from_sv4({value})"),
                IrDpiType::Int { bytes, signed } => format!(
                    "({})sv4_to_{}({value})",
                    int_c_type(*bytes, *signed)?,
                    if *signed { "i64" } else { "u64" }
                ),
                other => return Err(format!("DPI-C export result type {other:?}")),
            })
        }
    };
    // Outputs are written back in declaration order after the call (35.6.1);
    // with aliased foreign buffers the last formal wins.
    body.push_str(&copy_back);
    if let Some(result) = &result {
        body.push_str(&format!("    {ret} _dpi_result = {result};\n"));
    }
    body.push_str("    llg_value_scopes_end_since(_dpi_mark);\n");
    if f.is_task {
        body.push_str("    return llg_dpi_export_leave(&_dpi_call, 1);\n}\n\n");
    } else {
        body.push_str("    (void)llg_dpi_export_leave(&_dpi_call, 0);\n");
        body.push_str(if result.is_some() {
            "    return _dpi_result;\n}\n\n"
        } else {
            "    return;\n}\n\n"
        });
    }
    for retained in keep {
        out.push_str(&format!("static llg_string_t {retained};\n"));
    }
    out.push_str(&body);
    Ok(out)
}

/// The call of exported task `f` from its thunk when the task can consume
/// time, whose coroutine frame type is `frame_type`: `arguments` are the
/// plain call's, in parameter order. A caller that
/// cannot suspend gets the fatal error of `llg_dpi_export_begin` and 0; a
/// killed caller process gets 1 (35.9 item a) without the thunk touching
/// the process's storage again.
fn suspending_call(f: &IrFunc, frame_type: &str, arguments: &[String]) -> Result<String, String> {
    let parameters = super::functions::func_param_fields(f);
    if parameters.len() != arguments.len() {
        return Err(format!(
            "DPI-C export `{}` has {} arguments for {} parameters",
            f.c_name,
            arguments.len(),
            parameters.len()
        ));
    }
    let name = &f.c_name;
    let mut out = format!(
        "    llg_co_anchor_t* _dpi_slot = llg_dpi_export_begin(&{name}_desc, {});\n    if (!_dpi_slot) {{\n        llg_value_scopes_end_since(_dpi_mark);\n        (void)llg_dpi_export_leave(&_dpi_call, 1);\n        return 0;\n    }}\n    {frame_type}* _dpi_frame = ({frame_type}*)LLG_CO_ANCHOR_FRAME(_dpi_slot);\n",
        c_string_literal(export_of(f)?.c_name())
    );
    for (ty, field) in super::functions::frame_param_fields(f) {
        let (_, value) = parameters
            .iter()
            .zip(arguments)
            .find(|((_, parameter), _)| *parameter == field)
            .ok_or_else(|| format!("DPI-C export `{name}` has no argument for `{field}`"))?;
        let value = if ty == "sv4_t" {
            format!("*({value})")
        } else {
            value.clone()
        };
        out.push_str(&format!("    _dpi_frame->{field} = {value};\n"));
    }
    out.push_str(
        "    if (llg_dpi_export_wait(_dpi_slot)) return llg_dpi_export_abandoned(&_dpi_call);\n",
    );
    Ok(out)
}
