//! Vpi.

use super::*;

mod dynamic;
pub(in crate::sim::emit_c) mod pack;
use crate::sim::ir::{
    IrSdfTarget, IrVpiArgRef, IrVpiCode, IrVpiCompileCall, IrVpiConn, IrVpiConstant,
    IrVpiContainer, IrVpiInstance, IrVpiObject, IrVpiTiming, IrVpiTypespec, IrVpiTypespecKind,
    IrVpiVarKind,
};

/// Marker suffix that keeps a port apart from the declaration it connects,
/// which has the same hierarchical name (see `collection/vpi.rs`).
const PORT_MARKER: &str = "\u{1f}\u{1e}port";
/// The hidden key part of an io declaration, which shares its name with
/// the port or declaration it declares.
const IO_MARKER: &str = "\u{1f}\u{1e}io";

/// The displayed path of a catalog key: ports and io declarations live in
/// their own namespaces under the name they share.
fn strip_marker(path: &str) -> &str {
    path.strip_suffix(PORT_MARKER)
        .or_else(|| path.strip_suffix(IO_MARKER))
        .unwrap_or(path)
}

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
    fixed_cell: bool,
    record: bool,
    detail: &'static str,
    event: String,
    conns: Vec<IrVpiConn>,
    unnamed: bool,
    qualifier: bool,
    constant_variable: bool,
    activation: Option<(u32, u32)>,
    timing: Option<IrVpiTiming>,
    /// `(storage, dependency, shape)` of a dynamic object.
    dynamic: Option<(String, String, usize)>,
    code: Option<u32>,
    instance: Option<IrVpiInstance>,
    strength: Option<(&'static str, &'static str)>,
    /// `(state, gate process C name)` of a sequential UDP that takes
    /// `vpi_put_value`.
    udp_put: Option<(usize, String)>,
    implicit: bool,
    qualifiers: crate::sim::ir::IrVpiQualifiers,
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
            fixed_cell: false,
            record: false,
            detail: "0",
            event: "NULL".to_owned(),
            conns: Vec::new(),
            unnamed: false,
            qualifier: false,
            constant_variable: false,
            activation: None,
            timing: None,
            dynamic: None,
            code: None,
            instance: None,
            strength: None,
            udp_put: None,
            implicit: false,
            qualifiers: Default::default(),
        }
    }
}

/// The `vpiFullName` of a catalog key: `.` between name components, `::`
/// after a package or `$unit`, whose own full name is its name (37.10
/// detail 5; the package's own name omits the `::`, decision A19-D29).
fn display_name(key: &str) -> String {
    let name = key.replace("::\u{1f}", "::").replace('\u{1f}', ".");
    match name.strip_suffix("::") {
        Some(package) if !package.contains("::") => package.to_owned(),
        _ => name,
    }
}

fn split_name(name: &str) -> Vec<&str> {
    name.split('\u{1f}').collect()
}

