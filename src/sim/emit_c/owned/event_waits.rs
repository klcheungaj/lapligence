//! Atomic multi-source waits. Runtime descriptors adopt one reference per field.
use super::captures::CapturedValue;
use super::*;

fn context_for(model: &IrModel, helper: &str) -> Option<IrEventContext> {
    let helpers = model
        .processes
        .iter()
        .flat_map(|process| process.pre_fns.iter())
        .chain(
            model
                .funcs
                .iter()
                .flat_map(|function| function.pre_fns.iter()),
        );
    for pre in helpers {
        match pre {
            IrPreFn::MonEval {
                c_name, context, ..
            }
            | IrPreFn::RealEval {
                c_name, context, ..
            } if c_name == helper => return context.clone(),
            _ => {}
        }
    }
    None
}

fn helper_names(source: &IrWaitSrc) -> Vec<&str> {
    match source {
        IrWaitSrc::Evaluated {
            eval, condition, ..
        }
        | IrWaitSrc::EvaluatedReal {
            eval, condition, ..
        } => std::iter::once(eval.as_str())
            .chain(condition.as_deref())
            .collect(),
        IrWaitSrc::FilteredEvent { condition, .. } => vec![condition.as_str()],
        _ => Vec::new(),
    }
}

impl Frame<'_, '_> {
    fn wait_event_address(&mut self, event: &IrEventRef) -> Result<String, String> {
        let address = self.event_address(event)?;
        if matches!(event, IrEventRef::Static(_)) {
            return Ok(address);
        }
        // A null handle (also an invalid/X array index) is an inert source,
        // not an expression descriptor with a missing evaluator. The runtime
        // snapshots the object before suspension; this handle only borrows it.
        let empty = self.declare("llg_event_t", "null_event", "{ NULL }".to_owned());
        Ok(self.scalar(
            "llg_event_t*",
            format!("({address}) ? ({address}) : &{empty}"),
        ))
    }

