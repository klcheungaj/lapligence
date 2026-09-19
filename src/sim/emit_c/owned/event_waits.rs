//! Atomic multi-source waits. Runtime descriptors adopt one reference per field.
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
        // A null handle (also an invalid/X array index) is an inert source,
        // not an expression descriptor with a missing evaluator. The runtime
        // snapshots the object before suspension; this handle only borrows it.
        let empty = self.name("null_event");
        self.line(format!("llg_event_t {empty} = {{ NULL }};"));
        Ok(self.scalar(
            "llg_event_t*",
            format!("({address}) ? ({address}) : &{empty}"),
        ))
    }

    pub(super) fn wait_events(&mut self, specs: &[(IrWaitSrc, IrEdge)]) -> Result<(), String> {
        let array = self.event_specs(specs)?;
        self.line(format!("llg_wait_expressions({array}, {});", specs.len()));
        Ok(())
    }

    // All user expressions must finish before this returns. The caller must
    // immediately pass these owned context references to a runtime consumer.
    pub(super) fn event_specs(&mut self, specs: &[(IrWaitSrc, IrEdge)]) -> Result<String, String> {
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
        for context in &contexts {
            prepared.push(
                self.prepare_captures(
                    context
                        .captures()
                        .iter()
                        .map(|capture| (capture.storage(), capture.initial())),
                )?,
            );
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
                    if !reads.is_empty() {
                        let dependencies = reads
                            .iter()
                            .map(|item| self.dependency(item))
                            .collect::<Result<Vec<_>, _>>()?;
                        let name = self.name("event_dependencies");
                        self.line(format!(
                            "llg_wait_dependency_t {name}[] = {{ {} }};",
                            dependencies.join(", ")
                        ));
                        fields.push(format!(
                            ".dependencies = {name}, .n_dependencies = {}",
                            reads.len()
                        ));
                    }
                    if let Some(context) = context_for(model, eval) {
                        let name = &names[&context.frame()];
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
            self.line(format!("llg_frame_retain({name});"));
        }
        let array = self.name("events");
        self.line(format!(
            "llg_expr_event_spec_t {array}[] = {{ {} }};",
            entries.join(", ")
        ));
        for context in &contexts {
            self.line(format!("llg_frame_release({});", names[&context.frame()]));
        }
        Ok(array)
    }
}
