//! DPI-C import thunks (SV 35.5-35.6, Annex H).
//!
//! A thunk keeps the internal calling convention towards the model and the
//! C-layer convention of [`IrDpiImport`] towards foreign code. Small inputs
//! pass by value (H.8.7); every other argument passes a buffer the thunk owns
//! for the call: canonical chunks for packed values (H.7.7), the C layout for
//! sized unpacked aggregates (H.7.8), and an `svOpenArrayHandle` over such a
//! layout for open arrays (H.12). Foreign code therefore never addresses model
//! storage (H.6.7). Results and outputs are staged after the call and then
//! published in declaration order, each publication a cancellation boundary.

use super::*;
use crate::sim::ir::{IrDpiImport, IrDpiType, IrFormal, IrFormalMode};

/// Canonical vectors up to this many bytes are staged in the thunk's C
/// stack frame; wider ones in a scope-owned heap block, keeping a thunk's
/// frame within the foreign-call headroom of the host-stack estimate.
const DPI_STACK_VECTOR_BYTES: u64 = 512;

fn dpi_import(f: &IrFunc) -> Result<&IrDpiImport, String> {
    let dpi = f
        .dpi_import()
        .ok_or_else(|| "DPI thunk requested for an ordinary function".to_owned())?;
    if dpi.formals().len() > f.formals.len() {
        return Err(format!(
            "DPI-C import `{}` records more foreign formals than it declares",
            dpi.c_name()
        ));
    }
    Ok(dpi)
}

fn formal_type(dpi: &IrDpiImport, index: usize) -> Result<&IrDpiType, String> {
    dpi.formals().get(index).ok_or_else(|| {
        format!(
            "DPI-C import `{}` formal {index} has no foreign type",
            dpi.c_name()
        )
    })
}

fn int_c_type(bytes: u8, signed: bool) -> Result<&'static str, String> {
    Ok(match (bytes, signed) {
        (1, true) => "int8_t",
        (1, false) => "uint8_t",
        (2, true) => "int16_t",
        (2, false) => "uint16_t",
        (4, true) => "int32_t",
        (4, false) => "uint32_t",
        (8, true) => "int64_t",
        (8, false) => "uint64_t",
        _ => return Err(format!("unsupported DPI-C integer of {bytes} bytes")),
    })
}

/// C type of a small value passed or returned by value (H.8.3).
fn small_c_type(ty: &IrDpiType) -> Result<&'static str, String> {
    Ok(match ty {
        IrDpiType::Bit => "svBit",
        IrDpiType::Logic => "svLogic",
        IrDpiType::Int { bytes, signed } => int_c_type(*bytes, *signed)?,
        IrDpiType::Real => "double",
        IrDpiType::ShortReal => "float",
        IrDpiType::Chandle => "void *",
        IrDpiType::String => "const char *",
        other => return Err(format!("DPI-C type {other:?} is not a small value")),
    })
}

fn is_aggregate(ty: &IrDpiType) -> bool {
    matches!(
        ty,
        IrDpiType::Array { .. } | IrDpiType::Struct(_) | IrDpiType::Open { .. }
    )
}

fn chunks(width: u32) -> u64 {
    u64::from(width).div_ceil(32)
}

pub(super) fn internal_return_type(f: &IrFunc) -> &'static str {
    if f.ret_string {
        "llg_string_t"
    } else if f.ret_chandle {
        "void *"
    } else if matches!(f.ret, Some(IrType::Real { .. })) {
        "double"
    } else {
        // Packed results use the `_llg_result` destination parameter.
        "void"
    }
}

/// C return type of the foreign function. An imported task is an `int`
/// function of the disable protocol (SV 35.5.4, 35.9).
fn dpi_external_return_type(f: &IrFunc, dpi: &IrDpiImport) -> Result<&'static str, String> {
    if f.is_task {
        return Ok("int");
    }
    dpi.ret().map_or(Ok("void"), small_c_type)
}

