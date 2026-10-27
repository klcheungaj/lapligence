//! Vpi.

use super::*;
use crate::sim::ir::{
    IrVpiArgRef, IrVpiCompileCall, IrVpiConn, IrVpiConstant, IrVpiObject, IrVpiTypespec,
    IrVpiTypespecKind, IrVpiVarKind,
};

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
    typespec: Option<usize>,
    member: bool,
    slice: bool,
    record: bool,
    detail: &'static str,
    event: String,
    conns: Vec<IrVpiConn>,
    unnamed: bool,
    qualifier: bool,
    activation: Option<(u32, u32)>,
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
            typespec: None,
            member: false,
            slice: false,
            record: false,
            detail: "0",
            event: "NULL".to_owned(),
            conns: Vec::new(),
            unnamed: false,
            qualifier: false,
            activation: None,
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
        let event = object
            .event
            .and_then(|event| model.events.get(event))
            .filter(|event| !event.is_array())
            .map_or_else(|| "NULL".to_owned(), |event| format!("&{}", event.c_name));
        if object.kind == IrVpiObjectKind::NamedEvent && event == "NULL" {
            continue;
        }
        let dims = if object.kind == IrVpiObjectKind::InstanceArray {
            let Some(dims) = object
                .dims
                .iter()
                .map(|(left, right)| {
                    Some((i32::try_from(*left).ok()?, i32::try_from(*right).ok()?))
                })
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            dims
        } else {
            dims
        };
        let (type_code, subtype) = match object.kind {
            IrVpiObjectKind::Reg => (object.var_kind.c_type(), "0"),
            IrVpiObjectKind::RealVar => (element_type, "0"),
            IrVpiObjectKind::Net => ("vpiNet", object.net_type.unwrap_or("vpiWire")),
            IrVpiObjectKind::RegArray | IrVpiObjectKind::NetArray => {
                (object.kind.c_type(), element_type)
            }
            IrVpiObjectKind::Port => (object.kind.c_type(), object.direction.c_code()),
            IrVpiObjectKind::Record => (object.var_kind.c_type(), "0"),
            kind => (object.type_code.unwrap_or(kind.c_type()), "0"),
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
                typespec: object.typespec,
                member: object.member,
                slice: object.slice,
                record: object.kind == IrVpiObjectKind::Record,
                detail: object.detail.unwrap_or("0"),
                event,
                conns: object.conns.clone(),
                unnamed: object.unnamed,
                qualifier: object.qualifier,
                activation: object.activation,
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
             static const size_t llg_vpi_object_count = 0;\n\
             static const llg_vpi_type_tables_t* const llg_vpi_type_tables = NULL;\n",
        );
        return;
    }
    render_vpi_typespecs(model.vpi_typespecs(), out);
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
    let object = |name: &str| indices.get(name).copied();
    for (index, entry) in entries.iter().enumerate() {
        if entry.conns.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "static const llg_vpi_conn_t llg_vpi_conns_{index}[] = {{\n"
        ));
        for conn in &entry.conns {
            out.push_str(&format!(
                "    {{ {{ .width = {}, .is_signed = {}, .ref_kind = {} }}, .direction = {}, .value = {} }},\n",
                conn.width,
                u8::from(conn.signed),
                arg_reference(&conn.reference, &object),
                if conn.direction == crate::sim::ir::IrVpiDirection::None {
                    "0"
                } else {
                    conn.direction.c_code()
                },
                conn.value
                    .as_deref()
                    .map_or_else(|| "NULL".to_owned(), c_string_literal),
            ));
        }
        out.push_str("};\n");
    }
    out.push_str("static llg_vpi_model_object_t llg_vpi_objects[] = {\n");
    for (index, entry) in entries.iter().enumerate() {
        let path = entry.full.strip_suffix(PORT_MARKER).unwrap_or(&entry.full);
        let parts = split_name(path);
        let parent_path = (parts.len() > 1).then(|| parts[..parts.len() - 1].join("\u{1f}"));
        // An unnamed object's key ends in a hidden part; its displayed
        // name is its scope's (it reports no vpiFullName).
        let path = if entry.unnamed {
            parent_path.clone().unwrap_or_default()
        } else {
            path.to_owned()
        };
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
        if entry.member {
            flags.push("LLG_VPI_OBJ_MEMBER");
        }
        if entry.slice {
            flags.push("LLG_VPI_OBJ_SLICE");
        }
        if entry.record {
            flags.push("LLG_VPI_OBJ_RECORD");
        }
        if entry.unnamed {
            flags.push("LLG_VPI_OBJ_UNNAMED");
        }
        if entry.qualifier {
            flags.push("LLG_VPI_OBJ_QUALIFIER");
        }
        if entry.activation.is_some() {
            flags.push("LLG_VPI_OBJ_ACTIVATION");
        }
        let (declaration, instance) = entry.activation.unwrap_or((0, 0));
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
        let conns = if entry.conns.is_empty() {
            "NULL".to_owned()
        } else {
            format!("llg_vpi_conns_{index}")
        };
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
.constant = {constant}, .real_constant = {real_constant}, .typespec = {}u, .detail = {}, \
.event = {}, .conns = {conns}, .conn_count = {}u, .activation_declaration = {declaration}u, \
.activation_instance = {instance}u }},\n",
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
            entry.typespec.map_or(0, |typespec| typespec + 1),
            entry.detail,
            entry.event,
            entry.conns.len(),
        ));
    }
    out.push_str("};\n");
    out.push_str(
        "static const size_t llg_vpi_object_count = sizeof(llg_vpi_objects) / sizeof(llg_vpi_objects[0]);\n",
    );
}

