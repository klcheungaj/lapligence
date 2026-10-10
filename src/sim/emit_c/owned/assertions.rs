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
    model: &IrModel,
    constants: &super::super::constants::PackedConstants,
    backend: crate::sim::value_backend::ValueBackend,
    index: usize,
    role: &str,
    expression: &IrExpr,
    sampled: bool,
) -> Result<String, String> {
    let ctx = RCtx {
        value_backend: backend,
        model,
        func: None,
        sampled,
        activation_label: None,
        constants: Some(constants),
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
fn sampled_value(
    model: &IrModel,
    constants: &super::super::constants::PackedConstants,
    backend: crate::sim::value_backend::ValueBackend,
    name: &str,
    expression: &IrExpr,
) -> Result<String, String> {
    let ctx = RCtx {
        value_backend: backend,
        model,
        func: None,
        sampled: true,
        activation_label: None,
        constants: Some(constants),
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

/// A sampled clock's gate reads current values when the clock occurs, like
/// the `iff` of an event control (SV 9.4.2.3, 16.9.3).
fn sampled_clock_gate(
    model: &IrModel,
    constants: &super::super::constants::PackedConstants,
    backend: crate::sim::value_backend::ValueBackend,
    name: &str,
    expression: &IrExpr,
) -> Result<String, String> {
    let ctx = RCtx {
        value_backend: backend,
        model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: Some(constants),
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

pub(in crate::sim::emit_c) fn callbacks(
    model: &IrModel,
    constants: &super::super::constants::PackedConstants,
    backend: crate::sim::value_backend::ValueBackend,
) -> Result<String, String> {
    let mut out = String::new();
    for (index, clock) in model.sampled_clocks().iter().enumerate() {
        if let Some(gate) = &clock.gate {
            out.push_str(&sampled_clock_gate(
                model,
                constants,
                backend,
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
            model,
            constants,
            backend,
            &sampled_domain_callback_name(index),
            &domain.sample,
        )?);
    }
    for (index, assertion) in model.assertions().iter().enumerate() {
        for (role, expression, sampled) in [
            ("antecedent", assertion.antecedent(), true),
            ("consequent", assertion.consequent(), true),
            ("abort", assertion.abort_condition(), assertion.abort_sync()),
        ] {
            if let Some(expression) = expression {
                out.push_str(&predicate(
                    model, constants, backend, index, role, expression, sampled,
                )?);
            }
        }
        if let Some(property) = assertion.property() {
            out.push_str(&property::render(
                model, constants, backend, index, property,
            )?);
        }
        for (role, graph) in [
            ("antecedent", assertion.antecedent_sequence()),
            ("consequent", assertion.consequent_sequence()),
        ] {
            if let Some(graph) = graph {
                out.push_str(&sequence::render(
                    model, constants, backend, index, role, graph,
                )?);
            }
        }
    }
    Ok(out)
}
