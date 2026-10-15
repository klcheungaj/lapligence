//! Vpi.

use super::*;
use crate::sim::ir::{IrVpiArgRef, IrVpiCompileCall, IrVpiConstant, IrVpiObject, IrVpiVarKind};

/// Marker suffix that keeps a port apart from the declaration it connects,
/// which has the same hierarchical name (see `collection/vpi.rs`).
const PORT_MARKER: &str = "\u{1f}\u{1e}port";

/// One emitted catalog row.
struct Entry {
    full: String,
    name: String,
    definition_name: Option<String>,
    file: Option<String>,
    line: u32,
    type_code: &'static str,
    subtype: &'static str,
    width: u32,
    signed: bool,
    real: bool,
    net: bool,
    two_state: bool,
    shortreal: bool,
    top: bool,
    local_param: bool,
    range: Option<(i64, i64)>,
    dims: Vec<(i32, i32)>,
    port_index: u32,
    port_target: Option<String>,
    packed: String,
    real_value: String,
    net_pointer: String,
    constant: Option<IrVpiConstant>,
    time_unit_fs: u64,
}

impl Entry {
    fn scope(full: String, name: String, type_code: &'static str) -> Self {
        Self {
            full,
            name,
            definition_name: None,
            file: None,
            line: 0,
            type_code,
            subtype: "0",
            width: 0,
            signed: false,
            real: false,
            net: false,
            two_state: false,
            shortreal: false,
            top: false,
            local_param: false,
            range: None,
            dims: Vec::new(),
            port_index: 0,
            port_target: None,
            packed: "NULL".to_owned(),
            real_value: "NULL".to_owned(),
            net_pointer: "NULL".to_owned(),
            constant: None,
            time_unit_fs: 0,
        }
    }
}

fn split_name(name: &str) -> Vec<&str> {
    name.split('\u{1f}').collect()
}

/// Storage of one signal after alias resolution: `(packed, real, net)`.
fn signal_storage(model: &IrModel, index: usize) -> Option<(String, String, String)> {
    let mut current = index;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return None;
        }
        let signal = model.signals.get(current)?;
        if let Some(alias) = signal.alias {
            current = alias;
            continue;
        }
        let net = signal
            .net_driver
            .and_then(|(group, _)| model.net_groups.get(group))
            .map_or_else(|| "NULL".to_owned(), |group| format!("&{}", group.c_name));
        return match signal.ty {
            IrType::Real { .. } => Some((
                "NULL".to_owned(),
                format!("&{}", signal.c_name),
                "NULL".to_owned(),
            )),
            IrType::Packed { .. } if !signal.net_alias.is_empty() => Some((
                format!("&llg_net_alias_{current}.visible"),
                "NULL".to_owned(),
                "NULL".to_owned(),
            )),
            IrType::Packed { .. } => Some((format!("&{}", signal.c_name), "NULL".to_owned(), net)),
        };
    }
}

