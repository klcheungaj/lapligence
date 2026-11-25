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
//!
//! An aggregate with real, shortreal, string or chandle leaves, and a dynamic
//! array or queue actual of an open array, have no payload: the formal is a
//! native value, a real cell array or a container that the caller made for
//! the call ([`Transport`]), and the thunk converts it to the same C-layout
//! buffer and back. The buffer of a dynamic array or queue is sized from the
//! container at the call, and so is the open-array handle's first dimension.

use super::*;
use crate::sim::ir::{
    IrContainer, IrContainerElement, IrContainerKind, IrDpiImport, IrDpiType, IrFormal,
    IrFormalMode, IrModel,
};

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

/// Foreign formals of a subroutine with `formals` IR formals: a column-layout
/// record's further columns follow the declared formals and are no foreign
/// parameters. `columns` is `IrDpiImport::record_columns`/`IrDpiExport`'s.
pub(super) fn foreign_formals<'a>(
    declared: usize,
    columns: impl Fn(usize) -> &'a [usize],
    formals: usize,
    name: &str,
) -> Result<usize, String> {
    let extra = (0..declared)
        .map(|index| columns(index).len().saturating_sub(1))
        .sum::<usize>();
    if declared + extra != formals {
        return Err(format!(
            "DPI-C `{name}` has {declared} foreign formals and {extra} further record columns for {formals} formals"
        ));
    }
    Ok(declared)
}

/// Statements writing the payload of a record formal to `slot` from its
/// `columns`, named in member order.
pub(super) fn dpi_columns_image(index: usize, columns: &[String], slot: &str) -> String {
    format!(
        "    const llg_fixed_array_t* _dpi_c{index}[] = {{ {} }};\n    llg_fixed_array_columns_image_to({slot}, _dpi_c{index}, {}u);\n",
        columns.join(", "),
        columns.len()
    )
}

/// Blocking stores of the record payload `slot`, `payload` bits wide, into
/// the columns at `positions` of `f.formals`, named by `names`.
pub(super) fn dpi_columns_store(
    model: &IrModel,
    f: &IrFunc,
    positions: &[usize],
    names: &[String],
    slot: &str,
    payload: u64,
) -> Result<String, String> {
    let mut out = String::new();
    let mut left = payload;
    for (position, name) in positions.iter().zip(names) {
        let array = f.formals[*position]
            .fixed_array
            .ok_or("DPI-C record column without a descriptor")?;
        let array = model.array(array);
        let bits = array.total * u64::from(array.elem_width);
        out.push_str(&format!(
            "    llg_fixed_array_cells_store({name}, 0, {}ULL, {slot}, {}, {}u, {}, {}, 0);\n",
            array.total,
            left - 1,
            array.elem_width,
            u8::from(array.signed),
            u8::from(array.two_state),
        ));
        left -= bits;
    }
    Ok(out)
}

/// Payload width of an aggregate formal crossing as a whole record.
pub(super) fn record_payload(ty: &IrDpiType) -> Result<u64, String> {
    ty.payload_width()
        .ok_or_else(|| "DPI-C record formal without a payload width".to_owned())
}

fn import_formals(dpi: &IrDpiImport, f: &IrFunc) -> Result<usize, String> {
    foreign_formals(
        dpi.formals().len(),
        |index| dpi.record_columns(index),
        f.formals.len(),
        dpi.c_name(),
    )
}

fn formal_type(dpi: &IrDpiImport, index: usize) -> Result<&IrDpiType, String> {
    dpi.formals().get(index).ok_or_else(|| {
        format!(
            "DPI-C import `{}` formal {index} has no foreign type",
            dpi.c_name()
        )
    })
}

