//! Registration precedes process startup and uses canonical alias dependencies.
use super::*;

pub(super) fn signal_name(model: &IrModel, index: usize) -> String {
    let signal = model.signal(index);
    if signal.net_alias.is_empty() {
        signal.c_name.clone()
    } else {
        format!("llg_net_alias_{index}.visible")
    }
}

pub(in crate::sim::emit_c) fn render(model: &IrModel) -> Result<String, String> {
    let mut out = String::from("static int llg_model_assertions_init(void) {\n");
    if !model.assertions.is_empty() || !model.sampled_domains.is_empty() {
        for (index, signal) in model.signals.iter().enumerate() {
            if (!signal.omit || !signal.net_alias.is_empty()) && signal.ty.width() != 0 {
                out.push_str(&format!(
                    "    llg_sampled_register(&{});\n",
                    signal_name(model, index)
                ));
            }
        }
    }
    for (index, domain) in model.sampled_domains().iter().enumerate() {
        let clock = signal_name(model, domain.clock_signal);
        let edge = if domain.posedge {
            "LLG_EV_POSEDGE"
        } else {
            "LLG_EV_NEGEDGE"
        };
        let gate = if domain.gate.is_some() {
            sampled_domain_callback_name(index, "gate")
        } else {
            "NULL".to_owned()
        };
        out.push_str(&format!(
            "    if (!llg_sampled_domain_register({}ULL, &{}, {}, {}, {}, NULL)) return 0;\n",
            index,
            clock,
            edge,
            sampled_domain_callback_name(index, "value"),
            gate,
        ));
    }
    for (index, assertion) in model.assertions().iter().enumerate() {
        let clock = signal_name(model, assertion.clock_signal());
        let disable = assertion
            .disable_signal()
            .map(|signal| format!("&{}", signal_name(model, signal)))
            .unwrap_or_else(|| "NULL".to_owned());
        let antecedent = assertion
            .antecedent()
            .map(|_| assertion_predicate_name(index, "antecedent"))
            .unwrap_or_else(|| "NULL".to_owned());
        let abort_condition = assertion
            .abort_condition()
            .map(|_| assertion_predicate_name(index, "abort"))
            .unwrap_or_else(|| "NULL".to_owned());
        let pass_action = assertion.pass_action().unwrap_or("NULL");
        let fail_action = assertion.fail_action().unwrap_or("NULL");
        let kind = match assertion.kind() {
            IrConcurrentAssertionKind::Assert => "LLG_ASSERTION_ASSERT",
            IrConcurrentAssertionKind::Assume => "LLG_ASSERTION_ASSUME",
            IrConcurrentAssertionKind::Cover => "LLG_ASSERTION_COVER",
            IrConcurrentAssertionKind::Expect => "LLG_ASSERTION_EXPECT",
        };
        let edge = if assertion.posedge() {
            "LLG_EV_POSEDGE"
        } else {
            "LLG_EV_NEGEDGE"
        };
        if assertion.consequent_sequence().is_some() {
            let antecedent = assertion
                .antecedent_sequence()
                .map(|_| format!("&{}", assertion_sequence_name(index, "antecedent")))
                .unwrap_or_else(|| "NULL".to_owned());
            let consequent = format!("&{}", assertion_sequence_name(index, "consequent"));
            if assertion.abort_condition().is_some() {
                out.push_str(&format!(
                    "    if (!llg_assertion_register_sequence_control(&{}, {}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}, {}, {}ULL, {}, {}, {})) return 0;\n",
                    clock,
                    edge,
                    disable,
                    antecedent,
                    consequent,
                    abort_condition,
                    pass_action,
                    fail_action,
                    kind,
                    assertion.overlapped() as u8,
                    assertion.abort_reject() as u8,
                    assertion.abort_sync() as u8,
                    assertion.identity(),
                    c_string_literal(assertion.label()),
                    c_string_literal(assertion.location()),
                    c_string_literal(assertion.scope()),
                ));
            } else {
                out.push_str(&format!(
                    "    if (!llg_assertion_register_sequence(&{}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}ULL, {}, {}, {})) return 0;\n",
                    clock,
                    edge,
                    disable,
                    antecedent,
                    consequent,
                    pass_action,
                    fail_action,
                    kind,
                    assertion.overlapped() as u8,
                    assertion.identity(),
                    c_string_literal(assertion.label()),
                    c_string_literal(assertion.location()),
                    c_string_literal(assertion.scope()),
                ));
            }
        } else {
            if assertion.consequent().is_none() {
                return Err(format!("assertion {index} has no consequent"));
            }
            let consequent = assertion_predicate_name(index, "consequent");
            if assertion.abort_condition().is_some() {
                out.push_str(&format!(
                    "    if (!llg_assertion_register_control(&{}, {}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}, {}, {}ULL, {}, {}, {})) return 0;\n",
                    clock,
                    edge,
                    disable,
                    antecedent,
                    consequent,
                    abort_condition,
                    pass_action,
                    fail_action,
                    kind,
                    assertion.overlapped() as u8,
                    assertion.abort_reject() as u8,
                    assertion.abort_sync() as u8,
                    assertion.identity(),
                    c_string_literal(assertion.label()),
                    c_string_literal(assertion.location()),
                    c_string_literal(assertion.scope()),
                ));
            } else {
                out.push_str(&format!(
                    "    if (!llg_assertion_register(&{}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}ULL, {}, {}, {})) return 0;\n",
                    clock,
                    edge,
                    disable,
                    antecedent,
                    consequent,
                    pass_action,
                    fail_action,
                    kind,
                    assertion.overlapped() as u8,
                    assertion.identity(),
                    c_string_literal(assertion.label()),
                    c_string_literal(assertion.location()),
                    c_string_literal(assertion.scope()),
                ));
            }
        }
    }
    out.push_str("    return !llg_rt_failed();\n}\n\n");
    Ok(out)
}