/// Storage of one signal after alias resolution: `(packed, real, net)`.
pub(super) fn signal_storage(model: &IrModel, index: usize) -> Option<(String, String, String)> {
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

    // Entries keyed by full name, each with its first position in
    // `objects`: rows keep the collector's declaration order, which the
    // runtime's relation iterations follow (decision A19-D47).
    let mut entries = BTreeMap::<String, (usize, Entry)>::new();
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
            // cells; they are not VPI-visible (documented limit), except
            // the one cell of a scalar record member.
            if object.fixed_cell {
                (
                    format!("&{}", array.c_name),
                    "NULL".to_owned(),
                    "NULL".to_owned(),
                    Vec::new(),
                )
            } else if array.sparse() || array.activation || array.descriptor {
                continue;
            } else if array.real {
                (
                    "NULL".to_owned(),
                    format!("&{}[0]", array.c_name),
                    "NULL".to_owned(),
                    array.dims.clone(),
                )
            } else {
                (
                    format!("&{}[0]", array.c_name),
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
            object.var_kind.net_c_type()
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
        let dims = if matches!(
            object.kind,
            IrVpiObjectKind::InstanceArray | IrVpiObjectKind::NamedEventArray
        ) || (object.kind == IrVpiObjectKind::Dynamic && !object.dims.is_empty())
        {
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
        let dynamic = if object.kind == IrVpiObjectKind::Dynamic {
            let Some(storage) = dynamic::row_storage(model, object) else {
                continue;
            };
            Some(storage)
        } else {
            None
        };
        let (type_code, subtype) = match object.kind {
            IrVpiObjectKind::Dynamic => (dynamic::row_type(model, object), "0"),
            IrVpiObjectKind::Reg => (object.var_kind.c_type(), "0"),
            IrVpiObjectKind::RealVar => (element_type, "0"),
            IrVpiObjectKind::Net => (
                object.var_kind.net_c_type(),
                object.net_type.unwrap_or("vpiWire"),
            ),
            IrVpiObjectKind::RegArray | IrVpiObjectKind::NetArray => {
                (object.kind.c_type(), element_type)
            }
            IrVpiObjectKind::Port | IrVpiObjectKind::ClockingIoDecl | IrVpiObjectKind::IoDecl => {
                (object.kind.c_type(), object.direction.c_code())
            }
            IrVpiObjectKind::Record => (object.var_kind.c_type(), "0"),
            IrVpiObjectKind::NamedEventArray => (object.kind.c_type(), "vpiNamedEvent"),
            kind => (object.type_code.unwrap_or(kind.c_type()), "0"),
        };
        let order = entries
            .get(&object.full_name)
            .map_or(entries.len(), |(order, _)| *order);
        let entry = Entry {
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
            fixed_cell: object.fixed_cell,
            record: object.kind == IrVpiObjectKind::Record,
            detail: object.detail.unwrap_or("0"),
            event,
            conns: object.conns.clone(),
            unnamed: object.unnamed,
            qualifier: object.qualifier,
            constant_variable: object.constant_variable,
            activation: object.activation,
            timing: object.timing.clone(),
            dynamic,
            code: object.code,
            instance: object.instance.as_deref().cloned(),
            strength: object.strength,
            udp_put: object.udp_put.clone(),
            implicit: object.implicit,
            qualifiers: object.qualifiers,
        };
        entries.insert(object.full_name.clone(), (order, entry));
    }

    // A declaration may be retained under a scope that has no captured
    // node. Add only missing prefixes and leave their source location and
    // definition empty rather than inventing frontend provenance.
    let paths = entries
        .iter()
        .map(|(path, (order, _))| (path.clone(), *order))
        .collect::<Vec<_>>();
    let mut synthesized = std::collections::HashSet::new();
    for (path, order) in paths {
        let path = strip_marker(&path);
        let parts = split_name(path);
        for depth in 1..parts.len() {
            let prefix = parts[..depth].join("\u{1f}");
            let kind = if depth == 1 {
                "vpiModule"
            } else {
                "vpiGenScope"
            };
            // A synthesized scope sits where its first descendant does.
            match entries.entry(prefix.clone()) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    let mut scope = Entry::scope(prefix.clone(), parts[depth - 1].to_owned(), kind);
                    scope.top = depth == 1;
                    slot.insert((order, scope));
                    synthesized.insert(prefix);
                }
                std::collections::btree_map::Entry::Occupied(mut slot) => {
                    if synthesized.contains(&prefix) {
                        let first = &mut slot.get_mut().0;
                        *first = (*first).min(order);
                    }
                }
            }
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
        entries.insert(root.full.clone(), (0, root));
    }
    let mut rows = entries.into_values().collect::<Vec<_>>();
    rows.sort_by_key(|(order, _)| *order);
    rows.into_iter().map(|(_, entry)| entry).collect()
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

/// The gate process of a sequential UDP that takes `vpi_put_value` (38.34):
/// the inertial driver handle of a delayed output (`None`: not delayed),
/// the process's root descriptor and, for an instance of a shared body, its
/// instance record and the record's `offsetof` in the root frame.
pub(super) struct UdpPutSite {
    pub(super) handle: Option<String>,
    pub(super) desc: String,
    pub(super) record: Option<(String, String)>,
}

/// [`UdpPutSite`]s by IR process C name.
pub(super) type UdpPutSites = std::collections::HashMap<String, UdpPutSite>;

/// The put records of the catalog's sequential UDPs and each row's record
/// index. A UDP whose gate process has no site gets no record: its put is
/// refused. The records name descriptors and instance records defined after
/// the catalog, so the table is declared here (a tentative definition) and
/// defined by the returned text, which the caller emits after them.
fn render_udp_puts(
    entries: &[Entry],
    sites: &UdpPutSites,
    out: &mut String,
) -> (Vec<Option<usize>>, String) {
    let mut records = Vec::new();
    let indices = entries
        .iter()
        .map(|entry| {
            let (state, process) = entry.udp_put.as_ref()?;
            let site = sites.get(process)?;
            let (record, offset) = site
                .record
                .as_ref()
                .map_or(("NULL", "0"), |(record, offset)| {
                    (record.as_str(), offset.as_str())
                });
            records.push(format!(
                "    {{ &llg_udp_state_{state}.state, {}, &{}, {record}, {offset}, -1, 0u }},\n",
                site.handle
                    .as_deref()
                    .map_or_else(|| "NULL".to_owned(), |handle| format!("&{handle}")),
                site.desc,
            ));
            Some(records.len() - 1)
        })
        .collect();
    if records.is_empty() {
        return (indices, String::new());
    }
    out.push_str(&format!(
        "static llg_vpi_udp_put_t llg_vpi_udp_puts[{}];\n",
        records.len()
    ));
    let definition = format!(
        "static llg_vpi_udp_put_t llg_vpi_udp_puts[{}] = {{\n{}}};\n",
        records.len(),
        records.concat()
    );
    (indices, definition)
}

/// Emit the VPI object catalog. The catalog is generated from owned
/// metadata, so aliases retain separate HDL names while pointing at the
/// canonical runtime-visible value. Its object and code rows are packed
/// ([`pack`]) and unpacked by the runtime on the first VPI use that can
/// reach a design object: models that never load a VPI application do not
/// decode, index or validate it.
/// Returns the definition of the UDP put records ([`render_udp_puts`]),
/// which the caller emits after the coroutine metadata.
pub(super) fn render_vpi_metadata(
    model: &IrModel,
    udp_sites: &UdpPutSites,
    out: &mut String,
) -> String {
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
             static const llg_vpi_type_tables_t* const llg_vpi_type_tables = NULL;\n\
             static const llg_vpi_code_tables_t* const llg_vpi_code_tables = NULL;\n",
        );
        dynamic::render_vpi_dynamic(model, &std::collections::HashMap::new(), out);
        return String::new();
    }
    let entries = catalog_for(model);
    let indices = catalog_indices(&entries);
    // A typedef of a scope without a catalog row (a class) is not listed.
    let mut scope_typedefs = model
        .vpi_scope_typedefs
        .iter()
        .filter_map(|(scope, typespec)| Some((*indices.get(scope.as_str())?, *typespec)))
        .collect::<Vec<_>>();
    scope_typedefs.sort_by_key(|(row, _)| *row);
    // 37.12 detail 4: (scope row, object) pairs in row order; an object is
    // a catalog row, or a class layout index with LLG_VPI_IMPORT_CLASS set.
    // An item without a catalog row (a typedef, an enum constant) is no
    // instance item and is not listed.
    let mut scope_imports = model
        .vpi_scope_imports
        .iter()
        .filter_map(|(scope, item)| {
            let scope = *indices.get(scope.as_str())?;
            let object = match item {
                crate::sim::ir::IrVpiImport::Row(key) => {
                    u32::try_from(*indices.get(key.as_str())?).ok()?
                }
                crate::sim::ir::IrVpiImport::Class(class) => {
                    u32::try_from(*class)
                        .ok()
                        .filter(|class| *class < IMPORT_CLASS)?
                        | IMPORT_CLASS
                }
            };
            Some((scope, object))
        })
        .collect::<Vec<_>>();
    // A class method's imports are keyed by its item in the class item
    // table with LLG_VPI_IMPORT_CLASS set, after every catalog row.
    let mut first_item = 0usize;
    for class in &model.vpi_dynamic().classes {
        for (index, item) in class.items.iter().enumerate() {
            let Some(key) = u32::try_from(first_item + index)
                .ok()
                .filter(|key| *key < IMPORT_CLASS)
            else {
                continue;
            };
            for import in &item.imports {
                let object = match import {
                    crate::sim::ir::IrVpiImport::Row(key) => indices
                        .get(key.as_str())
                        .and_then(|row| u32::try_from(*row).ok()),
                    crate::sim::ir::IrVpiImport::Class(class) => u32::try_from(*class)
                        .ok()
                        .filter(|class| *class < IMPORT_CLASS)
                        .map(|class| class | IMPORT_CLASS),
                };
                if let Some(object) = object {
                    scope_imports.push(((key | IMPORT_CLASS) as usize, object));
                }
            }
        }
        first_item += class.items.len();
    }
    scope_imports.sort_by_key(|(row, _)| *row);
    let mut listed = std::collections::HashSet::new();
    scope_imports.retain(|pair| listed.insert(*pair));
    render_vpi_typespecs(model.vpi_typespecs(), &scope_typedefs, &scope_imports, out);
    dynamic::render_vpi_dynamic(model, &indices, out);
    let timing = render_vpi_timing(model, &entries, out);
    let (udp_puts, udp_put_definition) = render_udp_puts(&entries, udp_sites, out);
    let mut pack = pack::Pack::default();
    pack_objects(&entries, &indices, &timing, &udp_puts, &mut pack);
    let objects = pack.take_stream();
    // Zero storage the runtime fills from the packed rows on the first VPI
    // use that can reach a design object (`llg_vpi_model_packed`).
    out.push_str(&format!(
        "static llg_vpi_model_object_t llg_vpi_objects[{}];\n\
         static const size_t llg_vpi_object_count = {}u;\n",
        entries.len().max(1),
        entries.len()
    ));
    render_vpi_code(model, &indices, &mut pack, out);
    let code = pack.take_stream();
    pack.render(&objects, &code, "llg_vpi_packed_value", out);
    out.push_str("static const llg_vpi_packed_t* const llg_vpi_packed = &llg_vpi_packed_value;\n");
    udp_put_definition
}

