//! Dpi.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum DpiScalar {
    Bit,
    Logic,
    Int { width: u32, signed: bool },
    Real { shortreal: bool },
    Chandle,
    String,
}

fn dpi_scalar(form: &crate::sim::ir::IrFormal) -> Result<DpiScalar, String> {
    if form.is_ref() || form.event {
        return Err("DPI-C ref/event formal reached the C emitter".to_owned());
    }
    if form.string {
        return Ok(DpiScalar::String);
    }
    if form.chandle {
        return Ok(DpiScalar::Chandle);
    }
    if form.real {
        return Ok(DpiScalar::Real {
            shortreal: form.shortreal,
        });
    }
    match (form.width, form.two_state) {
        (1, true) => Ok(DpiScalar::Bit),
        (1, false) => Ok(DpiScalar::Logic),
        (8 | 16 | 32 | 64, true) => Ok(DpiScalar::Int {
            width: form.width,
            signed: form.signed,
        }),
        _ => Err(format!(
            "unsupported DPI-C integral width {} in generated thunk",
            form.width
        )),
    }
}

fn dpi_return_scalar(f: &IrFunc) -> Result<Option<DpiScalar>, String> {
    if f.ret_string {
        return Ok(Some(DpiScalar::String));
    }
    if f.ret_chandle {
        return Ok(Some(DpiScalar::Chandle));
    }
    Ok(match f.ret {
        None => None,
        Some(IrType::Real { shortreal }) => Some(DpiScalar::Real { shortreal }),
        Some(IrType::Packed {
            width,
            signed,
            two_state,
        }) => match (width, two_state) {
            (1, true) => Some(DpiScalar::Bit),
            (1, false) => Some(DpiScalar::Logic),
            (8 | 16 | 32 | 64, true) => Some(DpiScalar::Int { width, signed }),
            _ => {
                return Err(format!(
                    "unsupported DPI-C return width {width} in generated thunk"
                ));
            }
        },
    })
}

fn dpi_scalar_c_type(scalar: DpiScalar) -> &'static str {
    match scalar {
        DpiScalar::Bit => "svBit",
        DpiScalar::Logic => "svLogic",
        DpiScalar::Int {
            width: 8,
            signed: true,
        } => "int8_t",
        DpiScalar::Int {
            width: 8,
            signed: false,
        } => "uint8_t",
        DpiScalar::Int {
            width: 16,
            signed: true,
        } => "int16_t",
        DpiScalar::Int {
            width: 16,
            signed: false,
        } => "uint16_t",
        DpiScalar::Int {
            width: 32,
            signed: true,
        } => "int32_t",
        DpiScalar::Int {
            width: 32,
            signed: false,
        } => "uint32_t",
        DpiScalar::Int {
            width: 64,
            signed: true,
        } => "int64_t",
        DpiScalar::Int {
            width: 64,
            signed: false,
        } => "uint64_t",
        DpiScalar::Real { shortreal: true } => "float",
        DpiScalar::Real { shortreal: false } => "double",
        DpiScalar::Chandle => "void *",
        DpiScalar::String => "const char *",
        DpiScalar::Int { .. } => "uint64_t",
    }
}

pub(super) fn internal_return_type(f: &IrFunc) -> &'static str {
    if f.ret_string {
        "llg_string_t"
    } else if f.ret_chandle {
        "void *"
    } else if matches!(f.ret, Some(IrType::Real { .. })) {
        "double"
    } else if f.ret.is_some() {
        "sv4_t"
    } else {
        "void"
    }
}

fn dpi_external_return_type(f: &IrFunc) -> Result<&'static str, String> {
    Ok(match dpi_return_scalar(f)? {
        Some(DpiScalar::String) => "const char *",
        Some(scalar) => dpi_scalar_c_type(scalar),
        None => "void",
    })
}

pub(super) fn dpi_external_prototype(f: &IrFunc) -> Result<String, String> {
    let dpi = f
        .dpi_import()
        .ok_or_else(|| "DPI prototype requested for an ordinary function".to_owned())?;
    let mut params = Vec::new();
    for (idx, form) in f.formals.iter().enumerate() {
        let scalar = dpi_scalar(form)?;
        let param = if form.is_address() {
            match scalar {
                DpiScalar::String => format!("char **p{idx}"),
                DpiScalar::Chandle => format!("void **p{idx}"),
                _ => format!("{} *p{idx}", dpi_scalar_c_type(scalar)),
            }
        } else if matches!(scalar, DpiScalar::String) {
            format!("const char *p{idx}")
        } else {
            format!("{} p{idx}", dpi_scalar_c_type(scalar))
        };
        params.push(param);
    }
    let params = if params.is_empty() {
        "void".to_owned()
    } else {
        params.join(", ")
    };
    Ok(format!(
        "/* llg DPI-C import: c_name={} context={} pure={}; external calls remain observable. */\nextern {} {}({});\n",
        dpi.c_name(),
        dpi.is_context() as u8,
        dpi.is_pure() as u8,
        dpi_external_return_type(f)?,
        dpi.c_name(),
        params
    ))
}

