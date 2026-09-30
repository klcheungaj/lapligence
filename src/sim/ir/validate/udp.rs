//! UDP definition and evaluation shape checks.
use super::*;

impl Validator<'_> {
    pub(super) fn validate_udp_table(&self, table: &IrUdpTable, path: &str) -> ValidationResult {
        if table.input_count == 0
            || table
                .input_count
                .checked_add(1)
                .and_then(|stride| stride.checked_mul(table.rows.len()))
                .is_none()
        {
            return self.fail(
                path,
                "UDP table requires a nonzero input count and representable row storage",
            );
        }
        for (index, row) in table.rows.iter().enumerate() {
            if row.inputs.len() != table.input_count {
                return self.fail(
                    format!("{path}.rows[{index}]"),
                    "UDP row input count differs from its definition",
                );
            }
        }
        Ok(())
    }

    pub(super) fn validate_udp_eval(
        &self,
        expr: &IrExpr,
        table: usize,
        inputs: &[IrExpr],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let Some(table) = self.model.udp_tables.get(table) else {
            return self.fail(path, "UDP evaluation references a missing table");
        };
        self.validate_udp_table(table, path)?;
        if expr.width != 1
            || expr.signed
            || expr.fill.is_some()
            || inputs.len() != table.input_count
        {
            return self.fail(
                path,
                "UDP evaluation requires scalar inputs and an unsigned one-bit result",
            );
        }
        for (index, input) in inputs.iter().enumerate() {
            let path = format!("{path}.inputs[{index}]");
            self.validate_expr(input, formals, &path)?;
            if input.width != 1 {
                return self.fail(path, "UDP input must be scalar");
            }
        }
        Ok(())
    }
}
