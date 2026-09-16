//! Ownership-safe procedures and model lifetime boundaries.
use super::*;
use crate::sim::execution::{ExecutionModel, ExecutionProcess, ExecutionTerminator, TriggerPlan};

mod initialization;
mod lifecycle;
pub(in crate::sim::emit_c) use initialization::storage_lifecycle;
pub(in crate::sim::emit_c) use lifecycle::main;

pub(in crate::sim::emit_c) fn check_function(function: &IrFunc) -> Result<(), String> {
    if function.dpi.is_some() || function.receiver_class.is_some() || function.virtual_slot.is_some()
        || function.ret_string || function.ret_chandle {
        return Err(pending("DPI, class, and native-object procedures"));
    }
    if function.formals.iter().any(|formal| formal.string || formal.chandle || formal.event || formal.is_ref())
        || function.locals.iter().any(|local| local.string) {
        return Err(pending("native-object and ref formal/local owners"));
    }
    Ok(())
}

pub(in crate::sim::emit_c) fn check_model(model: &IrModel) -> Result<(), String> {
    if !model.classes.is_empty() || !model.objects.is_empty() || !model.containers.is_empty() || !model.virtual_interfaces.is_empty() {
        return Err(pending("class, container, object and virtual-interface model storage"));
    }
    if !model.assertions.is_empty() || !model.sampled_domains.is_empty() || !model.vpi_compile_calls.is_empty() {
        return Err(pending("assertion/sampling or VPI system-call callbacks"));
    }
    if model.signals.iter().any(|signal| !signal.net_alias.is_empty()) {
        return Err(pending("true-net-alias model storage"));
    }
    if model.events.iter().any(|event| event.is_array()) {
        return Err(pending("event-array descriptors"));
    }
    for function in &model.funcs { check_function(function)?; }
    Ok(())
}

pub(in crate::sim::emit_c) fn persistent_returns(model: &IrModel, out: &mut String) {
    for (index, function) in model.funcs.iter().enumerate() {
        if !function.automatic {
            if let Some(ty) = function.ret {
                out.push_str(&format!("static {} _llg_ret_{index} = {};\n",
                    if ty.width() == 0 { "double" } else { "sv4_t" },
                    if ty.width() == 0 { "0.0" } else { "SV4_EMPTY" }));
            }
        }
    }
}

pub(in crate::sim::emit_c) fn function(ctx: &RCtx<'_>, function: &IrFunc) -> Result<String, String> {
    check_function(function)?;
    let return_type = match function.ret { None => "void", Some(IrType::Real { .. }) => "double", _ => "sv4_t" };
    let mut frame = Frame::new(ctx);
    if let Some(ty) = function.ret {
        if function.automatic {
            frame.local("_ret", ty.width(), ty.signed(), ty.two_state(), None)?;
            if let Some(binding) = frame.bindings[0].get_mut("_ret") {
                binding.shortreal = matches!(ty, IrType::Real { shortreal: true });
            }
        }
        else {
            let index = ctx.model.funcs.iter().position(|candidate| candidate.c_name == function.c_name)
                .ok_or_else(|| "function is missing from its model".to_owned())?;
            frame.bindings[0].insert("_ret".to_owned(), Binding {
                address: format!("&_llg_ret_{index}"), width: ty.width(), signed: ty.signed(),
                two_state: ty.two_state(), shortreal: matches!(ty, IrType::Real { shortreal: true }), automatic: false });
        }
        frame.return_address = Some("&_ret".to_owned());
    }
    for (index, formal) in function.formals.iter().enumerate() {
        if formal.is_address() { frame.line(format!("(void)o{index};")); continue; }
        let name = format!("a{index}");
        frame.local(&name, if formal.real { 0 } else { formal.width }, formal.signed, formal.two_state, None)?;
        if let Some(binding) = frame.bindings[0].get_mut(&name) {
            binding.shortreal = formal.shortreal;
        }
        let binding = frame.lookup(&name).ok_or_else(|| "input owner was not created".to_owned())?;
        if formal.real { frame.line(format!("*({}) = {};", binding.address, round_shortreal(name, formal.shortreal))); }
        else { frame.line(format!("sv4_copy({}, &{name});", binding.address)); }
    }
    frame.block(&function.body)?;
    frame.line("goto _llg_return;");
    frame.line("_llg_return: ;");
    if let Some(ty) = function.ret {
        let binding = frame.lookup("_ret").ok_or_else(|| "return owner was not created".to_owned())?;
        if ty.width() == 0 {
            frame.line(format!("double _llg_returned = {};", round_shortreal(format!("*({})", binding.address), matches!(ty, IrType::Real { shortreal: true }))));
        } else {
            // A queued selected NBA may still retain this return cell. Clone
            // instead of emptying its payload before that NBA commits.
            frame.line(format!("sv4_t _llg_returned = sv4_clone({});", binding.address));
        }
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        frame.line("return _llg_returned;");
    } else { frame.line("llg_value_scopes_end_since(_llg_frame_base);"); frame.line("return;"); }
    let guard = if function.ret.is_some() { format!("return {};", function.ret_x()) } else { "return;".to_owned() };
    Ok(format!("static {return_type} {}({}) {{\n    if (depth >= 256) {{ fprintf(stderr, \"llg: recursion limit exceeded\\n\"); {guard} }}\n{}{}\n}}\n",
        function.c_name, super::super::model::owned_func_params(function), frame.prologue(), frame.body()))
}

pub(in crate::sim::emit_c) fn process(ctx: &RCtx<'_>, process: &IrProcess, execution: &ExecutionProcess) -> Result<String, String> {
    let mut frame = Frame::new(ctx);
    let label = |block| format!("_llg_exec_{}_b{block}", execution.semantic_process);
    frame.line(format!("goto {};", label(execution.entry)));
    for (index, block) in execution.blocks.iter().enumerate() {
        frame.line(format!("{}: ;", label(index)));
        frame.begin_block(&block.operations);
        for statement in &block.operations { frame.statement(statement)?; }
        if let ExecutionTerminator::Suspend { trigger: TriggerPlan::Signals(reads), region, .. } = &block.terminator {
            frame.wait_any(reads, Some(*region))?;
        }
        frame.end_block();
        match &block.terminator {
            ExecutionTerminator::Complete => frame.line("goto _llg_return;"),
            ExecutionTerminator::Jump { target } | ExecutionTerminator::Suspend { resume: target, .. } => {
                if *target <= index { frame.line(format!("llg_budget_point({});", c_string_literal(process.label()))); }
                frame.line(format!("goto {};", label(*target)));
            }
        }
    }
    frame.line("goto _llg_return;");
    frame.line("_llg_return: ;");
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    frame.line("llg_proc_done(self);");
    frame.line("return;");
    Ok(format!("static void {}(llg_proc_t* self) {{\n{}{}\n}}\n", process.c_name, frame.prologue(), frame.body()))
}

pub(in crate::sim::emit_c) fn pre_function(ctx: &RCtx<'_>, pre: &IrPreFn) -> Result<String, String> {
    let IrPreFn::Branch { c_name, body } = pre else {
        return Err(pending("captured branches and deferred/evaluated callbacks"));
    };
    let mut frame = Frame::new(ctx);
    frame.block(body)?;
    frame.line("goto _llg_return;");
    frame.line("_llg_return: ;");
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    frame.line("llg_proc_done(self);");
    frame.line("return;");
    Ok(format!("static void {c_name}(llg_proc_t* self) {{\n{}{}\n}}\n", frame.prologue(), frame.body()))
}
