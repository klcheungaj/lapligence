//! Assertions.

use super::*;

pub(super) fn assertion_predicate_name(index: usize, role: &str) -> String {
    format!("llg_assertion_{index}_{role}")
}

fn render_assertion_predicate(
    model: &IrModel,
    index: usize,
    role: &str,
    expression: &crate::sim::ir::IrExpr,
    sampled: bool,
) -> Result<String, String> {
    let ctx = RCtx {
        model,
        func: None,
        sampled,
        activation_label: None,
    };
    let rendered = super::super::expressions::render_expr_impl(&ctx, expression)?;
    if rendered.width == 0 {
        return Err(format!(
            "concurrent assertion predicate {role} must be packed"
        ));
    }
    let value = format!("sv4_to_bool({})", rendered.code);
    Ok(format!(
        "static int {}(void* data) {{\n    (void)data;\n    return {};\n}}\n\n",
        assertion_predicate_name(index, role),
        value
    ))
}

pub(super) fn assertion_sequence_name(index: usize, role: &str) -> String {
    format!("llg_assertion_sequence_{index}_{role}")
}

fn assertion_sequence_atom_name(index: usize, role: &str) -> String {
    format!("llg_assertion_sequence_{index}_{role}_atom")
}

fn assertion_sequence_match_name(index: usize, role: &str) -> String {
    format!("llg_assertion_sequence_{index}_{role}_match")
}

fn assertion_sequence_init_name(index: usize, role: &str) -> String {
    format!("llg_assertion_sequence_{index}_{role}_init")
}

fn render_assertion_sequence(
    model: &IrModel,
    index: usize,
    role: &str,
    sequence: &IrSequence,
) -> Result<String, String> {
    let ctx = RCtx {
        model,
        func: None,
        sampled: true,
        activation_label: None,
    };
    let atom_name = assertion_sequence_atom_name(index, role);
    let mut out = String::new();
    out.push_str(&format!(
        "static int {atom_name}(uint32_t atom, void* data) {{\n"
    ));
    out.push_str("    (void)data;\n    switch (atom) {\n");
    for (atom_index, atom) in sequence.atoms().iter().enumerate() {
        let rendered = super::super::expressions::render_expr_impl(&ctx, atom)?;
        if rendered.width == 0 {
            return Err(format!(
                "concurrent assertion sequence {role} atom {atom_index} must be packed"
            ));
        }
        out.push_str(&format!(
            "    case {atom_index}u: return sv4_to_bool({});\n",
            rendered.code
        ));
    }
    out.push_str("    default: return 0;\n    }\n}\n\n");
    let init_name = assertion_sequence_init_name(index, role);
    if !sequence.initializers().is_empty() {
        out.push_str(&format!("static void {init_name}(void* data) {{\n"));
        out.push_str("    (void)data;\n");
        for (initializer_index, initializer) in sequence.initializers().iter().enumerate() {
            let rendered = super::super::expressions::render_expr_impl(&ctx, initializer)?;
            out.push_str(&format!(
                "    if (!llg_sequence_local_inherited(data, {}u)) (void)({}); /* local initializer {initializer_index} */\n",
                sequence.initializer_slots[initializer_index], rendered.code
            ));
        }
        out.push_str("}\n\n");
    }
    let match_name = assertion_sequence_match_name(index, role);
    if !sequence.match_items().is_empty() {
        out.push_str(&format!(
            "static void {match_name}(uint32_t item, void* data) {{\n    (void)data;\n    switch (item) {{\n"
        ));
        for (item_index, item) in sequence.match_items().iter().enumerate() {
            let rendered = super::super::expressions::render_expr_impl(&ctx, item)?;
            out.push_str(&format!(
                "    case {item_index}u: (void)({}); break; /* sequence match item {item_index} */\n",
                rendered.code,
            ));
        }
        out.push_str("    default: break;\n    }\n}\n\n");
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
            .map(|signal| format!("&{}", model.signal(signal).c_name()))
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
        .map(|signal| format!("&{}", model.signal(signal).c_name()))
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

pub(super) fn sampled_domain_callback_name(index: usize, role: &str) -> String {
    format!("llg_sampled_domain_{index}_{role}")
}

pub(super) fn render_sampled_domain_callbacks(model: &IrModel) -> Result<String, String> {
    let mut out = String::new();
    for (index, domain) in model.sampled_domains().iter().enumerate() {
        let ctx = RCtx {
            model,
            func: None,
            sampled: true,
            activation_label: None,
        };
        let value = super::super::expressions::render_expr_impl(&ctx, &domain.sample)?;
        if value.width == 0 {
            return Err(format!(
                "sampled domain {index} callback must return a packed expression"
            ));
        }
        out.push_str(&format!(
            "static sv4_t {}(void* data) {{\n    (void)data;\n    return {};\n}}\n\n",
            sampled_domain_callback_name(index, "value"),
            value.code
        ));
        if let Some(gate) = &domain.gate {
            let gate = super::super::expressions::render_expr_impl(&ctx, gate)?;
            if gate.width == 0 {
                return Err(format!("sampled domain {index} gate must be packed"));
            }
            out.push_str(&format!(
                "static sv4_t {}(void* data) {{\n    (void)data;\n    return {};\n}}\n\n",
                sampled_domain_callback_name(index, "gate"),
                gate.code
            ));
        }
    }
    Ok(out)
}

pub(super) fn render_assertion_callbacks(model: &IrModel) -> Result<String, String> {
    let mut out = String::new();
    for (index, assertion) in model.assertions().iter().enumerate() {
        if let Some(antecedent) = assertion.antecedent() {
            out.push_str(&render_assertion_predicate(
                model,
                index,
                "antecedent",
                antecedent,
                true,
            )?);
        }
        if let Some(consequent) = assertion.consequent() {
            out.push_str(&render_assertion_predicate(
                model,
                index,
                "consequent",
                consequent,
                true,
            )?);
        }
        if let Some(condition) = assertion.abort_condition() {
            out.push_str(&render_assertion_predicate(
                model,
                index,
                "abort",
                condition,
                assertion.abort_sync(),
            )?);
        }
        if let Some(sequence) = assertion.antecedent_sequence() {
            out.push_str(&render_assertion_sequence(
                model,
                index,
                "antecedent",
                sequence,
            )?);
        }
        if let Some(sequence) = assertion.consequent_sequence() {
            out.push_str(&render_assertion_sequence(
                model,
                index,
                "consequent",
                sequence,
            )?);
        }
    }
    Ok(out)
}
