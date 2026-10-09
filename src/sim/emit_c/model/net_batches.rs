//! Model-level electrical contribution tables and sharing operands.
use super::*;

/// File-scope table declarations and their sharing operands
/// `(name, row type, shape)`.
pub(super) struct Tables {
    pub declarations: String,
    pub operands: Vec<(String, String, String)>,
}

pub(super) fn collect(model: &IrModel, processes: &[Option<CoroutineArtifact>]) -> Tables {
    let mut tables = Tables {
        declarations: String::new(),
        operands: Vec::new(),
    };
    if processes
        .iter()
        .flatten()
        .any(|artifact| !artifact.net_batches.is_empty())
    {
        tables.declarations.push_str("typedef struct { llg_net_t* net; int slot; int64_t left, right; uint32_t cast_width; uint8_t cast_signed; } llg_net_drive_row_t;\n");
    }
    for artifact in processes.iter().flatten() {
        for batch in &artifact.net_batches {
            tables.declarations.push_str(&format!(
                "static const llg_net_drive_row_t {}[] = {{\n",
                batch.name
            ));
            for row in &batch.rows {
                tables.declarations.push_str(&format!(
                    "    {{ &{}, {}, {}LL, {}LL, {}, {} }},\n",
                    model.net_group(row.group).c_name,
                    row.slot,
                    row.left,
                    row.right,
                    row.cast_width,
                    u8::from(row.cast_signed)
                ));
            }
            tables.declarations.push_str("};\n");
            tables.operands.push((
                batch.name.clone(),
                "llg_net_drive_row_t".to_owned(),
                format!("net:{}", batch.rows.len()),
            ));
        }
    }
    tables
}