pub(super) fn dpi_helpers() -> &'static str {
    "\n/* Canonical DPI scalar conversions. Native pointers never escape this thunk. */\n\
static svBit llg_dpi_bit_from_sv4(sv4_t value) {\n\
    return (svBit)(value.bits[0] & 1u);\n\
}\n\
static svLogic llg_dpi_logic_from_sv4(sv4_t value) {\n\
    if (value.x[0] & 1u) return sv_x;\n\
    if (value.z[0] & 1u) return sv_z;\n\
    return (svLogic)(value.bits[0] & 1u);\n\
}\n\
static sv4_t llg_dpi_sv4_from_logic(svLogic value, int8_t is_signed) {\n\
    switch (value) {\n\
    case sv_x: return sv4_x(1, is_signed);\n\
    case sv_z: return sv4_fill(3, 1, is_signed);\n\
    default: return sv4_from_u64((uint64_t)(value & 1u), 1, is_signed);\n\
    }\n\
}\n\n"
}

fn dpi_input_expr(form: &crate::sim::ir::IrFormal, idx: usize) -> Result<String, String> {
    Ok(match dpi_scalar(form)? {
        DpiScalar::Bit => format!("llg_dpi_bit_from_sv4(a{idx})"),
        DpiScalar::Logic => format!("llg_dpi_logic_from_sv4(a{idx})"),
        DpiScalar::Int { signed, .. } => format!(
            "({})sv4_to_{}(a{idx})",
            dpi_scalar_c_type(dpi_scalar(form)?),
            if signed { "i64" } else { "u64" }
        ),
        DpiScalar::Real { shortreal: true } => format!("(float)a{idx}"),
        DpiScalar::Real { shortreal: false } => format!("a{idx}"),
        DpiScalar::Chandle => format!("a{idx}"),
        DpiScalar::String => format!("(a{idx}.data ? a{idx}.data : \"\")"),
    })
}

fn dpi_output_init(form: &crate::sim::ir::IrFormal, idx: usize) -> Result<String, String> {
    let inout = form.mode() == crate::sim::ir::IrFormalMode::Inout;
    Ok(match dpi_scalar(form)? {
        DpiScalar::Bit => {
            if inout {
                format!("llg_dpi_bit_from_sv4(*o{idx})")
            } else {
                "0".to_owned()
            }
        }
        DpiScalar::Logic => {
            if inout {
                format!("llg_dpi_logic_from_sv4(*o{idx})")
            } else {
                "sv_x".to_owned()
            }
        }
        DpiScalar::Int { signed, .. } => {
            if inout {
                format!(
                    "({})sv4_to_{}(*o{idx})",
                    dpi_scalar_c_type(dpi_scalar(form)?),
                    if signed { "i64" } else { "u64" }
                )
            } else {
                "0".to_owned()
            }
        }
        DpiScalar::Real { .. } => {
            if inout {
                format!("*o{idx}")
            } else {
                "0.0".to_owned()
            }
        }
        DpiScalar::Chandle => {
            if inout {
                format!("*o{idx}")
            } else {
                "NULL".to_owned()
            }
        }
        DpiScalar::String => {
            if inout {
                format!("(o{idx}->data ? o{idx}->data : \"\")")
            } else {
                "NULL".to_owned()
            }
        }
    })
}

