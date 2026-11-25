//! Concurrent predicates, sequence-local mutations and sampled history callbacks.
use super::*;
mod property;
mod registration;
mod sequence;
pub(in crate::sim::emit_c) use registration::render as registrations;

fn assertion_predicate_name(index: usize, role: &str) -> String {
    format!("llg_assertion_{index}_{role}")
}
fn assertion_sequence_name(index: usize, role: &str) -> String {
    format!("llg_assertion_sequence_{index}_{role}")
}
fn sampled_domain_callback_name(index: usize) -> String {
    format!("llg_sampled_domain_{index}_value")
}
fn sampled_clock_gate_name(index: usize) -> String {
    format!("llg_sampled_clock_{index}_gate")
}
fn assertion_capture_name(index: usize) -> String {
    format!("llg_assertion_{index}_capture")
}

fn callback_frame<'a, 'm>(ctx: &'a RCtx<'m>) -> Frame<'a, 'm> {
    let mut frame = Frame::new(ctx);
    frame.line("(void)data;");
    frame.line("int depth = 0; (void)depth;");
    frame
}

// These are exact addresses made by typed assertion lowering, not arbitrary
// fragment evaluation. Only slots present in this validated graph are bound.
fn bind_sequence(frame: &mut Frame<'_, '_>, sequence: &IrSequence) {
    for (index, local) in sequence.locals().iter().enumerate() {
        let address = format!("llg_sequence_local_addr(data, {index}u)");
        let binding = Binding {
            address: address.clone(),
            width: local.width,
            signed: local.signed,
            two_state: local.two_state,
            shortreal: false,
            automatic: true,
        };
        frame.sequence_addresses.insert(address, binding.clone());
        frame.bindings[0].insert(format!("llg_sequence_local_read(data, {index}u)"), binding);
    }
}

fn predicate(
    base: &RCtx<'_>,
    index: usize,
    role: &str,
    expression: &IrExpr,
    sampled: bool,
) -> Result<String, String> {
    let ctx = RCtx {
        sampled,
        activation_label: None,
        ..*base
    };
    let mut frame = callback_frame(&ctx);
    let value = frame.expression(expression)?;
    if value.width == 0 {
        return Err(format!(
            "concurrent assertion predicate {role} must be packed"
        ));
    }
    let result = frame.scalar("int", value.truth());
    frame.discard(value);
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    frame.line(format!("return {result};"));
    Ok(format!(
        "static int {}(void* data) {{\n{}{}\n}}\n\n",
        assertion_predicate_name(index, role),
        frame.prologue(),
        frame.body()
    ))
}

/// Store a history domain's Preponed sample into the runtime-owned slot.
fn sampled_value(base: &RCtx<'_>, name: &str, expression: &IrExpr) -> Result<String, String> {
    let ctx = RCtx {
        sampled: true,
        activation_label: None,
        ..*base
    };
    let mut frame = callback_frame(&ctx);
    let value = frame.expression(expression)?;
    if value.width == 0 {
        return Err("sampled domain must return a packed value".to_owned());
    }
    frame.line(format!("sv4_move(out, &{});", value.code));
    frame.discard(value);
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    Ok(format!(
        "static void {name}(void* data, sv4_t* out) {{\n{}{}\n}}\n\n",
        frame.prologue(),
        frame.body()
    ))
}