/// The extra start-up registration of a catalog model: its packed rows
/// (`llg_vpi_model_packed`), registered right after `llg_vpi_model_init`.
/// Empty for a model without the catalog, whose start-up is unchanged.
pub(in crate::sim::emit_c) fn vpi_packed_registration(model: &IrModel) -> &'static str {
    if model.vpi_catalog {
        " || !llg_vpi_model_packed(llg_vpi_packed)"
    } else {
        ""
    }
}

/// Bits of the reference mask that ends a packed object row: which
/// `llg_vpi_model_object_t` pointers to model storage follow in the
/// reference table, in this order (`catalog_unpack`).
const REF_PACKED: u64 = 0x01;
const REF_REAL: u64 = 0x02;
const REF_NET: u64 = 0x04;
const REF_EVENT: u64 = 0x08;
const REF_STORAGE: u64 = 0x10;
const REF_DEPENDENCY: u64 = 0x20;

/// Write the object stream: the totals the decoder allocates for, then one
/// record per catalog row with the fields of `llg_vpi_model_object_t` in
/// the order `catalog_unpack` reads them.
fn pack_objects(
    entries: &[Entry],
    indices: &std::collections::HashMap<&str, usize>,
    timing: &[Option<(Option<String>, usize)>],
    udp_puts: &[Option<usize>],
    pack: &mut pack::Pack,
) {
    let row = |path: Option<&str>| {
        path.and_then(|path| indices.get(path).copied())
            .map_or(0, |row| row as u64 + 1)
    };
    pack.uint(entries.iter().map(|entry| entry.conns.len() as u64).sum());
    pack.uint(entries.iter().map(|entry| entry.dims.len() as u64).sum());
    pack.uint(
        entries
            .iter()
            .filter(|entry| entry.instance.is_some())
            .count() as u64,
    );
    pack.uint(
        entries
            .iter()
            .filter(|entry| entry.timing.is_some())
            .count() as u64,
    );
    pack.uint(
        entries
            .iter()
            .filter_map(|entry| entry.timing.as_ref())
            .map(|timing| timing.declared.len() as u64)
            .sum(),
    );
    for (index, entry) in entries.iter().enumerate() {
        let path = strip_marker(&entry.full);
        let parts = split_name(path);
        let parent_path = (parts.len() > 1).then(|| parts[..parts.len() - 1].join("\u{1f}"));
        // An unnamed object's key ends in a hidden part; its displayed
        // name is its scope's (it reports no vpiFullName). A named object
        // keyed that way (a generated primitive name) displays as its
        // scope and name.
        let hidden = parts.last().is_some_and(|part| part.starts_with('\u{1e}'));
        let path = if entry.unnamed {
            parent_path.clone().unwrap_or_default()
        } else if hidden {
            match &parent_path {
                Some(parent) => format!("{parent}\u{1f}{}", entry.name),
                None => entry.name.clone(),
            }
        } else {
            path.to_owned()
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
        if entry.type_code == "vpiNamedEvent" && entry.port_target.is_some() {
            flags.push("LLG_VPI_OBJ_ELEMENT");
        }
        if entry.slice {
            flags.push("LLG_VPI_OBJ_SLICE");
        }
        if entry.fixed_cell {
            flags.push("LLG_VPI_OBJ_FIXED_CELL");
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
        if entry.dynamic.is_some() {
            flags.push("LLG_VPI_OBJ_DYNAMIC");
        }
        if entry.constant_variable {
            flags.push("LLG_VPI_OBJ_CONST");
        }
        if entry.implicit {
            flags.push("LLG_VPI_OBJ_IMPLICIT");
        }
        for (set, flag) in [
            (entry.qualifiers.scalared, "LLG_VPI_OBJ_SCALARED"),
            (entry.qualifiers.vectored, "LLG_VPI_OBJ_VECTORED"),
            (entry.qualifiers.by_name, "LLG_VPI_OBJ_BY_NAME"),
            (entry.qualifiers.explicit_name, "LLG_VPI_OBJ_EXPLICIT"),
            (entry.qualifiers.dpi_import, "LLG_VPI_OBJ_DPI_IMPORT"),
            (entry.qualifiers.dpi_export, "LLG_VPI_OBJ_DPI_EXPORT"),
            (entry.qualifiers.dpi_pure, "LLG_VPI_OBJ_DPI_PURE"),
            (entry.qualifiers.dpi_context, "LLG_VPI_OBJ_DPI_CONTEXT"),
        ] {
            if set {
                flags.push(flag);
            }
        }
        let (constant, real_constant) = match &entry.constant {
            Some(IrVpiConstant::Bits(digits)) => (Some(digits.as_str()), 0.0),
            Some(IrVpiConstant::Text(text)) => {
                flags.push("LLG_VPI_OBJ_TEXT");
                (Some(text.as_str()), 0.0)
            }
            Some(IrVpiConstant::Real(value)) if value.is_finite() => (None, *value),
            Some(IrVpiConstant::Real(_)) | None => (None, 0.0),
        };
        let flags = if flags.is_empty() {
            "0u".to_owned()
        } else {
            flags.join(" | ")
        };
        let (left, right) = entry.range.unwrap_or((0, 0));
        pack.symbol(entry.type_code);
        pack.string(Some(&entry.name));
        pack.string(Some(&display_name(&path)));
        pack.string(entry.definition_name.as_deref());
        pack.string(entry.file.as_deref());
        pack.uint(u64::from(entry.line));
        pack.uint(u64::from(entry.width));
        pack.uint(u64::from(entry.signed) | u64::from(entry.real) << 1 | u64::from(entry.net) << 2);
        pack.uint(row(parent_path.as_deref()));
        pack.uint(entry.time_unit_fs);
        pack.symbol(entry.subtype);
        pack.symbol(&flags);
        pack.int(left);
        pack.int(right);
        pack.uint(entry.dims.len() as u64);
        for (left, right) in &entry.dims {
            pack.int(i64::from(*left));
            pack.int(i64::from(*right));
        }
        pack.uint(u64::from(entry.port_index));
        pack.uint(row(entry.port_target.as_deref()));
        pack.string(constant);
        pack.uint(real_constant.to_bits());
        pack.uint(entry.typespec.map_or(0, |typespec| typespec as u64 + 1));
        pack.symbol(entry.detail);
        pack.uint(entry.conns.len() as u64);
        for conn in &entry.conns {
            pack_conn(conn, indices, pack);
        }
        let (declaration, instance) = entry.activation.unwrap_or((0, 0));
        pack.uint(u64::from(declaration));
        pack.uint(u64::from(instance));
        match (&entry.timing, &timing[index]) {
            (Some(entry_timing), Some((ops, op_count))) => {
                pack.uint(1);
                pack.uint(*op_count as u64);
                if let Some(ops) = ops {
                    pack.reference(ops.clone());
                }
                pack.uint(entry_timing.declared.len() as u64);
                for value in &entry_timing.declared {
                    pack.int(*value);
                }
                pack.uint(u64::from(entry_timing.count));
                pack.symbol(if entry_timing.polarity.is_empty() {
                    "0"
                } else {
                    entry_timing.polarity
                });
                pack.symbol(if entry_timing.data_polarity.is_empty() {
                    "0"
                } else {
                    entry_timing.data_polarity
                });
                pack.flag(entry_timing.has_ifnone);
                pack.uint(entry_timing.delay.map_or(0, |entry| u64::from(entry) + 1));
            }
            _ => pack.uint(0),
        }
        pack.uint(
            entry
                .dynamic
                .as_ref()
                .map_or(0, |(_, _, shape)| *shape as u64 + 1),
        );
        pack.uint(entry.code.map_or(0, |code| u64::from(code) + 1));
        match &entry.instance {
            Some(instance) => {
                pack.uint(1);
                pack.string(instance.def_file.as_deref());
                pack.uint(u64::from(instance.def_line));
                pack.string(Some(&instance.library));
                pack.string(Some(&instance.cell));
                pack.string(instance.config.as_deref());
                pack.symbol(instance.net_type);
                pack.symbol(instance.unconnected_drive);
                pack.flag(instance.cell_define);
                pack.flag(instance.automatic);
            }
            None => pack.uint(0),
        }
        match entry.strength {
            Some((strength0, strength1)) => {
                pack.uint(1);
                pack.symbol(strength0);
                pack.symbol(strength1);
            }
            None => pack.uint(0),
        }
        // Pointers to model storage, in mask order. A sequential UDP's put
        // record replaces any dynamic storage.
        let present = |text: &str| (text != "NULL").then(|| text.to_owned());
        let (storage, dependency) = entry
            .dynamic
            .as_ref()
            .map_or((None, None), |(storage, dependency, _)| {
                (present(storage), present(dependency))
            });
        let storage = match udp_puts[index] {
            Some(record) => Some(format!("&llg_vpi_udp_puts[{record}]")),
            None => storage,
        };
        let references = [
            (REF_PACKED, present(&entry.packed)),
            (REF_REAL, present(&entry.real_value)),
            (REF_NET, present(&entry.net_pointer)),
            (REF_EVENT, present(&entry.event)),
            (REF_STORAGE, storage),
            (REF_DEPENDENCY, dependency),
        ];
        let mut mask = 0;
        for (bit, reference) in references {
            if let Some(reference) = reference {
                mask |= bit;
                pack.reference(reference);
            }
        }
        pack.uint(mask);
    }
}

/// One `llg_vpi_conn_t` of a packed object row.
fn pack_conn(
    conn: &IrVpiConn,
    indices: &std::collections::HashMap<&str, usize>,
    pack: &mut pack::Pack,
) {
    let object = |name: &str| indices.get(name).map(|row| *row as i64);
    // (kind, const_type, object, offset, select_width, left, right)
    let expression = ("LLG_VPI_ARG_EXPRESSION", "0", -1, 0, 0, 0, 0);
    let reference = match &conn.reference {
        IrVpiArgRef::Expression => expression,
        IrVpiArgRef::Constant(kind) => ("LLG_VPI_ARG_CONSTANT", *kind, -1, 0, 0, 0, 0),
        IrVpiArgRef::Object(name) => object(name).map_or(expression, |row| {
            ("LLG_VPI_ARG_OBJECT", "0", row, 0, 0, 0, 0)
        }),
        IrVpiArgRef::Select {
            object: name,
            lsb,
            width,
            left,
            right,
        } => object(name).map_or(expression, |row| {
            (
                "LLG_VPI_ARG_SELECT",
                "0",
                row,
                u64::from(*lsb),
                *width,
                *left,
                *right,
            )
        }),
        IrVpiArgRef::Element {
            object: name,
            index,
        } => object(name).map_or(expression, |row| {
            ("LLG_VPI_ARG_ELEMENT", "0", row, *index, 0, 0, 0)
        }),
    };
    let (kind, const_type, row, offset, select_width, left, right) = reference;
    pack.symbol(kind);
    pack.symbol(const_type);
    pack.int(row);
    pack.uint(offset);
    pack.uint(u64::from(select_width));
    pack.int(left);
    pack.int(right);
    pack.uint(u64::from(conn.width));
    pack.flag(conn.signed);
    pack.uint(conn.code.map_or(0, |code| u64::from(code) + 1));
    pack.symbol(if conn.direction == crate::sim::ir::IrVpiDirection::None {
        "0"
    } else {
        conn.direction.c_code()
    });
    pack.string(conn.value.as_deref());
    // A plain connection leaves role, edge and condition zero.
    if conn.role == crate::sim::ir::IrVpiConnRole::Connection {
        pack.symbol("0");
        pack.uint(0);
        pack.uint(0);
    } else {
        pack.symbol(conn.role.c_code());
        pack.uint(u64::from(conn.edge));
        pack.uint(conn.condition.map_or(0, |index| u64::from(index) + 1));
    }
}

/// The `.code` initializer of a catalog row, connection or argument that
/// has an expression or statement object; nothing otherwise.
fn code_field(code: Option<u32>) -> String {
    code.map_or_else(String::new, |code| {
        format!(", .code = {}u", u64::from(code) + 1)
    })
}

/// Emit the expression and statement objects (`llg_vpi_code_tables_t`):
/// one positional row per object, deduplicated texts and select ranges, and
/// the source files. A model generated without them gets NULL tables.
fn render_vpi_code(
    model: &IrModel,
    indices: &std::collections::HashMap<&str, usize>,
    pack: &mut pack::Pack,
    out: &mut String,
) {
    use std::collections::HashMap;
    let codes = model.vpi_code();
    if codes.is_empty() {
        out.push_str("static const llg_vpi_code_tables_t* const llg_vpi_code_tables = NULL;\n");
        return;
    }
    // IR objects resolve to emitted rows by name: emission may add scope
    // rows or drop objects without storage.
    let row = |object: Option<usize>| {
        object
            .and_then(|object| model.vpi_objects().get(object))
            .and_then(|object| indices.get(object.full_name.as_str()))
            .map_or(-1, |row| i64::try_from(*row).unwrap_or(-1))
    };
    let mut texts = Vec::<&str>::new();
    let mut text_index = HashMap::<&str, usize>::new();
    let mut ranges = Vec::<(i64, i64)>::new();
    let mut range_index = HashMap::<(i64, i64), usize>::new();
    let mut spans = Vec::<(u32, u32, u32)>::new();
    // Lines, scopes and first children follow the row order closely, so
    // they are packed as differences (`catalog_unpack_code`).
    let mut previous_line = 0i64;
    let mut previous_scope = 0i64;
    for (row_index, code) in codes.iter().enumerate() {
        let span = code.span.map_or(0, |span| {
            spans.push(span);
            spans.len()
        });
        let data = match (&code.text, code.range) {
            (Some(text), _) => {
                let next = texts.len();
                let index = *text_index.entry(text.as_str()).or_insert_with(|| {
                    texts.push(text.as_str());
                    next
                });
                index + 1
            }
            (None, Some(range)) => {
                let next = ranges.len();
                let index = *range_index.entry(range).or_insert_with(|| {
                    ranges.push(range);
                    next
                });
                index + 1
            }
            (None, None) => 0,
        };
        let flags = IrVpiCode::FLAG_NAMES
            .iter()
            .enumerate()
            .filter(|(bit, _)| code.flags & (1 << bit) != 0)
            .map(|(_, name)| *name)
            .collect::<Vec<_>>();
        let flags = if flags.is_empty() {
            "0u".to_owned()
        } else {
            flags.join("|")
        };
        let object = if code.flags & IrVpiCode::ALIAS != 0 || code.object.is_some() {
            row(code.object)
        } else {
            -1
        };
        let scope = row(code.scope);
        pack.symbol(code.type_code);
        pack.symbol(code.subtype);
        pack.symbol(code.role);
        pack.symbol(code.qualifier);
        pack.symbol(&flags);
        pack.uint(u64::from(code.width));
        pack.uint(code.file.map_or(0, |file| u64::from(file) + 1));
        pack.int(i64::from(code.line) - previous_line);
        previous_line = i64::from(code.line);
        pack.int(object);
        pack.int(scope + 1 - previous_scope);
        previous_scope = scope + 1;
        pack.int(i64::from(code.first) - row_index as i64);
        pack.uint(u64::from(code.count));
        pack.uint(data as u64);
        pack.uint(code.typespec.map_or(0, |typespec| typespec as u64 + 1));
        pack.uint(span as u64);
    }
    let list = |name: &str, items: Vec<String>, ty: &str, out: &mut String| {
        if items.is_empty() {
            return "NULL".to_owned();
        }
        out.push_str(&format!("static const {ty} {name}[] = {{\n"));
        for item in items {
            out.push_str(&format!("    {item},\n"));
        }
        out.push_str("};\n");
        name.to_owned()
    };
    let text_count = texts.len();
    let texts = list(
        "llg_vpi_code_texts",
        texts.into_iter().map(c_string_literal).collect(),
        "char* const",
        out,
    );
    let file_count = model.vpi_code_files().len();
    let files = list(
        "llg_vpi_code_files",
        model
            .vpi_code_files()
            .iter()
            .map(|file| c_string_literal(file))
            .collect(),
        "char* const",
        out,
    );
    let range_count = ranges.len();
    let ranges = list(
        "llg_vpi_code_ranges",
        ranges
            .into_iter()
            .map(|(left, right)| format!("{left}LL, {right}LL"))
            .collect(),
        "int64_t",
        out,
    );
    let span_count = spans.len();
    let spans = list(
        "llg_vpi_code_spans",
        spans
            .into_iter()
            .map(|(column, end_line, end_column)| format!("{column}u, {end_line}u, {end_column}u"))
            .collect(),
        "uint32_t",
        out,
    );
    let (udps, udp_count) = render_vpi_udps(model, out);
    out.push_str(&format!(
        "static const llg_vpi_code_tables_t llg_vpi_code_value = {{ NULL, {}u, \
{texts}, {text_count}u, {files}, {file_count}u, {ranges}, {range_count}u, {spans}, {span_count}u, \
{udps}, {udp_count}u }};\n\
static const llg_vpi_code_tables_t* const llg_vpi_code_tables = &llg_vpi_code_value;\n",
        codes.len()
    ));
}

/// The UDP definitions (`llg_vpi_udp_t`, 37.32): returns the table name (or
/// `NULL`) and its length.
fn render_vpi_udps(model: &IrModel, out: &mut String) -> (String, usize) {
    use std::fmt::Write as _;
    let udps = model.vpi_udps();
    if udps.is_empty() {
        return ("NULL".to_owned(), 0);
    }
    let mut rows = String::new();
    for (index, udp) in udps.iter().enumerate() {
        let _ = writeln!(
            out,
            "static const char* const llg_vpi_udp_ports_{index}[] = {{ {} }};",
            udp.ports
                .iter()
                .map(|(name, _)| c_string_literal(name))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(
            out,
            "static const int llg_vpi_udp_directions_{index}[] = {{ {} }};",
            udp.ports
                .iter()
                .map(|(_, direction)| *direction)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(
            out,
            "static const char* const llg_vpi_udp_entries_{index}[] = {{ {} }};",
            udp.entries
                .iter()
                .map(|(text, _)| c_string_literal(text))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(
            out,
            "static const int llg_vpi_udp_lines_{index}[] = {{ {} }};",
            udp.entries
                .iter()
                .map(|(_, line)| line.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let initial = udp
            .initial
            .map_or_else(|| "0".to_owned(), |c| format!("'{}'", char::from(c)));
        let _ = writeln!(
            rows,
            "    {{ {}, {}, {}, {}u, {}, {initial}, llg_vpi_udp_ports_{index}, \
llg_vpi_udp_directions_{index}, {}u, llg_vpi_udp_entries_{index}, llg_vpi_udp_lines_{index}, {}u }},",
            c_string_literal(&udp.name),
            c_string_literal(&udp.file),
            udp.line,
            udp.input_count,
            i32::from(udp.sequential),
            udp.ports.len(),
            udp.entries.len(),
        );
    }
    out.push_str("static const llg_vpi_udp_t llg_vpi_udps[] = {\n");
    out.push_str(&rows);
    out.push_str("};\n");
    ("llg_vpi_udps".to_owned(), udps.len())
}

/// The model values each timing row's delays live in (`llg_vpi_timing_t`
/// ops), as `llg_sdf_op_t` operations on the path, check and gate pools.
/// Emits the ops arrays and returns, per entry, the array's name (`None`
/// without targets) and length; `None` for an entry without timing. The
/// rest of a row's timing (declared values, polarity) is packed.
fn render_vpi_timing(
    model: &IrModel,
    entries: &[Entry],
    out: &mut String,
) -> Vec<Option<(Option<String>, usize)>> {
    let row_starts = super::storage::path_row_starts(model);
    let check_starts = super::storage::timing_check_starts(model);
    let path_row = |set: u32, destination: u32, path: u32| {
        row_starts
            .get(set as usize)
            .and_then(|set| set.get(destination as usize))
            .map(|first| first + path as usize)
    };
    let gates = model.sdf_gate_delays().len();
    let loads = model.sdf_loads().len();
    let checks = model.timing_checks();
    let mut fields = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let Some(timing) = &entry.timing else {
            fields.push(None);
            continue;
        };
        // A target outside the model's pools (no such row) is left out:
        // the value is then read as declared and cannot be written.
        let ops = timing
            .targets
            .iter()
            .filter_map(|(limit, target)| {
                let target = match *target {
                    IrSdfTarget::Path {
                        set,
                        destination,
                        path,
                    } => format!(
                        ".kind = LLG_SDF_PATH, .path = &llg_path_row_pool[{}]",
                        path_row(set, destination, path)?
                    ),
                    IrSdfTarget::Check { set, check, second } => {
                        let set_checks = checks.get(set as usize)?;
                        if check as usize >= set_checks.checks().len() {
                            return None;
                        }
                        format!(
                            ".kind = LLG_SDF_CHECK, .flags = {}u, .check = \
                             &llg_timing_check_pool[{}], .checks = &llg_timing_set_{set}",
                            if second { 4 } else { 0 },
                            check_starts.get(set as usize)? + check as usize
                        )
                    }
                    IrSdfTarget::Pair { set, pair, after } => {
                        checks.get(set as usize)?;
                        format!(
                            ".kind = LLG_SDF_PAIR, .flags = {}u, .index = {pair}u, .checks = \
                             &llg_timing_set_{set}",
                            if after { 4 } else { 0 }
                        )
                    }
                    IrSdfTarget::Gate { slot } if (slot as usize) < gates => format!(
                        ".kind = LLG_SDF_GATE, .gate = &llg_sdf_gate_delays[{}]",
                        3 * slot
                    ),
                    // A load port bit (limit: its bit in the port).
                    IrSdfTarget::Load { load, .. } if (load as usize) < loads => {
                        format!(".kind = LLG_SDF_LOAD, .load = &llg_sdf_load_{load}")
                    }
                    _ => return None,
                };
                Some(format!("{{ {target}, .first = {limit}u }}"))
            })
            .collect::<Vec<_>>();
        if ops.is_empty() {
            fields.push(Some((None, 0)));
            continue;
        }
        out.push_str(&format!(
            "static const llg_sdf_op_t llg_vpi_ops_{index}[] = {{ {} }};\n",
            ops.join(", ")
        ));
        fields.push(Some((Some(format!("llg_vpi_ops_{index}")), ops.len())));
    }
    fields
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
/// The interned typespecs, their ranges, members, enum constants and member
/// nodes, and the typedefs of each catalog scope as (row, typespec) pairs in
/// row order (declaration order within a row).
fn render_vpi_typespecs(
    typespecs: &[IrVpiTypespec],
    scope_typedefs: &[(usize, usize)],
    scope_imports: &[(usize, u32)],
    out: &mut String,
) {
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
        match typespec.container {
            Some(IrVpiContainer::Dynamic) => flags.push("LLG_VPI_TS_DYNAMIC"),
            Some(IrVpiContainer::Queue) => flags.push("LLG_VPI_TS_QUEUE"),
            Some(IrVpiContainer::Associative) => flags.push("LLG_VPI_TS_ASSOC"),
            None => {}
        }
        let flags = if flags.is_empty() {
            "0u".to_owned()
        } else {
            flags.join(" | ")
        };
        let optional = |entry: Option<usize>| entry.map_or(0, |entry| entry + 1);
        rows.push(format!(
            "    {{ {}, {}, {}u, {}u, {}u, {}u, {flags}, {}u, {}u, {}u, {}u, {}u, {}u, {}u, {}u, {}u, {}, {}u }},\n",
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
            typespec.line,
            typespec
                .file
                .as_deref()
                .map_or_else(|| "NULL".to_owned(), c_string_literal),
            optional(typespec.index),
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
    let typedef_count = scope_typedefs.len();
    table(
        out,
        "uint32_t",
        "llg_vpi_ts_scope_typedefs",
        scope_typedefs
            .iter()
            .map(|(row, typespec)| format!("    {row}u, {typespec}u,\n"))
            .collect(),
    );
    let import_count = scope_imports.len();
    table(
        out,
        "uint32_t",
        "llg_vpi_ts_scope_imports",
        scope_imports
            .iter()
            .map(|(row, object)| format!("    {row}u, {object}u,\n"))
            .collect(),
    );
    out.push_str(&format!(
        "static const llg_vpi_type_tables_t llg_vpi_type_tables_value = {{ llg_vpi_ts_types, {typespec_count}u, \
llg_vpi_ts_ranges, {range_count}u, llg_vpi_ts_members, {member_count}u, llg_vpi_ts_consts, {const_count}u, \
llg_vpi_ts_nodes, {node_count}u, llg_vpi_ts_scope_typedefs, {typedef_count}u, \
llg_vpi_ts_scope_imports, {import_count}u }};\n\
static const llg_vpi_type_tables_t* const llg_vpi_type_tables = &llg_vpi_type_tables_value;\n"
    ));
}

/// `LLG_VPI_IMPORT_CLASS`: marks a class layout index among the objects of
/// the scope import table.
const IMPORT_CLASS: u32 = 0x8000_0000;

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
                "    {{ .width = {}, .is_signed = {}, .is_real = {}, .ref_kind = {reference}{} }},\n",
                arg.width,
                u8::from(arg.signed),
                u8::from(arg.real),
                code_field(arg.code),
            ));
        }
        out.push_str("};\n");
    }
}

#[cfg(test)]
mod tests {
    use super::pack::{Code, Rendered};
    use super::{
        arg_reference, member_tree, render_vpi_metadata, render_vpi_typespecs, IrVpiArgRef,
        IrVpiConstant, IrVpiTiming,
    };
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
        let _ = render_vpi_metadata(&model, &Default::default(), &mut out);
        let rendered = Rendered::parse(&out);
        let objects = rendered.objects(rendered.object_count.expect("count"));
        let row = |name: &str| {
            objects
                .iter()
                .find(|object| object.full_name.as_deref() == Some(name))
                .unwrap_or_else(|| panic!("{name}: {objects:?}"))
        };
        let blk = row("tb.blk");
        assert!(blk.flags.contains("LLG_VPI_OBJ_ACTIVATION"), "{blk:?}");
        assert_eq!(blk.activation, (7, 3));
        let other = row("tb.other");
        assert!(!other.flags.contains("LLG_VPI_OBJ_ACTIVATION"), "{other:?}");
        assert_eq!(other.activation, (0, 0));
    }

    /// A generated primitive name keeps its hidden source-order key but
    /// displays as scope and name; an unnamed object displays as its scope.
    #[test]
    fn hidden_keys_display_generated_names_and_hide_unnamed_ones() {
        let mut model = IrModel::new("tb".to_owned(), 1).expect("model");
        model.vpi_objects.push(IrVpiObject {
            full_name: "tb".to_owned(),
            name: "tb".to_owned(),
            kind: IrVpiObjectKind::Module,
            ..IrVpiObject::default()
        });
        model.vpi_objects.push(IrVpiObject {
            full_name: "tb\u{1f}\u{1e}00000001".to_owned(),
            name: "and$1".to_owned(),
            kind: IrVpiObjectKind::Primitive,
            ..IrVpiObject::default()
        });
        model.vpi_objects.push(IrVpiObject {
            full_name: "tb\u{1f}\u{1e}00000002".to_owned(),
            name: String::new(),
            kind: IrVpiObjectKind::Process,
            unnamed: true,
            ..IrVpiObject::default()
        });
        let mut out = String::new();
        let _ = render_vpi_metadata(&model, &Default::default(), &mut out);
        let rendered = Rendered::parse(&out);
        assert_eq!(rendered.object_count, Some(3));
        let objects = rendered.objects(3);
        let names = objects
            .iter()
            .map(|object| object.full_name.as_deref().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(names, ["tb", "tb.and$1", "tb"]);
        assert!(!out.contains('\u{1e}'), "{out}");
        assert!(!rendered.pool.contains(&0x1e), "{objects:?}");
    }

    #[test]
    fn typespec_tables_use_index_plus_one_for_optional_references() {
        let mut typespecs = nested();
        typespecs[0].element = Some(2);
        typespecs[0].name = Some("nib_t".to_owned());
        let mut out = String::new();
        render_vpi_typespecs(&typespecs, &[], &[], &mut out);
        assert!(out.contains("{ vpiLogicTypespec, \"nib_t\", 0u, 3u, 0u, 4u, 0u, 0u, 1u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, NULL, 0u },"), "{out}");
        assert!(out.contains("#define llg_vpi_ts_consts NULL"), "{out}");
        assert!(out.contains("llg_vpi_ts_nodes, 6u"), "{out}");
    }

    /// An associative array typespec carries its array kind as a flag and
    /// its key typespec as index + 1 in the last field (A19-D60).
    #[test]
    fn container_typespecs_emit_their_kind_and_key() {
        let typespecs = vec![
            IrVpiTypespec {
                kind: IrVpiTypespecKind::String,
                ..IrVpiTypespec::default()
            },
            IrVpiTypespec {
                kind: IrVpiTypespecKind::Array,
                element: Some(0),
                container: Some(crate::sim::ir::IrVpiContainer::Associative),
                index: Some(0),
                ..IrVpiTypespec::default()
            },
            IrVpiTypespec {
                kind: IrVpiTypespecKind::Array,
                element: Some(0),
                container: Some(crate::sim::ir::IrVpiContainer::Queue),
                ..IrVpiTypespec::default()
            },
        ];
        let mut out = String::new();
        render_vpi_typespecs(&typespecs, &[], &[], &mut out);
        assert!(out.contains("{ vpiArrayTypespec, NULL, 0u, 1u, 0u, 0u, LLG_VPI_TS_ASSOC, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, NULL, 1u },"), "{out}");
        assert!(out.contains("{ vpiArrayTypespec, NULL, 0u, 1u, 0u, 0u, LLG_VPI_TS_QUEUE, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, NULL, 0u },"), "{out}");
    }

    /// `assign y = a + 2` with `a` a catalog variable: children follow
    /// their parent, optional references are index + 1, an alias names the
    /// emitted row and literal texts are interned once.
    #[test]
    fn code_rows_are_positional_with_interned_texts() {
        use crate::sim::ir::IrVpiCode;
        let mut model = IrModel::new("tb".to_owned(), 1).expect("model");
        model.vpi_objects.push(IrVpiObject {
            full_name: "tb".to_owned(),
            name: "tb".to_owned(),
            kind: IrVpiObjectKind::Module,
            ..IrVpiObject::default()
        });
        model.vpi_objects.push(IrVpiObject {
            full_name: "tb\u{1f}a".to_owned(),
            name: "a".to_owned(),
            kind: IrVpiObjectKind::Reg,
            width: 8,
            ..IrVpiObject::default()
        });
        let mut add = IrVpiCode::new("vpiOperation");
        add.subtype = "vpiAddOp";
        add.width = 32;
        add.file = Some(0);
        add.line = 4;
        add.scope = Some(0);
        add.first = 1;
        add.count = 2;
        let mut a = IrVpiCode::new("0");
        a.role = "vpiOperand";
        a.flags = IrVpiCode::ALIAS;
        a.object = Some(1);
        a.text = Some("a".to_owned());
        let mut two = IrVpiCode::new("vpiConstant");
        two.subtype = "vpiDecConst";
        two.role = "vpiOperand";
        two.flags = IrVpiCode::SIGNED;
        two.width = 32;
        two.text = Some("2".to_owned());
        let mut again = IrVpiCode::new("0");
        again.flags = IrVpiCode::ALIAS;
        again.object = Some(1);
        again.text = Some("a".to_owned());
        model.vpi_code = vec![add, a, two, again];
        model.vpi_code_files = vec!["t.sv".to_owned()];
        let mut out = String::new();
        let _ = render_vpi_metadata(&model, &Default::default(), &mut out);
        let code = Rendered::parse(&out).code(4);
        let row = |type_code: &str, subtype: &str, role: &str, flags: &str| Code {
            type_code: type_code.to_owned(),
            subtype: subtype.to_owned(),
            role: role.to_owned(),
            qualifier: "0".to_owned(),
            flags: flags.to_owned(),
            ..Code::default()
        };
        let add = Code {
            width: 32,
            file: 1,
            line: 4,
            object: -1,
            scope: 1,
            first: 1,
            count: 2,
            ..row("vpiOperation", "vpiAddOp", "0", "0u")
        };
        let a = Code {
            object: 1,
            data: 1,
            ..row("0", "0", "vpiOperand", "LLG_VPI_CODE_ALIAS")
        };
        let two = Code {
            width: 32,
            object: -1,
            data: 2,
            ..row(
                "vpiConstant",
                "vpiDecConst",
                "vpiOperand",
                "LLG_VPI_CODE_SIGNED",
            )
        };
        let again = Code {
            object: 1,
            data: 1,
            ..row("0", "0", "0", "LLG_VPI_CODE_ALIAS")
        };
        assert_eq!(code, [add, a, two, again]);
        assert!(
            out.contains(
                "{ NULL, 4u, llg_vpi_code_texts, 2u, llg_vpi_code_files, 1u, NULL, 0u, NULL, 0u, NULL, 0u }"
            ),
            "{out}"
        );
        assert!(
            out.contains("static llg_vpi_model_object_t llg_vpi_objects[2];\n"),
            "{out}"
        );
        assert!(
            out.contains(
                "static const llg_vpi_packed_t* const llg_vpi_packed = &llg_vpi_packed_value;\n"
            ),
            "{out}"
        );
    }

    /// Every field of a catalog row survives packing: the decoder (the C
    /// `pack_objects` layout, mirrored by `Rendered::objects`) reads back
    /// what the entries hold, references in mask order.
    #[test]
    fn packed_object_rows_round_trip_every_field() {
        use super::{pack, pack_objects, Entry};
        use crate::sim::ir::{
            IrSdfTarget, IrVpiConn, IrVpiConnRole, IrVpiDirection, IrVpiInstance,
        };
        let mut top = Entry::scope("tb".to_owned(), "tb".to_owned(), "vpiModule");
        top.top = true;
        top.instance = Some(IrVpiInstance {
            def_file: Some("t.sv".to_owned()),
            def_line: 3,
            library: "work".to_owned(),
            config: None,
            cell: "tb".to_owned(),
            cell_define: true,
            automatic: false,
            net_type: "vpiWire",
            unconnected_drive: "vpiHighZ",
        });
        top.file = Some("t.sv".to_owned());
        top.line = 3;
        top.time_unit_fs = 1_000_000;
        let mut mem = Entry::scope("tb\u{1f}mem".to_owned(), "mem".to_owned(), "vpiRegArray");
        mem.dims = vec![(0, 7), (-3, i32::MAX)];
        mem.range = Some((i64::MIN, i64::MAX));
        mem.width = 9;
        mem.signed = true;
        mem.net = true;
        mem.packed = "&G_mem".to_owned();
        mem.net_pointer = "&g_net_1".to_owned();
        mem.subtype = "vpiReg";
        mem.typespec = Some(4);
        mem.constant = Some(IrVpiConstant::Text("caf\u{e9} \"q\"??".to_owned()));
        mem.qualifiers.scalared = true;
        let mut assign = Entry::scope(
            "tb\u{1f}\u{1e}1".to_owned(),
            "assign".to_owned(),
            "vpiContAssign",
        );
        assign.unnamed = true;
        assign.port_target = Some("tb\u{1f}mem".to_owned());
        assign.strength = Some(("vpiStrongDrive", "vpiPullDrive"));
        assign.code = Some(41);
        assign.detail = "vpiAssignStmt";
        assign.conns = vec![
            IrVpiConn {
                reference: IrVpiArgRef::Select {
                    object: "tb\u{1f}mem".to_owned(),
                    lsb: 3,
                    width: 2,
                    left: 4,
                    right: 3,
                },
                width: 2,
                signed: true,
                direction: IrVpiDirection::Output,
                role: IrVpiConnRole::TchkRef,
                edge: 0x12,
                condition: Some(1),
                ..IrVpiConn::default()
            },
            IrVpiConn {
                reference: IrVpiArgRef::Constant("vpiBinaryConst"),
                width: 4,
                value: Some("10xz".to_owned()),
                code: Some(7),
                ..IrVpiConn::default()
            },
            IrVpiConn {
                reference: IrVpiArgRef::Object("tb\u{1f}gone".to_owned()),
                ..IrVpiConn::default()
            },
        ];
        assign.timing = Some(IrVpiTiming {
            targets: vec![(0, IrSdfTarget::Gate { slot: 0 })],
            declared: vec![i64::MIN, 0, 5],
            count: 3,
            polarity: "vpiPositive",
            data_polarity: "",
            has_ifnone: true,
            delay: Some(9),
        });
        let mut real = Entry::scope("tb\u{1f}r".to_owned(), "r".to_owned(), "vpiParameter");
        real.constant = Some(IrVpiConstant::Real(-1.25e-300));
        real.activation = Some((2, 5));
        real.real_value = "&G_r".to_owned();
        real.event = "&G_e".to_owned();
        real.dynamic = Some(("&G_s".to_owned(), "NULL".to_owned(), 6));
        let entries = vec![top, mem, assign, real];
        let indices = super::catalog_indices(&entries);
        let timing = vec![
            None,
            None,
            Some((Some("llg_vpi_ops_2".to_owned()), 1)),
            None,
        ];
        let udp_puts = vec![None, None, None, Some(0)];
        let mut packer = pack::Pack::default();
        pack_objects(&entries, &indices, &timing, &udp_puts, &mut packer);
        let objects = packer.take_stream();
        let mut out = String::new();
        packer.render(&objects, &Vec::new(), "llg_vpi_packed_value", &mut out);
        let rows = Rendered::parse(&out).objects(4);
        let symbol = |text: &str| text.to_owned();
        let some = |text: &str| Some(text.to_owned());

        assert_eq!(rows[0].type_code, "vpiModule");
        assert_eq!(rows[0].flags, "LLG_VPI_OBJ_TOP");
        assert_eq!(rows[0].file, some("t.sv"));
        assert_eq!(
            (rows[0].line, rows[0].time_unit_fs, rows[0].parent),
            (3, 1_000_000, 0)
        );
        assert_eq!(
            rows[0].instance,
            Some(pack::Instance {
                def_file: some("t.sv"),
                def_line: 3,
                library: some("work"),
                cell: some("tb"),
                config: None,
                net_type: symbol("vpiWire"),
                unconnected_drive: symbol("vpiHighZ"),
                cell_define: 1,
                automatic: 0,
            })
        );

        let mem = &rows[1];
        assert_eq!(mem.full_name, some("tb.mem"));
        assert_eq!(mem.parent, 1);
        assert_eq!((mem.width, mem.bits), (9, 0b101));
        assert_eq!((mem.left, mem.right), (i64::MIN, i64::MAX));
        assert_eq!(mem.dims, [(0, 7), (-3, i64::from(i32::MAX))]);
        assert_eq!(
            mem.flags,
            "LLG_VPI_OBJ_RANGE | LLG_VPI_OBJ_SCALARED | LLG_VPI_OBJ_TEXT"
        );
        assert_eq!(mem.constant, some("caf\u{e9} \"q\"??"));
        assert_eq!((mem.subtype.as_str(), mem.typespec), ("vpiReg", 5));
        assert_eq!(
            mem.refs,
            [(1, "&G_mem".to_owned()), (4, "&g_net_1".to_owned())]
        );

        let assign = &rows[2];
        assert_eq!(assign.name, some("assign"));
        assert_eq!(assign.full_name, some("tb"));
        assert_eq!(assign.flags, "LLG_VPI_OBJ_UNNAMED");
        assert_eq!((assign.target, assign.code), (2, 42));
        assert_eq!(assign.detail, "vpiAssignStmt");
        assert_eq!(
            assign.strength,
            Some((symbol("vpiStrongDrive"), symbol("vpiPullDrive")))
        );
        assert_eq!(
            assign.conns,
            [
                pack::Conn {
                    kind: symbol("LLG_VPI_ARG_SELECT"),
                    const_type: symbol("0"),
                    object: 1,
                    offset: 3,
                    select_width: 2,
                    left: 4,
                    right: 3,
                    width: 2,
                    signed: 1,
                    code: 0,
                    direction: symbol("vpiOutput"),
                    value: None,
                    role: symbol("LLG_VPI_ROLE_TCHK_REF"),
                    edge: 0x12,
                    condition: 2,
                },
                pack::Conn {
                    kind: symbol("LLG_VPI_ARG_CONSTANT"),
                    const_type: symbol("vpiBinaryConst"),
                    object: -1,
                    width: 4,
                    code: 8,
                    direction: symbol("0"),
                    value: some("10xz"),
                    role: symbol("0"),
                    ..pack::Conn::default()
                },
                pack::Conn {
                    kind: symbol("LLG_VPI_ARG_EXPRESSION"),
                    const_type: symbol("0"),
                    object: -1,
                    direction: symbol("0"),
                    role: symbol("0"),
                    ..pack::Conn::default()
                },
            ]
        );
        assert_eq!(
            assign.timing,
            Some(pack::Timing {
                op_count: 1,
                ops: some("llg_vpi_ops_2"),
                declared: vec![i64::MIN, 0, 5],
                count: 3,
                polarity: symbol("vpiPositive"),
                data_polarity: symbol("0"),
                has_ifnone: 1,
                delay: 10,
            })
        );
        assert!(assign.refs.is_empty());

        let real = &rows[3];
        assert_eq!(real.real_constant.to_bits(), (-1.25e-300f64).to_bits());
        assert_eq!(real.constant, None);
        assert_eq!(real.activation, (2, 5));
        assert_eq!(real.shape, 7);
        assert_eq!(real.flags, "LLG_VPI_OBJ_ACTIVATION | LLG_VPI_OBJ_DYNAMIC");
        // A sequential UDP's put record replaces the dynamic storage.
        assert_eq!(
            real.refs,
            [
                (2, "&G_r".to_owned()),
                (8, "&G_e".to_owned()),
                (16, "&llg_vpi_udp_puts[0]".to_owned())
            ]
        );
    }
}
