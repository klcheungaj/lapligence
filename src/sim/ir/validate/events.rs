//! Events.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_capture(
        &self,
        storage: StorageRef,
        initial: &IrExpr,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let event_initial = matches!(
            initial.kind(),
            IrExprKind::ObjectQuery(query) if matches!(query.as_ref(), IrObjectQuery::EventCapture(_))
        );
        if event_initial != (storage.kind() == StorageKind::Event) {
            return self.fail(
                path,
                "event capture requires event storage and a typed event initializer",
            );
        }
        if storage.kind() == StorageKind::Event
            && (storage.ownership() == StorageOwnership::Shared
                || storage.ownership() == StorageOwnership::Borrowed
                    && storage.lifetime() != StorageLifetime::Automatic)
        {
            return self.fail(
                path,
                "event captures require an owned snapshot or borrowed automatic handle",
            );
        }
        if storage.kind() == StorageKind::Event && storage.ownership() == StorageOwnership::Borrowed
        {
            let valid = matches!(initial.kind(), IrExprKind::ObjectQuery(query)
                if matches!(query.as_ref(), IrObjectQuery::EventCapture(IrEventRef::Formal(_) | IrEventRef::Captured(_))));
            if !valid {
                return self.fail(path, "borrowed event capture requires an automatic handle");
            }
        }
        self.validate_expr(initial, formals, path)
    }

    pub(super) fn validate_stmts(
        &self,
        stmts: &[IrStmt],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        self.fixed_activations.borrow_mut().push(HashSet::new());
        self.native_activations.borrow_mut().push(HashSet::new());
        let result = stmts.iter().enumerate().try_for_each(|(idx, stmt)| {
            self.validate_stmt(stmt, formals, &format!("{path}[{idx}]"))
        });
        self.native_activations.borrow_mut().pop();
        self.fixed_activations.borrow_mut().pop();
        result
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
