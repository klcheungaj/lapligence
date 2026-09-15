//! Events.

use super::*;

pub(super) fn activation_guard(ctx: &RCtx<'_>) -> String {
    ctx.activation_label
        .as_deref()
        .map(|label| format!("    if (llg_activation_cancelled()) goto {label};\n"))
        .unwrap_or_default()
}

/// Suspend on the dependency set. An empty set never wakes; it is not a
/// zero-delay loop, which would starve all future simulation time slots.
pub(in super::super) fn wait_any_text(ctx: &RCtx<'_>, sens: &[IrDependency]) -> String {
    if sens.is_empty() {
        return "    llg_wait_any(NULL, 0);\n".to_string();
    }
    if sens.iter().any(|dependency| match dependency {
        IrDependency::Real(_) | IrDependency::PackedRange { .. } => true,
        IrDependency::ArrayElement { array, .. } => ctx.model.array(*array).real,
        _ => false,
    }) {
        let entries = sens
            .iter()
            .map(|dependency| dependency_entry(ctx, dependency))
            .collect::<Vec<_>>()
            .join(", ");
        return format!(
            "    {{\n        llg_wait_dependency_t deps[] = {{{entries}}};\n        llg_wait_any_dependencies(deps, {});\n    }}\n",
            sens.len()
        );
    }
    let list = sens
        .iter()
        .map(|dependency| dependency_pointer(ctx, dependency))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "    {{\n        sv4_t* s0[] = {{{list}}};\n        llg_wait_any(s0, {});\n    }}\n",
        sens.len()
    )
}

/// Render a typed sensitivity wait whose continuation is explicitly assigned
/// to a different execution region. Body-controlled waits retain the runtime's
/// current-region inference; only executable signal terminators use this path.
pub(in super::super) fn wait_any_text_in_region(
    ctx: &RCtx<'_>,
    sens: &[IrDependency],
    region: ScheduleRegion,
) -> String {
    let mut text = format!(
        "    llg_wait_resume_in_region({});\n",
        region.runtime_symbol()
    );
    text.push_str(&wait_any_text(ctx, sens));
    text
}

fn dependency_pointer(ctx: &RCtx<'_>, dependency: &IrDependency) -> String {
    match dependency {
        IrDependency::PackedRange { storage, .. } => dependency_pointer(ctx, storage),
        IrDependency::Scalar(name) => format!("&{name}"),
        IrDependency::Real(_) => {
            unreachable!("real dependency requires typed wait entries")
        }
        IrDependency::ArrayElement { array, index } => format!(
            "&{}_llg_element_deps[{}]",
            ctx.model.array(*array).c_name(),
            index
        ),
        IrDependency::ArrayContents(array) => {
            format!("&{}_llg_contents_dep", ctx.model.array(*array).c_name())
        }
        IrDependency::ContainerContents(container) => format!(
            "&{}_llg_contents_dep",
            ctx.model.containers[*container].c_name
        ),
        IrDependency::ContainerShape(container) => {
            format!("&{}_llg_shape_dep", ctx.model.containers[*container].c_name)
        }
        IrDependency::Object(object) => {
            format!("&{}_llg_dep", ctx.model.objects[*object].c_name)
        }
    }
}

pub(super) fn display_dependency_pointer(ctx: &RCtx<'_>, dependency: &IrDependency) -> String {
    match dependency {
        IrDependency::Real(name) => format!("{{ LLG_FMT_REAL, &{name} }}"),
        _ => format!(
            "{{ LLG_FMT_PACKED, {} }}",
            dependency_pointer(ctx, dependency)
        ),
    }
}

