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
        if let Some(sequential) = &table.sequential {
            if !table.rows.is_empty() {
                return self.fail(path, "a sequential UDP keeps no combinational rows");
            }
            for (index, row) in sequential.rows.iter().enumerate() {
                if row.inputs.len() != table.input_count
                    || row.edge.is_some_and(|edge| edge.input >= table.input_count)
                {
                    return self.fail(
                        format!("{path}.sequential.rows[{index}]"),
                        "sequential UDP row inputs or transition differ from its definition",
                    );
                }
            }
        }
        Ok(())
    }

    /// The sequential definition owning instance state `state`.
    fn sequential_udp_state(
        &self,
        state: usize,
        path: &str,
    ) -> Result<&IrUdpTable, IrValidationError> {
        let table = self
            .model
            .udp_states
            .get(state)
            .and_then(|table| self.model.udp_tables.get(*table))
            .filter(|table| table.sequential.is_some());
        match table {
            Some(table) => Ok(table),
            None => self.fail(path, "UDP state references no sequential UDP definition"),
        }
    }

    pub(super) fn validate_udp_step(
        &self,
        state: usize,
        inputs: &[IrExpr],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let table = self.sequential_udp_state(state, path)?;
        if inputs.len() != table.input_count {
            return self.fail(path, "UDP step input count differs from its definition");
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

    pub(super) fn validate_udp_state(
        &self,
        expr: &IrExpr,
        state: usize,
        path: &str,
    ) -> ValidationResult {
        self.sequential_udp_state(state, path)?;
        if expr.width != 1 || expr.signed || expr.fill.is_some() {
            return self.fail(path, "UDP state read requires an unsigned one-bit result");
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
        if table.sequential.is_some() {
            return self.fail(
                path,
                "a sequential UDP is evaluated through its instance state",
            );
        }
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

impl Validator<'_> {
    /// Module path sets: packed sources, existing destination groups that
    /// belong to one set at most, terminal ranges inside their storage and
    /// equal widths for parallel paths.
    pub(super) fn validate_module_paths(&self) -> ValidationResult {
        let mut destinations = HashSet::new();
        for (index, set) in self.model.module_paths.iter().enumerate() {
            let path = format!("module_paths[{index}]");
            if set.destinations.is_empty() {
                return self.fail(path, "module path set has no destination");
            }
            let mut source_widths = Vec::with_capacity(set.sources.len());
            for (source_index, &signal) in set.sources.iter().enumerate() {
                match self.model.signals.get(signal).map(|signal| &signal.ty) {
                    Some(IrType::Packed { width, .. }) => source_widths.push(*width),
                    _ => {
                        return self.fail(
                            format!("{path}.sources[{source_index}]"),
                            "module path source must be a packed signal",
                        )
                    }
                }
            }
            for (destination_index, destination) in set.destinations.iter().enumerate() {
                let path = format!("{path}.destinations[{destination_index}]");
                let Some(group) = self.model.net_groups.get(destination.group) else {
                    return self.fail(path, "module path destination names no net group");
                };
                if !destinations.insert(destination.group) {
                    return self.fail(path, "net group is the destination of two path sets");
                }
                if destination.paths.is_empty() {
                    return self.fail(path, "module path destination has no path");
                }
                for (path_index, module_path) in destination.paths.iter().enumerate() {
                    let path = format!("{path}.paths[{path_index}]");
                    let fits = |lsb: u32, width: u32, limit: u32| {
                        width != 0 && lsb.checked_add(width).is_some_and(|end| end <= limit)
                    };
                    let Some(&source_width) = source_widths.get(module_path.source) else {
                        return self.fail(path, "module path names no source of its set");
                    };
                    if !fits(
                        module_path.source_lsb,
                        module_path.source_width,
                        source_width,
                    ) || !fits(module_path.dest_lsb, module_path.dest_width, group.width)
                    {
                        return self.fail(path, "module path terminal lies outside its storage");
                    }
                    if !module_path.full && module_path.source_width != module_path.dest_width {
                        return self.fail(path, "parallel module path terminals differ in width");
                    }
                    match module_path.condition {
                        IrPathCondition::If(condition) if condition >= set.conditions.len() => {
                            return self.fail(path, "module path names no condition of its set");
                        }
                        IrPathCondition::IfNone if module_path.group == 0 => {
                            return self.fail(path, "an ifnone module path needs a path group");
                        }
                        _ => {}
                    }
                    if let Some(limits) = &module_path.limits {
                        if limits.error.iter().zip(&limits.reject).any(|(e, r)| e < r) {
                            return self
                                .fail(path, "module path error limit below its reject limit");
                        }
                    }
                }
                if destination
                    .styles
                    .as_ref()
                    .is_some_and(|styles| styles.len() != group.width as usize)
                {
                    return self.fail(path, "module path styles must cover the destination bits");
                }
            }
            for (condition_index, condition) in set.conditions.iter().enumerate() {
                self.validate_expr(
                    condition,
                    &[],
                    &format!("{path}.conditions[{condition_index}]"),
                )?;
            }
        }
        let options = self.model.path_pulse;
        if options.reject_percent > 100
            || options.error_percent > 100
            || options.error_percent < options.reject_percent
        {
            return self.fail(
                "path_pulse".to_owned(),
                "pulse limit percentages must be at most 100, error no lower than reject",
            );
        }
        Ok(())
    }
}
