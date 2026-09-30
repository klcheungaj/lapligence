//! Shared immutable UDP rows and synchronous borrowed evaluation.
use super::*;

pub(in crate::sim::emit_c) fn tables(model: &IrModel, out: &mut String) {
    for (index, table) in model.udp_tables.iter().enumerate() {
        out.push_str(&format!(
            "static const uint8_t llg_udp_table_{index}[] = {{\n"
        ));
        if table.rows.is_empty() {
            out.push_str("  0,\n");
        }
        for row in &table.rows {
            let fields = row
                .inputs
                .iter()
                .map(|mask| (*mask as u8).to_string())
                .chain(std::iter::once((row.output as u8).to_string()))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  {fields},\n"));
        }
        out.push_str("};\n");
    }
}

impl Frame<'_, '_> {
    pub(super) fn udp_eval(&mut self, table: usize, inputs: &[IrExpr]) -> Result<Value, String> {
        let definition = &self.ctx.model.udp_tables[table];
        let mut values = Vec::with_capacity(inputs.len());
        for input in inputs {
            values.push(self.expression(input)?);
        }
        let addresses = values
            .iter()
            .map(|value| format!("&{}", value.code))
            .collect::<Vec<_>>()
            .join(", ");
        let result = self.value(
            format!(
            "sv4_udp_eval(llg_udp_table_{table}, {}, {}, (const sv4_t* const[]){{{addresses}}})",
            definition.rows.len(), definition.input_count,
        ),
            1,
            false,
        );
        for value in values {
            self.discard(value);
        }
        Ok(result)
    }
}
