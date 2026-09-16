//! Dynamic startup replaces static packed constructors. Cleanup is idempotent.
use super::*;
use std::collections::HashSet;

pub(in crate::sim::emit_c) fn storage_lifecycle(model: &IrModel, out: &mut String) -> Result<(), String> {
    let mut initialize = String::new();
    let mut destroy = String::new();
    let mut emitted = HashSet::new();
    for signal in &model.signals {
        if signal.net_driver.is_some() || signal.alias.is_some() || signal.omit || !emitted.insert(signal.c_name.clone()) { continue; }
        defaults(&mut initialize, &mut destroy, &signal.c_name, signal.ty.width(), signal.ty.signed(), signal.ty.two_state());
    }
    emitted.clear();
    for group in &model.net_groups {
        if !emitted.insert(group.c_name.clone()) { continue; }
        for slot in 0..group.n_drivers {
            initialize.push_str(&format!("    sv4_replace(&{}_d{slot}, sv4_fill(3, {}, {}));\n", group.c_name, group.width, u8::from(group.signed)));
            destroy.push_str(&format!("    sv4_destroy(&{}_d{slot});\n", group.c_name));
        }
        let fill = match group.kind { IrNetKind::Tri0 | IrNetKind::Supply0 => 0, IrNetKind::Tri1 | IrNetKind::Supply1 => 1, _ => 3 };
        initialize.push_str(&format!("    sv4_replace(&{}.resolved, sv4_fill({fill}, {}, {}));\n", group.c_name, group.width, u8::from(group.signed)));
        destroy.push_str(&format!("    sv4_destroy(&{}.resolved);\n    {}.propagation = NULL;\n", group.c_name, group.c_name));
    }
    emitted.clear();
    for (index, function) in model.funcs.iter().enumerate() {
        if !function.automatic {
            if let Some(ty) = function.ret {
                defaults(&mut initialize, &mut destroy, &format!("_llg_ret_{index}"), ty.width(), ty.signed(), ty.two_state());
            }
        }
        for local in &function.locals {
            if emitted.insert(local.c_name().to_owned()) {
                defaults(&mut initialize, &mut destroy, local.c_name(), if local.real { 0 } else { local.width() }, local.signed(), local.two_state);
            }
        }
    }
    for array in &model.arrays {
        defaults(&mut initialize, &mut destroy, &format!("{}_llg_contents_dep", array.c_name), 1, false, true);
        initialize.push_str(&format!("    for (uint64_t _i = 0; _i < {}ULL; ++_i) {{\n", array.total));
        destroy.push_str(&format!("    for (uint64_t _i = 0; _i < {}ULL; ++_i) {{\n", array.total));
        defaults(&mut initialize, &mut destroy, &format!("{}[_i]", array.c_name), if array.real { 0 } else { array.elem_width }, array.signed, array.two_state);
        defaults(&mut initialize, &mut destroy, &format!("{}_llg_element_deps[_i]", array.c_name), 1, false, true);
        let bind = if array.real { "llg_dependency_bind_real" } else { "llg_dependency_bind" };
        initialize.push_str(&format!("    {bind}(&{}[_i], &{}_llg_element_deps[_i]);\n    {bind}(&{}[_i], &{}_llg_contents_dep);\n    }}\n", array.c_name, array.c_name, array.c_name, array.c_name));
        destroy.push_str("    }\n");
    }
    for event in &model.events {
        initialize.push_str(&format!("    {}__object = (llg_event_object_t){{0}};\n    {}.object = &{}__object;\n", event.c_name, event.c_name, event.c_name));
    }
    out.push_str(&format!("static void llg_model_storage_defaults(void) {{\n{initialize}}}\n\nstatic void llg_model_storage_destroy(void) {{\n{destroy}}}\n\n"));
    let ctx = RCtx { model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.allow_calls = false;
    for step in &model.init_steps { initialization_step(&mut frame, step)?; }
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    out.push_str(&format!("static void llg_model_initializers(void) {{\n{}{}\n}}\n\n", frame.prologue(), frame.body()));
    Ok(())
}

fn defaults(init: &mut String, destroy: &mut String, name: &str, width: u32, signed: bool, two_state: bool) {
    if width == 0 { init.push_str(&format!("    {name} = 0.0;\n")); }
    else {
        init.push_str(&format!("    sv4_replace(&{name}, {});\n", super::super::super::expressions::packed_default(width, signed, two_state)));
        destroy.push_str(&format!("    sv4_destroy(&{name});\n"));
    }
}

fn set_initial(frame: &mut Frame<'_, '_>, target: Binding, value: Value) {
    let value = frame.convert(value, target.width, target.signed, target.two_state, target.shortreal);
    if target.width == 0 { frame.line(format!("*({}) = {};", target.address, value.code)); }
    else { frame.line(format!("sv4_move({}, &{});", target.address, value.code)); }
    frame.discard(value);
}

fn initialization_step(frame: &mut Frame<'_, '_>, step: &IrInitStep) -> Result<(), String> {
    let model = frame.ctx.model;
    match step {
        IrInitStep::FillArrayX(index) | IrInitStep::FillArrayZ(index) => {
            let array = model.array(*index);
            if array.real && matches!(step, IrInitStep::FillArrayZ(_)) {
                return Err("real arrays cannot be initialized to Z".to_owned());
            }
            let value = if array.real { "0.0".to_owned() }
                else if matches!(step, IrInitStep::FillArrayZ(_)) { format!("sv4_fill(3, {}, {})", array.elem_width, u8::from(array.signed)) }
                else { super::super::super::expressions::packed_default(array.elem_width, array.signed, array.two_state) };
            frame.line(format!("for (uint64_t _i = 0; _i < {}ULL; ++_i) {{", array.total));
            if array.real { frame.line(format!("{}[_i] = {value};", array.c_name)); }
            else { frame.line(format!("sv4_replace(&{}[_i], {value});", array.c_name)); }
            frame.line("}");
        }
        IrInitStep::SetScalar { sig, value } => {
            let signal = model.signal(*sig);
            let target = Binding { address: format!("&{}", signal.c_name), width: signal.ty.width(), signed: signal.ty.signed(),
                two_state: signal.ty.two_state(), shortreal: matches!(signal.ty, IrType::Real { shortreal: true }), automatic: false };
            let mut result = frame.value(emit_const(value), value.width, value.signed);
            result.fill = value.fill;
            set_initial(frame, target, result);
        }
        IrInitStep::SetArrayElem { arr, index, value } => {
            let array = model.array(*arr);
            let target = Binding { address: format!("&{}[{index}]", array.c_name), width: if array.real { 0 } else { array.elem_width },
                signed: array.signed, two_state: array.two_state, shortreal: array.shortreal, automatic: false };
            let mut result = frame.value(emit_const(value), value.width, value.signed);
            result.fill = value.fill;
            set_initial(frame, target, result);
        }
        IrInitStep::RegisterSampled(index) => {
            let signal = model.signal(*index);
            if signal.ty.width() == 0 { return Err(pending("real-valued sampling registrations")); }
            frame.line(format!("llg_sampled_register(&{});", signal.c_name));
        }
        IrInitStep::WriteNet { group, slot, value } => {
            let net = model.net_group(*group);
            let mut result = frame.value(emit_const(value), value.width, value.signed);
            result.fill = value.fill;
            let value = frame.convert(result, net.width, net.signed, false, false);
            frame.line(format!("llg_net_write(&{}, {slot}, {});", net.c_name, value.code));
            frame.discard(value);
        }
        IrInitStep::Initialize(initialization) => {
            if initialization.phase() != IrInitPhase::BeforeProcesses { return Ok(()); }
            let target = match initialization.target() {
                IrInitTarget::Signal(index) => {
                    let signal = model.signal(*index);
                    Binding { address: format!("&{}", signal.c_name), width: signal.ty.width(), signed: signal.ty.signed(),
                        two_state: signal.ty.two_state(), shortreal: matches!(signal.ty, IrType::Real { shortreal: true }), automatic: false }
                }
                IrInitTarget::StaticLocal { name, .. } => frame.lookup(name).ok_or_else(|| format!("unknown static initializer target {name}"))?,
            };
            let value = frame.expression(initialization.value())?;
            set_initial(frame, target, value);
        }
    }
    Ok(())
}