/// One node of the member tree of a packed value (or fixed unpacked image):
/// a member of the value, or of an enclosing member, at an absolute bit
/// offset in that value.
struct MemberNode {
    name: String,
    typespec: usize,
    lsb: u32,
    width: u32,
    parent: Option<usize>,
    children: (usize, usize),
}

/// Append the member tree of a value of typespec `root` to `nodes`, children
/// of each node contiguous (breadth first). Returns `(first, count)` of the
/// top-level members.
fn member_tree(
    typespecs: &[IrVpiTypespec],
    root: usize,
    nodes: &mut Vec<MemberNode>,
) -> (usize, usize) {
    // (typespec, absolute lsb of that value, parent node)
    let mut queue = std::collections::VecDeque::from([(root, 0u32, None::<usize>)]);
    let mut top = (nodes.len(), 0);
    while let Some((typespec, base, parent)) = queue.pop_front() {
        let members = &typespecs[typespec].members;
        if members.iter().any(|member| member.lsb.is_none()) {
            continue;
        }
        let first = nodes.len();
        for member in members {
            let lsb = base.saturating_add(member.lsb.unwrap_or(0));
            nodes.push(MemberNode {
                name: member.name.clone(),
                typespec: member.typespec,
                lsb,
                width: member.width,
                parent,
                children: (0, 0),
            });
            let index = nodes.len() - 1;
            if matches!(
                typespecs[member.typespec].kind,
                IrVpiTypespecKind::Struct | IrVpiTypespecKind::Union
            ) {
                queue.push_back((member.typespec, lsb, Some(index)));
            }
        }
        match parent {
            Some(parent) => nodes[parent].children = (first, members.len()),
            None => top = (first, members.len()),
        }
    }
    top
}

