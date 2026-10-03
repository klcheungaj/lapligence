//! Dynamic startup replaces static packed constructors. Cleanup is idempotent.
use super::*;
use std::collections::{HashMap, HashSet};

/// Value representation shared by every object in one storage table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum StorageKind {
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Real,
}

/// Plain, statically allocated storage objects whose default and teardown
/// calls are emitted as one table-driven loop per value representation rather
/// than one call per object. A block flushes at the position its objects
/// previously occupied, so its ordering relative to the other initialization
/// and teardown statements is unchanged; objects inside a block are
/// independent, so their relative order does not matter.
#[derive(Default)]
struct StorageBlock {
    groups: Vec<(StorageKind, Vec<String>)>,
    index: HashMap<StorageKind, usize>,
}

impl StorageBlock {
    fn push(&mut self, name: &str, width: u32, signed: bool, two_state: bool) {
        let kind = if width == 0 {
            StorageKind::Real
        } else {
            StorageKind::Packed {
                width,
                signed,
                two_state,
            }
        };
        let groups = &mut self.groups;
        let slot = *self.index.entry(kind).or_insert_with(|| {
            groups.push((kind, Vec::new()));
            groups.len() - 1
        });
        self.groups[slot].1.push(name.to_owned());
    }

    fn flush(&mut self, tables: &mut StorageTables, init: &mut String, destroy: &mut String) {
        for (kind, names) in std::mem::take(&mut self.groups) {
            let default = match kind {
                StorageKind::Packed {
                    width,
                    signed,
                    two_state,
                } => super::super::super::expressions::packed_default(width, signed, two_state),
                StorageKind::Real => String::new(),
            };
            if names.len() < 2 {
                for name in &names {
                    match kind {
                        StorageKind::Packed { .. } => {
                            init.push_str(&format!("    sv4_replace(&{name}, {default});\n"));
                            destroy.push_str(&format!("    sv4_destroy(&{name});\n"));
                        }
                        StorageKind::Real => init.push_str(&format!("    {name} = 0.0;\n")),
                    }
                }
                continue;
            }
            let element = match kind {
                StorageKind::Packed { .. } => "sv4_t",
                StorageKind::Real => "double",
            };
            let table = tables.pointer_table(element, &names);
            let each = format!(
                "for (size_t _llg_n = 0; _llg_n < sizeof({table}) / sizeof({table}[0]); ++_llg_n)"
            );
            match kind {
                StorageKind::Packed { .. } => {
                    init.push_str(&format!(
                        "    {each}\n        sv4_replace({table}[_llg_n], {default});\n"
                    ));
                    destroy.push_str(&format!(
                        "    {each}\n        sv4_destroy({table}[_llg_n]);\n"
                    ));
                }
                StorageKind::Real => {
                    init.push_str(&format!("    {each}\n        *{table}[_llg_n] = 0.0;\n"));
                }
            }
        }
        self.index.clear();
    }
}

/// File-scope pointer tables referenced by the storage lifecycle loops.
#[derive(Default)]
struct StorageTables {
    source: String,
    count: usize,
}

impl StorageTables {
    fn pointer_table(&mut self, element: &str, names: &[String]) -> String {
        let table = format!("llg_storage_{}", self.count);
        self.count += 1;
        self.named_pointer_table(element, &table, names);
        table
    }

    fn named_pointer_table(&mut self, element: &str, table: &str, names: &[String]) {
        self.source.push_str(&format!(
            "static {element}* const {table}[{}] = {{\n",
            names.len()
        ));
        for chunk in names.chunks(8) {
            let entries = chunk
                .iter()
                .map(|name| format!("&{name}"))
                .collect::<Vec<_>>()
                .join(", ");
            self.source.push_str(&format!("    {entries},\n"));
        }
        self.source.push_str("};\n");
    }
}

fn table_loop(table: &str) -> String {
    format!("for (size_t _llg_n = 0; _llg_n < sizeof({table}) / sizeof({table}[0]); ++_llg_n)")
}

