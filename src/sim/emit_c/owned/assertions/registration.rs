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
        // Assertions read only Preponed snapshots. Per-slot history belongs to
        // clocking skews, which register their sources separately; keeping it
        // here would grow with simulated time.
        for (index, signal) in model.signals.iter().enumerate() {
            if (!signal.omit || !signal.net_alias.is_empty()) && signal.ty.width() != 0 {
                out.push_str(&format!(
                    "    llg_sampled_register_value(&{});\n",
                    signal_name(model, index)
                ));
            } else if !signal.omit && matches!(signal.ty, IrType::Real { .. }) {
                out.push_str(&format!(
                    "    llg_sampled_register_real(&{});\n",
                    signal.c_name
                ));
            }
        }
    }
    // Clocks precede the domains sampled on them; identities are IR indices.
    for (index, clock) in model.sampled_clocks().iter().enumerate() {
        let gate = if clock.gate.is_some() {
            sampled_clock_gate_name(index)
        } else {
            "NULL".to_owned()
        };
        match &clock.kind {
            IrSampledClockKind::Edge { signal, posedge } => {
                let edge = if *posedge {
                    "LLG_EV_POSEDGE"
                } else {
                    "LLG_EV_NEGEDGE"
                };
                out.push_str(&format!(
                    "    if (!llg_sampled_clock_register_edge({index}ULL, &{}, {edge}, {gate}, NULL)) return 0;\n",
                    signal_name(model, *signal),
                ));
            }
            IrSampledClockKind::Event => out.push_str(&format!(
                "    if (!llg_sampled_clock_register_event({index}ULL, {gate}, NULL)) return 0;\n"
            )),
        }
    }
    for (index, domain) in model.sampled_domains().iter().enumerate() {
        if domain.history_ticks == 0 {
            continue;
        }
        out.push_str(&format!(
            "    if (!llg_sampled_domain_register({index}ULL, {}ULL, {}, NULL, {}ULL)) return 0;\n",
            domain.clock,
            sampled_domain_callback_name(index),
            domain.history_ticks,
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
        let pass_desc = assertion
            .pass_action()
            .map(|action| format!("&{action}_desc"))
            .unwrap_or_else(|| "NULL".to_owned());
        let fail_desc = assertion
            .fail_action()
            .map(|action| format!("&{action}_desc"))
            .unwrap_or_else(|| "NULL".to_owned());
        let kind = match assertion.kind() {
            IrConcurrentAssertionKind::Assert => "LLG_ASSERTION_ASSERT",
            IrConcurrentAssertionKind::Assume => "LLG_ASSERTION_ASSUME",
            IrConcurrentAssertionKind::Cover => "LLG_ASSERTION_COVER",
            // A predicate-registered sequence has at most one match per
            // attempt, so only the sequence engine needs the match count.
            IrConcurrentAssertionKind::CoverSequence
                if assertion.consequent_sequence().is_some() =>
            {
                "LLG_ASSERTION_COVER_SEQUENCE"
            }
            IrConcurrentAssertionKind::CoverSequence => "LLG_ASSERTION_COVER",
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
                    pass_desc,
                    fail_desc,
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
                    pass_desc,
                    fail_desc,
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
                    pass_desc,
                    fail_desc,
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
                    pass_desc,
                    fail_desc,
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