fn dependency_entry(ctx: &RCtx<'_>, dependency: &IrDependency) -> String {
    match dependency {
        IrDependency::PackedRange { storage, lsb, width } => {
            let trigger = dependency_pointer(ctx, storage);
            let value = match storage.as_ref() {
                IrDependency::Scalar(name) => format!("&{name}"),
                IrDependency::ArrayElement { array, index } => format!("&{}[{index}]", ctx.model.array(*array).c_name()),
                _ => unreachable!("validated packed-prefix storage"),
            };
            format!("{{ .sig = {trigger}, .value = {value}, .lsb = {lsb}u, .width = {width}u }}")
        }
        IrDependency::Scalar(name) => format!("{{ .sig = &{name} }}"),
        IrDependency::Real(name) => format!("{{ .real = &{name} }}"),
        IrDependency::ArrayElement { array, index } => {
            let array = ctx.model.array(*array);
            if array.real {
                format!("{{ .real = &{}[{}] }}", array.c_name(), index)
            } else {
                format!("{{ .sig = &{}_llg_element_deps[{}] }}", array.c_name(), index)
            }
        }
        IrDependency::ArrayContents(array) => format!(
            "{{ .sig = &{}_llg_contents_dep }}",
            ctx.model.array(*array).c_name()
        ),
        IrDependency::ContainerContents(container) => format!(
            "{{ .sig = &{}_llg_contents_dep }}",
            ctx.model.containers[*container].c_name
        ),
        IrDependency::ContainerShape(container) => format!(
            "{{ .sig = &{}_llg_shape_dep }}",
            ctx.model.containers[*container].c_name
        ),
        IrDependency::Object(object) => {
            format!("{{ .sig = &{}_llg_dep }}", ctx.model.objects[*object].c_name)
        }
    }
}

fn event_context_for<'a>(
    ctx: &'a RCtx<'_>,
    helper: &str,
) -> Option<&'a crate::sim::ir::IrEventContext> {
    for process in &ctx.model.processes {
        for pre in &process.pre_fns {
            match pre {
                crate::sim::ir::IrPreFn::MonEval {
                    c_name, context, ..
                }
                | crate::sim::ir::IrPreFn::RealEval {
                    c_name, context, ..
                } if c_name == helper => {
                    return context.as_ref();
                }
                _ => {}
            }
        }
    }
    for function in &ctx.model.funcs {
        for pre in &function.pre_fns {
            match pre {
                crate::sim::ir::IrPreFn::MonEval {
                    c_name, context, ..
                }
                | crate::sim::ir::IrPreFn::RealEval {
                    c_name, context, ..
                } if c_name == helper => {
                    return context.as_ref();
                }
                _ => {}
            }
        }
    }
    None
}

fn event_frame_name(frame: crate::sim::ir::FrameId) -> String {
    format!("_event_frame_{}", frame.index())
}

pub(in super::super) fn event_ref_code(
    ctx: &RCtx<'_>,
    event: &crate::sim::ir::IrEventRef,
) -> Result<String, String> {
    match event {
        crate::sim::ir::IrEventRef::Null => Ok("NULL".to_string()),
        crate::sim::ir::IrEventRef::Captured(name) => Ok(format!("&{name}")),
        crate::sim::ir::IrEventRef::Static(index) => {
            let event = ctx
                .model
                .events()
                .get(*index)
                .ok_or_else(|| "event index is out of bounds during emission".to_string())?;
            if event.is_array() {
                return Err("event array descriptor cannot be emitted as a handle".into());
            }
            Ok(format!("&{}", event.c_name()))
        }
        crate::sim::ir::IrEventRef::Array { array, indices } => {
            let descriptor =
                ctx.model.events().get(*array).ok_or_else(|| {
                    "event array index is out of bounds during emission".to_string()
                })?;
            let dims = descriptor
                .array_dims()
                .ok_or_else(|| "event handle references a non-array descriptor".to_string())?;
            if dims.len() != indices.len() {
                return Err("event array index rank does not match dimensions".into());
            }
            let values = indices
                .iter()
                .map(|index| render_expr(ctx, index).map(|value| value.code))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!(
                "llg_event_array_select({}__elements, {}ULL, {}__left, {}__right, (sv4_t[]){{ {} }}, {})",
                descriptor.c_name(),
                descriptor.array_elements().len(),
                descriptor.c_name(),
                descriptor.c_name(),
                values.join(", "),
                values.len()
            ))
        }
    }
}