fn net_storage_lifecycle(
    model: &IrModel,
    tables: &mut StorageTables,
    initialize: &mut String,
    destroy: &mut String,
) {
    let mut emitted = HashSet::new();
    let groups = model
        .net_groups
        .iter()
        .filter(|group| emitted.insert(group.c_name.as_str()))
        .collect::<Vec<_>>();
    if groups.is_empty() {
        return;
    }
    tables
        .source
        .push_str("static const struct { llg_net_t* net; uint8_t fill; } llg_net_storage[] = {\n");
    for group in groups {
        let fill = match group.kind {
            IrNetKind::Tri0 | IrNetKind::Supply0 => 0,
            IrNetKind::Tri1 | IrNetKind::Supply1 => 1,
            _ => 3,
        };
        tables
            .source
            .push_str(&format!("    {{ &{}, {fill} }},\n", group.c_name));
    }
    tables.source.push_str("};\n");
    let each = table_loop("llg_net_storage");
    initialize.push_str(&format!(
        "    {each} {{\n        llg_net_t* net = llg_net_storage[_llg_n].net;\n        for (int slot = 0; slot < net->n_drivers; ++slot)\n            sv4_replace(net->drivers[slot], sv4_fill(3, net->width, net->is_signed));\n        llg_net_index_reset(net);\n        sv4_replace(&net->resolved, sv4_fill(llg_net_storage[_llg_n].fill, net->width, net->is_signed));\n        net->n_aliases = 0;\n    }}\n"
    ));
    destroy.push_str(&format!(
        "    {each} {{\n        llg_net_t* net = llg_net_storage[_llg_n].net;\n        for (int slot = 0; slot < net->n_drivers; ++slot)\n            sv4_destroy(net->drivers[slot]);\n        sv4_destroy(&net->resolved);\n        net->propagation = NULL;\n    }}\n"
    ));
}

fn alias_visible_lifecycle(table: &str, initialize: &mut String, destroy: &mut String) {
    let each = table_loop(table);
    initialize.push_str(&format!(
        "    {each} {{\n        llg_net_alias_t* alias = {table}[_llg_n];\n        sv4_copy(&alias->visible, alias->storage);\n    }}\n"
    ));
    destroy.push_str(&format!(
        "    {each}\n        sv4_destroy(&{table}[_llg_n]->visible);\n"
    ));
}