    pub(super) fn wait_events(&mut self, specs: &[(IrWaitSrc, IrEdge)]) -> Result<(), String> {
        if specs.is_empty() {
            return self.await_arm(SuspensionOperation::EventWait, "llg_arm_time(self, 0ULL)");
        }
        if specs.len() == 1 {
            match &specs[0] {
                (IrWaitSrc::Sig(name), edge) => {
                    let signal = self.resolve_lookup(name)?.address;
                    let arm = match edge {
                        IrEdge::Any => format!("llg_arm_any(self, (sv4_t*[]){{ {signal} }}, 1)"),
                        IrEdge::Posedge => format!("llg_arm_edge(self, {signal}, 1)"),
                        IrEdge::Negedge => format!("llg_arm_edge(self, {signal}, 0)"),
                    };
                    return self.await_arm(SuspensionOperation::EventWait, arm);
                }
                (IrWaitSrc::Event(event), _) => {
                    let event = self.event_address(event)?;
                    return self.await_arm(
                        SuspensionOperation::EventWait,
                        format!("llg_arm_event(self, {event})"),
                    );
                }
                _ => {}
            }
        }
        if specs
            .iter()
            .all(|(source, _)| matches!(source, IrWaitSrc::Sig(_)))
        {
            let entries = specs
                .iter()
                .map(|(source, edge)| {
                    let IrWaitSrc::Sig(name) = source else {
                        unreachable!("all sources were checked as signals")
                    };
                    let signal = self.resolve_lookup(name)?.address;
                    let edge = match edge {
                        IrEdge::Any => "LLG_EV_ANY",
                        IrEdge::Posedge => "LLG_EV_POSEDGE",
                        IrEdge::Negedge => "LLG_EV_NEGEDGE",
                    };
                    Ok(format!("{{ .sig = {signal}, .kind = {edge} }}"))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let array = self.arm_array(
                "llg_event_spec_t",
                "event_specs",
                entries.len(),
                &entries.join(", "),
            );
            return self.await_arm(
                SuspensionOperation::EventWait,
                format!("llg_arm_any_events(self, {array}, {})", entries.len()),
            );
        }
        if specs
            .iter()
            .all(|(source, _)| matches!(source, IrWaitSrc::Event(_)))
        {
            let events = specs
                .iter()
                .map(|(source, _)| {
                    let IrWaitSrc::Event(event) = source else {
                        unreachable!("all sources were checked as events")
                    };
                    self.event_address(event)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let array = self.arm_array(
                "const llg_event_t*",
                "events",
                events.len(),
                &events.join(", "),
            );
            return self.await_arm(
                SuspensionOperation::EventWait,
                format!("llg_arm_events(self, {array}, {})", events.len()),
            );
        }
        if specs
            .iter()
            .all(|(source, _)| matches!(source, IrWaitSrc::Sig(_) | IrWaitSrc::Event(_)))
        {
            let mut entries = Vec::with_capacity(specs.len());
            for (source, edge) in specs {
                let edge = match edge {
                    IrEdge::Any => "LLG_EV_ANY",
                    IrEdge::Posedge => "LLG_EV_POSEDGE",
                    IrEdge::Negedge => "LLG_EV_NEGEDGE",
                };
                entries.push(match source {
                    IrWaitSrc::Sig(name) => format!(
                        "{{ .sig = {}, .kind = {edge}, .ev = NULL }}",
                        self.resolve_lookup(name)?.address
                    ),
                    IrWaitSrc::Event(event) => format!(
                        "{{ .sig = NULL, .kind = {edge}, .ev = {} }}",
                        self.event_address(event)?
                    ),
                    _ => unreachable!("simple mixed sources were checked"),
                });
            }
            let array = self.arm_array(
                "llg_wait_src_t",
                "wait_sources",
                entries.len(),
                &entries.join(", "),
            );
            return self.await_arm(
                SuspensionOperation::EventWait,
                format!("llg_arm_mixed(self, {array}, {})", entries.len()),
            );
        }
        let array = self.event_specs(specs, true)?;
        self.await_arm(
            SuspensionOperation::EventWait,
            format!("llg_arm_expressions(self, {array}, {})", specs.len()),
        )
    }

    // All user expressions must finish before this returns. The caller must
    // immediately pass these owned context references to a runtime consumer.
    // `arm` marks a consumer that copies the descriptors and their dependency
    // lists during the call (`llg_arm_expressions`), so both may be compound
    // literals in the call expression; otherwise they stay declared storage.
    pub(super) fn event_specs(
        &mut self,
        specs: &[(IrWaitSrc, IrEdge)],
        arm: bool,
    ) -> Result<String, String> {
        if specs.is_empty() {
            return Ok("NULL".to_owned());
        }
        let model = self.ctx.model;
        // Preserve source order and evaluate a shared context's initializers
        // once. All capture owners remain registered until frame publication.
        let mut contexts: Vec<IrEventContext> = Vec::new();
        let mut names = HashMap::new();
        for (source, _) in specs {
            for helper in helper_names(source) {
                if let Some(context) = context_for(model, helper) {
                    if let Some(previous) =
                        contexts.iter().find(|item| item.frame() == context.frame())
                    {
                        if previous != &context {
                            return Err("inconsistent shared event context".to_owned());
                        }
                    } else {
                        names.insert(context.frame(), self.name("event_frame"));
                        contexts.push(context);
                    }
                }
            }
        }
        let mut prepared = Vec::new();
        // An evaluator copy of a local that lives in a shared frame cell (as
        // in a call-site expansion whose `ref` actual is such a local) would
        // miss other processes' stores: alias the cell and subscribe to it.
        let mut shared_reads: HashMap<_, Vec<String>> = HashMap::new();
        for context in &contexts {
            let mut values = self.prepare_captures(
                context
                    .captures()
                    .iter()
                    .map(|capture| (capture.storage(), capture.initial())),
            )?;
            for ((storage, value), capture) in values.iter_mut().zip(context.captures()) {
                let IrExprKind::LocalRead(name) = capture.initial().kind() else {
                    continue;
                };
                if storage.ownership() != StorageOwnership::Owned
                    || !matches!(storage.kind(), StorageKind::Packed | StorageKind::Real)
                {
                    continue;
                }
                let Some((frame, slot)) = self.shared_cells.get(name).cloned() else {
                    continue;
                };
                if let CapturedValue::Numeric(copy) =
                    std::mem::replace(value, CapturedValue::Shared(frame, slot))
                {
                    self.discard(copy);
                }
                let dependency = self.dependency(&IrDependency::SharedCell {
                    local: name.clone(),
                    real: storage.kind() == StorageKind::Real,
                })?;
                shared_reads
                    .entry(context.frame())
                    .or_default()
                    .push(dependency);
            }
            prepared.push(values);
        }
        let mut retained = Vec::new();
        let mut entries = Vec::new();
        for (source, edge) in specs {
            let edge = match edge {
                IrEdge::Any => "LLG_EV_ANY",
                IrEdge::Posedge => "LLG_EV_POSEDGE",
                IrEdge::Negedge => "LLG_EV_NEGEDGE",
            };
            let mut fields = vec![format!(".kind = {edge}")];
            match source {
                IrWaitSrc::Sig(name) | IrWaitSrc::Real(name) => {
                    let binding = self.resolve_lookup(name)?;
                    if binding.width == 0 {
                        fields.push(format!(".real_sig = {}, .real = 1", binding.address));
                    } else {
                        fields.push(format!(".sig = {}", binding.address));
                    }
                }
                IrWaitSrc::Event(event) | IrWaitSrc::FilteredEvent { event, .. } => {
                    fields.push(format!(".event = {}", self.wait_event_address(event)?));
                }
                IrWaitSrc::Evaluated { eval, reads, .. }
                | IrWaitSrc::EvaluatedReal { eval, reads, .. } => {
                    let field = if matches!(source, IrWaitSrc::EvaluatedReal { .. }) {
                        fields.push(".real = 1".to_owned());
                        "real_eval"
                    } else {
                        "eval"
                    };
                    fields.push(format!(".{field} = {eval}"));
                    let mut dependencies = reads
                        .iter()
                        .map(|item| self.dependency(item))
                        .collect::<Result<Vec<_>, _>>()?;
                    if let Some(cells) = context_for(model, eval)
                        .and_then(|context| shared_reads.get(&context.frame()))
                    {
                        // A read can already name the cell (an expanded `ref`
                        // formal bound to a shared automatic).
                        for cell in cells {
                            if !dependencies.contains(cell) {
                                dependencies.push(cell.clone());
                            }
                        }
                    }
                    if !dependencies.is_empty() {
                        let name = if arm {
                            self.arm_array(
                                "llg_wait_dependency_t",
                                "event_dependencies",
                                dependencies.len(),
                                &dependencies.join(", "),
                            )
                        } else {
                            self.declare_array_init(
                                "llg_wait_dependency_t",
                                "event_dependencies",
                                dependencies.len(),
                                &dependencies.join(", "),
                            )
                        };
                        fields.push(format!(
                            ".dependencies = {name}, .n_dependencies = {}",
                            dependencies.len()
                        ));
                    }
                    if let Some(context) = context_for(model, eval) {
                        let name = &names[&context.frame()];
                        // The context field is declared after all source
                        // expressions have been prepared. Keep its bare name
                        // here so the declaration-time frame-path rewrite can
                        // resolve the block that owns it.
                        fields.push(format!(".eval_context = {name}"));
                        retained.push(name.clone());
                    }
                }
            }
            let condition = match source {
                IrWaitSrc::Evaluated { condition, .. }
                | IrWaitSrc::EvaluatedReal { condition, .. } => condition.as_deref(),
                IrWaitSrc::FilteredEvent { condition, .. } => Some(condition.as_str()),
                _ => None,
            };
            if let Some(condition) = condition {
                fields.push(format!(".condition = {condition}"));
                if let Some(context) = context_for(model, condition) {
                    let name = &names[&context.frame()];
                    fields.push(format!(".condition_context = {name}"));
                    retained.push(name.clone());
                }
            }
            entries.push(format!("{{ {} }}", fields.join(", ")));
        }
        // No generated expression/callback may run in this publication window.
        // Each context field owns a reference, even if two fields share a frame.
        for (context, values) in contexts.iter().zip(prepared) {
            self.publish_captures(&names[&context.frame()], values);
        }
        for name in retained {
            self.line(format!("llg_frame_retain({});", self.access(&name)));
        }
        let entries = entries.join(", ");
        let array = if arm {
            self.arm_array("llg_expr_event_spec_t", "events", specs.len(), &entries)
        } else {
            self.declare_array_init("llg_expr_event_spec_t", "events", specs.len(), &entries)
        };
        for context in &contexts {
            self.line(format!(
                "llg_frame_release({});",
                self.access(&names[&context.frame()])
            ));
        }
        Ok(array)
    }
}
