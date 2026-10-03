//! Ownership-safe procedures and model lifetime boundaries.
use super::native::{NativeBinding, NativeKind};
use super::*;
use crate::sim::execution::{
    CoroutineId, ExecutionAnalysis, ExecutionModel, ExecutionProcess, ExecutionTerminator,
    TriggerPlan,
};
use std::collections::BTreeMap;

type ProcessBody = (
    String,
    Option<super::super::frame_layout::FrameLayout>,
    Vec<super::super::statements::pca_batches::Batch>,
    Vec<super::net_batches::NetBatch>,
);
type CoroutineProcessBody = (
    String,
    super::super::frame_layout::FrameLayout,
    Vec<super::super::statements::pca_batches::Batch>,
    Vec<super::net_batches::NetBatch>,
);

mod callbacks;
mod initialization;
mod lifecycle;
pub(in crate::sim::emit_c) use initialization::storage_lifecycle;
pub(in crate::sim::emit_c) use lifecycle::main;

/// A retained definition that lowering expands into each caller. Only
/// by-value input event formals have a typed C parameter (`llg_event_t`);
/// output, inout and ref event formals stay inline-only.
pub(in crate::sim::emit_c) fn inline_template(function: &IrFunc) -> bool {
    function.is_inline_expanded()
}

pub(in crate::sim::emit_c) fn check_function(function: &IrFunc) -> Result<(), String> {
    if function
        .formals
        .iter()
        .any(|formal| formal.event && formal.is_address())
    {
        return Err(pending("native-object and ref formal/local owners"));
    }
    Ok(())
}

