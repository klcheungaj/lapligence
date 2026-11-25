//! Redrive rows: the continuous drivers a release re-establishes, as const
//! tables the runtime searches by storage address. `IrModel::redrives` give
//! the rows that act in place (a copy, a constant, a delayed driver's
//! committed value, or one plain C function per driver a `release`
//! statement names); in a catalog model the final processes give the rows
//! that wake a driver for a release through VPI.
use super::*;
use crate::sim::emit_c::model::vpi::{array_storage, signal_storage};
use crate::sim::ir::{IrRedrive, IrRedriveAction, IrRedriveTarget};

/// `(storage, cells, stride)` of a redrive target: its cell, the first of
/// the cells of a dense array, or the keyed container of a sparse array
/// (`cells` 0). `None` for storage the model does not emit.
fn target_storage(model: &IrModel, target: IrRedriveTarget) -> Option<(String, u64, &'static str)> {
    match target {
        IrRedriveTarget::Signal(signal) => {
            if model.signals.get(signal)?.omit {
                return None;
            }
            let (packed, real, _) = signal_storage(model, signal)?;
            if packed != "NULL" {
                Some((packed, 1, "sizeof(sv4_t)"))
            } else if real != "NULL" {
                Some((real, 1, "sizeof(double)"))
            } else {
                None
            }
        }
        IrRedriveTarget::Array(array) => {
            let array = model.arrays.get(array)?;
            let storage = array_storage(array)?;
            let stride = if array.real {
                "sizeof(double)"
            } else {
                "sizeof(sv4_t)"
            };
            Some((
                storage,
                if array.sparse() { 0 } else { array.total },
                stride,
            ))
        }
    }
}

/// Storage of the packed variable a `Copy` redrive reads, if the model
/// emits it.
fn copy_source(model: &IrModel, source: usize) -> Option<String> {
    if model.signals.get(source)?.omit {
        return None;
    }
    let (packed, _, _) = signal_storage(model, source)?;
    (packed != "NULL").then_some(packed)
}

/// The rows of one redrive: `(storage, cells, stride)` per emitted target;
/// none for a copy whose source the model does not emit.
fn rows(model: &IrModel, redrive: &IrRedrive) -> Vec<(String, u64, &'static str)> {
    if let IrRedriveAction::Copy(source) = redrive.action {
        if copy_source(model, source).is_none() {
            return Vec::new();
        }
    }
    redrive
        .targets
        .iter()
        .filter_map(|target| target_storage(model, *target))
        .collect()
}

/// C name of the function of redrive `index`.
pub(in crate::sim::emit_c) fn redrive_function(index: usize) -> String {
    format!("llg_redrive_{index}")
}

/// The functions of the redrives that evaluate their driver: name, source
/// and the redrive. They take the condition evaluators' `context` argument,
/// so the instances of one module share one body through the same records
/// (`model::sharing`); a function no body replaced ignores it.
pub(in crate::sim::emit_c) fn redrive_sources<'m>(
    ctx: &RCtx<'m>,
) -> Result<Vec<(String, String, &'m IrRedrive)>, String> {
    let model = ctx.model;
    let mut sources = Vec::new();
    for (index, redrive) in model.redrives.iter().enumerate() {
        let IrRedriveAction::Evaluate(body) = &redrive.action else {
            continue;
        };
        if rows(model, redrive).is_empty() {
            continue;
        }
        // A plain function like the static initializers: a driver's
        // statements never wait. Temporaries live in a block that closes
        // before `_llg_return`.
        let mut frame = Frame::new(ctx);
        frame.line("(void)context;");
        frame.line("{");
        frame.statements(body)?;
        frame.line("}");
        frame.line("if (0) goto _llg_return;");
        frame.line("_llg_return: ;");
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        // Cold: a release is rare, and the helpers these functions call
        // must not change how the processes inline them.
        let name = redrive_function(index);
        sources.push((
            name.clone(),
            format!(
                "static LLG_COLD void {name}(void* context) {{\n{}{}\n}}\n\n",
                frame.prologue(),
                frame.body()
            ),
            redrive,
        ));
    }
    Ok(sources)
}

/// A continuous driver process a release through VPI wakes: the storage it
/// assigns, its root descriptor and, for a body shared between instances,
/// its instance record and where the root frame keeps it.
pub(in crate::sim::emit_c) struct ProcessRow {
    pub(in crate::sim::emit_c) target: IrRedriveTarget,
    pub(in crate::sim::emit_c) desc: String,
    pub(in crate::sim::emit_c) record: Option<(String, String)>,
}

/// Shape of a class of rows: `(action, cells, stride, instance offset,
/// evaluate, process)`.
type ClassShape = (&'static str, u64, &'static str, String, String, String);