/// The ordered catalog rows: owned objects, then synthesized missing scope
/// prefixes. The row index is the C array index referenced by call sites.
fn catalog(model: &IrModel, objects: &[IrVpiObject]) -> Vec<Entry> {
    use std::collections::BTreeMap;

    let mut entries = BTreeMap::<String, Entry>::new();
    for object in objects {
        let (packed, real_value, net_pointer, dims) = if let Some(signal) = object.signal {
            let Some((packed, real_value, net)) = signal_storage(model, signal) else {
                // A malformed/optimized-away storage reference is not a
                // valid public object; omitting it keeps emitted C fail-closed.
                continue;
            };
            (packed, real_value, net, Vec::new())
        } else if let Some(array) = object.array {
            let Some(array) = model.arrays.get(array) else {
                continue;
            };
            // Sparse arrays live in a keyed container, not flat element
            // cells; they are not VPI-visible (documented limit).
            if array.sparse() || array.activation || array.descriptor {
                continue;
            }
            let base = format!("&{}[0]", array.c_name);
            if array.real {
                (
                    "NULL".to_owned(),
                    base,
                    "NULL".to_owned(),
                    array.dims.clone(),
                )
            } else {
                (
                    base,
                    "NULL".to_owned(),
                    "NULL".to_owned(),
                    array.dims.clone(),
                )
            }
        } else {
            (
                "NULL".to_owned(),
                "NULL".to_owned(),
                "NULL".to_owned(),
                Vec::new(),
            )
        };
        let element_type = if object.real {
            if object.var_kind == IrVpiVarKind::ShortReal {
                "vpiShortRealVar"
            } else {
                "vpiRealVar"
            }
        } else if object.net {
            "vpiNet"
        } else {
            object.var_kind.c_type()
        };
        let (type_code, subtype) = match object.kind {
            IrVpiObjectKind::Reg => (object.var_kind.c_type(), "0"),
            IrVpiObjectKind::RealVar => (element_type, "0"),
            IrVpiObjectKind::Net => ("vpiNet", object.net_type.unwrap_or("vpiWire")),
            IrVpiObjectKind::RegArray | IrVpiObjectKind::NetArray => {
                (object.kind.c_type(), element_type)
            }
            IrVpiObjectKind::Port => (object.kind.c_type(), object.direction.c_code()),
            kind => (kind.c_type(), "0"),
        };
        entries.insert(
            object.full_name.clone(),
            Entry {
                full: object.full_name.clone(),
                name: object.name.clone(),
                definition_name: object.definition_name.clone(),
                file: object.file.clone(),
                line: object.line,
                type_code,
                subtype,
                width: object.width,
                signed: object.signed,
                real: object.real,
                net: object.net,
                two_state: object.two_state,
                shortreal: object.var_kind == IrVpiVarKind::ShortReal,
                top: object.top,
                local_param: object.local_param,
                range: object.packed_range,
                dims,
                port_index: object.port_index,
                port_target: object.port_target.clone(),
                packed,
                real_value,
                net_pointer,
                constant: object.constant.clone(),
                time_unit_fs: object.time_unit_fs,
            },
        );
    }

    // A declaration may be retained under a scope that has no captured
    // node. Add only missing prefixes and leave their source location and
    // definition empty rather than inventing frontend provenance.
    let paths = entries.keys().cloned().collect::<Vec<_>>();
    for path in paths {
        let path = path.strip_suffix(PORT_MARKER).unwrap_or(&path);
        let parts = split_name(path);
        for depth in 1..parts.len() {
            let prefix = parts[..depth].join("\u{1f}");
            let kind = if depth == 1 {
                "vpiModule"
            } else {
                "vpiGenScope"
            };
            entries.entry(prefix.clone()).or_insert_with(|| {
                let mut scope = Entry::scope(prefix, parts[depth - 1].to_owned(), kind);
                scope.top = depth == 1;
                scope
            });
        }
    }

    if entries.is_empty() {
        let design_name = if model.design_name().is_empty() {
            "llg".to_owned()
        } else {
            model.design_name().to_owned()
        };
        let mut root = Entry::scope(design_name.clone(), design_name, "vpiModule");
        root.top = true;
        entries.insert(root.full.clone(), root);
    }
    entries.into_values().collect()
}

/// Catalog row of every full name, for parent links and call references.
fn catalog_indices(entries: &[Entry]) -> std::collections::HashMap<&str, usize> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.full.as_str(), index))
        .collect()
}

/// The fallback for models built without owned catalog capture: one entry
/// per waveform-visible signal and array under synthesized scopes.
fn fallback_objects(model: &IrModel) -> Vec<IrVpiObject> {
    let mut objects = Vec::new();
    for (index, signal) in model.signals.iter().enumerate() {
        let Some(hdl_name) = signal.hdl_name.as_deref() else {
            continue;
        };
        // A net driver or alias has externally visible identity even when
        // optimization found no generated HDL read.
        if signal.omit && signal.net_driver.is_none() && signal.net_alias.is_empty() {
            continue;
        }
        let parts = split_name(hdl_name);
        if parts.len() < 2 {
            continue;
        }
        let (width, signed, real) = match signal.ty {
            IrType::Packed { width, signed, .. } => (width, signed, false),
            IrType::Real { .. } => (0, false, true),
        };
        let net = signal.net_driver.is_some() || !signal.net_alias.is_empty();
        objects.push(IrVpiObject {
            full_name: hdl_name.to_owned(),
            name: parts.last().copied().unwrap_or("").to_owned(),
            kind: if real {
                IrVpiObjectKind::RealVar
            } else if net {
                IrVpiObjectKind::Net
            } else {
                IrVpiObjectKind::Reg
            },
            width,
            signed,
            real,
            net,
            signal: Some(index),
            var_kind: if real {
                IrVpiVarKind::Real
            } else {
                IrVpiVarKind::Logic
            },
            packed_range: (width > 1).then(|| (i64::from(width) - 1, 0)),
            ..IrVpiObject::default()
        });
    }
    for (index, array) in model.arrays.iter().enumerate() {
        let parts = split_name(&array.hdl_name);
        if array.activation || array.descriptor || array.sparse() || parts.len() < 2 {
            continue;
        }
        objects.push(IrVpiObject {
            full_name: array.hdl_name.clone(),
            name: parts.last().copied().unwrap_or("").to_owned(),
            kind: IrVpiObjectKind::RegArray,
            width: array.elem_width,
            signed: array.signed,
            real: array.real,
            array: Some(index),
            var_kind: if array.real {
                IrVpiVarKind::Real
            } else {
                IrVpiVarKind::Logic
            },
            packed_range: (array.elem_width > 1).then(|| (i64::from(array.elem_width) - 1, 0)),
            ..IrVpiObject::default()
        });
    }
    objects
}

