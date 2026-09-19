//! Ownership-safe procedures and model lifetime boundaries.
use super::native::{NativeBinding, NativeKind};
use super::*;
use crate::sim::execution::{ExecutionModel, ExecutionProcess, ExecutionTerminator, TriggerPlan};

mod callbacks;
mod initialization;
mod lifecycle;
pub(in crate::sim::emit_c) use initialization::storage_lifecycle;
pub(in crate::sim::emit_c) use lifecycle::main;

/// Named-event formals are expanded by typed inline lowering. Their retained
/// definitions are templates, not procedures using the numeric C ABI.
pub(in crate::sim::emit_c) fn inline_event_template(function: &IrFunc) -> bool {
    function.formals.iter().any(|formal| formal.event)
}

pub(in crate::sim::emit_c) fn check_function(function: &IrFunc) -> Result<(), String> {
    if function.formals.iter().any(|formal| formal.event) {
        return Err(pending("native-object and ref formal/local owners"));
    }
    Ok(())
}

pub(in crate::sim::emit_c) fn check_model(model: &IrModel) -> Result<(), String> {
    for function in &model.funcs {
        if !inline_event_template(function) {
            check_function(function)?;
        }
    }
    for interface in &model.virtual_interfaces {
        for method in &interface.methods {
            let function = model.func(method.function);
            if inline_event_template(function) {
                return Err(pending("event-formal virtual-interface dispatch"));
            }
        }
    }
    Ok(())
}

pub(in crate::sim::emit_c) fn persistent_returns(model: &IrModel, out: &mut String) {
    for (index, function) in model.funcs.iter().enumerate() {
        if !function.automatic {
            if function.ret_string || function.ret_chandle {
                out.push_str(&format!(
                    "static {} _llg_native_ret_{index} = {{0}};\n",
                    if function.ret_string {
                        "llg_string_t"
                    } else {
                        "void*"
                    }
                ));
            }
            if let Some(ty) = function.ret {
                out.push_str(&format!(
                    "static {} _llg_ret_{index} = {};\n",
                    if ty.width() == 0 { "double" } else { "sv4_t" },
                    if ty.width() == 0 { "0.0" } else { "SV4_EMPTY" }
                ));
            }
        }
    }
}

