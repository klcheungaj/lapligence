//! Sequence tables retain the original graph layout; callbacks own temporary values.
use super::*;

pub(super) fn render(model: &IrModel, index: usize, role: &str, sequence: &IrSequence) -> Result<String, String> {
    let ctx = RCtx { model, func: None, sampled: true, activation_label: None };
    let mut frame = callback_frame(&ctx);
    bind_sequence(&mut frame, sequence);
    frame.line("switch (atom) {");
    for (atom_index, atom) in sequence.atoms().iter().enumerate() {
        frame.line(format!("case {atom_index}u: {{"));
        let value = frame.expression(atom)?;
        if value.width == 0 { return Err("sequence atom must be packed".to_owned()); }
        let result = frame.scalar("int", value.truth());
        frame.discard(value);
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        frame.line(format!("return {result}; }}"));
    }
    frame.line("default: break; }");
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    frame.line("return 0;");
    let atom_name = format!("llg_assertion_sequence_{index}_{role}_atom");
    let mut out = format!("static int {atom_name}(uint32_t atom, void* data) {{\n{}{}\n}}\n\n", frame.prologue(), frame.body());
    let init_name = format!("llg_assertion_sequence_{index}_{role}_init");
    if !sequence.initializers().is_empty() {
        let mut frame = callback_frame(&ctx);
        bind_sequence(&mut frame, sequence);
        for (i, expression) in sequence.initializers().iter().enumerate() {
            frame.line(format!("if (!llg_sequence_local_inherited(data, {}u)) {{", sequence.initializer_slots[i]));
            let value = frame.expression(expression)?;
            frame.discard(value);
            frame.line("}");
        }
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        out.push_str(&format!("static void {init_name}(void* data) {{\n{}{}\n}}\n\n", frame.prologue(), frame.body()));
    }
    let match_name = format!("llg_assertion_sequence_{index}_{role}_match");
    if !sequence.match_items().is_empty() {
        let mut frame = callback_frame(&ctx);
        bind_sequence(&mut frame, sequence);
        frame.line("switch (item) {");
        for (i, expression) in sequence.match_items().iter().enumerate() {
            frame.line(format!("case {i}u: {{"));
            let value = frame.expression(expression)?;
            frame.discard(value);
            frame.line("break; }");
        }
        frame.line("default: break; }");
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        out.push_str(&format!("static void {match_name}(uint32_t item, void* data) {{\n{}{}\n}}\n\n", frame.prologue(), frame.body()));
    }
    let transition_name = format!(
        "{name}_transitions",
        name = assertion_sequence_name(index, role)
    );
    out.push_str(&format!(
        "static const llg_sequence_transition_t {transition_name}[{}] = {{\n",
        sequence.transitions().len().max(1)
    ));
    if sequence.transitions().is_empty() {
        out.push_str("    {0},\n");
    }
    for transition in sequence.transitions() {
        let max = transition
            .delay
            .max
            .map(|value| format!("{value}ULL"))
            .unwrap_or_else(|| "LLG_SEQUENCE_UNBOUNDED".to_owned());
        let clock = transition
            .clock_signal
            .map(|signal| format!("&{}", registration::signal_name(model, signal)))
            .unwrap_or_else(|| "NULL".to_owned());
        let edge = if transition.clock_signal.is_some() {
            if transition.clock_posedge {
                "LLG_EV_POSEDGE"
            } else {
                "LLG_EV_NEGEDGE"
            }
        } else {
            "0"
        };
        let atom = transition
            .atom
            .map(|value| format!("{value}u"))
            .unwrap_or_else(|| "LLG_SEQUENCE_EPSILON".to_owned());
        let match_start = transition
            .match_start
            .map(|value| format!("{value}u"))
            .unwrap_or_else(|| "0u".to_owned());
        out.push_str(&format!(
            "    {{{}u, {}u, {}ULL, {max}, {clock}, {edge}, {atom}, {match_start}, {}u, {}u, {}u}},\n",
            transition.from, transition.to, transition.delay.min, transition.match_count,
            transition.enter_scope.unwrap_or(0), transition.exit_scope.unwrap_or(0),
        ));
    }
    out.push_str("};\n");
    let sequence_name = assertion_sequence_name(index, role);
    let first_match_states_name = format!("{sequence_name}_first_match_states");
    let first_match_states = sequence.first_match_states();
    if !first_match_states.is_empty() {
        out.push_str(&format!(
            "static const uint32_t {first_match_states_name}[{}] = {{",
            first_match_states.len()
        ));
        for state in first_match_states {
            out.push_str(&format!(" {state}u,"));
        }
        out.push_str(" };\n");
    }
    let first_match_states_ptr = if first_match_states.is_empty() {
        "NULL".to_owned()
    } else {
        first_match_states_name.clone()
    };
    let locals_name = format!("{sequence_name}_locals");
    if !sequence.locals().is_empty() {
        out.push_str(&format!(
            "static const llg_sequence_local_t {locals_name}[{}] = {{\n",
            sequence.locals().len()
        ));
        for local in sequence.locals() {
            out.push_str(&format!(
                "    {{{}u, {}, {}, {}ULL}},\n",
                local.width, local.signed as u8, local.two_state as u8, local.declaration
            ));
        }
        out.push_str("};\n");
    }
    let locals_ptr = if sequence.locals().is_empty() {
        "NULL".to_owned()
    } else {
        locals_name
    };
    let match_ptr = if sequence.match_items().is_empty() {
        "NULL".to_owned()
    } else {
        match_name
    };
    let init_ptr = if sequence.initializers().is_empty() {
        "NULL".to_owned()
    } else {
        init_name
    };
    let leading_clock = sequence
        .leading_clock
        .map(|signal| format!("&{}", registration::signal_name(model, signal)))
        .unwrap_or_else(|| "NULL".to_owned());
    let leading_edge = if sequence.leading_posedge {
        "LLG_EV_POSEDGE"
    } else {
        "LLG_EV_NEGEDGE"
    };
    out.push_str(&format!(
        "static const llg_sequence_graph_t {sequence_name} = {{ {}u, {}u, {}u, {}u, {transition_name}, {}u, {first_match_states_ptr}, {atom_name}, NULL, {init_ptr}, {}, {}u, {locals_ptr}, {}u, {match_ptr}, {}, {leading_clock}, {leading_edge} }};\n\n",
        sequence.states(),
        sequence.start(),
        sequence.accept(),
        sequence.transitions().len(),
        first_match_states.len(),
        sequence.first_match() as u8,
        sequence.locals().len(),
        sequence.match_items().len(),
        sequence.admits_empty as u8,
    ));
    Ok(out)
}