/// The operands an assertion with global clocking future sampled value
/// functions captures at each clock event (SV 16.9.4), one per slot: the
/// argument of every captured operand and future status call, evaluated
/// against the event's sampled values like a predicate.
fn future_capture(
    base: &RCtx<'_>,
    index: usize,
    assertion: &IrAssertion,
) -> Result<String, String> {
    let mut slots = std::collections::BTreeMap::new();
    for expression in assertion.expressions() {
        let mut copy = expression.clone();
        crate::sim::opt::walk_expr_mut(&mut copy, &mut |node| {
            if let IrExprKind::SysFunc(function) = &node.kind {
                if let IrSysFunc::Sampled(call) = &**function {
                    if call.kind.is_future() && call.kind != IrSampledFunc::Future
                        || call.kind == IrSampledFunc::Captured
                    {
                        slots
                            .entry(call.ticks)
                            .or_insert_with(|| (*call.argument).clone());
                    }
                }
            }
        });
    }
    let ctx = RCtx {
        sampled: true,
        activation_label: None,
        ..*base
    };
    let mut frame = callback_frame(&ctx);
    for (slot, argument) in &slots {
        let value = frame.expression(argument)?;
        if value.width == 0 {
            return Err("captured assertion operand must be packed".to_owned());
        }
        frame.line(format!("sv4_copy(&slots[{slot}], {});", value.address()));
        frame.discard(value);
    }
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    Ok(format!(
        "static void {}(void* data, sv4_t* slots) {{\n{}{}\n}}\n\n",
        assertion_capture_name(index),
        frame.prologue(),
        frame.body()
    ))
}

/// A sampled clock's gate reads current values when the clock occurs, like
/// the `iff` of an event control (SV 9.4.2.3, 16.9.3).
fn sampled_clock_gate(base: &RCtx<'_>, name: &str, expression: &IrExpr) -> Result<String, String> {
    let ctx = RCtx {
        sampled: false,
        activation_label: None,
        ..*base
    };
    let mut frame = callback_frame(&ctx);
    let value = frame.expression(expression)?;
    if value.width == 0 {
        return Err("sampled clock gate must be packed".to_owned());
    }
    let result = frame.scalar("int", value.truth());
    frame.discard(value);
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    frame.line(format!("return {result};"));
    Ok(format!(
        "static int {name}(void* data) {{\n{}{}\n}}\n\n",
        frame.prologue(),
        frame.body()
    ))
}

pub(in crate::sim::emit_c) fn callbacks(base: &RCtx<'_>) -> Result<String, String> {
    let model = base.model;
    let mut out = String::new();
    for (index, clock) in model.sampled_clocks().iter().enumerate() {
        if let Some(gate) = &clock.gate {
            out.push_str(&sampled_clock_gate(
                base,
                &sampled_clock_gate_name(index),
                gate,
            )?);
        }
    }
    for (index, domain) in model.sampled_domains().iter().enumerate() {
        if domain.history_ticks == 0 {
            continue;
        }
        out.push_str(&sampled_value(
            base,
            &sampled_domain_callback_name(index),
            &domain.sample,
        )?);
    }
    for (index, assertion) in model.assertions().iter().enumerate() {
        assertion_callbacks(base, index, assertion, &mut out)
            .map_err(|error| crate::sim::located::locate_at(assertion.location(), error))?;
    }
    Ok(out)
}

/// The predicate, future-capture, property and sequence callbacks of one
/// concurrent assertion.
fn assertion_callbacks(
    base: &RCtx<'_>,
    index: usize,
    assertion: &crate::sim::ir::IrAssertion,
    out: &mut String,
) -> Result<(), String> {
    for (role, expression, sampled) in [
        ("antecedent", assertion.antecedent(), true),
        ("consequent", assertion.consequent(), true),
        ("abort", assertion.abort_condition(), assertion.abort_sync()),
    ] {
        if let Some(expression) = expression {
            out.push_str(&predicate(base, index, role, expression, sampled)?);
        }
    }
    if assertion.future().is_some_and(|future| future.slots() != 0) {
        out.push_str(&future_capture(base, index, assertion)?);
    }
    if let Some(property) = assertion.property() {
        out.push_str(&property::render(base, index, property)?);
    }
    for (role, graph) in [
        ("antecedent", assertion.antecedent_sequence()),
        ("consequent", assertion.consequent_sequence()),
    ] {
        if let Some(graph) = graph {
            out.push_str(&sequence::render(base, index, role, graph)?);
        }
    }
    Ok(())
}