pub(in crate::sim::emit_c) fn function(
    ctx: &RCtx<'_>,
    function: &IrFunc,
) -> Result<String, String> {
    check_function(function)?;
    if function.dpi.is_some() {
        return super::super::model::owned_dpi_thunk(function);
    }
    let return_type = if function.ret_string {
        "llg_string_t"
    } else if function.ret_chandle {
        "void*"
    } else {
        match function.ret {
            None => "void",
            Some(IrType::Real { .. }) => "double",
            _ => "sv4_t",
        }
    };
    let mut frame = Frame::new(ctx);
    frame.cancellation_return = true;
    if function.receiver_class.is_some() {
        frame.line("(void)llg_class_require(_this, \"method call\");");
    }
    if function.ret_string || function.ret_chandle {
        let kind = if function.ret_string {
            NativeKind::String
        } else {
            NativeKind::Chandle
        };
        if function.automatic {
            frame.native_local("_ret", kind);
        } else {
            let index = ctx
                .model
                .funcs
                .iter()
                .position(|candidate| candidate.c_name == function.c_name)
                .ok_or_else(|| "native function is missing from model".to_owned())?;
            frame.native_bindings[0].insert(
                "_ret".to_owned(),
                NativeBinding {
                    address: format!("&_llg_native_ret_{index}"),
                    kind,
                    automatic: false,
                },
            );
        }
    }
    if let Some(ty) = function.ret {
        if function.automatic {
            let default = function.return_default.as_ref().map(|value| {
                IrExpr::new(
                    IrExprKind::Const(value.clone()),
                    value.width,
                    value.signed,
                    None,
                )
            });
            frame.local(
                "_ret",
                ty.width(),
                ty.signed(),
                ty.two_state(),
                default.as_ref(),
            )?;
            if let Some(binding) = frame.bindings[0].get_mut("_ret") {
                binding.shortreal = matches!(ty, IrType::Real { shortreal: true });
            }
        } else {
            let index = ctx
                .model
                .funcs
                .iter()
                .position(|candidate| candidate.c_name == function.c_name)
                .ok_or_else(|| "function is missing from its model".to_owned())?;
            frame.bindings[0].insert(
                "_ret".to_owned(),
                Binding {
                    address: format!("&_llg_ret_{index}"),
                    width: ty.width(),
                    signed: ty.signed(),
                    two_state: ty.two_state(),
                    shortreal: matches!(ty, IrType::Real { shortreal: true }),
                    automatic: false,
                },
            );
        }
        frame.return_address = Some("&_ret".to_owned());
    }
    for (index, formal) in function.formals.iter().enumerate() {
        if formal.is_address() {
            frame.line(format!(
                "(void){}{index};",
                if formal.is_ref() { "r" } else { "o" }
            ));
            continue;
        }
        let name = format!("a{index}");
        if formal.string || formal.chandle {
            let kind = if formal.string {
                NativeKind::String
            } else {
                NativeKind::Chandle
            };
            let binding = frame.native_local(&name, kind);
            let initial = if formal.string {
                format!("llg_string_clone(&a{index})")
            } else {
                format!("a{index}")
            };
            frame.line(format!("*({}) = {initial};", binding.address));
            continue;
        }
        frame.local(
            &name,
            if formal.real { 0 } else { formal.width },
            formal.signed,
            formal.two_state,
            None,
        )?;
        if let Some(binding) = frame.bindings[0].get_mut(&name) {
            binding.shortreal = formal.shortreal;
        }
        let binding = frame
            .lookup(&name)
            .ok_or_else(|| "input owner was not created".to_owned())?;
        if formal.real {
            frame.line(format!(
                "*({}) = {};",
                binding.address,
                round_shortreal(name, formal.shortreal)
            ));
        } else {
            frame.line(format!("sv4_copy({}, &{name});", binding.address));
        }
    }
    frame.block(&function.body)?;
    frame.line("goto _llg_return;");
    frame.line("_llg_return: ;");
    if function.ret_string || function.ret_chandle {
        let kind = if function.ret_string {
            NativeKind::String
        } else {
            NativeKind::Chandle
        };
        let binding = frame.native_lookup("_ret", kind)?;
        let value = if function.ret_string {
            format!("llg_string_clone({})", binding.address)
        } else {
            format!("*({})", binding.address)
        };
        frame.line(format!("{return_type} _llg_returned = {value};"));
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        frame.line("return _llg_returned;");
    } else if let Some(ty) = function.ret {
        let binding = frame
            .lookup("_ret")
            .ok_or_else(|| "return owner was not created".to_owned())?;
        if ty.width() == 0 {
            frame.line(format!(
                "double _llg_returned = {};",
                round_shortreal(
                    format!("*({})", binding.address),
                    matches!(ty, IrType::Real { shortreal: true })
                )
            ));
        } else {
            // A queued selected NBA may still retain this return cell. Clone
            // instead of emptying its payload before that NBA commits.
            frame.line(format!(
                "sv4_t _llg_returned = sv4_clone({});",
                binding.address
            ));
        }
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        frame.line("return _llg_returned;");
    } else {
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        frame.line("return;");
    }
    let guard = if function.ret_string {
        "return (llg_string_t){0};".to_owned()
    } else if function.ret_chandle {
        "return NULL;".to_owned()
    } else if function.ret.is_some() {
        format!("return {};", function.ret_x())
    } else {
        "return;".to_owned()
    };
    Ok(format!("static {return_type} {}({}) {{\n    if (depth >= 256) {{ fprintf(stderr, \"llg: recursion limit exceeded\\n\"); {guard} }}\n{}{}\n}}\n",
        function.c_name, super::super::model::owned_func_params(function), frame.prologue(), frame.body()))
}

pub(in crate::sim::emit_c) fn process(
    ctx: &RCtx<'_>,
    process: &IrProcess,
    execution: &ExecutionProcess,
) -> Result<String, String> {
    let mut frame = Frame::new(ctx);
    let label = |block| format!("_llg_exec_{}_b{block}", execution.semantic_process);
    frame.line(format!("goto {};", label(execution.entry)));
    for (index, block) in execution.blocks.iter().enumerate() {
        frame.line(format!("{}: ;", label(index)));
        frame.begin_block(&block.operations);
        for statement in &block.operations {
            frame.statement(statement)?;
        }
        if let ExecutionTerminator::Suspend {
            trigger: TriggerPlan::Signals(reads),
            region,
            ..
        } = &block.terminator
        {
            frame.wait_any(reads, Some(*region))?;
        }
        frame.end_block();
        match &block.terminator {
            ExecutionTerminator::Complete => frame.line("goto _llg_return;"),
            ExecutionTerminator::Jump { target }
            | ExecutionTerminator::Suspend { resume: target, .. } => {
                if *target <= index {
                    frame.line(format!(
                        "llg_budget_point({});",
                        c_string_literal(process.label())
                    ));
                }
                frame.line(format!("goto {};", label(*target)));
            }
        }
    }
    frame.line("goto _llg_return;");
    frame.line("_llg_return: ;");
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    frame.line("llg_proc_done(self);");
    frame.line("return;");
    Ok(format!(
        "static void {}(llg_proc_t* self) {{\n{}{}\n}}\n",
        process.c_name,
        frame.prologue(),
        frame.body()
    ))
}

pub(in crate::sim::emit_c) fn pre_function(
    ctx: &RCtx<'_>,
    pre: &IrPreFn,
) -> Result<String, String> {
    callbacks::render(ctx, pre)
}
