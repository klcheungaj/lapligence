//! File-scope PCA tables and one non-inlined helper per exact typed shape.
use super::super::statements::pca_batches::Shape;
use super::*;

pub(super) struct Tables {
    pub declarations: String,
    pub operands: Vec<(String, String, String)>,
}

pub(super) fn collect(
    model: &IrModel,
    constants: &super::super::constants::PackedConstants,
    processes: &mut [Option<CoroutineArtifact>],
) -> Result<Tables, String> {
    let mut shapes = Vec::<Shape>::new();
    let mut tables = Tables {
        declarations: String::new(),
        operands: Vec::new(),
    };
    for artifact in processes.iter_mut().flatten() {
        let mut helpers = HashMap::new();
        for batch in &artifact.pca_batches {
            let index = if let Some(index) = shapes.iter().position(|shape| shape == &batch.shape) {
                index
            } else {
                let index = shapes.len();
                shapes.push(batch.shape.clone());
                let row_type = format!("llg_pca_row_{index}_t");
                let (body, _) =
                    super::super::owned::pca_batches::helper_body(&batch.shape, Some(constants))?;
                tables.declarations.push_str(&format!(
                    "typedef struct {{ const {}* source; {}* target; sv4_t* enable; uint64_t binding; }} {row_type};\nstatic LLG_MODEL_SHARED void llg_pca_batch_{index}(sv4_t* _llg_t, const {row_type}* row) {{\n{body}}}\n",
                    if batch.shape.source_type.width() == 0 { "double" } else { "sv4_t" },
                    if batch.shape.target_type.width() == 0 { "double" } else { "sv4_t" },
                ));
                index
            };
            helpers.insert(batch.helper(), format!("llg_pca_batch_{index}"));
            let row_type = format!("llg_pca_row_{index}_t");
            tables
                .declarations
                .push_str(&format!("static const {row_type} {}[] = {{\n", batch.name));
            for row in &batch.rows {
                tables.declarations.push_str(&format!(
                    "    {{ {}, &{}, &{}, {}ULL }},\n",
                    row.source,
                    model.signal(row.target).c_name,
                    model.signal(row.enable).c_name,
                    row.binding
                ));
            }
            tables.declarations.push_str("};\n");
            tables.operands.push((
                batch.name.clone(),
                row_type,
                format!("{index}:{}", batch.rows.len()),
            ));
        }
        artifact.source = rewrite_identifiers(&artifact.source, |identifier| {
            helpers.get(identifier).cloned()
        });
    }
    Ok(tables)
}