pub(in crate::sim::emit_c) fn storage_lifecycle(
    model: &IrModel,
    out: &mut String,
) -> Result<(), String> {
    let mut initialize = String::new();
    let mut destroy = String::new();
    let mut tables = StorageTables::default();
    let mut block = StorageBlock::default();
    let mut fixed_defaults = String::new();
    let mut emitted = HashSet::new();
    for signal in &model.signals {
        if signal.net_driver.is_some()
            || signal.alias.is_some()
            || (signal.omit && signal.net_alias.is_empty())
            || !emitted.insert(signal.c_name.clone())
        {
            continue;
        }
        block.push(
            &signal.c_name,
            signal.ty.width(),
            signal.ty.signed(),
            signal.ty.two_state(),
        );
        if let Some(value) = &signal.fixed_default {
            fixed_defaults.push_str(&format!(
                "    sv4_replace(&{}, {});\n",
                signal.c_name,
                emit_const(value)
            ));
        }
    }
    block.flush(&mut tables, &mut initialize, &mut destroy);
    initialize.push_str(&std::mem::take(&mut fixed_defaults));
    net_storage_lifecycle(model, &mut tables, &mut initialize, &mut destroy);
    let aliases = model
        .signals
        .iter()
        .enumerate()
        .filter(|(_, signal)| !signal.net_alias.is_empty())
        .map(|(index, _)| format!("llg_net_alias_{index}"))
        .collect::<Vec<_>>();
    if !aliases.is_empty() {
        let table = tables.pointer_table("llg_net_alias_t", &aliases);
        alias_visible_lifecycle(&table, &mut initialize, &mut destroy);
        // All visible owners exist before bindings refresh any of them.
        initialize.push_str(&format!(
            "    {}\n        llg_net_alias_bind({table}[_llg_n]);\n",
            table_loop(&table)
        ));
    }
    if !model.net_groups.is_empty() {
        destroy.push_str(&format!(
            "    {}\n        llg_net_alias_clear(llg_net_storage[_llg_n].net);\n",
            table_loop("llg_net_storage")
        ));
    }
    emitted.clear();
    // Function return slots and static locals are independent persistent
    // objects: tabled packed/real defaults first, then string/chandle resets
    // and explicit fixed defaults in their original order.
    let mut native_init = String::new();
    let mut native_destroy = String::new();
    for (index, function) in model.funcs.iter().enumerate() {
        if !function.automatic {
            if function.ret_string {
                string_defaults(
                    &mut native_init,
                    &mut native_destroy,
                    &format!("_llg_native_ret_{index}"),
                );
            } else if function.ret_chandle {
                native_init.push_str(&format!("    _llg_native_ret_{index} = NULL;\n"));
            }
            if let Some(ty) = function.ret.filter(|_| function.return_signal.is_none()) {
                block.push(
                    &format!("_llg_ret_{index}"),
                    ty.width(),
                    ty.signed(),
                    ty.two_state(),
                );
                if let Some(value) = &function.return_default {
                    fixed_defaults.push_str(&format!(
                        "    sv4_replace(&_llg_ret_{index}, {});\n",
                        emit_const(value)
                    ));
                }
            }
        }
        for local in &function.locals {
            if emitted.insert(local.c_name().to_owned()) {
                if local.string {
                    string_defaults(&mut native_init, &mut native_destroy, local.c_name());
                } else {
                    block.push(
                        local.c_name(),
                        if local.real { 0 } else { local.width() },
                        local.signed(),
                        local.two_state,
                    );
                    if let Some(value) = &local.fixed_default {
                        fixed_defaults.push_str(&format!(
                            "    sv4_replace(&{}, {});\n",
                            local.c_name(),
                            emit_const(value)
                        ));
                    }
                }
            }
        }
    }
    block.flush(&mut tables, &mut initialize, &mut destroy);
    initialize.push_str(&native_init);
    initialize.push_str(&std::mem::take(&mut fixed_defaults));
    destroy.push_str(&native_destroy);
    for array in &model.arrays {
        defaults(
            &mut initialize,
            &mut destroy,
            &format!("{}_llg_contents_dep", array.c_name),
            1,
            false,
            true,
        );
        initialize.push_str(&format!(
            "    for (uint64_t _i = 0; _i < {}ULL; ++_i) {{\n",
            array.total
        ));
        destroy.push_str(&format!(
            "    for (uint64_t _i = 0; _i < {}ULL; ++_i) {{\n",
            array.total
        ));
        defaults(
            &mut initialize,
            &mut destroy,
            &format!("{}[_i]", array.c_name),
            if array.real { 0 } else { array.elem_width },
            array.signed,
            array.two_state,
        );
        if let Some(value) = &array.element_default {
            initialize.push_str(&format!(
                "    sv4_replace(&{}[_i], {});\n",
                array.c_name,
                emit_const(value)
            ));
        }
        defaults(
            &mut initialize,
            &mut destroy,
            &format!("{}_llg_element_deps[_i]", array.c_name),
            1,
            false,
            true,
        );
        let bind = if array.real {
            "llg_dependency_bind_real"
        } else {
            "llg_dependency_bind"
        };
        initialize.push_str(&format!("    {bind}(&{}[_i], &{}_llg_element_deps[_i]);\n    {bind}(&{}[_i], &{}_llg_contents_dep);\n    }}\n", array.c_name, array.c_name, array.c_name, array.c_name));
        destroy.push_str("    }\n");
    }
    let array_aliases = model
        .arrays
        .iter()
        .enumerate()
        .flat_map(|(array_index, array)| {
            array
                .net_elements
                .iter()
                .map(move |(index, _)| format!("llg_array_net_{array_index}_{index}"))
        })
        .collect::<Vec<_>>();
    if !array_aliases.is_empty() {
        let table = "llg_array_net_storage";
        tables.named_pointer_table("llg_net_alias_t", table, &array_aliases);
        // Preserve array order and bind each view immediately after its copy.
        initialize.push_str(&format!(
            "    {} {{\n        llg_net_alias_t* alias = {table}[_llg_n];\n        sv4_copy(&alias->visible, alias->storage);\n        llg_net_alias_bind(alias);\n    }}\n",
            table_loop(table)
        ));
        destroy.push_str(&format!(
            "    {}\n        sv4_destroy(&{table}[_llg_n]->visible);\n",
            table_loop(table)
        ));
    }
    for container in &model.containers {
        let name = &container.c_name;
        defaults(
            &mut initialize,
            &mut destroy,
            &format!("{name}_llg_contents_dep"),
            1,
            false,
            true,
        );
        defaults(
            &mut initialize,
            &mut destroy,
            &format!("{name}_llg_shape_dep"),
            1,
            false,
            true,
        );
        initialize.push_str(&super::super::super::containers::declaration_and_init(container)?.1);
        destroy.push_str(&super::super::super::containers::destroy(container));
        if let Some(size) = container.initial_size {
            let function = if container.element.is_packed() {
                "llg_dyn_new"
            } else {
                "llg_dyn_value_new"
            };
            initialize.push_str(&format!("    {{ sv4_t size = sv4_from_u64({size}ULL, 64, 0); {function}(&{name}, size, NULL); sv4_destroy(&size); }}\n"));
        }
    }
    for object in &model.objects {
        let name = &object.c_name;
        match object.ty {
            IrObjectType::String => {
                string_defaults(&mut initialize, &mut destroy, name);
                defaults(
                    &mut initialize,
                    &mut destroy,
                    &format!("{name}_llg_dep"),
                    1,
                    false,
                    true,
                );
                initialize.push_str(&format!("    {name}.notify = llg_dependency_changed; {name}.dependency = &{name}_llg_dep;\n"));
            }
            IrObjectType::Chandle => initialize.push_str(&format!("    {name} = NULL;\n")),
            IrObjectType::Process => {
                initialize.push_str(&format!("    {name} = NULL;\n"));
                destroy.push_str(&format!("    llg_process_assign(&{name}, NULL);\n"));
            }
            IrObjectType::Semaphore => initialize.push_str(&format!("    {name} = NULL;\n")),
        }
    }
    for event in model.events.iter().filter(|event| !event.is_array()) {
        initialize.push_str(&format!(
            "    llg_event_object_reset(&{name}__object);\n    {name}.object = &{name}__object;\n",
            name = event.c_name
        ));
        destroy.push_str(&format!(
            "    llg_event_object_reset(&{name}__object);\n",
            name = event.c_name
        ));
    }
    if !model.classes.is_empty() {
        destroy.push_str("    llg_class_storage_destroy();\n");
    }
    out.push_str(&tables.source);
    out.push_str(&format!("static void llg_model_storage_defaults(void) {{\n{initialize}}}\n\nstatic void llg_model_storage_destroy(void) {{\n{destroy}}}\n\n"));
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    // SV static initialization may call legal zero-time user functions. The
    // runtime is already initialized and the frame is not a coroutine, so
    // timing-bearing callees remain impossible (functions cannot contain
    // timing); the recursion-depth guard still bounds any user call.
    for step in &model.init_steps {
        initialization_step(&mut frame, step)?;
    }
    for object in &model.objects {
        if let Some(initial) = &object.initial {
            frame.string_assign(&format!("&{}", object.c_name), initial)?;
        }
    }
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    out.push_str(&format!(
        "static void llg_model_initializers(void) {{\n{}{}\n}}\n\n",
        frame.prologue(),
        frame.body()
    ));
    Ok(())
}