/// C type of a DPI integer (H.7.4, Table H.1). `byte` is plain `char`,
/// whose signedness is the ABI's (unsigned on AArch64 Linux, PowerPC and
/// RISC-V), so a foreign `char` is sign-extended explicitly when it becomes
/// an SV value ([`dpi_packed_result`]); declaring `int8_t` instead would give
/// the import a prototype that differs from the foreign definition.
pub(super) fn int_c_type(bytes: u8, signed: bool) -> Result<&'static str, String> {
    Ok(match (bytes, signed) {
        (1, true) => "char",
        (1, false) => "unsigned char",
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
pub(super) fn small_c_type(ty: &IrDpiType) -> Result<&'static str, String> {
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

pub(super) fn is_aggregate(ty: &IrDpiType) -> bool {
    matches!(
        ty,
        IrDpiType::Array { .. }
            | IrDpiType::Struct(_)
            | IrDpiType::Open { .. }
            | IrDpiType::OpenDynamic { .. }
    )
}

/// What an aggregate formal is to the model, which decides how its thunk
/// converts it.
enum Transport<'a> {
    /// One packed payload (`sv4_t`).
    Payload,
    /// A native value (`llg_value_t*`) of this type.
    Native(&'a IrContainerElement),
    /// The cells of a real fixed array (`double*`).
    Reals,
    /// A container of packed cells, each the payload of one element.
    Cells(&'a IrContainer),
    /// A container of values: a fixed array of strings, handles or records,
    /// or a dynamic array or queue of an element type that is not packed.
    Items(&'a IrContainer),
}

fn transport<'a>(model: &'a IrModel, formal: &IrFormal) -> Result<Transport<'a>, String> {
    if let Some(value) = formal.native_value {
        let value = model
            .native_values
            .get(value)
            .ok_or("DPI-C native formal is out of range")?;
        return model
            .native_types
            .get(value.ty)
            .map(Transport::Native)
            .ok_or_else(|| "DPI-C native formal type is out of range".to_owned());
    }
    if formal.real_array.is_some() {
        return Ok(Transport::Reals);
    }
    if let Some(container) = formal.container {
        let container = model
            .containers
            .get(container)
            .ok_or("DPI-C container formal is out of range")?;
        if matches!(container.kind, IrContainerKind::Associative { .. }) {
            return Err("an associative array reached a DPI-C thunk".to_owned());
        }
        return Ok(if container.element.is_packed() {
            Transport::Cells(container)
        } else {
            Transport::Items(container)
        });
    }
    Ok(Transport::Payload)
}

/// C type of the runtime container a [`Transport::Cells`] or
/// [`Transport::Items`] formal points at.
fn container_c_type(container: &IrContainer) -> &'static str {
    match (container.element.is_packed(), &container.kind) {
        (true, IrContainerKind::Queue { .. }) => "llg_queue_t",
        (true, _) => "llg_dyn_array_t",
        (false, IrContainerKind::Queue { .. }) => "llg_queue_value_array_t",
        (false, _) => "llg_dyn_value_array_t",
    }
}

/// Sized unpacked dimensions (slowest first) and the type below them.
type ArrayLevels<'a> = (Vec<(i32, i32)>, &'a IrDpiType);

/// The sized unpacked dimensions at the top of `ty` and the type below them.
fn array_levels(ty: &IrDpiType) -> Result<ArrayLevels<'_>, String> {
    let mut dims = Vec::new();
    let mut current = ty;
    loop {
        match current {
            IrDpiType::Array {
                dims: levels,
                element,
            } => {
                dims.extend(levels.iter().copied());
                current = element;
            }
            IrDpiType::Open {
                dims: levels,
                element,
            } if dims.is_empty() => {
                for level in levels {
                    dims.push(level.ok_or("unspecialized DPI open array")?);
                }
                current = element;
            }
            _ => return Ok((dims, current)),
        }
    }
}

/// Unpacked dimensions directly below a container's own: those of its
/// element type, which the container does not flatten.
fn element_dimensions(element: &IrContainerElement) -> usize {
    match element {
        IrContainerElement::FixedArray {
            dimensions,
            element,
        } => dimensions.len() + element_dimensions(element),
        _ => 0,
    }
}

/// Whether an output formal of type `ty` is a record or an array of records,
/// whose C image starts all zero (decision S40-D1).
fn struct_output(ty: &IrDpiType) -> bool {
    match ty {
        IrDpiType::Struct(_) => true,
        IrDpiType::Array { element, .. }
        | IrDpiType::Open { element, .. }
        | IrDpiType::OpenDynamic { element, .. } => struct_output(element),
        _ => false,
    }
}