pub(in crate::sim::emit_c) fn check_model(model: &IrModel) -> Result<(), String> {
    for function in &model.funcs {
        if !inline_template(function) {
            check_function(function)?;
        }
    }
    for interface in &model.virtual_interfaces {
        for method in &interface.methods {
            let function = model.func(method.function);
            if function.formals.iter().any(|formal| formal.event) {
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
            if let Some(ty) = function.ret.filter(|_| function.return_signal.is_none()) {
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
    render_function(ctx, function, Frame::new(ctx), false).map(|(source, _)| source)
}

pub(in crate::sim::emit_c) fn coroutine_function(
    ctx: &RCtx<'_>,
    function: &IrFunc,
    function_index: usize,
    analysis: &ExecutionAnalysis,
    frame_upper_bounds: &BTreeMap<usize, usize>,
) -> Result<(String, super::super::frame_layout::FrameLayout), String> {
    let mut frame = Frame::new_coframe(
        ctx,
        analysis,
        CoroutineId::Function(function_index),
        frame_upper_bounds,
    )?;
    for (ty, name) in super::super::model::owned_func_param_fields(function) {
        frame.frame_field(&ty, &name)?;
    }
    let (source, layout) = render_function(ctx, function, frame, true)?;
    Ok((
        source,
        layout.ok_or_else(|| "coroutine function has no frame layout".to_owned())?,
    ))
}

fn render_function(
    ctx: &RCtx<'_>,
    function: &IrFunc,
    mut frame: Frame<'_, '_>,
    coroutine: bool,
) -> Result<(String, Option<super::super::frame_layout::FrameLayout>), String> {
    check_function(function)?;
    frame.cell_eligibility = frame_cells::CellEligibility::analyze(ctx, &function.body);
    if function.dpi.is_some() {
        if coroutine {
            return Err("a DPI subprogram cannot be a coroutine".to_owned());
        }
        return Ok((super::super::model::owned_dpi_thunk(function)?, None));
    }
    let return_type = if coroutine {
        "llg_co_status_t"
    } else if function.ret_string {
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
            let address = function.return_signal.map_or_else(
                || format!("&_llg_ret_{index}"),
                |signal| format!("&{}", ctx.model.signal(signal).c_name),
            );
            frame.bindings[0].insert(
                "_ret".to_owned(),
                Binding {
                    address,
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
        if let Some(array) = formal.fixed_array {
            let parameter = format!("{}{index}", if formal.is_ref() { "r" } else if formal.is_out { "o" } else { "a" });
            if formal.is_ref() || ctx.model.array(array).activation {
                frame.fixed_arrays.insert(array, parameter);
            } else if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                let address = frame.fixed_array_address(array)?;
                frame.line(format!("llg_fixed_array_copy({address}, {parameter}, {}, 0);", u8::from(ctx.model.array(array).two_state)));
            }
            continue;
        }

        if formal.is_address() {
            frame.line(format!(
                "(void){}{index};",
                if formal.is_ref() { "r" } else { "o" }
            ));
            continue;
        }
        let name = format!("a{index}");
        if formal.event {
            // A private handle copy: assigning the formal rebinds only this
            // activation, exactly like a by-value input.
            let handle = frame.declare("llg_event_t", "event_formal", name);
            frame
                .event_bindings
                .first_mut()
                .expect("event scope")
                .insert(
                    super::events::event_formal_binding(index),
                    format!("&{handle}"),
                );
            continue;
        }
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
    for (index, formal) in function.formals.iter().enumerate() {
        if let Some(array) = formal.fixed_array.filter(|array| !ctx.model.array(*array).activation && formal.is_out) {
            let address = frame.fixed_array_address(array)?;
            frame.line(format!("llg_fixed_array_copy(o{index}, {address}, {}, 0);", u8::from(ctx.model.array(array).two_state)));
        }
    }

    if coroutine {
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        frame.poison_completed_frame();
        frame.line("return LLG_CO_DONE;");
    } else if function.ret_string || function.ret_chandle {
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
    let guard = if coroutine {
        "return LLG_CO_DONE;".to_owned()
    } else if function.ret_string {
        "return (llg_string_t){0};".to_owned()
    } else if function.ret_chandle {
        "return NULL;".to_owned()
    } else if function.ret.is_some() {
        format!("return {};", function.ret_x())
    } else {
        "return;".to_owned()
    };
    let signature = if coroutine {
        "llg_co_frame_t* co, llg_co_chain_t* ch".to_owned()
    } else {
        super::super::model::owned_func_params(function)
    };
    let prologue = frame.prologue();
    let dispatch = frame.dispatch();
    let macro_epilogue = frame.macro_epilogue();
    let (body, layout, cached_locals) = if coroutine {
        let finished = frame.into_coframe()?;
        (finished.body, Some(finished.layout), finished.cached_locals)
    } else {
        (frame.body().to_owned(), None, String::new())
    };
    let depth = if coroutine { "F->depth" } else { "depth" };
    let coroutine_prologue = if coroutine {
        format!(
            "    {}_frame_t* F = ({}_frame_t*)co;\n{cached_locals}{dispatch}",
            function.c_name, function.c_name
        )
    } else {
        String::new()
    };
    let source = format!("static {return_type} {}({signature}) {{\n{coroutine_prologue}    if ({depth} >= 256) {{ fprintf(stderr, \"llg: recursion limit exceeded\\n\"); {guard} }}\n{prologue}{body}\n}}\n{macro_epilogue}",
        function.c_name);
    Ok((source, layout))
}

pub(in crate::sim::emit_c) fn coroutine_process(
    ctx: &RCtx<'_>,
    process: &IrProcess,
    process_index: usize,
    execution: &ExecutionProcess,
    analysis: &ExecutionAnalysis,
    frame_upper_bounds: &BTreeMap<usize, usize>,
) -> Result<CoroutineProcessBody, String> {
    let mut frame = Frame::new_coframe(
        ctx,
        analysis,
        CoroutineId::Process(process_index),
        frame_upper_bounds,
    )?;
    frame.pca_owner = Some(process.c_name.clone());
    let (source, layout, batches, net_batches) =
        render_process(ctx, process, execution, frame, true)?;
    Ok((
        source,
        layout.ok_or_else(|| "coroutine process has no frame layout".to_owned())?,
        batches,
        net_batches,
    ))
}

pub(in crate::sim::emit_c) fn process(
    ctx: &RCtx<'_>,
    process: &IrProcess,
    execution: &ExecutionProcess,
) -> Result<String, String> {
    render_process(ctx, process, execution, Frame::new(ctx), false).map(|(source, _, _, _)| source)
}

fn render_process(
    ctx: &RCtx<'_>,
    process: &IrProcess,
    execution: &ExecutionProcess,
    mut frame: Frame<'_, '_>,
    coroutine: bool,
) -> Result<ProcessBody, String> {
    frame.cell_eligibility = frame_cells::CellEligibility::analyze(
        ctx,
        execution.blocks.iter().flat_map(|block| &block.operations),
    );
    for block in &execution.blocks {
        if let ExecutionTerminator::Suspend {
            trigger: TriggerPlan::Signals(reads),
            ..
        } = &block.terminator
        {
            frame.cell_eligibility.exclude_dependencies(reads);
        }
    }
    let label = |block| format!("_llg_exec_{}_b{block}", execution.semantic_process);
    frame.line(format!("goto {};", label(execution.entry)));
    for (index, block) in execution.blocks.iter().enumerate() {
        frame.line(format!("{}: ;", label(index)));
        frame.begin_block(&block.operations);
        frame.statements(&block.operations)?;
        if let ExecutionTerminator::Suspend {
            trigger: TriggerPlan::Signals(reads),
            region,
            ..
        } = &block.terminator
        {
            frame.wait_any(
                reads,
                Some(*region),
                crate::sim::execution::SuspensionOperation::ProcessTrigger,
            )?;
        }
        frame.end_block();
        match &block.terminator {
            ExecutionTerminator::Complete => frame.line("goto _llg_return;"),
            ExecutionTerminator::Jump { target }
            | ExecutionTerminator::Suspend { resume: target, .. } => {
                if *target <= index {
                    frame.line(format!(
                        "if (LLG_CO_UNLIKELY(llg_budget_point({}))) {}",
                        c_string_literal(process.label()),
                        if coroutine {
                            "return LLG_CO_EXIT;"
                        } else {
                            "goto _llg_return;"
                        }
                    ));
                }
                frame.line(format!("goto {};", label(*target)));
            }
        }
    }
    frame.line("goto _llg_return;");
    frame.line("_llg_return: ;");
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    if coroutine {
        frame.poison_completed_frame();
        frame.line("return LLG_CO_DONE;");
    } else {
        frame.line("return;");
    }
    let prologue = frame.prologue();
    let dispatch = frame.dispatch();
    let macro_epilogue = frame.macro_epilogue();
    let batches = std::mem::take(&mut frame.pca_batches);
    let net_batches = std::mem::take(&mut frame.net_batches);
    let (body, layout, cached_locals) = if coroutine {
        let finished = frame.into_coframe()?;
        (finished.body, Some(finished.layout), finished.cached_locals)
    } else {
        (frame.body().to_owned(), None, String::new())
    };
    let frame_pointer = if coroutine {
        format!(
            "    {}_frame_t* F = ({}_frame_t*)co;\n{cached_locals}{dispatch}",
            process.c_name, process.c_name
        )
    } else {
        String::new()
    };
    let source = format!(
        "static {} {}({}) {{\n{frame_pointer}{prologue}{body}\n}}\n{macro_epilogue}",
        if coroutine { "llg_co_status_t" } else { "void" },
        process.c_name,
        if coroutine {
            "llg_co_frame_t* co, llg_co_chain_t* ch"
        } else {
            "void"
        },
    );
    Ok((source, layout, batches, net_batches))
}

pub(in crate::sim::emit_c) fn pre_function(
    ctx: &RCtx<'_>,
    pre: &IrPreFn,
) -> Result<String, String> {
    callbacks::render(ctx, pre)
}

pub(in crate::sim::emit_c) fn coroutine_branch(
    ctx: &RCtx<'_>,
    pre: &IrPreFn,
    owner: CoroutineId,
    analysis: &ExecutionAnalysis,
    frame_upper_bounds: &BTreeMap<usize, usize>,
) -> Result<(String, super::super::frame_layout::FrameLayout), String> {
    callbacks::coroutine_branch(ctx, pre, owner, analysis, frame_upper_bounds)
}