/// C parameter type of one foreign formal (H.8).
fn dpi_param_type(ty: &IrDpiType, form: &IrFormal) -> Result<String, String> {
    let address = form.is_address();
    Ok(match ty {
        IrDpiType::Open { .. } => "const svOpenArrayHandle".to_owned(),
        IrDpiType::Vector { logic, .. } => format!(
            "{}{} *",
            if address { "" } else { "const " },
            if *logic {
                "svLogicVecVal"
            } else {
                "svBitVecVal"
            }
        ),
        IrDpiType::Array { .. } | IrDpiType::Struct(_) => {
            if address { "void *" } else { "const void *" }.to_owned()
        }
        IrDpiType::String if address => "const char **".to_owned(),
        IrDpiType::Chandle if address => "void **".to_owned(),
        small if address => format!("{} *", small_c_type(small)?),
        small => small_c_type(small)?.to_owned(),
    })
}

pub(super) fn dpi_external_prototype(f: &IrFunc) -> Result<String, String> {
    let dpi = dpi_import(f)?;
    let mut params = Vec::new();
    for (idx, form) in f.formals.iter().enumerate() {
        if form.is_ref() || form.event {
            return Err("DPI-C ref/event formal reached the C emitter".to_owned());
        }
        let ty = dpi_param_type(formal_type(dpi, idx)?, form)?;
        params.push(if ty.ends_with('*') {
            format!("{ty}p{idx}")
        } else {
            format!("{ty} p{idx}")
        });
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
        dpi_external_return_type(f, dpi)?,
        dpi.c_name(),
        params
    ))
}

pub(super) fn dpi_helpers() -> &'static str {
    "\n/* Canonical DPI scalar conversions. Native pointers never escape this thunk. */\n\
static svBit llg_dpi_bit_from_sv4(sv4_t value) {\n\
    return (svBit)(llg_sv4_word(value, 0, LLG_SV4_BITS) & 1u);\n\
}\n\
static svLogic llg_dpi_logic_from_sv4(sv4_t value) {\n\
    return (svLogic)llg_sv4_state_to_dpi(llg_sv4_state(value, 0));\n\
}\n\
static sv4_t llg_dpi_sv4_from_logic(svLogic value, int8_t is_signed) {\n\
    switch (value) {\n\
    case sv_x: return sv4_x(1, is_signed);\n\
    case sv_z: return sv4_fill(3, 1, is_signed);\n\
    default: return sv4_from_u64((uint64_t)(value & 1u), 1, is_signed);\n\
    }\n\
}\n\
static void llg_dpi_sv4_from_logic_to(sv4_t* dst, svLogic value, int8_t is_signed) {\n\
    sv4_replace(dst, llg_dpi_sv4_from_logic(value, is_signed));\n\
}\n\n"
}

/// Static layout tables of the aggregate formals of one thunk.
struct LayoutTables {
    prefix: String,
    count: usize,
    out: String,
}

impl LayoutTables {
    fn fresh(&mut self) -> (String, String) {
        let index = self.count;
        self.count += 1;
        (
            format!("{}c{index}", self.prefix),
            format!("{}t{index}", self.prefix),
        )
    }

    fn table(&mut self, name: &str, fields: &str) {
        self.out.push_str(&format!(
            "static const llg_dpi_type_t {name} = {{ {fields} }};\n"
        ));
    }