fn string_defaults(init: &mut String, destroy: &mut String, name: &str) {
    init.push_str(&format!("    {name} = (llg_string_t){{0}};\n"));
    destroy.push_str(&format!(
        "    llg_string_destroy(&{name}); {name} = (llg_string_t){{0}};\n"
    ));
}

fn defaults(
    init: &mut String,
    destroy: &mut String,
    name: &str,
    width: u32,
    signed: bool,
    two_state: bool,
) {
    if width == 0 {
        init.push_str(&format!("    {name} = 0.0;\n"));
    } else {
        init.push_str(&format!(
            "    sv4_replace(&{name}, {});\n",
            super::super::super::expressions::packed_default(width, signed, two_state)
        ));
        destroy.push_str(&format!("    sv4_destroy(&{name});\n"));
    }
}

fn set_initial(frame: &mut Frame<'_, '_>, target: Binding, value: Value) {
    let value = frame.convert(
        value,
        target.width,
        target.signed,
        target.two_state,
        target.shortreal,
    );
    if target.width == 0 {
        frame.line(format!("*({}) = {};", target.address, value.code));
    } else {
        frame.line(format!("sv4_move({}, &{});", target.address, value.code));
    }
    frame.discard(value);
}

fn initialization_step(frame: &mut Frame<'_, '_>, step: &IrInitStep) -> Result<(), String> {
    let model = frame.ctx.model;
    match step {
        IrInitStep::FillArrayX(index) | IrInitStep::FillArrayZ(index) => {
            let array = model.array(*index);
            if array.real && matches!(step, IrInitStep::FillArrayZ(_)) {
                return Err("real arrays cannot be initialized to Z".to_owned());
            }
            let value = if array.real {
                "0.0".to_owned()
            } else if matches!(step, IrInitStep::FillArrayZ(_)) {
                format!(
                    "sv4_fill(3, {}, {})",
                    array.elem_width,
                    u8::from(array.signed)
                )
            } else if let Some(value) = &array.element_default {
                emit_const(value)
            } else {
                super::super::super::expressions::packed_default(
                    array.elem_width,
                    array.signed,
                    array.two_state,
                )
            };
            frame.line(format!(
                "for (uint64_t _i = 0; _i < {}ULL; ++_i) {{",
                array.total
            ));
            if array.real {
                frame.line(format!("{}[_i] = {value};", array.c_name));
            } else {
                frame.line(format!("sv4_replace(&{}[_i], {value});", array.c_name));
            }
            frame.line("}");
            if !array.net_elements.is_empty() {
                let offset = model.arrays[..*index]
                    .iter()
                    .map(|array| array.net_elements.len())
                    .sum::<usize>();
                let end = offset + array.net_elements.len();
                frame.line(format!(
                    "for (size_t _llg_n = {offset}; _llg_n < {end}; ++_llg_n) {{"
                ));
                frame.line("llg_net_alias_t* alias = llg_array_net_storage[_llg_n];");
                frame.line("sv4_replace(alias->storage, llg_net_alias_read(alias));");
                frame.line("}");
            }
        }
        IrInitStep::SetScalar { sig, value } => {
            let signal = model.signal(*sig);
            let target = Binding {
                address: format!("&{}", signal.c_name),
                width: signal.ty.width(),
                signed: signal.ty.signed(),
                two_state: signal.ty.two_state(),
                shortreal: matches!(signal.ty, IrType::Real { shortreal: true }),
                automatic: false,
            };
            let mut result = frame.value(emit_const(value), value.width, value.signed);
            result.fill = value.fill;
            set_initial(frame, target, result);
        }
        IrInitStep::SetArrayElem { arr, index, value } => {
            let array = model.array(*arr);
            let target = Binding {
                address: format!("&{}[{index}]", array.c_name),
                width: if array.real { 0 } else { array.elem_width },
                signed: array.signed,
                two_state: array.two_state,
                shortreal: array.shortreal,
                automatic: false,
            };
            let mut result = frame.value(emit_const(value), value.width, value.signed);
            result.fill = value.fill;
            set_initial(frame, target, result);
        }
        IrInitStep::RegisterSampled(index) => {
            let signal = model.signal(*index);
            if signal.ty.width() == 0 {
                return Err(pending("real-valued sampling registrations"));
            }
            frame.line(format!(
                "llg_sampled_register({});",
                frame.canonical_signal(*index)
            ));
        }
        IrInitStep::WriteNet { group, slot, value } => {
            let net = model.net_group(*group);
            let mut result = frame.value(emit_const(value), value.width, value.signed);
            result.fill = value.fill;
            let value = frame.convert(result, net.width, net.signed, false, false);
            frame.line(format!(
                "llg_net_write(&{}, {slot}, {});",
                net.c_name, value.code
            ));
            frame.discard(value);
        }
        IrInitStep::Initialize(initialization) => {
            if initialization.phase() != IrInitPhase::BeforeProcesses {
                return Ok(());
            }
            if let IrInitTarget::Fixed(lhs) = initialization.target() {
                return frame.statement(&IrStmt::Assign {
                    lhs: *lhs.clone(),
                    rhs: initialization.value().clone(),
                    nba: false,
                });
            }
            let target = match initialization.target() {
                IrInitTarget::Fixed(_) => unreachable!("fixed initializer emitted above"),
                IrInitTarget::Signal(index) => {
                    let signal = model.signal(*index);
                    Binding {
                        address: format!("&{}", signal.c_name),
                        width: signal.ty.width(),
                        signed: signal.ty.signed(),
                        two_state: signal.ty.two_state(),
                        shortreal: matches!(signal.ty, IrType::Real { shortreal: true }),
                        automatic: false,
                    }
                }
                IrInitTarget::StaticLocal { name, .. } => frame
                    .lookup(name)
                    .ok_or_else(|| format!("unknown static initializer target {name}"))?,
            };
            let value = frame.expression(initialization.value())?;
            set_initial(frame, target, value);
        }
    }
    Ok(())
}

