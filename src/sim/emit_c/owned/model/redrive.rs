//! Redrive rows (`IrModel::redrives`): the continuous drivers a release
//! repeats in place, as a const table the runtime searches by storage
//! address and one plain C function per driver that is not a plain copy.
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

/// The rows of one redrive: `(storage, cells, stride)` per emitted target.
fn rows(model: &IrModel, redrive: &IrRedrive) -> Vec<(String, u64, &'static str)> {
    redrive
        .targets
        .iter()
        .filter_map(|target| target_storage(model, *target))
        .collect()
}

/// Number of rows `redrive_tables` emits.
fn row_count(model: &IrModel) -> usize {
    model
        .redrives
        .iter()
        .map(|redrive| rows(model, redrive).len())
        .sum()
}

/// Emit the redrive functions and the `llg_redrives` table. A model without
/// redrive rows emits nothing.
pub(in crate::sim::emit_c) fn redrive_tables(
    model: &IrModel,
    constants: &super::super::super::constants::PackedConstants,
    backend: crate::sim::value_backend::ValueBackend,
    out: &mut String,
) -> Result<(), String> {
    if row_count(model) == 0 {
        return Ok(());
    }
    let ctx = RCtx {
        value_backend: backend,
        model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: Some(constants),
    };
    let mut table = String::new();
    for (index, redrive) in model.redrives.iter().enumerate() {
        let rows = rows(model, redrive);
        if rows.is_empty() {
            continue;
        }
        let (evaluate, source) = match &redrive.action {
            IrRedriveAction::Copy(source) => {
                let Some((packed, _, _)) = signal_storage(model, *source) else {
                    continue;
                };
                ("NULL".to_owned(), packed)
            }
            IrRedriveAction::Committed => ("NULL".to_owned(), "NULL".to_owned()),
            IrRedriveAction::Evaluate(body) => {
                // A plain function like the static initializers: a driver's
                // statements never wait. Temporaries live in a block that
                // closes before `_llg_return`.
                let mut frame = Frame::new(&ctx);
                frame.line("{");
                frame.statements(body)?;
                frame.line("}");
                frame.line("if (0) goto _llg_return;");
                frame.line("_llg_return: ;");
                frame.line("llg_value_scopes_end_since(_llg_frame_base);");
                out.push_str(&format!(
                    "static void llg_redrive_{index}(void) {{\n{}{}\n}}\n\n",
                    frame.prologue(),
                    frame.body()
                ));
                (format!("llg_redrive_{index}"), "NULL".to_owned())
            }
        };
        for (storage, cells, stride) in rows {
            table.push_str(&format!(
                "    {{ {storage}, {cells}u, (uint32_t){stride}, {evaluate}, {source} }},\n"
            ));
        }
    }
    out.push_str("/* continuous drivers a release repeats in place */\n");
    out.push_str(&format!(
        "static const llg_redrive_t llg_redrives[] = {{\n{table}}};\n\n"
    ));
    Ok(())
}

/// The start-up statement that hands the runtime the redrive table, or
/// `None` for a model without rows, whose start-up is unchanged.
pub(in crate::sim::emit_c) fn redrive_registration(model: &IrModel) -> Option<String> {
    let rows = row_count(model);
    (rows > 0).then(|| format!("llg_redrives_register(llg_redrives, {rows}u);"))
}
