//! Model-level electrical contribution tables and sharing operands.
use super::*;

/// File-scope table declarations and their sharing operands
/// `(name, row type, shape)`, plus each process's hoisted per-site state
/// `(name, element type, element count)`.
pub(super) struct Tables {
    pub declarations: String,
    pub operands: Vec<(String, String, String)>,
    pub site_statics: Vec<(String, &'static str, Option<u64>)>,
}

pub(super) fn collect(model: &IrModel, processes: &[Option<CoroutineArtifact>]) -> Tables {
    let mut tables = Tables {
        declarations: String::new(),
        operands: Vec::new(),
        site_statics: Vec::new(),
    };
    for site in processes
        .iter()
        .flatten()
        .flat_map(|artifact| &artifact.net_batches.statics)
    {
        tables.declarations.push_str(&match site.len {
            Some(len) => format!("static {} {}[{len}] = {{0}};\n", site.ty, site.name),
            None if site.ty.ends_with('*') => format!("static {} {} = NULL;\n", site.ty, site.name),
            None => format!("static {} {} = 0;\n", site.ty, site.name),
        });
        tables
            .site_statics
            .push((site.name.clone(), site.ty, site.len));
    }
    if processes
        .iter()
        .flatten()
        .any(|artifact| !artifact.net_batches.batches.is_empty())
    {
        tables.declarations.push_str("typedef struct { llg_net_t* net; int slot; int64_t left, right; uint32_t cast_width; uint8_t cast_signed; } llg_net_drive_row_t;\n");
    }
    for artifact in processes.iter().flatten() {
        for batch in &artifact.net_batches.batches {
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