    /// Emit the C type and layout table of `ty`; returns their names.
    fn emit(&mut self, ty: &IrDpiType) -> Result<(String, String), String> {
        match ty {
            IrDpiType::Bit | IrDpiType::Logic | IrDpiType::Int { .. } => {
                let (c, t) = self.fresh();
                let (c_type, kind, width) = match ty {
                    IrDpiType::Bit => ("svBit", "LLG_DPI_BIT", 1),
                    IrDpiType::Logic => ("svLogic", "LLG_DPI_LOGIC", 1),
                    IrDpiType::Int { bytes, signed } => (
                        int_c_type(*bytes, *signed)?,
                        "LLG_DPI_INT",
                        u32::from(*bytes) * 8,
                    ),
                    _ => return Err("unexpected DPI scalar layout".to_owned()),
                };
                self.out.push_str(&format!("typedef {c_type} {c};\n"));
                self.table(
                    &t,
                    &format!(".kind = {kind}, .width = {width}u, .size = sizeof({c})"),
                );
                Ok((c, t))
            }
            IrDpiType::Vector { width, logic } => {
                let (c, t) = self.fresh();
                self.out.push_str(&format!(
                    "typedef {} {c}[{}];\n",
                    if *logic {
                        "svLogicVecVal"
                    } else {
                        "svBitVecVal"
                    },
                    chunks(*width)
                ));
                self.table(
                    &t,
                    &format!(
                        ".kind = {}, .width = {width}u, .size = sizeof({c})",
                        if *logic {
                            "LLG_DPI_LOGICVEC"
                        } else {
                            "LLG_DPI_BITVEC"
                        }
                    ),
                );
                Ok((c, t))
            }
            IrDpiType::Array { dims, element } => self.emit_array(dims, element),
            IrDpiType::Open { dims, element } => {
                let dims = dims
                    .iter()
                    .map(|dim| dim.ok_or_else(|| "unspecialized DPI open array".to_owned()))
                    .collect::<Result<Vec<_>, _>>()?;
                self.emit_array(&dims, element)
            }
            IrDpiType::Struct(members) => {
                let emitted = members
                    .iter()
                    .map(|member| self.emit(member))
                    .collect::<Result<Vec<_>, _>>()?;
                let (c, t) = self.fresh();
                let fields = emitted
                    .iter()
                    .enumerate()
                    .map(|(index, (member, _))| format!("{member} m{index};"))
                    .collect::<Vec<_>>()
                    .join(" ");
                self.out
                    .push_str(&format!("typedef struct {{ {fields} }} {c};\n"));
                let entries = emitted
                    .iter()
                    .enumerate()
                    .map(|(index, (_, table))| format!("{{ offsetof({c}, m{index}), &{table} }}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.out.push_str(&format!(
                    "static const llg_dpi_member_t {t}_m[] = {{ {entries} }};\n"
                ));
                let width = ty
                    .payload_width()
                    .ok_or("DPI structure without a packed payload")?;
                self.table(
                    &t,
                    &format!(
                        ".kind = LLG_DPI_STRUCT, .width = {width}u, .size = sizeof({c}), .count = {}u, .members = {t}_m",
                        members.len()
                    ),
                );
                Ok((c, t))
            }
            other => Err(format!(
                "DPI-C type {other:?} cannot be part of an unpacked aggregate argument"
            )),
        }
    }

    /// One `LLG_DPI_ARRAY` level per dimension, slowest outermost.
    fn emit_array(
        &mut self,
        dims: &[(i32, i32)],
        element: &IrDpiType,
    ) -> Result<(String, String), String> {
        let (mut inner_c, mut inner_t) = self.emit(element)?;
        let mut inner_width = element
            .payload_width()
            .ok_or("DPI array element without a packed payload")?;
        for (left, right) in dims.iter().rev() {
            let count = crate::sim::ir::dimension_size(*left, *right);
            let (c, t) = self.fresh();
            self.out
                .push_str(&format!("typedef {inner_c} {c}[{count}];\n"));
            inner_width *= count;
            self.table(
                &t,
                &format!(
                    ".kind = LLG_DPI_ARRAY, .width = {inner_width}u, .size = sizeof({c}), .left = {left}, .right = {right}, .count = {count}u, .element = &{inner_t}"
                ),
            );
            inner_c = c;
            inner_t = t;
        }
        Ok((inner_c, inner_t))
    }
}

fn dpi_input_expr(ty: &IrDpiType, idx: usize) -> Result<String, String> {
    Ok(match ty {
        IrDpiType::Bit => format!("llg_dpi_bit_from_sv4(*a{idx})"),
        IrDpiType::Logic => format!("llg_dpi_logic_from_sv4(*a{idx})"),
        IrDpiType::Int { bytes, signed } => format!(
            "({})sv4_to_{}(*a{idx})",
            int_c_type(*bytes, *signed)?,
            if *signed { "i64" } else { "u64" }
        ),
        IrDpiType::ShortReal => format!("(float)a{idx}"),
        IrDpiType::Real | IrDpiType::Chandle => format!("a{idx}"),
        IrDpiType::String => format!("(a{idx}.data ? a{idx}.data : \"\")"),
        other => return Err(format!("DPI-C type {other:?} is not passed by value")),
    })
}