/// Emit the `llg_redrives` table: flat per-row arrays (a structure per row
/// costs several times their compile time), the classes of rows of one
/// shape and the scratch arrays. `shared` maps a redrive function a shared
/// body replaced to that body and the instance record to pass it;
/// `processes` are the drivers a VPI release wakes. A variable a copy or a
/// constant row re-establishes whole needs no process row. A model without
/// rows emits nothing and returns false; else the model start registers
/// `llg_redrives`.
pub(in crate::sim::emit_c) fn redrive_tables(
    model: &IrModel,
    shared: &HashMap<String, (String, String)>,
    processes: &[ProcessRow],
    out: &mut String,
) -> bool {
    let mut classes: Vec<(ClassShape, Vec<(String, String)>)> = Vec::new();
    let mut class_of = HashMap::new();
    let mut push = |shape: ClassShape, storage: String, operand: String| {
        let class = *class_of.entry(shape.clone()).or_insert_with(|| {
            classes.push((shape, Vec::new()));
            classes.len() - 1
        });
        classes[class].1.push((storage, operand));
    };
    let null = || "NULL".to_owned();
    let mut limbs = String::new();
    let mut limb_tables: HashMap<Vec<u64>, String> = HashMap::new();
    let mut whole = std::collections::BTreeSet::new();
    for (index, redrive) in model.redrives.iter().enumerate() {
        let rows = rows(model, redrive);
        if rows.is_empty() {
            continue;
        }
        let (action, evaluate, operand) = match &redrive.action {
            IrRedriveAction::Copy(source) => {
                let Some(packed) = copy_source(model, *source) else {
                    continue;
                };
                whole.extend(redrive.targets.iter().copied());
                ("LLG_REDRIVE_COPY", null(), packed)
            }
            IrRedriveAction::Constant(constant) => {
                // Value, X and Z limbs of the variable's width, in that order.
                let count = constant.width.div_ceil(64) as usize;
                let column = |limbs: &[u64]| {
                    (0..count)
                        .map(|limb| limbs.get(limb).copied().unwrap_or(0))
                        .collect::<Vec<_>>()
                };
                let mut words = column(&constant.bits);
                words.extend(column(&constant.x));
                words.extend(column(&constant.z));
                let next = limb_tables.len();
                let name = limb_tables.entry(words.clone()).or_insert_with(|| {
                    let name = format!("llg_redrive_limbs_{next}");
                    limbs.push_str(&format!(
                        "static const uint64_t {name}[{}] = {{ {} }};\n",
                        words.len(),
                        words
                            .iter()
                            .map(|word| format!("{word}ULL"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    name
                });
                whole.extend(redrive.targets.iter().copied());
                ("LLG_REDRIVE_LIMBS", null(), name.clone())
            }
            IrRedriveAction::Committed => ("LLG_REDRIVE_COMMITTED", null(), null()),
            IrRedriveAction::Evaluate(_) => {
                let name = redrive_function(index);
                let (evaluate, operand) = shared.get(&name).cloned().unwrap_or((name, null()));
                ("LLG_REDRIVE_EVALUATE", evaluate, operand)
            }
        };
        for (storage, cells, stride) in rows {
            push(
                (
                    action,
                    cells,
                    stride,
                    "0".to_owned(),
                    evaluate.clone(),
                    null(),
                ),
                storage,
                operand.clone(),
            );
        }
    }
    for row in processes {
        if whole.contains(&row.target) {
            continue;
        }
        let Some((storage, cells, stride)) = target_storage(model, row.target) else {
            continue;
        };
        let (operand, offset) = row
            .record
            .clone()
            .map_or((null(), "0".to_owned()), |(record, offset)| {
                (record, format!("(uint32_t){offset}"))
            });
        push(
            (
                "LLG_REDRIVE_PROCESS",
                cells,
                stride,
                offset,
                null(),
                format!("&{}", row.desc),
            ),
            storage,
            operand,
        );
    }
    let count: usize = classes.iter().map(|(_, rows)| rows.len()).sum();
    if count == 0 {
        return false;
    }
    let flat = |name: &str, column: fn(&(String, String)) -> &str| {
        let mut text = format!("static const void* const {name}[{count}] = {{\n");
        let items = classes
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(column))
            .collect::<Vec<_>>();
        for line in items.chunks(8) {
            text.push_str(&format!("    {},\n", line.join(", ")));
        }
        text.push_str("};\n");
        text
    };
    out.push_str("/* continuous drivers a release re-establishes */\n");
    out.push_str(&limbs);
    out.push_str(&flat("llg_redrive_storage", |row| &row.0));
    out.push_str(&flat("llg_redrive_operand", |row| &row.1));
    out.push_str(&format!(
        "static const llg_redrive_class_t llg_redrive_classes[{}] = {{\n",
        classes.len()
    ));
    let mut first = 0;
    for ((action, cells, stride, offset, evaluate, process), rows) in &classes {
        out.push_str(&format!(
            "    {{ {first}u, {cells}u, (uint32_t){stride}, {action}, {offset}, {evaluate}, \
             {process} }},\n"
        ));
        first += rows.len();
    }
    out.push_str(&format!(
        "}};\n\
         static uint32_t llg_redrive_order[{count}];\n\
         static uint8_t llg_redrive_busy[{count}];\n\
         static const llg_redrive_table_t llg_redrives = {{\n    \
         llg_redrive_storage, llg_redrive_operand, llg_redrive_classes,\n    \
         llg_redrive_order, llg_redrive_busy, {}u, {count}u,\n}};\n\n",
        classes.len()
    ));
    true
}