pub(super) fn event_capture_code(code: &str, context: Option<&crate::sim::ir::IrEventContext>) -> String {
    let Some(context) = context else {
        return code.to_owned();
    };
    let mut replaced = code.to_owned();
    for capture in context.captures() {
        let replacement = match capture.storage().kind() {
            StorageKind::Real => format!(
                "llg_frame_read_real((const llg_frame_t*)context, {}u)",
                capture.storage().slot()
            ),
            StorageKind::Packed | StorageKind::Opaque => format!(
                "llg_frame_read_value((const llg_frame_t*)context, {}u)",
                capture.storage().slot()
            ),
        };
        replaced = replace_c_identifier(&replaced, capture.local(), &replacement);
    }
    replaced
}

pub(super) fn format_frame_capture(
    frame: &str,
    storage: crate::sim::ir::StorageRef,
    initial: &str,
) -> Result<String, String> {
    let call = match storage.kind() {
        StorageKind::Packed => format!(
            "    llg_frame_capture_value({frame}, {}u, {initial});\n",
            storage.slot()
        ),
        StorageKind::Real => format!(
            "    llg_frame_capture_real({frame}, {}u, {initial});\n",
            storage.slot()
        ),
        StorageKind::Opaque => format!(
            "    llg_frame_capture_opaque({frame}, {}u, {initial});\n",
            storage.slot()
        ),
    };
    Ok(call)
}

fn replace_c_identifier(code: &str, identifier: &str, replacement: &str) -> String {
    if identifier.is_empty() {
        return code.to_owned();
    }
    let bytes = code.as_bytes();
    let mut out = String::with_capacity(code.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if code[cursor..].starts_with(identifier)
            && (cursor == 0 || !is_c_identifier_byte(bytes[cursor - 1]))
            && (cursor + identifier.len() == bytes.len()
                || !is_c_identifier_byte(bytes[cursor + identifier.len()]))
        {
            out.push_str(replacement);
            cursor += identifier.len();
        } else {
            let character = code[cursor..].chars().next().expect("cursor in string");
            out.push(character);
            cursor += character.len_utf8();
        }
    }
    out
}