#[cfg(test)]
mod storage_table_tests {
    use super::{StorageBlock, StorageTables};

    #[test]
    fn groups_of_two_or_more_become_loops_and_singletons_stay_direct() {
        let mut block = StorageBlock::default();
        block.push("G_a", 8, false, false);
        block.push("G_r", 0, false, false);
        block.push("G_b", 8, false, false);
        block.push("G_wide", 70, true, true);
        block.push("G_s", 0, false, false);
        let mut tables = StorageTables::default();
        let (mut init, mut destroy) = (String::new(), String::new());
        block.flush(&mut tables, &mut init, &mut destroy);

        assert!(tables
            .source
            .contains("static sv4_t* const llg_storage_0[2] = {\n    &G_a, &G_b,\n};"));
        assert!(tables
            .source
            .contains("static double* const llg_storage_1[2] = {\n    &G_r, &G_s,\n};"));
        assert!(
            init.contains("sv4_replace(llg_storage_0[_llg_n], sv4_x(8, 0));"),
            "{init}"
        );
        assert!(init.contains("*llg_storage_1[_llg_n] = 0.0;"), "{init}");
        assert!(init.contains("sv4_replace(&G_wide, "), "{init}");
        assert!(
            destroy.contains("sv4_destroy(llg_storage_0[_llg_n]);"),
            "{destroy}"
        );
        assert!(destroy.contains("sv4_destroy(&G_wide);"), "{destroy}");
        assert!(
            !destroy.contains("llg_storage_1"),
            "reals own no payload: {destroy}"
        );
        // Flushing leaves the block empty for the next storage section.
        let (mut again_init, mut again_destroy) = (String::new(), String::new());
        block.flush(&mut tables, &mut again_init, &mut again_destroy);
        assert!(again_init.is_empty() && again_destroy.is_empty());
    }
}