/// Initial value of a small output (H.6.3 leaves it undetermined): the SV
/// default of the type; an inout starts from the actual.
fn dpi_output_init(ty: &IrDpiType, form: &IrFormal, idx: usize) -> Result<String, String> {
    let inout = form.mode() == IrFormalMode::Inout;
    Ok(match ty {
        IrDpiType::Bit if inout => format!("llg_dpi_bit_from_sv4(*o{idx})"),
        IrDpiType::Logic if inout => format!("llg_dpi_logic_from_sv4(*o{idx})"),
        IrDpiType::Logic => "sv_x".to_owned(),
        IrDpiType::Int { bytes, signed } if inout => format!(
            "({})sv4_to_{}(*o{idx})",
            int_c_type(*bytes, *signed)?,
            if *signed { "i64" } else { "u64" }
        ),
        IrDpiType::Real if inout => format!("*o{idx}"),
        IrDpiType::ShortReal if inout => format!("(float)*o{idx}"),
        IrDpiType::Real | IrDpiType::ShortReal => "0.0".to_owned(),
        IrDpiType::Chandle if inout => format!("*o{idx}"),
        IrDpiType::Chandle => "NULL".to_owned(),
        IrDpiType::String if inout => format!("(o{idx}->data ? o{idx}->data : \"\")"),
        IrDpiType::String => "NULL".to_owned(),
        _ => "0".to_owned(),
    })
}

fn dpi_packed_result(ty: &IrDpiType, source: &str, signed: bool) -> Result<String, String> {
    Ok(match ty {
        IrDpiType::Bit => format!("sv4_from_u64((uint64_t){source}, 1, {})", u8::from(signed)),
        IrDpiType::Logic => format!("llg_dpi_sv4_from_logic({source}, {})", u8::from(signed)),
        IrDpiType::Int {
            bytes,
            signed: true,
        } => format!("sv4_from_i64((int64_t){source}, {})", u32::from(*bytes) * 8),
        IrDpiType::Int {
            bytes,
            signed: false,
        } => format!(
            "sv4_from_u64((uint64_t){source}, {}, 0)",
            u32::from(*bytes) * 8
        ),
        _ => return Err("DPI result is not packed".to_owned()),
    })
}

