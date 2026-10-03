//! Model-level electrical contribution tables and sharing operands.
use super::*;

pub(super) fn collect(
    model: &IrModel,
    processes: &[Option<CoroutineArtifact>],
) -> pca_batches::Tables {
    let mut tables = pca_batches::Tables {
        declarations: String::new(),
        operands: Vec::new(),
    };
    if processes
        .iter()
        .flatten()
        .any(|artifact| !artifact.net_batches.is_empty())
    {
        tables.declarations.push_str("typedef struct { llg_net_t* net; int slot; int64_t left, right; uint32_t width; uint8_t is_signed; } llg_net_drive_row_t;\n");
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
                    row.width,
                    u8::from(row.signed)
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