pub(super) fn chunks(width: u32) -> u64 {
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
pub(super) fn dpi_param_type(ty: &IrDpiType, form: &IrFormal) -> Result<String, String> {
    let address = form.is_address();
    Ok(match ty {
        IrDpiType::Open { .. } | IrDpiType::OpenDynamic { .. } => {
            "const svOpenArrayHandle".to_owned()
        }
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
    for (idx, form) in f.formals.iter().take(import_formals(dpi, f)?).enumerate() {
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
}\n\
/* SV byte from a C char (Table H.1) whatever the ABI's char signedness. */\n\
static int64_t llg_dpi_byte_value(char value) {\n\
    return (int64_t)((unsigned char)value ^ 0x80u) - 0x80;\n\
}\n\n"
}

/// The emitted layout of one aggregate formal: its C type and table, or,
/// for a dynamic array or queue (`dynamic`), those of one element.
struct Layout {
    c_type: String,
    table: String,
    dynamic: bool,
}

/// Static layout tables of the aggregate formals of one thunk.
pub(super) struct LayoutTables {
    pub(super) prefix: String,
    pub(super) count: usize,
    pub(super) out: String,
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
    pub(super) fn emit(&mut self, ty: &IrDpiType) -> Result<(String, String), String> {
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

    /// Emit the C type and layout table of `ty` for a value stored as
    /// `shape` outside the payload. A packed leaf, or an aggregate that the
    /// value keeps as one packed leaf, gets its payload table.
    fn emit_native(
        &mut self,
        ty: &IrDpiType,
        shape: &IrContainerElement,
    ) -> Result<(String, String), String> {
        let leaf = |tables: &mut Self, c_type: &str, kind: &str| {
            let (c, t) = tables.fresh();
            tables.out.push_str(&format!("typedef {c_type} {c};\n"));
            tables.table(&t, &format!(".kind = {kind}, .size = sizeof({c})"));
            (c, t)
        };
        match (ty, shape) {
            (_, IrContainerElement::Packed { .. }) => self.emit(ty),
            (IrDpiType::Real, IrContainerElement::Real { .. }) => {
                Ok(leaf(self, "double", "LLG_DPI_REAL"))
            }
            (IrDpiType::ShortReal, IrContainerElement::Real { .. }) => {
                Ok(leaf(self, "float", "LLG_DPI_SHORTREAL"))
            }
            (IrDpiType::String, IrContainerElement::String) => {
                Ok(leaf(self, "const char *", "LLG_DPI_STRING"))
            }
            (IrDpiType::Chandle, IrContainerElement::Chandle) => {
                Ok(leaf(self, "void *", "LLG_DPI_CHANDLE"))
            }
            (
                IrDpiType::Struct(members),
                IrContainerElement::Aggregate {
                    members: shapes, ..
                },
            ) if members.len() == shapes.len() => {
                let emitted = members
                    .iter()
                    .zip(shapes)
                    .map(|(member, shape)| self.emit_native(member, &shape.element))
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
                self.table(
                    &t,
                    &format!(
                        ".kind = LLG_DPI_STRUCT, .size = sizeof({c}), .count = {}u, .members = {t}_m",
                        members.len()
                    ),
                );
                Ok((c, t))
            }
            (IrDpiType::Array { .. }, IrContainerElement::FixedArray { .. }) => {
                let (dims, element) = array_levels(ty)?;
                self.emit_native_levels(&dims, element, shape)
            }
            _ => Err(format!(
                "DPI-C type {ty:?} does not match the storage of its formal"
            )),
        }
    }

    /// [`Self::emit_native`] for the array type of dimensions `dims` over
    /// `element`: each fixed-array level of `shape` takes its dimensions.
    fn emit_native_levels(
        &mut self,
        dims: &[(i32, i32)],
        element: &IrDpiType,
        shape: &IrContainerElement,
    ) -> Result<(String, String), String> {
        if dims.is_empty() {
            return self.emit_native(element, shape);
        }
        match shape {
            IrContainerElement::FixedArray {
                dimensions,
                element: below,
            } if dimensions.len() <= dims.len()
                && dimensions.iter().zip(dims).all(|(stored, declared)| {
                    crate::sim::ir::dimension_size(stored.0, stored.1)
                        == crate::sim::ir::dimension_size(declared.0, declared.1)
                }) =>
            {
                let (level, rest) = dims.split_at(dimensions.len());
                let inner = self.emit_native_levels(rest, element, below)?;
                Ok(self.wrap_levels(level, inner))
            }
            IrContainerElement::Packed { .. } => self.emit(&IrDpiType::Array {
                dims: dims.to_vec(),
                element: Box::new(element.clone()),
            }),
            _ => Err(format!(
                "DPI-C array of {element:?} does not match the storage of its formal"
            )),
        }
    }

    /// The `LLG_DPI_ARRAY` levels of the dimensions `dims` of one array
    /// value around `inner`, slowest outermost; every level but the
    /// outermost continues its flat item list.
    fn wrap_levels(&mut self, dims: &[(i32, i32)], inner: (String, String)) -> (String, String) {
        let (mut inner_c, mut inner_t) = inner;
        for (position, (left, right)) in dims.iter().enumerate().rev() {
            let count = crate::sim::ir::dimension_size(*left, *right);
            let (c, t) = self.fresh();
            self.out
                .push_str(&format!("typedef {inner_c} {c}[{count}];\n"));
            self.table(
                &t,
                &format!(
                    ".kind = LLG_DPI_ARRAY, {}.size = sizeof({c}), .left = {left}, .right = {right}, .count = {count}u, .element = &{inner_t}",
                    if position == 0 { "" } else { ".joined = 1u, " }
                ),
            );
            inner_c = c;
            inner_t = t;
        }
        (inner_c, inner_t)
    }

    /// The layout of aggregate formal `ty` reaching the thunk as
    /// `transport`.
    fn emit_formal(&mut self, ty: &IrDpiType, transport: &Transport<'_>) -> Result<Layout, String> {
        let fixed = |(c_type, table)| Layout {
            c_type,
            table,
            dynamic: false,
        };
        // The element of a dynamic array or queue: the first dimension is a
        // table the thunk fills at each call.
        if let IrDpiType::OpenDynamic { element } = ty {
            let (c_type, table) = match transport {
                Transport::Cells(_) => self.emit(element)?,
                Transport::Items(container) => self.emit_native(element, &container.element)?,
                _ => return Err("a DPI-C open array of a container has no container".to_owned()),
            };
            return Ok(Layout {
                c_type,
                table,
                dynamic: true,
            });
        }
        match transport {
            Transport::Payload => self.emit(ty).map(fixed),
            Transport::Native(shape) => {
                let (dims, element) = array_levels(ty)?;
                self.emit_native_levels(&dims, element, shape).map(fixed)
            }
            Transport::Reals => {
                let (dims, element) = array_levels(ty)?;
                let inner =
                    self.emit_native(element, &IrContainerElement::Real { shortreal: false })?;
                Ok(fixed(self.wrap_levels(&dims, inner)))
            }
            Transport::Items(container) => {
                // The container lists the elements of its own dimensions
                // flat; the dimensions of its element type stay below.
                let (dims, element) = array_levels(ty)?;
                let own = dims
                    .len()
                    .checked_sub(element_dimensions(&container.element))
                    .filter(|own| *own > 0)
                    .ok_or("DPI-C array formal has fewer dimensions than its storage")?;
                let (level, rest) = dims.split_at(own);
                let inner = self.emit_native_levels(rest, element, &container.element)?;
                Ok(fixed(self.wrap_levels(level, inner)))
            }
            Transport::Cells(_) => {
                Err("a sized DPI-C aggregate formal is a container of packed cells".to_owned())
            }
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

pub(super) fn dpi_packed_result(
    ty: &IrDpiType,
    source: &str,
    signed: bool,
) -> Result<String, String> {
    Ok(match ty {
        IrDpiType::Bit => format!("sv4_from_u64((uint64_t){source}, 1, {})", u8::from(signed)),
        IrDpiType::Logic => format!("llg_dpi_sv4_from_logic({source}, {})", u8::from(signed)),
        IrDpiType::Int {
            bytes: 1,
            signed: true,
        } => format!("sv4_from_i64(llg_dpi_byte_value({source}), 8)"),
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
pub(super) fn render_dpi_thunk(model: &IrModel, f: &IrFunc) -> Result<String, String> {
    render_thunk(model, f, &f.c_name, false)
}

/// Name of the thunk body a foreign call context runs for suspendable
/// import `f` (see [`render_dpi_context_shim`]).
fn context_body_name(f: &IrFunc) -> String {
    format!("{}_fcb", f.c_name)
}

/// A context imported task of a design whose exported tasks can consume
/// time (ADV-018): the ordinary thunk, renamed, runs on a foreign call
/// context, and the coroutine `<c_name>` started by the caller hands it the
/// run token, runs every exported task frame it requests as an arena call
/// (resume point `resume`) and finishes when the thunk returned. `dispatch`
/// is the frame's resume dispatch. The frame holds the thunk's parameters as
/// coroutine fields, `_dpi_ctx` and `_dpi_slot`.
pub(super) fn render_dpi_context_shim(
    model: &IrModel,
    f: &IrFunc,
    resume: u32,
    dispatch: &str,
) -> Result<String, String> {
    if !f.is_task || !dpi_import(f)?.is_context() {
        return Err(format!(
            "DPI-C import `{}` cannot suspend: only a context imported task can",
            f.c_name
        ));
    }
    let name = &f.c_name;
    let body = context_body_name(f);
    let mut out = render_thunk(model, f, &body, true)?;
    let arguments = super::functions::frame_param_fields(f)
        .into_iter()
        .map(|(ty, field)| {
            if ty == "sv4_t" {
                format!("&F->{field}")
            } else {
                format!("F->{field}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!(
        "static void {name}_fct(void* frame) {{
    {name}_frame_t* F = ({name}_frame_t*)frame;
    {body}({arguments});
}}
"
    ));
    out.push_str(&format!(
        "static llg_co_status_t {name}(llg_co_frame_t* co, llg_co_chain_t* ch) {{
    {name}_frame_t* F = ({name}_frame_t*)co;
{dispatch}    F->_dpi_ctx = llg_dpi_context_begin(ch, {name}_fct, F);
    if (!F->_dpi_ctx) {{
        {name}_fct(F);
        LLG_CO_EXIT_CHECK(ch);
        return LLG_CO_DONE;
    }}
    for (;;) {{
        F->_dpi_slot = llg_dpi_context_run(F->_dpi_ctx);
        if (!F->_dpi_slot) break;
        LLG_CO_CALL_ARENA(co, ch, {resume}, F->_dpi_slot->desc, F->_dpi_slot);
    }}
    llg_dpi_context_end(F->_dpi_ctx);
    F->_dpi_ctx = NULL;
    LLG_CO_EXIT_CHECK(ch);
    return LLG_CO_DONE;
}}

"
    ));
    Ok(out)
}

/// The packed cells of a thunk of `f`, one per formal and one for the result,
/// in a value scope: the thunk's own storage, or the heap when the cleanup of
/// a killed process may release the scope after the thunk `suspends`.
pub(super) fn thunk_values(f: &IrFunc, suspends: bool) -> String {
    let count = f.formals.len() + 1;
    if suspends {
        format!(
            "    llg_value_scope_t* _dpi_scope = llg_value_scope_begin({count});\n    sv4_t* _dpi_values = llg_value_scope_values(_dpi_scope);\n    (void)_dpi_values;\n"
        )
    } else {
        format!(
            "    llg_value_scope_t _dpi_scope;\n    sv4_t _dpi_values[{count}] = {{0}};\n    (void)llg_value_scope_register(&_dpi_scope, _dpi_values, {count});\n"
        )
    }
}

/// The thunk of import `f`, named `name`. An `abandonable` thunk runs on a
/// foreign call context: when the context unwinds a killed process, it
/// returns right after the foreign call without touching the process's
/// storage.
fn render_thunk(
    model: &IrModel,
    f: &IrFunc,
    name: &str,
    abandonable: bool,
) -> Result<String, String> {
    let dpi = dpi_import(f)?;
    // Without a context import or an export no foreign code can observe
    // the call-chain state, so calls stay uninstrumented (SV 35.5.3).
    let instrumented = model.dpi_context_used();
    let scope = if dpi.is_context() {
        let scope = dpi
            .scope()
            .ok_or_else(|| format!("context DPI-C import `{}` has no scope", dpi.c_name()))?;
        format!("&llg_dpi_scopes[{scope}]")
    } else {
        "NULL".to_owned()
    };
    let ret = if f.is_task { None } else { dpi.ret() };
    let mut tables = LayoutTables {
        prefix: format!("llg_dpi_{}_", f.c_name),
        count: 0,
        out: String::new(),
    };
    let declared = import_formals(dpi, f)?;
    let column_names = |index: usize| {
        let prefix = if f.formals[index].is_address() {
            "o"
        } else {
            "a"
        };
        dpi.record_columns(index)
            .iter()
            .map(|position| format!("{prefix}{position}"))
            .collect::<Vec<_>>()
    };
    let mut layouts = Vec::with_capacity(declared);
    for (index, formal) in f.formals.iter().take(declared).enumerate() {
        let ty = formal_type(dpi, index)?;
        layouts.push(if is_aggregate(ty) {
            Some(
                tables
                    .emit_formal(ty, &transport(model, formal)?)
                    .map_err(|error| {
                        format!("DPI-C import `{}` formal {index}: {error}", dpi.c_name())
                    })?,
            )
        } else {
            None
        });
    }
    let mut out = tables.out;
    out.push_str(&format!(
        "static {} {}({}) {{\n",
        internal_return_type(f),
        name,
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
    out.push_str(&format!("    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{ llg_msg(LLG_MSG_RUNTIME_ERROR, NULL, \"recursion limit exceeded\"); {guard} }}\n"));
    out.push_str("    llg_value_scope_t* _dpi_mark = llg_value_scope_mark();\n");
    out.push_str(&thunk_values(f, abandonable));
    let mut arguments = Vec::with_capacity(declared);
    for (index, formal) in f.formals.iter().take(declared).enumerate() {
        let ty = formal_type(dpi, index)?;
        let columns = column_names(index);
        let copy_in = formal.mode() != IrFormalMode::Output;
        let source = if !columns.is_empty() {
            let slot = format!("&_dpi_values[{index}]");
            if copy_in {
                out.push_str(&dpi_columns_image(index, &columns, &slot));
            }
            slot
        } else if formal.is_address() {
            format!("o{index}")
        } else {
            format!("a{index}")
        };
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
            _ if is_aggregate(ty) && !matches!(transport(model, formal)?, Transport::Payload) => {
                let layout = layouts[index]
                    .as_ref()
                    .ok_or("DPI aggregate formal without a layout table")?;
                let (c_type, table) = (&layout.c_type, &layout.table);
                let stored = transport(model, formal)?;
                // The container behind the formal, private to this call.
                if let Transport::Cells(container) | Transport::Items(container) = &stored {
                    let container_type = container_c_type(container);
                    out.push_str(&format!(
                        "    {}{container_type}* _dpi_q{index} = ({}{container_type}*){source};\n",
                        if formal.is_address() { "" } else { "const " },
                        if formal.is_address() { "" } else { "const " },
                    ));
                }
                // The table and buffer size of the whole value: static, or
                // built here from the container's size at this call.
                let (whole, bytes) = if layout.dynamic {
                    out.push_str(&format!(
                        "    llg_dpi_type_t _dpi_t{index};\n    llg_dpi_dynamic_level(&_dpi_t{index}, &{table}, _dpi_q{index}->size);\n"
                    ));
                    (
                        format!("&_dpi_t{index}"),
                        format!("_dpi_t{index}.size ? _dpi_t{index}.size : 1u"),
                    )
                } else {
                    (format!("&{table}"), format!("sizeof({c_type})"))
                };
                out.push_str(&format!(
                    "    unsigned char* _dpi_b{index} = (unsigned char*)llg_value_scope_object(llg_value_scope_begin_object({bytes}, NULL));\n"
                ));
                if copy_in {
                    out.push_str(&match &stored {
                        Transport::Native(_) => format!(
                            "    llg_dpi_value_to_c({whole}, {source}, _dpi_b{index});\n"
                        ),
                        Transport::Reals => format!(
                            "    llg_dpi_reals_to_c({whole}, {source}, _dpi_b{index});\n"
                        ),
                        Transport::Cells(_) => format!(
                            "    llg_dpi_cells_to_c({whole}, _dpi_q{index}->data, _dpi_q{index}->size, _dpi_b{index});\n"
                        ),
                        Transport::Items(_) => format!(
                            "    llg_dpi_items_to_c({whole}, _dpi_q{index}->element, _dpi_q{index}->data, _dpi_q{index}->size, _dpi_b{index});\n"
                        ),
                        Transport::Payload => String::new(),
                    });
                } else {
                    // Outputs start at the type default: every byte zero and
                    // each string leaf an empty string (decision S40-D1).
                    out.push_str(&format!(
                        "    llg_dpi_init_native({whole}, _dpi_b{index});\n"
                    ));
                }
                match ty {
                    IrDpiType::Open { dims, .. } => {
                        out.push_str(&format!(
                            "    llg_dpi_open_t _dpi_h{index} = {{ {whole}, {}, _dpi_b{index} }};\n",
                            dims.len()
                        ));
                        arguments.push(format!("&_dpi_h{index}"));
                    }
                    IrDpiType::OpenDynamic { .. } => {
                        out.push_str(&format!(
                            "    llg_dpi_open_t _dpi_h{index} = {{ {whole}, 1, _dpi_b{index} }};\n"
                        ));
                        arguments.push(format!("&_dpi_h{index}"));
                    }
                    _ => arguments.push(format!("_dpi_b{index}")),
                }
            }
            _ if is_aggregate(ty) => {
                let layout = layouts[index]
                    .as_ref()
                    .ok_or("DPI aggregate formal without a layout table")?;
                let (c_type, table) = (&layout.c_type, &layout.table);
                out.push_str(&format!(
                    "    unsigned char* _dpi_b{index} = (unsigned char*)llg_value_scope_object(llg_value_scope_begin_object(sizeof({c_type}), NULL));\n"
                ));
                if copy_in {
                    out.push_str(&format!(
                        "    llg_dpi_to_c(&{table}, {source}, _dpi_b{index});\n"
                    ));
                } else if struct_output(ty) {
                    // An output record, or array of records, starts with every
                    // byte zero, so its 4-state members read 0, not X (decision
                    // S40-D1; 35.5.1.2 leaves output initial values open).
                    out.push_str(&format!(
                        "    memset(_dpi_b{index}, 0, sizeof({c_type}));\n"
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
    let c_name_literal = c_string_literal(dpi.c_name());
    if instrumented {
        // Save the chain state and enter this call's context (35.5.3).
        out.push_str(&format!(
            "    llg_dpi_call_t _dpi_call;\n    llg_dpi_import_enter(&_dpi_call, {scope}, {}, depth);\n",
            u8::from(f.is_task)
        ));
    }
    // Foreign code is abandoned at the fatal svdpi.h access of an open-array
    // handle (S40-D6) and when the run ended in an export it called (S41-D9);
    // other imports pay no setjmp.
    let guarded = dpi.is_context() && model.dpi_exports_used()
        || (0..declared)
            .map(|index| formal_type(dpi, index))
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|ty| matches!(ty, IrDpiType::Open { .. } | IrDpiType::OpenDynamic { .. }));
    if guarded {
        let restore = if instrumented {
            "llg_dpi_import_abort(&_dpi_call);"
        } else {
            "llg_dpi_next_site = NULL;"
        };
        out.push_str(&format!(
            "    llg_dpi_guard_t _dpi_guard;\n    _dpi_guard.mark = _dpi_mark;\n    llg_dpi_guard_push(&_dpi_guard);\n    if (setjmp(_dpi_guard.jump) != 0) {{\n        /* The foreign code was abandoned: nothing is copied out. */\n        {restore}\n        llg_value_scopes_end_since(_dpi_guard.mark);\n        {guard}\n    }}\n"
        ));
    }
    if let Some(scalar) = ret {
        out.push_str(&format!(
            "    {} _dpi_ret = {call};\n",
            small_c_type(scalar)?
        ));
    } else if f.is_task {
        out.push_str(&format!("    int _dpi_status = {call};\n"));
    } else {
        out.push_str(&format!("    {call};\n"));
    }
    if guarded {
        out.push_str("    llg_dpi_guard_pop(&_dpi_guard);\n");
    }
    if abandonable {
        // The process was killed while the foreign code waited in an
        // export: its storage belongs to its own cleanup now (ADV-018).
        out.push_str(&format!(
            "    if (LLG_CO_UNLIKELY(llg_dpi_context_abandoned())) {{\n        llg_dpi_import_abandoned(&_dpi_call, _dpi_status, {c_name_literal});\n        return;\n    }}\n"
        ));
    }
    if instrumented {
        out.push_str(&format!(
            "    llg_dpi_import_leave(&_dpi_call, {c_name_literal}, {});\n",
            u8::from(f.is_task)
        ));
    } else {
        // Without call-chain state the caller position stays readable
        // through `llg_dpi_next_site` for the call alone (svGetCallerInfo).
        out.push_str("    llg_dpi_next_site = NULL;\n");
    }
    let has_outputs = f.formals.iter().any(|formal| formal.is_address());
    if f.is_task {
        // SV 35.9 b): an imported task returns 1 exactly when it returns
        // because of a disable, which only an exported subroutine it calls
        // can cause. Any other result is a fatal error; in both cases
        // nothing is published. The publication loop below tests the
        // status, so no jump crosses the snapshot declarations.
        out.push_str(&format!(
            "    {}llg_dpi_task_status(_dpi_status, {c_name_literal});\n",
            if has_outputs {
                "int _dpi_skip = "
            } else {
                "(void)"
            }
        ));
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
        .take(declared)
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
            // A value outside the payload is read back from its C buffer
            // when it is published.
            _ if is_aggregate(ty) && !matches!(transport(model, formal)?, Transport::Payload) => {}
            _ if is_aggregate(ty) => {
                let table = &layouts[index]
                    .as_ref()
                    .ok_or("DPI aggregate formal without a layout table")?
                    .table;
                let width = if dpi.record_columns(index).is_empty() {
                    u64::from(formal.width)
                } else {
                    record_payload(ty)?
                };
                out.push_str(&format!(
                    "    sv4_x_to({slot}, {width}, {});\n    llg_dpi_from_c(&{table}, _dpi_b{index}, {slot});\n",
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
    for (index, formal) in f
        .formals
        .iter()
        .take(declared)
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
    {
        out.push_str(if f.is_task {
            "    if (_dpi_skip || llg_activation_cancelled()) goto _dpi_return;\n"
        } else {
            "    if (llg_activation_cancelled()) goto _dpi_return;\n"
        });
        let stored = transport(model, formal)?;
        let whole = layouts[index].as_ref().map(|layout| {
            if layout.dynamic {
                format!("&_dpi_t{index}")
            } else {
                format!("&{}", layout.table)
            }
        });
        let statement = match formal_type(dpi, index)? {
            // The formal is storage the caller made for this call, which
            // copies it to the actual once the thunk has returned.
            ty if is_aggregate(ty) && !matches!(stored, Transport::Payload) => {
                let whole = whole.ok_or("DPI aggregate formal without a layout table")?;
                match stored {
                    Transport::Native(_) => {
                        format!("llg_dpi_value_from_c({whole}, _dpi_b{index}, o{index});")
                    }
                    Transport::Reals => {
                        format!("llg_dpi_reals_from_c({whole}, _dpi_b{index}, o{index});")
                    }
                    Transport::Cells(_) => format!(
                        "llg_dpi_cells_from_c({whole}, _dpi_b{index}, _dpi_q{index}->data, _dpi_q{index}->size);"
                    ),
                    _ => format!(
                        "llg_dpi_items_from_c({whole}, _dpi_b{index}, _dpi_q{index}->element, _dpi_q{index}->data, _dpi_q{index}->size);"
                    ),
                }
            }
            IrDpiType::String => {
                format!("llg_string_move_take(o{index}, _dpi_s{index});")
            }
            IrDpiType::Chandle => format!("*o{index} = _dpi_o{index};"),
            IrDpiType::Real | IrDpiType::ShortReal => {
                format!("llg_ba_d(o{index}, (double)_dpi_o{index});")
            }
            ty if !dpi.record_columns(index).is_empty() => {
                out.push_str(&dpi_columns_store(
                    model,
                    f,
                    dpi.record_columns(index),
                    &column_names(index),
                    &format!("&_dpi_values[{index}]"),
                    record_payload(ty)?,
                )?);
                continue;
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