fn catalog_for(model: &IrModel) -> Vec<Entry> {
    if model.vpi_objects.is_empty() {
        return catalog(model, &fallback_objects(model));
    }
    catalog(model, &model.vpi_objects)
}

/// Emit the VPI object catalog. The catalog is generated from owned
/// metadata, so aliases retain separate HDL names while pointing at the
/// canonical runtime-visible value. The runtime only borrows it: models
/// that never load a VPI application do not index or validate it.
pub(super) fn render_vpi_metadata(model: &IrModel, out: &mut String) {
    out.push_str("\n/* owned hierarchy catalog for the VPI bridge */\n");
    // A VPI application linked as a static archive (`--dpi-lib`, the Windows
    // route) has its startup table run before LLG_VPI_PLUGIN libraries; the
    // build defines LLG_VPI_STATIC_STARTUP only when an archive defines it.
    out.push_str(
        "#ifdef LLG_VPI_STATIC_STARTUP
         #define LLG_VPI_STATIC_ROUTINES ((void (*const*)(void))vlog_startup_routines)
         #else
         #define LLG_VPI_STATIC_ROUTINES NULL
         #endif
",
    );
    if !model.vpi_catalog {
        // Applications loaded into this model see no design objects.
        out.push_str(
            "static llg_vpi_model_object_t* const llg_vpi_objects = NULL;\n\
             static const size_t llg_vpi_object_count = 0;\n",
        );
        return;
    }
    let entries = catalog_for(model);
    let indices = catalog_indices(&entries);
    for (index, entry) in entries.iter().enumerate() {
        if entry.dims.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "static const int64_t llg_vpi_dims_{index}[] = {{ {} }};\n",
            entry
                .dims
                .iter()
                .map(|(left, right)| format!("{left}LL, {right}LL"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out.push_str("static llg_vpi_model_object_t llg_vpi_objects[] = {\n");
    for (index, entry) in entries.iter().enumerate() {
        let path = entry.full.strip_suffix(PORT_MARKER).unwrap_or(&entry.full);
        let parts = split_name(path);
        let parent_path = (parts.len() > 1).then(|| parts[..parts.len() - 1].join("\u{1f}"));
        let pointer = |index: Option<usize>| {
            index.map_or_else(
                || "NULL".to_owned(),
                |index| format!("&llg_vpi_objects[{index}]"),
            )
        };
        let parent = pointer(
            parent_path
                .as_deref()
                .and_then(|path| indices.get(path).copied()),
        );
        let target = pointer(
            entry
                .port_target
                .as_deref()
                .and_then(|path| indices.get(path).copied()),
        );
        let optional = |text: &Option<String>| {
            text.as_deref()
                .map(c_string_literal)
                .unwrap_or_else(|| "NULL".to_owned())
        };
        let mut flags = Vec::new();
        if entry.two_state {
            flags.push("LLG_VPI_OBJ_TWO_STATE");
        }
        if entry.top {
            flags.push("LLG_VPI_OBJ_TOP");
        }
        if entry.local_param {
            flags.push("LLG_VPI_OBJ_LOCAL_PARAM");
        }
        if entry.shortreal {
            flags.push("LLG_VPI_OBJ_SHORTREAL");
        }
        if entry.range.is_some() {
            flags.push("LLG_VPI_OBJ_RANGE");
        }
        let (constant, real_constant) = match &entry.constant {
            Some(IrVpiConstant::Bits(digits)) => (c_string_literal(digits), "0.0".to_owned()),
            Some(IrVpiConstant::Text(text)) => {
                flags.push("LLG_VPI_OBJ_TEXT");
                (c_string_literal(text), "0.0".to_owned())
            }
            // `{:e}` prints the shortest round-trip form, a valid C literal.
            Some(IrVpiConstant::Real(value)) if value.is_finite() => {
                ("NULL".to_owned(), format!("{value:e}"))
            }
            Some(IrVpiConstant::Real(_)) => ("NULL".to_owned(), "0.0".to_owned()),
            None => ("NULL".to_owned(), "0.0".to_owned()),
        };
        let flags = if flags.is_empty() {
            "0u".to_owned()
        } else {
            flags.join(" | ")
        };
        let (left, right) = entry.range.unwrap_or((0, 0));
        let (dims, dim_count) = if entry.dims.is_empty() {
            ("NULL".to_owned(), 0)
        } else {
            (format!("llg_vpi_dims_{index}"), entry.dims.len())
        };
        out.push_str(&format!(
            "    {{ .type = {}, .name = {}, .full_name = {}, .definition_name = {}, .file = {}, .line = {}, \
.width = {}u, .is_signed = {}, .is_real = {}, .is_net = {}, .packed = {}, .real = {}, .parent = {}, \
.time_unit_fs = {}ULL, .subtype = {}, .flags = {}, .left = {left}LL, .right = {right}LL, \
.dims = {dims}, .dim_count = {dim_count}u, .port_index = {}u, .target = {target}, .net = {}, \
.constant = {constant}, .real_constant = {real_constant} }},\n",
            entry.type_code,
            c_string_literal(&entry.name),
            c_string_literal(&path.replace('\u{1f}', ".")),
            optional(&entry.definition_name),
            optional(&entry.file),
            entry.line,
            entry.width,
            u8::from(entry.signed),
            u8::from(entry.real),
            u8::from(entry.net),
            entry.packed,
            entry.real_value,
            parent,
            entry.time_unit_fs,
            entry.subtype,
            flags,
            entry.port_index,
            entry.net_pointer,
        ));
    }
    out.push_str("};\n");
    out.push_str(
        "static const size_t llg_vpi_object_count = sizeof(llg_vpi_objects) / sizeof(llg_vpi_objects[0]);\n",
    );
}

/// Catalog row of the scope enclosing each call site, for `vpiScope`.
pub(in crate::sim::emit_c) fn vpi_call_scopes(model: &IrModel) -> Vec<String> {
    let entries = catalog_for(model);
    let indices = catalog_indices(&entries);
    model
        .vpi_compile_calls
        .iter()
        .map(|call: &IrVpiCompileCall| {
            call.scope
                .as_deref()
                .and_then(|scope| indices.get(scope).copied())
                .map_or_else(
                    || "NULL".to_owned(),
                    |index| format!("&llg_vpi_objects[{index}]"),
                )
        })
        .collect()
}

/// Emit type-only descriptors for every source-level VPI call site. The
/// generated `main` invokes these after plugin startup, so compiletf/sizetf
/// runs before start-of-simulation callbacks or user processes. Values are
/// deliberately not evaluated here; argument side effects belong to the
/// runtime call site.
pub(super) fn render_vpi_compile_calls(model: &IrModel, out: &mut String) {
    if model.vpi_compile_calls.is_empty() {
        return;
    }
    let entries = catalog_for(model);
    let indices = catalog_indices(&entries);
    let object = |name: &str| indices.get(name).copied();
    out.push_str("\n/* compile-time descriptors for VPI system calls */\n");
    for (index, call) in model.vpi_compile_calls.iter().enumerate() {
        out.push_str(&format!(
            "static const llg_vpi_compile_arg_t llg_vpi_compile_args_{index}[] = {{\n"
        ));
        if call.args.is_empty() {
            out.push_str("    { 0, 0, 0 },\n");
        }
        for arg in &call.args {
            let expression = "LLG_VPI_ARG_EXPRESSION, .object = -1".to_owned();
            let reference = match &arg.reference {
                IrVpiArgRef::Expression => expression,
                IrVpiArgRef::Constant(kind) => {
                    format!("LLG_VPI_ARG_CONSTANT, .const_type = {kind}, .object = -1")
                }
                IrVpiArgRef::Object(name) => object(name).map_or(expression, |index| {
                    format!("LLG_VPI_ARG_OBJECT, .object = {index}")
                }),
                IrVpiArgRef::Select {
                    object: name,
                    lsb,
                    width,
                    left,
                    right,
                } => object(name).map_or(expression, |index| {
                    format!(
                        "LLG_VPI_ARG_SELECT, .object = {index}, .offset = {lsb}ULL, \
.select_width = {width}u, .left = {left}LL, .right = {right}LL"
                    )
                }),
                IrVpiArgRef::Element {
                    object: name,
                    index: element,
                } => object(name).map_or(expression, |index| {
                    format!("LLG_VPI_ARG_ELEMENT, .object = {index}, .offset = {element}ULL")
                }),
            };
            out.push_str(&format!(
                "    {{ .width = {}, .is_signed = {}, .is_real = {}, .ref_kind = {reference} }},\n",
                arg.width,
                u8::from(arg.signed),
                u8::from(arg.real)
            ));
        }
        out.push_str("};\n");
    }
}