/// Emit the interned typespec tables (IEEE 1800-2009 37.23) and the member
/// trees of packed structure and union values. Indices are table positions;
/// fields that name an optional entry hold index + 1, 0 for none.
fn render_vpi_typespecs(typespecs: &[IrVpiTypespec], out: &mut String) {
    let mut ranges = Vec::new();
    let mut members = Vec::new();
    let mut consts = Vec::new();
    let mut nodes = Vec::new();
    let mut rows = Vec::with_capacity(typespecs.len());
    for (index, typespec) in typespecs.iter().enumerate() {
        let range = (ranges.len(), typespec.ranges.len());
        ranges.extend(typespec.ranges.iter().copied());
        let member = (members.len(), typespec.members.len());
        members.extend(
            typespec
                .members
                .iter()
                .map(|member| (member.name.clone(), member.typespec)),
        );
        let constant = (consts.len(), typespec.enum_consts.len());
        consts.extend(typespec.enum_consts.iter().cloned());
        let tree = if matches!(
            typespec.kind,
            IrVpiTypespecKind::Struct | IrVpiTypespecKind::Union
        ) && typespec.width > 0
        {
            member_tree(typespecs, index, &mut nodes)
        } else {
            (0, 0)
        };
        let mut flags = Vec::new();
        if typespec.signed {
            flags.push("LLG_VPI_TS_SIGNED");
        }
        if typespec.two_state {
            flags.push("LLG_VPI_TS_TWO_STATE");
        }
        match typespec.packed {
            Some(true) => flags.push("LLG_VPI_TS_PACKED"),
            Some(false) => flags.push("LLG_VPI_TS_UNPACKED"),
            None => {}
        }
        if typespec.tagged {
            flags.push("LLG_VPI_TS_TAGGED");
        }
        let flags = if flags.is_empty() {
            "0u".to_owned()
        } else {
            flags.join(" | ")
        };
        let optional = |entry: Option<usize>| entry.map_or(0, |entry| entry + 1);
        rows.push(format!(
            "    {{ {}, {}, {}u, {}u, {}u, {}u, {flags}, {}u, {}u, {}u, {}u, {}u, {}u, {}u, {}u }},\n",
            typespec.kind.c_type(),
            typespec
                .name
                .as_deref()
                .map_or_else(|| "NULL".to_owned(), c_string_literal),
            optional(typespec.alias),
            optional(typespec.element),
            optional(typespec.base),
            typespec.width,
            range.0,
            range.1,
            member.0,
            member.1,
            constant.0,
            constant.1,
            tree.0,
            tree.1,
        ));
    }
    let table = |out: &mut String, ty: &str, name: &str, items: Vec<String>| {
        if items.is_empty() {
            out.push_str(&format!("#define {name} NULL\n"));
        } else {
            out.push_str(&format!("static const {ty} {name}_rows[] = {{\n"));
            for item in items {
                out.push_str(&item);
            }
            out.push_str(&format!("}};\n#define {name} {name}_rows\n"));
        }
    };
    out.push_str("\n/* typespecs of the VPI catalog (37.23) */\n");
    let range_count = ranges.len();
    table(
        out,
        "int64_t",
        "llg_vpi_ts_ranges",
        ranges
            .iter()
            .map(|(left, right)| format!("    {left}LL, {right}LL,\n"))
            .collect(),
    );
    let member_count = members.len();
    table(
        out,
        "llg_vpi_typespec_member_t",
        "llg_vpi_ts_members",
        members
            .iter()
            .map(|(name, typespec)| format!("    {{ {}, {typespec}u }},\n", c_string_literal(name)))
            .collect(),
    );
    let const_count = consts.len();
    table(
        out,
        "llg_vpi_enum_const_t",
        "llg_vpi_ts_consts",
        consts
            .iter()
            .map(|(name, digits)| {
                format!(
                    "    {{ {}, {} }},\n",
                    c_string_literal(name),
                    c_string_literal(digits)
                )
            })
            .collect(),
    );
    let node_count = nodes.len();
    table(
        out,
        "llg_vpi_member_node_t",
        "llg_vpi_ts_nodes",
        nodes
            .iter()
            .map(|node| {
                let typespec = &typespecs[node.typespec];
                let var_type = typespec.kind.var_type();
                let mut flags = Vec::new();
                if typespec.signed {
                    flags.push("LLG_VPI_TS_SIGNED");
                }
                if typespec.two_state {
                    flags.push("LLG_VPI_TS_TWO_STATE");
                }
                let flags = if flags.is_empty() {
                    "0u".to_owned()
                } else {
                    flags.join(" | ")
                };
                format!(
                    "    {{ {}, {var_type}, {}u, {}u, {}u, {flags}, {}u, {}u, {}u }},\n",
                    c_string_literal(&node.name),
                    node.typespec,
                    node.lsb,
                    node.width,
                    node.parent.map_or(0, |parent| parent + 1),
                    node.children.0,
                    node.children.1,
                )
            })
            .collect(),
    );
    let typespec_count = rows.len();
    table(out, "llg_vpi_typespec_t", "llg_vpi_ts_types", rows);
    out.push_str(&format!(
        "static const llg_vpi_type_tables_t llg_vpi_type_tables_value = {{ llg_vpi_ts_types, {typespec_count}u, \
llg_vpi_ts_ranges, {range_count}u, llg_vpi_ts_members, {member_count}u, llg_vpi_ts_consts, {const_count}u, \
llg_vpi_ts_nodes, {node_count}u }};\n\
static const llg_vpi_type_tables_t* const llg_vpi_type_tables = &llg_vpi_type_tables_value;\n"
    ));
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

/// Emit the built-in system subroutines the model runs natively, checked
/// against VPI registrations after plugin startup (decision S99-D6).
pub(super) fn render_vpi_builtin_calls(model: &IrModel, out: &mut String) {
    if model.vpi_builtin_calls.is_empty() {
        return;
    }
    out.push_str("\n/* built-in system subroutines a VPI registration must not override */\n");
    out.push_str("static const llg_vpi_builtin_call_t llg_vpi_builtin_calls[] = {\n");
    for call in &model.vpi_builtin_calls {
        out.push_str(&format!(
            "    {{ {}, {}, {} }},\n",
            c_string_literal(&call.name),
            call.file
                .as_deref()
                .map_or_else(|| "NULL".to_owned(), c_string_literal),
            call.line
        ));
    }
    out.push_str("};\n");
}

/// The startup statement that rejects VPI registrations of the built-ins
/// the model runs natively, or `None` when it calls none.
pub(in crate::sim::emit_c) fn vpi_builtin_check(model: &IrModel) -> Option<String> {
    (!model.vpi_builtin_calls.is_empty()).then(|| {
        format!(
            "llg_vpi_check_builtin_overrides(llg_vpi_builtin_calls, {}u)",
            model.vpi_builtin_calls.len()
        )
    })
}

/// The `ref_kind` and reference fields of an argument or connection
/// descriptor; a reference to an object outside the catalog is an
/// expression.
fn arg_reference(reference: &IrVpiArgRef, object: &dyn Fn(&str) -> Option<usize>) -> String {
    let expression = "LLG_VPI_ARG_EXPRESSION, .object = -1".to_owned();
    match reference {
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
    }
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
            let reference = arg_reference(&arg.reference, &object);
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

#[cfg(test)]
mod tests {
    use super::{arg_reference, member_tree, render_vpi_metadata, render_vpi_typespecs};
    use crate::sim::ir::{
        IrModel, IrVpiObject, IrVpiObjectKind, IrVpiTypespec, IrVpiTypespecKind,
        IrVpiTypespecMember,
    };

    fn member(name: &str, typespec: usize, lsb: u32, width: u32) -> IrVpiTypespecMember {
        IrVpiTypespecMember {
            name: name.to_owned(),
            typespec,
            lsb: Some(lsb),
            width,
        }
    }

    /// `struct packed { struct packed { logic [3:0] hi; logic [3:0] lo; }
    /// inner; logic [7:0] s; }`: nested members get absolute offsets and the
    /// children of every node are contiguous.
    fn nested() -> Vec<IrVpiTypespec> {
        let logic = |width| IrVpiTypespec {
            kind: IrVpiTypespecKind::Logic,
            width,
            ranges: vec![(i64::from(width) - 1, 0)],
            ..IrVpiTypespec::default()
        };
        let inner = IrVpiTypespec {
            kind: IrVpiTypespecKind::Struct,
            width: 8,
            packed: Some(true),
            members: vec![member("hi", 0, 4, 4), member("lo", 0, 0, 4)],
            ..IrVpiTypespec::default()
        };
        let outer = IrVpiTypespec {
            kind: IrVpiTypespecKind::Struct,
            width: 16,
            packed: Some(true),
            members: vec![member("inner", 1, 8, 8), member("s", 2, 0, 8)],
            ..IrVpiTypespec::default()
        };
        vec![logic(4), inner, logic(8), outer]
    }

    #[test]
    fn member_trees_use_absolute_offsets_and_contiguous_children() {
        let typespecs = nested();
        let mut nodes = Vec::new();
        let top = member_tree(&typespecs, 3, &mut nodes);
        assert_eq!(top, (0, 2));
        let summary = nodes
            .iter()
            .map(|node| {
                (
                    node.name.as_str(),
                    node.lsb,
                    node.width,
                    node.parent,
                    node.children,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            summary,
            [
                ("inner", 8, 8, None, (2, 2)),
                ("s", 0, 8, None, (0, 0)),
                ("hi", 12, 4, Some(0), (0, 0)),
                ("lo", 8, 4, Some(0), (0, 0)),
            ]
        );
    }

    #[test]
    fn references_outside_the_catalog_become_expressions() {
        use crate::sim::ir::IrVpiArgRef;
        let object = |name: &str| (name == "tb\u{1f}r").then_some(4);
        let select = IrVpiArgRef::Select {
            object: "tb\u{1f}r".to_owned(),
            lsb: 2,
            width: 1,
            left: 2,
            right: 2,
        };
        assert_eq!(
            arg_reference(&select, &object),
            "LLG_VPI_ARG_SELECT, .object = 4, .offset = 2ULL, .select_width = 1u, .left = 2LL, .right = 2LL"
        );
        assert_eq!(
            arg_reference(&IrVpiArgRef::Object("tb\u{1f}gone".to_owned()), &object),
            "LLG_VPI_ARG_EXPRESSION, .object = -1"
        );
        assert_eq!(
            arg_reference(&IrVpiArgRef::Constant("vpiBinaryConst"), &object),
            "LLG_VPI_ARG_CONSTANT, .const_type = vpiBinaryConst, .object = -1"
        );
    }

    #[test]
    fn a_member_without_a_fixed_offset_has_no_member_tree() {
        let mut typespecs = nested();
        typespecs[3].members[1].lsb = None;
        let mut nodes = Vec::new();
        assert_eq!(member_tree(&typespecs, 3, &mut nodes).1, 0);
        assert!(nodes.is_empty());
    }

    #[test]
    fn named_blocks_carry_their_disable_identity() {
        let mut model = IrModel::new("tb".to_owned(), 1).expect("model");
        model.vpi_objects.push(IrVpiObject {
            full_name: "tb\u{1f}blk".to_owned(),
            name: "blk".to_owned(),
            kind: IrVpiObjectKind::NamedBegin,
            activation: Some((7, 3)),
            ..IrVpiObject::default()
        });
        model.vpi_objects.push(IrVpiObject {
            full_name: "tb\u{1f}other".to_owned(),
            name: "other".to_owned(),
            kind: IrVpiObjectKind::NamedBegin,
            ..IrVpiObject::default()
        });
        let mut out = String::new();
        render_vpi_metadata(&model, &mut out);
        let row = |name: &str| {
            out.lines()
                .find(|line| line.contains(&format!(".full_name = \"{name}\"")))
                .unwrap_or_else(|| panic!("{name}: {out}"))
                .to_owned()
        };
        let blk = row("tb.blk");
        assert!(blk.contains("LLG_VPI_OBJ_ACTIVATION"), "{blk}");
        assert!(
            blk.contains(".activation_declaration = 7u, .activation_instance = 3u"),
            "{blk}"
        );
        let other = row("tb.other");
        assert!(!other.contains("LLG_VPI_OBJ_ACTIVATION"), "{other}");
        assert!(other.contains(".activation_declaration = 0u"), "{other}");
    }

    #[test]
    fn typespec_tables_use_index_plus_one_for_optional_references() {
        let mut typespecs = nested();
        typespecs[0].element = Some(2);
        typespecs[0].name = Some("nib_t".to_owned());
        let mut out = String::new();
        render_vpi_typespecs(&typespecs, &mut out);
        assert!(out.contains("{ vpiLogicTypespec, \"nib_t\", 0u, 3u, 0u, 4u, 0u, 0u, 1u, 0u, 0u, 0u, 0u, 0u, 0u },"), "{out}");
        assert!(out.contains("#define llg_vpi_ts_consts NULL"), "{out}");
        assert!(out.contains("llg_vpi_ts_nodes, 6u"), "{out}");
    }
}