fn is_c_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Render one atomic multi-source wait.  The call shape follows the source
/// mix: all signals keep the historical `llg_wait_any_events` text, a single
/// event uses `llg_wait_event`, an event-only list uses `llg_wait_events`,
/// and mixed signal/event lists use ONE atomic `llg_wait_mixed` call (never
/// sequential waits, so no trigger can be lost between them).  Total over IR
/// shapes: an empty spec list renders as `llg_wait_time(0)` like `WaitAny`
/// with an empty read set — unreachable from today's lowering (which routes
/// empty spec lists to `WaitAny`) but the emitter must not produce
/// non-compilable C (`ev[] = {}`) if that ever changes.
pub(super) fn wait_events_text(
    ctx: &RCtx<'_>,
    specs: &[(crate::sim::ir::IrWaitSrc, crate::sim::ir::IrEdge)],
) -> Result<String, String> {
    use crate::sim::ir::IrEdge;
    if specs.is_empty() {
        return Ok("    llg_wait_time(0);\n".to_string());
    }
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    if specs.iter().any(|(source, _)| {
        matches!(
            source,
            IrWaitSrc::Evaluated { .. }
                | IrWaitSrc::EvaluatedReal { .. }
                | IrWaitSrc::FilteredEvent { .. }
                | IrWaitSrc::Real(_)
        )
    }) {
        let mut text = String::from("    {\n");
        let mut contexts = HashMap::new();
        for (source, _) in specs {
            let helpers = match source {
                IrWaitSrc::Evaluated {
                    eval, condition, ..
                }
                | IrWaitSrc::EvaluatedReal {
                    eval, condition, ..
                } => std::iter::once(Some(eval.as_str()))
                    .chain(std::iter::once(condition.as_deref()))
                    .collect::<Vec<_>>(),
                IrWaitSrc::FilteredEvent { condition, .. } => {
                    vec![Some(condition.as_str())]
                }
                _ => Vec::new(),
            };
            for helper in helpers.into_iter().flatten() {
                if let Some(context) = event_context_for(ctx, helper) {
                    contexts.entry(context.frame()).or_insert(context);
                }
            }
        }
        for context in contexts.values() {
            let frame = event_frame_name(context.frame());
            text.push_str(&format!(
                "        llg_frame_t* {frame} = llg_frame_new({}u);\n",
                context.captures().len()
            ));
            for capture in context.captures() {
                let initial = render_expr(ctx, capture.initial())?.code;
                text.push_str(
                    &format_frame_capture(&frame, capture.storage(), &initial)?
                        .replace("    ", "        "),
                );
            }
        }
        let mut entries = Vec::new();
        for (index, (source, edge)) in specs.iter().enumerate() {
            let kind = edge_kind(edge);
            let entry = match source {
                IrWaitSrc::Sig(name) => {
                    format!("{{ .sig = &{name}, .kind = {kind} }}")
                }
                IrWaitSrc::Real(name) => {
                    format!("{{ .kind = {kind}, .real_sig = &{name}, .real = 1 }}")
                }
                IrWaitSrc::Event(event) => format!(
                    "{{ .event = {}, .kind = {kind} }}",
                    event_ref_code(ctx, event)?
                ),
                IrWaitSrc::FilteredEvent { event, condition } => {
                    let context = event_context_for(ctx, condition)
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    format!(
                        "{{ .condition = {condition}, .condition_context = {context}, .event = {}, .kind = {kind} }}",
                        event_ref_code(ctx, event)?
                    )
                }
                IrWaitSrc::Evaluated {
                    eval,
                    condition,
                    reads,
                } => {
                    let deps = if reads.is_empty() {
                        "0".to_owned()
                    } else {
                        let deps = format!("_deps{index}");
                        text.push_str(&format!(
                            "        llg_wait_dependency_t {deps}[] = {{{}}};\n",
                            reads
                                .iter()
                                .map(|dependency| dependency_entry(ctx, dependency))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                        deps
                    };
                    let eval_context = event_context_for(ctx, eval)
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    let condition_context = condition
                        .as_deref()
                        .and_then(|condition| event_context_for(ctx, condition))
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    format!(
                        "{{ .eval = {eval}, .condition = {}, .eval_context = {eval_context}, .condition_context = {condition_context}, .kind = {kind}, .dependencies = {deps}, .n_dependencies = {} }}",
                        condition.as_deref().unwrap_or("0"),
                        reads.len()
                    )
                }
                IrWaitSrc::EvaluatedReal {
                    eval,
                    condition,
                    reads,
                } => {
                    let deps = if reads.is_empty() {
                        "0".to_owned()
                    } else {
                        let deps = format!("_deps{index}");
                        text.push_str(&format!(
                            "        llg_wait_dependency_t {deps}[] = {{{}}};\n",
                            reads
                                .iter()
                                .map(|dependency| dependency_entry(ctx, dependency))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                        deps
                    };
                    let eval_context = event_context_for(ctx, eval)
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    let condition_context = condition
                        .as_deref()
                        .and_then(|condition| event_context_for(ctx, condition))
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    format!(
                        "{{ .real_eval = {eval}, .condition = {}, .eval_context = {eval_context}, .condition_context = {condition_context}, .kind = {kind}, .dependencies = {deps}, .n_dependencies = {}, .real = 1 }}",
                        condition.as_deref().unwrap_or("0"),
                        reads.len()
                    )
                }
            };
            entries.push(entry);
        }
        text.push_str(&format!(
            "        llg_expr_event_spec_t _events[] = {{{}}};\n        llg_wait_expressions(_events, {});\n",
            entries.join(", "),
            entries.len()
        ));
        text.push_str("    }\n");
        return Ok(text);
    }
    let n_events = specs
        .iter()
        .filter(|(s, _)| matches!(s, IrWaitSrc::Event(_)))
        .count();
    if n_events == 0 {
        if specs
            .iter()
            .any(|(source, _)| matches!(source, IrWaitSrc::Real(_)))
        {
            if specs.iter().any(|(_, edge)| *edge != IrEdge::Any) {
                return Err("real event sources only support any-change controls".into());
            }
            let entries = specs
                .iter()
                .map(|(source, _)| match source {
                    IrWaitSrc::Sig(name) => Ok(format!("{{ &{name}, 0 }}")),
                    IrWaitSrc::Real(name) => Ok(format!("{{ 0, &{name} }}")),
                    _ => Err("unexpected event source in real dependency wait".into()),
                })
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(format!(
                "    {{\n        llg_wait_dependency_t deps[] = {{{}}};\n        llg_wait_any_dependencies(deps, {});\n    }}\n",
                entries.join(", "),
                specs.len()
            ));
        }
        // Pure signal or-list: the pre-events shape, byte for byte.
        let mut entries = Vec::with_capacity(specs.len());
        for (sig, edge) in specs {
            let IrWaitSrc::Sig(name) = sig else {
                unreachable!("n_events == 0 with an event entry")
            };
            entries.push(format!("{{ &{name}, {} }}", edge_kind(edge)));
        }
        return Ok(format!(
            "    {{\n        llg_event_spec_t ev[] = {{{}}};\n        \
             llg_wait_any_events(ev, {});\n    }}\n",
            entries.join(", "),
            specs.len()
        ));
    }
    if n_events == specs.len() {
        if specs.len() == 1 {
            let IrWaitSrc::Event(event) = &specs[0].0 else {
                unreachable!("n_events == specs.len() with a signal entry")
            };
            return Ok(format!(
                "    llg_wait_event({});\n",
                event_ref_code(ctx, event)?
            ));
        }
        let mut names = Vec::with_capacity(specs.len());
        for (src, _) in specs {
            let IrWaitSrc::Event(event) = src else {
                unreachable!("pure-event list with a signal entry")
            };
            names.push(event_ref_code(ctx, event)?);
        }
        return Ok(format!(
            "    {{\n        const llg_event_t* const ev[] = {{{}}};\n        \
             llg_wait_events(ev, {});\n    }}\n",
            names.join(", "),
            specs.len()
        ));
    }
    // Mixed signal + event sources: one atomic registration.
    let mut entries = Vec::with_capacity(specs.len());
    for (src, edge) in specs {
        match src {
            IrWaitSrc::Sig(name) => {
                entries.push(format!("{{ &{name}, {}, 0 }}", edge_kind(edge)));
            }
            IrWaitSrc::Event(event) => {
                entries.push(format!("{{ 0, 0, {} }}", event_ref_code(ctx, event)?));
            }
            IrWaitSrc::Real(_)
            | IrWaitSrc::Evaluated { .. }
            | IrWaitSrc::EvaluatedReal { .. }
            | IrWaitSrc::FilteredEvent { .. } => {
                unreachable!("evaluated events handled above")
            }
        }
    }
    Ok(format!(
        "    {{\n        llg_wait_src_t src[] = {{{}}};\n        \
         llg_wait_mixed(src, {});\n    }}\n",
        entries.join(", "),
        specs.len()
    ))
}

pub(super) fn clocking_cycle_wait_text(
    ctx: &RCtx<'_>,
    count: &IrExpr,
    specs: &[(crate::sim::ir::IrWaitSrc, crate::sim::ir::IrEdge)],
) -> Result<String, String> {
    if specs.is_empty() {
        return Err("clocking cycle wait requires at least one event source".into());
    }
    let count = render_expr(ctx, count)?.code;
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    let entries = specs
        .iter()
        .map(|(source, edge)| match source {
            IrWaitSrc::Sig(name) => Ok(format!(
                "{{ .sig = &{name}, .kind = {}, .ev = NULL }}",
                edge_kind(edge)
            )),
            IrWaitSrc::Event(event) => Ok(format!(
                "{{ .sig = NULL, .kind = {}, .ev = {} }}",
                edge_kind(edge),
                event_ref_code(ctx, event)?
            )),
            _ => Err("clocking cycle wait has an unsupported event source".into()),
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!(
        "{{ llg_wait_src_t _clocking_cycle_sources[] = {{{}}}; \
         llg_wait_clocking_cycles(_clocking_cycle_sources, {}, {}); }}\n",
        entries.join(", "),
        entries.len(),
        count
    ))
}

/// Render an issue-time event/repeat control for `->>`. Complex sources use
/// the same descriptor/frame construction as expression waits; only the
/// runtime operation changes from suspension to retained NBA registration.
pub(super) fn nonblocking_event_trigger_when_text(
    ctx: &RCtx<'_>,
    ev: &crate::sim::ir::IrEventRef,
    specs: &[(crate::sim::ir::IrWaitSrc, crate::sim::ir::IrEdge)],
    repeat: Option<&IrExpr>,
) -> Result<String, String> {
    use crate::sim::ir::IrEdge;
    let count = match repeat {
        Some(repeat) => {
            let rendered = render_expr(ctx, repeat)?;
            if rendered.width == 0 {
                return Err("repeat nonblocking event trigger count cannot be real".into());
            }
            format!("llg_repeat_count({})", rendered.code)
        }
        None => "1ULL".to_owned(),
    };
    let target = event_ref_code(ctx, ev)?;
    if specs.is_empty() {
        return Ok(format!(
            "    llg_nba_event_when(NULL, 0, {target}, {count});\n"
        ));
    }
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    let complex = specs.iter().any(|(source, _)| {
        matches!(
            source,
            IrWaitSrc::Evaluated { .. }
                | IrWaitSrc::EvaluatedReal { .. }
                | IrWaitSrc::FilteredEvent { .. }
                | IrWaitSrc::Real(_)
        )
    });
    if complex {
        let mut text = wait_events_text(ctx, specs)?;
        let needle = format!("llg_wait_expressions(_events, {});", specs.len());
        let replacement = format!(
            "llg_nba_event_when(_events, {}, {target}, {count});",
            specs.len()
        );
        if !text.contains(&needle) {
            return Err("internal: complex event trigger did not render descriptors".into());
        }
        text = text.replace(&needle, &replacement);
        return Ok(text);
    }
    let entries = specs
        .iter()
        .map(|(source, edge)| match source {
            IrWaitSrc::Sig(name) => {
                Ok(format!("{{ .sig = &{name}, .kind = {} }}", edge_kind(edge)))
            }
            IrWaitSrc::Event(event) => Ok(format!(
                "{{ .event = {}, .kind = {} }}",
                event_ref_code(ctx, event)?,
                edge_kind(edge)
            )),
            IrWaitSrc::Evaluated { .. }
            | IrWaitSrc::EvaluatedReal { .. }
            | IrWaitSrc::FilteredEvent { .. }
            | IrWaitSrc::Real(_) => {
                Err("internal: complex event source missed descriptor rendering".into())
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!(
        "    {{\n        llg_expr_event_spec_t _events[] = {{{}}};\n        \\
         llg_nba_event_when(_events, {}, {target}, {count});\n    }}\n",
        entries.join(", "),
        entries.len()
    ))
}

/// Render an issue-time nonblocking intra-assignment event control.  The
/// callback owns a frame containing the RHS and dynamic destination captures;
/// the issuer therefore never suspends and the eventual update observes the
/// values captured at issue time.
pub(super) fn nonblocking_event_assignment_when_text(
    ctx: &RCtx<'_>,
    specs: &[(crate::sim::ir::IrWaitSrc, crate::sim::ir::IrEdge)],
    repeat: Option<&IrExpr>,
    action: &str,
    frame: crate::sim::ir::FrameId,
    captures: &[crate::sim::ir::IrCapture],
) -> Result<String, String> {
    use crate::sim::ir::IrEdge;
    let count = match repeat {
        Some(repeat) => {
            let rendered = render_expr(ctx, repeat)?;
            if rendered.width == 0 {
                return Err("repeat nonblocking event assignment count cannot be real".into());
            }
            format!("llg_repeat_count({})", rendered.code)
        }
        None => "1ULL".to_owned(),
    };
    let frame_name = format!("_event_action_frame_{}", frame.index());
    let mut frame_setup = format!(
        "        llg_frame_t* {frame_name} = llg_frame_new({}u);\n",
        captures.len()
    );
    for capture in captures {
        let initial = render_expr(ctx, capture.initial())?.code;
        frame_setup.push_str(
            &format_frame_capture(&frame_name, capture.storage(), &initial)?
                .replace("    ", "        "),
        );
    }
    if specs.is_empty() {
        return Ok(format!(
            "    {{\n{frame_setup}        llg_nba_event_assign_when(NULL, 0, {count}, {action}, {frame_name});\n    }}\n"
        ));
    }
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    let complex = specs.iter().any(|(source, _)| {
        matches!(
            source,
            IrWaitSrc::Evaluated { .. }
                | IrWaitSrc::EvaluatedReal { .. }
                | IrWaitSrc::FilteredEvent { .. }
                | IrWaitSrc::Real(_)
        )
    });
    if complex {
        let mut text = wait_events_text(ctx, specs)?;
        let needle = format!("llg_wait_expressions(_events, {});", specs.len());
        let replacement = format!(
            "llg_nba_event_assign_when(_events, {}, {count}, {action}, {frame_name});",
            specs.len()
        );
        if !text.contains(&needle) {
            return Err("internal: complex event assignment did not render descriptors".into());
        }
        text = text.replace(&needle, &replacement);
        if !text.starts_with("    {\n") {
            return Err("internal: complex event assignment lost descriptor block".into());
        }
        text.insert_str("    {\n".len(), &frame_setup);
        return Ok(text);
    }
    let entries = specs
        .iter()
        .map(|(source, edge)| match source {
            IrWaitSrc::Sig(name) => {
                Ok(format!("{{ .sig = &{name}, .kind = {} }}", edge_kind(edge)))
            }
            IrWaitSrc::Event(event) => Ok(format!(
                "{{ .event = {}, .kind = {} }}",
                event_ref_code(ctx, event)?,
                edge_kind(edge)
            )),
            IrWaitSrc::Evaluated { .. }
            | IrWaitSrc::EvaluatedReal { .. }
            | IrWaitSrc::FilteredEvent { .. }
            | IrWaitSrc::Real(_) => {
                Err("internal: complex event source missed descriptor rendering".into())
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!(
        "    {{\n{frame_setup}        llg_expr_event_spec_t _events[] = {{{}}};\n        llg_nba_event_assign_when(_events, {}, {count}, {action}, {frame_name});\n    }}\n",
        entries.join(", "),
        entries.len()
    ))
}

pub(super) fn render_delay(ctx: &RCtx<'_>, delay: &crate::sim::ir::IrDelay) -> Result<String, String> {
    use crate::sim::ir::IrDelay;
    Ok(match delay {
        IrDelay::Constant(ticks) => format!("{ticks}ULL"),
        IrDelay::Runtime {
            value,
            unit_ticks,
            precision_ticks,
        } => {
            let value = render_expr(ctx, value)?;
            if value.width == 0 {
                format!(
                    "sv4_real_delay_ticks({}, {unit_ticks}ULL, {precision_ticks}ULL)",
                    value.code
                )
            } else {
                format!("sv4_delay_ticks({}, {unit_ticks}ULL)", value.code)
            }
        }
    })
}
