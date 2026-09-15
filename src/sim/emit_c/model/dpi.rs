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

pub(super) fn render_dpi_thunk(f: &IrFunc) -> Result<String, String> {
    let dpi = f
        .dpi_import()
        .ok_or_else(|| "DPI thunk requested for an ordinary function".to_owned())?;
    let ret_scalar = dpi_return_scalar(f)?;
    let ret_signed = f.ret.as_ref().is_some_and(IrType::signed);
    let ret_t = internal_return_type(f);
    let mut out = format!("static {ret_t} {}({}) {{\n", f.c_name, func_params(f));
    let guard_return = match ret_scalar {
        Some(DpiScalar::String) => "return llg_string_bytes(\"\", 0);".to_owned(),
        Some(DpiScalar::Chandle) => "return NULL;".to_owned(),
        Some(DpiScalar::Real { .. }) => "return 0.0;".to_owned(),
        Some(_) => format!("return {};", f.ret_x()),
        None => "return;".to_owned(),
    };
    out.push_str(&format!(
        "    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \"llg: recursion limit exceeded in %s\\n\", \"{}\");\n        {guard_return}\n    }}\n",
        f.c_name
    ));
    for (idx, form) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.is_address())
    {
        let scalar = dpi_scalar(form)?;
        let ty = match scalar {
            DpiScalar::String => "char *",
            DpiScalar::Chandle => "void *",
            _ => dpi_scalar_c_type(scalar),
        };
        out.push_str(&format!(
            "    {ty} _dpi_o{idx} = {};\n",
            dpi_output_init(form, idx)?
        ));
    }
    let mut args = Vec::new();
    for (idx, form) in f.formals.iter().enumerate() {
        args.push(if form.is_address() {
            format!("&_dpi_o{idx}")
        } else {
            dpi_input_expr(form, idx)?
        });
    }
    let call = format!("{}({})", dpi.c_name(), args.join(", "));
    if let Some(scalar) = ret_scalar {
        out.push_str(&format!(
            "    {} _dpi_ret = {call};\n",
            dpi_scalar_c_type(scalar)
        ));
    } else {
        out.push_str(&format!("    {call};\n"));
    }
    // Foreign string results can alias inputs or each other. Snapshot every
    // returned buffer before any copy-out can destroy an aliased destination.
    if matches!(ret_scalar, Some(DpiScalar::String)) {
        out.push_str(
            "    llg_string_t _dpi_string_ret = _dpi_ret ? llg_string_bytes(_dpi_ret, strlen(_dpi_ret)) : llg_string_bytes(\"\", 0);\n",
        );
    }
    for (idx, form) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.is_address())
    {
        if matches!(dpi_scalar(form)?, DpiScalar::String) {
            out.push_str(&format!(
                "    llg_string_t _dpi_s{idx} = _dpi_o{idx} ? llg_string_bytes(_dpi_o{idx}, strlen(_dpi_o{idx})) : llg_string_bytes(\"\", 0);\n"
            ));
        }
    }
    for (idx, form) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.is_address())
    {
        match dpi_scalar(form)? {
            DpiScalar::Bit => out.push_str(&format!(
                "    *o{idx} = sv4_from_u64((uint64_t)_dpi_o{idx}, 1, {});\n",
                form.signed as u8
            )),
            DpiScalar::Logic => out.push_str(&format!(
                "    *o{idx} = llg_dpi_sv4_from_logic(_dpi_o{idx}, {});\n",
                form.signed as u8
            )),
            DpiScalar::Int { width, signed } => out.push_str(&format!(
                "    *o{idx} = {};\n",
                if signed {
                    format!("sv4_from_i64((int64_t)_dpi_o{idx}, {width})")
                } else {
                    format!(
                        "sv4_from_u64((uint64_t)_dpi_o{idx}, {width}, {})",
                        form.signed as u8
                    )
                }
            )),
            DpiScalar::Real { .. } => {
                out.push_str(&format!("    *o{idx} = (double)_dpi_o{idx};\n"))
            }
            DpiScalar::Chandle => out.push_str(&format!("    *o{idx} = _dpi_o{idx};\n")),
            DpiScalar::String => {
                out.push_str(&format!("    llg_string_move(o{idx}, _dpi_s{idx});\n"))
            }
        }
    }
    for (idx, _form) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.string && !form.is_address())
    {
        out.push_str(&format!("    llg_string_destroy(&a{idx});\n"));
    }
    match ret_scalar {
        Some(DpiScalar::Bit) => out.push_str(&format!(
            "    return sv4_from_u64((uint64_t)_dpi_ret, 1, {});\n",
            ret_signed as u8
        )),
        Some(DpiScalar::Logic) => out.push_str(&format!(
            "    return llg_dpi_sv4_from_logic(_dpi_ret, {});\n",
            ret_signed as u8
        )),
        Some(DpiScalar::Int { width, signed }) => out.push_str(&format!(
            "    return {};\n",
            if signed {
                format!("sv4_from_i64((int64_t)_dpi_ret, {width})")
            } else {
                format!("sv4_from_u64((uint64_t)_dpi_ret, {width}, 0)")
            }
        )),
        Some(DpiScalar::Real { .. }) => out.push_str("    return (double)_dpi_ret;\n"),
        Some(DpiScalar::Chandle) => out.push_str("    return _dpi_ret;\n"),
        Some(DpiScalar::String) => out.push_str("    return _dpi_string_ret;\n"),
        None => out.push_str("    return;\n"),
    }
    out.push_str("}\n\n");
    Ok(out)
}
