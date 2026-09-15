//! Vpi.

use super::*;

/// Emit the bounded VPI object catalog. The catalog is generated from the
/// owned waveform identities, so aliases retain separate HDL names while
/// pointing at the canonical runtime-visible value. Scope entries are
/// synthesized for every hierarchy prefix and never expose frontend nodes.
pub(super) fn render_vpi_metadata(model: &IrModel, out: &mut String) {
    if !model.vpi_objects.is_empty() {
        render_owned_vpi_metadata(model, out);
        return;
    }

    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Clone)]
    struct SignalEntry {
        full: String,
        name: String,
        parent: usize,
        type_code: &'static str,
        width: u32,
        signed: bool,
        real: bool,
        net: bool,
        value: String,
    }

    fn split_name(name: &str) -> Vec<&str> {
        name.split('\u{1f}').collect()
    }
    let mut scopes = BTreeSet::<String>::new();
    let mut leaves = Vec::<(String, usize, &'static str, u32, bool, bool, bool, String)>::new();
    for (index, signal) in model.signals.iter().enumerate() {
        let Some(hdl_name) = signal.hdl_name.as_deref() else {
            continue;
        };
        // A net driver or alias has externally visible identity even when
        // optimization found no generated HDL read.  Its canonical storage
        // is still emitted by net lowering and must remain discoverable via
        // VPI.
        if signal.omit && signal.net_driver.is_none() && signal.net_alias.is_empty() {
            continue;
        }
        let parts = split_name(hdl_name);
        if parts.len() < 2 {
            continue;
        }
        let scope_count = parts.len() - 1;
        for depth in 1..=scope_count {
            scopes.insert(parts[..depth].join("\u{1f}"));
        }
        let (width, signed, real) = match signal.ty {
            IrType::Packed { width, signed, .. } => (width, signed, false),
            IrType::Real { shortreal: _ } => (0, false, true),
        };
        let type_code = if real {
            "vpiRealVar"
        } else if signal.net_driver.is_some() || !signal.net_alias.is_empty() {
            "vpiNet"
        } else {
            "vpiReg"
        };
        let value = if real {
            format!("NULL, &{}", signal.c_name)
        } else if !signal.net_alias.is_empty() {
            format!("&llg_net_alias_{index}.visible, NULL")
        } else {
            format!("&{}, NULL", signal.c_name)
        };
        leaves.push((
            hdl_name.to_owned(),
            scope_count,
            type_code,
            width,
            signed,
            real,
            signal.net_driver.is_some() || !signal.net_alias.is_empty(),
            value,
        ));
    }
    for array in &model.arrays {
        let parts = split_name(&array.hdl_name);
        if parts.len() < 2 {
            continue;
        }
        let scope_count = parts.len() - 1;
        for depth in 1..=scope_count {
            scopes.insert(parts[..depth].join("\u{1f}"));
        }
        leaves.push((
            array.hdl_name.clone(),
            scope_count,
            "vpiRegArray",
            array.elem_width,
            array.signed,
            array.real,
            false,
            "NULL, NULL".to_owned(),
        ));
    }
    if scopes.is_empty() {
        scopes.insert(if model.design_name().is_empty() {
            "llg".to_owned()
        } else {
            model.design_name().to_owned()
        });
    }

    let mut indices = BTreeMap::<String, usize>::new();
    let mut entries = Vec::<SignalEntry>::new();
    for scope in scopes {
        let parts = split_name(&scope);
        let parent = if parts.len() <= 1 {
            usize::MAX
        } else {
            let prefix = parts[..parts.len() - 1].join("\u{1f}");
            *indices.get(&prefix).unwrap_or(&usize::MAX)
        };
        let index = entries.len();
        indices.insert(scope.clone(), index);
        entries.push(SignalEntry {
            full: scope.clone(),
            name: parts.last().copied().unwrap_or(scope.as_str()).to_owned(),
            parent,
            type_code: "vpiModule",
            width: 0,
            signed: false,
            real: false,
            net: false,
            value: "NULL, NULL".to_owned(),
        });
    }
    for (full, scope_count, type_code, width, signed, real, net, value) in leaves {
        let parts = split_name(&full);
        let scope = parts[..scope_count].join("\u{1f}");
        let parent = *indices.get(&scope).unwrap_or(&usize::MAX);
        let name = parts.last().copied().unwrap_or("").to_owned();
        entries.push(SignalEntry {
            full,
            name,
            parent,
            type_code,
            width,
            signed,
            real,
            net,
            value,
        });
    }

    debug_assert!(
        !entries.is_empty(),
        "VPI fallback catalog always has a root"
    );
    out.push_str("\n/* owned hierarchy catalog for the bounded VPI bridge */\n");
    out.push_str("static llg_vpi_model_object_t llg_vpi_objects[] = {\n");
    for entry in &entries {
        let parent = if entry.parent == usize::MAX {
            "NULL".to_owned()
        } else {
            format!("&llg_vpi_objects[{}]", entry.parent)
        };
        let full = entry.full.replace('\u{1f}', ".");
        let definition = if entry.type_code == "vpiModule" {
            entry.name.clone()
        } else {
            String::new()
        };
        let (packed, real) = entry.value.split_once(", ").unwrap_or(("NULL", "NULL"));
        out.push_str(&format!(
            "    {{ {}, {}, {}, {}, NULL, 0, {}, {}, {}, {}, {}, {}, {} }},\n",
            entry.type_code,
            c_string_literal(&entry.name),
            c_string_literal(&full),
            if definition.is_empty() {
                "NULL".to_owned()
            } else {
                c_string_literal(&definition)
            },
            entry.width,
            entry.signed as u8,
            entry.real as u8,
            entry.net as u8,
            packed,
            real,
            parent,
        ));
    }
    out.push_str("};\n");
    out.push_str("static const size_t llg_vpi_object_count = sizeof(llg_vpi_objects) / sizeof(llg_vpi_objects[0]);\n");
}