/// Inputs are borrowed from the caller's registered slots. Every foreign result
/// is staged before any output publication can reenter HDL or invalidate inputs.
pub(super) fn render_dpi_thunk(f: &IrFunc) -> Result<String, String> {
    let dpi = f
        .dpi_import()
        .ok_or_else(|| "DPI thunk requested for ordinary function".to_owned())?;
    let ret = dpi_return_scalar(f)?;
    let mut out = format!(
        "static {} {}({}) {{\n",
        internal_return_type(f),
        f.c_name,
        func_params(f)
    );
    let guard = if f.ret_string {
        "return (llg_string_t){0};".to_owned()
    } else if f.ret_chandle {
        "return NULL;".to_owned()
    } else if f.ret.is_some() {
        format!("return {};", f.ret_x())
    } else {
        "return;".to_owned()
    };
    out.push_str(&format!("    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{ fprintf(stderr, \"llg: recursion limit exceeded\\n\"); {guard} }}\n"));
    out.push_str("    llg_value_scope_t* _dpi_mark = llg_value_scope_mark();\n");
    // Exact descriptor count, independent of the model's largest packed width.
    out.push_str(&format!("    llg_value_scope_t* _dpi_scope = llg_value_scope_begin({});\n    sv4_t* _dpi_values = llg_value_scope_values(_dpi_scope);\n    (void)_dpi_values;\n", f.formals.len() + 1));
    for (index, formal) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
    {
        let scalar = dpi_scalar(formal)?;
        let ty = if matches!(scalar, DpiScalar::String) {
            "char*"
        } else {
            dpi_scalar_c_type(scalar)
        };
        out.push_str(&format!(
            "    {ty} _dpi_o{index} = {};\n",
            dpi_output_init(formal, index)?
        ));
    }
    let arguments = f
        .formals
        .iter()
        .enumerate()
        .map(|(index, formal)| {
            if formal.is_address() {
                Ok(format!("&_dpi_o{index}"))
            } else {
                dpi_input_expr(formal, index)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let call = format!("{}({})", dpi.c_name(), arguments.join(", "));
    if let Some(scalar) = ret {
        out.push_str(&format!(
            "    {} _dpi_ret = {call};\n",
            dpi_scalar_c_type(scalar)
        ));
    } else {
        out.push_str(&format!("    {call};\n"));
    }
    let string_snapshot = |name: &str, source: &str| {
        format!(
        "    llg_value_scope_t* {name}_owner = llg_value_scope_begin_object(sizeof(llg_string_t), llg_owned_string_drop);\n    llg_string_t* {name} = (llg_string_t*)llg_value_scope_object({name}_owner);\n    *{name} = ({source}) ? llg_string_bytes(({source}), strlen({source})) : (llg_string_t){{0}};\n")
    };
    if matches!(ret, Some(DpiScalar::String)) {
        out.push_str(&string_snapshot("_dpi_string_ret", "_dpi_ret"));
    }
    for (index, formal) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
    {
        if formal.string {
            out.push_str(&string_snapshot(
                &format!("_dpi_s{index}"),
                &format!("_dpi_o{index}"),
            ));
        } else if !formal.chandle && !formal.real {
            let value = dpi_packed_result(
                dpi_scalar(formal)?,
                &format!("_dpi_o{index}"),
                formal.signed,
            )?;
            out.push_str(&format!(
                "    sv4_replace(&_dpi_values[{index}], {value});\n"
            ));
        }
    }
    let result_slot = f.formals.len();
    if let Some(scalar) = ret {
        if matches!(
            scalar,
            DpiScalar::Bit | DpiScalar::Logic | DpiScalar::Int { .. }
        ) {
            let value = dpi_packed_result(
                scalar,
                "_dpi_ret",
                f.ret.as_ref().is_some_and(IrType::signed),
            )?;
            out.push_str(&format!(
                "    sv4_replace(&_dpi_values[{result_slot}], {value});\n"
            ));
        }
    }
    // A cancelled yielding context import does not copy anything out. Each
    // publication is another cancellation/reentrancy boundary.
    for (index, formal) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
    {
        out.push_str("    if (llg_activation_cancelled()) goto _dpi_return;\n");
        let statement = match dpi_scalar(formal)? {
            DpiScalar::String => {
                format!("llg_string_move(o{index}, llg_string_take(_dpi_s{index}));")
            }
            DpiScalar::Chandle => format!("*o{index} = _dpi_o{index};"),
            DpiScalar::Real { .. } => format!("llg_ba_d(o{index}, (double)_dpi_o{index});"),
            _ => format!("llg_ba(o{index}, _dpi_values[{index}]);"),
        };
        out.push_str(&format!("    {statement}\n"));
    }
    // A label must precede a statement, not a declaration, in C11.
    if f.formals.iter().any(|formal| formal.is_address()) {
        out.push_str("_dpi_return: ;\n");
    }
    let result = match ret {
        Some(DpiScalar::String) => {
            Some("llg_string_t _dpi_result = llg_string_take(_dpi_string_ret);".to_owned())
        }
        Some(DpiScalar::Chandle) => Some("void* _dpi_result = _dpi_ret;".to_owned()),
        Some(DpiScalar::Real { .. }) => Some("double _dpi_result = (double)_dpi_ret;".to_owned()),
        Some(_) => Some(format!(
            "sv4_t _dpi_result = SV4_EMPTY; sv4_move(&_dpi_result, &_dpi_values[{result_slot}]);"
        )),
        None => None,
    };
    if let Some(result) = &result {
        out.push_str(&format!("    {result}\n"));
    }
    out.push_str("    llg_value_scopes_end_since(_dpi_mark);\n");
    out.push_str(if result.is_some() {
        "    return _dpi_result;\n}\n\n"
    } else {
        "    return;\n}\n\n"
    });
    Ok(out)
}

fn dpi_packed_result(scalar: DpiScalar, source: &str, signed: bool) -> Result<String, String> {
    Ok(match scalar {
        DpiScalar::Bit => format!("sv4_from_u64((uint64_t){source}, 1, {})", u8::from(signed)),
        DpiScalar::Logic => format!("llg_dpi_sv4_from_logic({source}, {})", u8::from(signed)),
        DpiScalar::Int {
            width,
            signed: true,
        } => format!("sv4_from_i64((int64_t){source}, {width})"),
        DpiScalar::Int {
            width,
            signed: false,
        } => format!("sv4_from_u64((uint64_t){source}, {width}, 0)"),
        _ => return Err("DPI result is not packed".to_owned()),
    })
}