/// Inputs are borrowed from the caller's registered slots. Every foreign result
/// is staged before any output publication can reenter HDL or invalidate inputs.
pub(super) fn render_dpi_thunk(f: &IrFunc) -> Result<String, String> {
    let dpi = dpi_import(f)?;
    let ret = if f.is_task { None } else { dpi.ret() };
    let mut tables = LayoutTables {
        prefix: format!("llg_dpi_{}_", f.c_name),
        count: 0,
        out: String::new(),
    };
    let mut layouts = Vec::with_capacity(f.formals.len());
    for index in 0..f.formals.len() {
        let ty = formal_type(dpi, index)?;
        layouts.push(if is_aggregate(ty) {
            Some(tables.emit(ty)?)
        } else {
            None
        });
    }
    let mut out = tables.out;
    out.push_str(&format!(
        "static {} {}({}) {{\n",
        internal_return_type(f),
        f.c_name,
        func_params(f)
    ));
    let guard = if f.ret_string {
        "return (llg_string_t){0};".to_owned()
    } else if f.ret_chandle {
        "return NULL;".to_owned()
    } else if packed_result(f) {
        format!(
            "{} return;",
            super::super::destinations::assign("_llg_result", &f.ret_x())
        )
    } else if f.ret.is_some() {
        format!("return {};", f.ret_x())
    } else {
        "return;".to_owned()
    };
    out.push_str(&format!("    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{ fprintf(stderr, \"llg: recursion limit exceeded\\n\"); {guard} }}\n"));
    out.push_str("    llg_value_scope_t* _dpi_mark = llg_value_scope_mark();\n");
    // Exact descriptor count, independent of the model's largest packed width.
    out.push_str(&format!("    llg_value_scope_t* _dpi_scope = llg_value_scope_begin({});\n    sv4_t* _dpi_values = llg_value_scope_values(_dpi_scope);\n    (void)_dpi_values;\n", f.formals.len() + 1));
    let mut arguments = Vec::with_capacity(f.formals.len());
    for (index, formal) in f.formals.iter().enumerate() {
        let ty = formal_type(dpi, index)?;
        let source = if formal.is_address() {
            format!("o{index}")
        } else {
            format!("a{index}")
        };
        let copy_in = formal.mode() != IrFormalMode::Output;
        match ty {
            IrDpiType::Vector { width, logic } => {
                let count = chunks(*width);
                let element = if *logic {
                    "svLogicVecVal"
                } else {
                    "svBitVecVal"
                };
                let bytes = count * if *logic { 8 } else { 4 };
                if bytes <= DPI_STACK_VECTOR_BYTES {
                    out.push_str(&format!("    {element} _dpi_v{index}[{count}];\n"));
                } else {
                    out.push_str(&format!(
                        "    {element}* _dpi_v{index} = ({element}*)llg_value_scope_object(llg_value_scope_begin_object(sizeof({element}) * {count}u, NULL));\n"
                    ));
                }
                let init = match (copy_in, logic) {
                    (true, false) => {
                        format!("llg_dpi_export_bits({source}, _dpi_v{index}, {count}u)")
                    }
                    (true, true) => {
                        format!("llg_dpi_export_logic({source}, _dpi_v{index}, {count}u)")
                    }
                    (false, false) => {
                        format!("memset(_dpi_v{index}, 0, sizeof(svBitVecVal) * {count}u)")
                    }
                    (false, true) => format!("llg_dpi_fill_x(_dpi_v{index}, {count}u)"),
                };
                out.push_str(&format!("    {init};\n"));
                arguments.push(format!("_dpi_v{index}"));
            }
            _ if is_aggregate(ty) => {
                let (c_type, table) = layouts[index]
                    .as_ref()
                    .ok_or("DPI aggregate formal without a layout table")?;
                out.push_str(&format!(
                    "    unsigned char* _dpi_b{index} = (unsigned char*)llg_value_scope_object(llg_value_scope_begin_object(sizeof({c_type}), NULL));\n"
                ));
                if copy_in {
                    out.push_str(&format!(
                        "    llg_dpi_to_c(&{table}, {source}, _dpi_b{index});\n"
                    ));
                } else {
                    out.push_str(&format!("    llg_dpi_init_c(&{table}, _dpi_b{index});\n"));
                }
                if let IrDpiType::Open { dims, .. } = ty {
                    out.push_str(&format!(
                        "    llg_dpi_open_t _dpi_h{index} = {{ &{table}, {}, _dpi_b{index} }};\n",
                        dims.len()
                    ));
                    arguments.push(format!("&_dpi_h{index}"));
                } else {
                    arguments.push(format!("_dpi_b{index}"));
                }
            }
            small if formal.is_address() => {
                let c_type = small_c_type(small)?;
                out.push_str(&format!(
                    "    {c_type} _dpi_o{index} = {};\n",
                    dpi_output_init(small, formal, index)?
                ));
                arguments.push(format!("&_dpi_o{index}"));
            }
            small => arguments.push(dpi_input_expr(small, index)?),
        }
    }
    let call = format!("{}({})", dpi.c_name(), arguments.join(", "));
    if let Some(scalar) = ret {
        out.push_str(&format!(
            "    {} _dpi_ret = {call};\n",
            small_c_type(scalar)?
        ));
    } else if f.is_task {
        // SV 35.9 b): an imported task returns 1 only when it returns because
        // of a disable. Without exported subroutines nothing can disable it
        // inside the call, so a nonzero result breaks the protocol, which
        // 35.9 makes a fatal simulation error; nothing is published. The
        // publication loop below tests the status, so no jump crosses the
        // snapshot declarations.
        out.push_str(&format!(
            "    int _dpi_status = {call};\n    if (_dpi_status != 0) llg_dpi_task_protocol_error(\"{}\");\n",
            dpi.c_name()
        ));
    } else {
        out.push_str(&format!("    {call};\n"));
    }
    let string_snapshot = |name: &str, source: &str| {
        format!(
        "    llg_value_scope_t* {name}_owner = llg_value_scope_begin_object(sizeof(llg_string_t), llg_owned_string_drop);\n    llg_string_t* {name} = (llg_string_t*)llg_value_scope_object({name}_owner);\n    *{name} = ({source}) ? llg_string_bytes(({source}), strlen({source})) : (llg_string_t){{0}};\n")
    };
    if matches!(ret, Some(IrDpiType::String)) {
        out.push_str(&string_snapshot("_dpi_string_ret", "_dpi_ret"));
    }
    for (index, formal) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
    {
        let ty = formal_type(dpi, index)?;
        let slot = format!("&_dpi_values[{index}]");
        match ty {
            IrDpiType::String => out.push_str(&string_snapshot(
                &format!("_dpi_s{index}"),
                &format!("_dpi_o{index}"),
            )),
            IrDpiType::Chandle | IrDpiType::Real | IrDpiType::ShortReal => {}
            IrDpiType::Vector { width, logic } => out.push_str(&format!(
                "    sv4_x_to({slot}, {width}, {});\n    llg_dpi_import_{}({slot}, _dpi_v{index}, {}u);\n",
                u8::from(formal.signed),
                if *logic { "logic" } else { "bits" },
                chunks(*width)
            )),
            _ if is_aggregate(ty) => {
                let (_, table) = layouts[index]
                    .as_ref()
                    .ok_or("DPI aggregate formal without a layout table")?;
                out.push_str(&format!(
                    "    sv4_x_to({slot}, {}, {});\n    llg_dpi_from_c(&{table}, _dpi_b{index}, {slot});\n",
                    formal.width,
                    u8::from(formal.signed)
                ));
            }
            small => {
                let value = dpi_packed_result(small, &format!("_dpi_o{index}"), formal.signed)?;
                out.push_str(&format!(
                    "    {}\n",
                    super::super::destinations::assign(&slot, &value)
                ));
            }
        }
    }
    let result_slot = f.formals.len();
    if let Some(scalar) = ret.filter(|ty| {
        matches!(
            ty,
            IrDpiType::Bit | IrDpiType::Logic | IrDpiType::Int { .. }
        )
    }) {
        let value = dpi_packed_result(
            scalar,
            "_dpi_ret",
            f.ret.as_ref().is_some_and(IrType::signed),
        )?;
        out.push_str(&format!(
            "    {}\n",
            super::super::destinations::assign(&format!("&_dpi_values[{result_slot}]"), &value)
        ));
    }
    // A cancelled yielding context import does not copy anything out. Each
    // publication is another cancellation/reentrancy boundary.
    for (index, _) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
    {
        out.push_str(if f.is_task {
            "    if (_dpi_status != 0 || llg_activation_cancelled()) goto _dpi_return;\n"
        } else {
            "    if (llg_activation_cancelled()) goto _dpi_return;\n"
        });
        let statement = match formal_type(dpi, index)? {
            IrDpiType::String => {
                format!("llg_string_move_take(o{index}, _dpi_s{index});")
            }
            IrDpiType::Chandle => format!("*o{index} = _dpi_o{index};"),
            IrDpiType::Real | IrDpiType::ShortReal => {
                format!("llg_ba_d(o{index}, (double)_dpi_o{index});")
            }
            _ => format!("llg_ba(o{index}, _dpi_values[{index}]);"),
        };
        out.push_str(&format!("    {statement}\n"));
    }
    // A label must precede a statement, not a declaration, in C11.
    if f.formals.iter().any(|formal| formal.is_address()) {
        out.push_str("_dpi_return: ;\n");
    }
    let result = match ret {
        Some(IrDpiType::String) => {
            Some("llg_string_t _dpi_result = llg_string_take(_dpi_string_ret);".to_owned())
        }
        Some(IrDpiType::Chandle) => Some("void* _dpi_result = _dpi_ret;".to_owned()),
        Some(IrDpiType::Real | IrDpiType::ShortReal) => {
            Some("double _dpi_result = (double)_dpi_ret;".to_owned())
        }
        Some(_) => {
            // Packed results move into the caller's `_llg_result` before the
            // thunk's scope ends.
            out.push_str(&format!(
                "    sv4_move(_llg_result, &_dpi_values[{result_slot}]);\n"
            ));
            None
        }
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