/// Render the catalog captured from owned database nodes.  Unlike the legacy
/// signal-derived fallback below, this keeps definition/source information
/// from the elaborated instance and resolves parent links by HDL path.
fn render_owned_vpi_metadata(model: &IrModel, out: &mut String) {
    use std::collections::BTreeMap;

    #[derive(Clone)]
    struct Entry {
        full: String,
        name: String,
        definition_name: Option<String>,
        file: Option<String>,
        line: u32,
        kind: IrVpiObjectKind,
        width: u32,
        signed: bool,
        real: bool,
        net: bool,
        packed: String,
        real_value: String,
        time_unit_fs: u64,
    }

    fn split_name(name: &str) -> Vec<&str> {
        name.split('\u{1f}').collect()
    }

    fn signal_storage(model: &IrModel, index: usize) -> Option<(String, String)> {
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
            return match signal.ty {
                IrType::Real { .. } => Some(("NULL".to_owned(), format!("&{}", signal.c_name))),
                IrType::Packed { .. } if !signal.net_alias.is_empty() => Some((
                    format!("&llg_net_alias_{current}.visible"),
                    "NULL".to_owned(),
                )),
                IrType::Packed { .. } => Some((format!("&{}", signal.c_name), "NULL".to_owned())),
            };
        }
    }

    let mut entries = BTreeMap::<String, Entry>::new();
    for object in &model.vpi_objects {
        let (packed, real_value) = if let Some(signal) = object.signal {
            let Some((packed, real_value)) = signal_storage(model, signal) else {
                // A malformed/optimized-away storage reference is not a
                // valid public object.  The database capture should prevent
                // this path; omitting it keeps emitted C fail-closed.
                continue;
            };
            (packed, real_value)
        } else {
            ("NULL".to_owned(), "NULL".to_owned())
        };
        entries.insert(
            object.full_name.clone(),
            Entry {
                full: object.full_name.clone(),
                name: object.name.clone(),
                definition_name: object.definition_name.clone(),
                file: object.file.clone(),
                line: object.line,
                kind: object.kind,
                width: object.width,
                signed: object.signed,
                real: object.real,
                net: object.net,
                packed,
                real_value,
                time_unit_fs: object.time_unit_fs,
            },
        );
    }

    // A signal may be retained under a generate scope that has no explicit
    // module-instance node.  Add only missing prefixes and leave their source
    // location/definition empty rather than inventing frontend provenance.
    let paths = entries.keys().cloned().collect::<Vec<_>>();
    for path in paths {
        let parts = split_name(&path);
        for depth in 1..parts.len() {
            let prefix = parts[..depth].join("\u{1f}");
            entries.entry(prefix.clone()).or_insert_with(|| Entry {
                full: prefix.clone(),
                name: parts[depth - 1].to_owned(),
                definition_name: None,
                file: None,
                line: 0,
                kind: IrVpiObjectKind::Module,
                width: 0,
                signed: false,
                real: false,
                net: false,
                packed: "NULL".to_owned(),
                real_value: "NULL".to_owned(),
                time_unit_fs: 0,
            });
        }
    }

    if entries.is_empty() {
        let design_name = if model.design_name().is_empty() {
            "llg".to_owned()
        } else {
            model.design_name().to_owned()
        };
        entries.insert(
            design_name.clone(),
            Entry {
                full: design_name.clone(),
                name: design_name,
                definition_name: None,
                file: None,
                line: 0,
                kind: IrVpiObjectKind::Module,
                width: 0,
                signed: false,
                real: false,
                net: false,
                packed: "NULL".to_owned(),
                real_value: "NULL".to_owned(),
                time_unit_fs: 0,
            },
        );
    }
    out.push_str("\n/* owned hierarchy catalog for the bounded VPI bridge */\n");
    out.push_str("static llg_vpi_model_object_t llg_vpi_objects[] = {\n");
    let indices = entries
        .keys()
        .enumerate()
        .map(|(index, path)| (path.clone(), index))
        .collect::<BTreeMap<_, _>>();
    for entry in entries.values() {
        let parts = split_name(&entry.full);
        let parent_path = (parts.len() > 1).then(|| parts[..parts.len() - 1].join("\u{1f}"));
        let parent = parent_path
            .as_ref()
            .and_then(|path| indices.get(path).copied())
            .map_or_else(
                || "NULL".to_owned(),
                |index| format!("&llg_vpi_objects[{index}]"),
            );
        let file = entry
            .file
            .as_deref()
            .map(c_string_literal)
            .unwrap_or_else(|| "NULL".to_owned());
        let definition = entry
            .definition_name
            .as_deref()
            .map(c_string_literal)
            .unwrap_or_else(|| "NULL".to_owned());
        out.push_str(&format!(
            "    {{ {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}ULL }},\n",
            entry.kind.c_type(),
            c_string_literal(&entry.name),
            c_string_literal(&entry.full.replace('\u{1f}', ".")),
            definition,
            file,
            entry.line,
            entry.width,
            entry.signed as u8,
            entry.real as u8,
            entry.net as u8,
            entry.packed,
            entry.real_value,
            parent,
            entry.time_unit_fs,
        ));
    }
    out.push_str("};\n");
    out.push_str(
        "static const size_t llg_vpi_object_count = sizeof(llg_vpi_objects) / sizeof(llg_vpi_objects[0]);\n",
    );
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
    out.push_str("\n/* compile-time descriptors for VPI system calls */\n");
    for (index, call) in model.vpi_compile_calls.iter().enumerate() {
        out.push_str(&format!(
            "static const llg_vpi_compile_arg_t llg_vpi_compile_args_{index}[] = {{\n"
        ));
        if call.args.is_empty() {
            out.push_str("    { 0, 0, 0 },\n");
        } else {
            for arg in &call.args {
                out.push_str(&format!(
                    "    {{ {}, {}, {} }},\n",
                    arg.width, arg.signed as u8, arg.real as u8
                ));
            }
        }
        out.push_str("};\n");
    }
}
