//! Events.

use super::*;

impl Validator<'_> {

    pub(super) fn validate_stmts(
        &self,
        stmts: &[IrStmt],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        for (idx, stmt) in stmts.iter().enumerate() {
            self.validate_stmt(stmt, formals, &format!("{path}[{idx}]"))?;
        }
        Ok(())
    }

    pub(super) fn validate_event_assignment_specs(
        &self,
        specs: &[(IrWaitSrc, IrEdge)],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        for (idx, (source, edge)) in specs.iter().enumerate() {
            if let IrWaitSrc::Event(event) | IrWaitSrc::FilteredEvent { event, .. } = source {
                self.validate_event_ref(event, formals, &format!("{path}[{idx}].event"))?;
            }
            let helpers: Vec<(&str, bool)> = match source {
                IrWaitSrc::Evaluated {
                    eval, condition, ..
                } => std::iter::once((eval.as_str(), false))
                    .chain(condition.as_deref().map(|condition| (condition, false)))
                    .collect(),
                IrWaitSrc::EvaluatedReal {
                    eval, condition, ..
                } => std::iter::once((eval.as_str(), true))
                    .chain(condition.as_deref().map(|condition| (condition, false)))
                    .collect(),
                IrWaitSrc::FilteredEvent { condition, .. } => {
                    vec![(condition.as_str(), false)]
                }
                _ => Vec::new(),
            };
            if matches!(source, IrWaitSrc::Real(_)) && *edge != IrEdge::Any {
                return self.fail(
                    format!("{path}[{idx}]"),
                    "real event sources only support any-change controls",
                );
            }
            for (helper, real) in helpers {
                let valid = self
                    .model
                    .processes
                    .iter()
                    .flat_map(|process| &process.pre_fns)
                    .chain(
                        self.model
                            .funcs
                            .iter()
                            .flat_map(|function| &function.pre_fns),
                    )
                    .any(|pre| {
                        if real {
                            matches!(
                                pre,
                                IrPreFn::RealEval { c_name, value, .. }
                                    if c_name == helper && value.is_real()
                            )
                        } else {
                            matches!(
                                pre,
                                IrPreFn::MonEval { c_name, args, .. }
                                    if c_name == helper && args.len() == 1 && !args[0].is_real()
                            )
                        }
                    });
                if !valid {
                    return self.fail(
                        format!("{path}[{idx}]"),
                        "event evaluator helper has an invalid value type",
                    );
                }
            }
            if let IrWaitSrc::Real(name) = source {
                if !self.valid_dependency(&IrDependency::real(name)) {
                    return self.fail(
                        format!("{path}[{idx}]"),
                        "real event source must name active real storage",
                    );
                }
            }
            if let IrWaitSrc::Evaluated { reads, .. } | IrWaitSrc::EvaluatedReal { reads, .. } =
                source
            {
                for read in reads {
                    if !self.valid_dependency(read) {
                        return self.fail(
                            format!("{path}[{idx}]"),
                            "event dependency must name active storage",
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
