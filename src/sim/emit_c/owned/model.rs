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
    Vec<super::net_batches::NetBatch>,
);
type CoroutineProcessBody = (
    String,
    super::super::frame_layout::FrameLayout,
    Vec<super::net_batches::NetBatch>,
);

mod callbacks;
mod initialization;
mod lifecycle;
pub(in crate::sim::emit_c) use initialization::storage_lifecycle;
pub(in crate::sim::emit_c) use lifecycle::main;

/// A retained definition that lowering expands into each caller. Input event
/// formals are typed `llg_event_t` parameters and output, inout and ref event
/// formals `llg_event_t*` handle addresses.
pub(in crate::sim::emit_c) fn inline_template(function: &IrFunc) -> bool {
    function.is_inline_expanded()
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
    render_function(ctx, function, Frame::new(ctx), false).map(|(source, _, _)| source)
}

/// A recursive subprogram's coroutine `<fn>_co` followed by its plain-ABI
/// entry `<fn>`, which runs the coroutine on a synchronous driver. Returns
/// the source, the frame layout and the number of resume points.
pub(in crate::sim::emit_c) fn recursive_function(
    ctx: &RCtx<'_>,
    function: &IrFunc,
    function_index: usize,
    analysis: &ExecutionAnalysis,
) -> Result<(String, super::super::frame_layout::FrameLayout, usize), String> {
    let mut frame = Frame::new_recursive(ctx, analysis, function_index)?;
    for (ty, name) in super::super::model::owned_frame_param_fields(function) {
        frame.frame_field(&ty, &name)?;
    }
    if let Some((ty, name)) = super::super::model::recursive_result_field(function) {
        frame.frame_field(&ty, &name)?;
    }
    let (mut source, layout, sites) = render_function(ctx, function, frame, true)?;
    source.push_str(&recursive_entry(function));
    Ok((
        source,
        layout.ok_or_else(|| "recursive function has no frame layout".to_owned())?,
        sites,
    ))
}

/// Plain-ABI entry of a recursive subprogram: copy the parameters into a
/// fresh arena frame and run the coroutine to completion. Every caller
/// outside the subprogram's component, including dispatch and DPI export
/// wrappers, keeps calling this unchanged signature.
fn recursive_entry(function: &IrFunc) -> String {
    let name = &function.c_name;
    let frame_type = format!("{name}_co_frame_t");
    let return_type = super::super::model::owned_function_return_type(function);
    let mut out = format!(
        "static {return_type} {name}({}) {{\n    llg_co_sync_t _llg_sync;\n    {frame_type}* _llg_callee = ({frame_type}*)llg_co_sync_begin(&_llg_sync, &{name}_co_desc, NULL);\n",
        super::super::model::owned_func_params(function)
    );
    for (ty, field) in super::super::model::owned_frame_param_fields(function) {
        let value = if ty == "sv4_t" {
            format!("*{field}")
        } else {
            field.clone()
        };
        out.push_str(&format!("    _llg_callee->{field} = {value};\n"));
    }
    let returned = match super::super::model::recursive_result_field(function) {
        Some(_) if super::super::model::owned_packed_result(function) => {
            out.push_str("    _llg_callee->_llg_result = _llg_result;\n");
            false
        }
        Some(_) => {
            let initial = if function.ret_string {
                "{0}"
            } else if function.ret_chandle {
                "NULL"
            } else {
                "0.0"
            };
            out.push_str(&format!(
                "    {return_type} _llg_returned = {initial};\n    _llg_callee->_llg_result = &_llg_returned;\n"
            ));
            true
        }
        None => false,
    };
    out.push_str("    (void)llg_co_sync_run(&_llg_sync);\n");
    if returned {
        out.push_str("    return _llg_returned;\n");
    }
    out.push_str("}\n");
    out
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
    for (ty, name) in super::super::model::owned_frame_param_fields(function) {
        frame.frame_field(&ty, &name)?;
    }
    let (source, layout, _) = render_function(ctx, function, frame, true)?;
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
) -> Result<
    (
        String,
        Option<super::super::frame_layout::FrameLayout>,
        usize,
    ),
    String,
> {
    // A recursive subprogram's coroutine returns its result through the
    // caller's `_llg_result` destination (see `recursive_function`).
    let synchronous = coroutine && frame.synchronous;
    frame.cell_eligibility = frame_cells::CellEligibility::analyze(ctx, &function.body);
    if function.dpi.is_some() {
        if coroutine {
            return Err("a DPI subprogram cannot be a coroutine".to_owned());
        }
        return Ok((super::super::model::owned_dpi_thunk(function)?, None, 0));
    }
    // Non-coroutine packed results go to the caller's `_llg_result`.
    let packed_result = !coroutine && super::super::model::owned_packed_result(function);
    let return_type = if coroutine {
        "llg_co_status_t"
    } else if function.ret_string {
        "llg_string_t"
    } else if function.ret_chandle {
        "void*"
    } else {
        match function.ret {
            Some(IrType::Real { .. }) => "double",
            _ => "void",
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
        if let Some(container) = formal.container {
            let parameter = format!("{}{index}", if formal.is_address() { "o" } else { "a" });
            let storage = &ctx.model.containers[container];
            let (ty, _, _) = super::super::containers::activation_storage(storage, "")?;
            let bound = format!("(*({ty}*){parameter})");
            if storage.activation && formal.shared_local.is_some() {
                // Shared with fork branches: a copy in a function-scope
                // shared frame, copied back to an output at return.
                let owner = frame.scalar(
                    "llg_frame_t**",
                    "(llg_frame_t**)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_frame_t*), llg_owned_frame_drop))"
                        .to_owned(),
                );
                frame.line(format!("*{owner} = llg_frame_new(3ULL);"));
                let target = frame.new_container_in(container, Some((&format!("*{owner}"), 0)))?;
                frame.line(format!(
                    "{}(&{target}, &{bound});",
                    super::containers::copy_function(storage)
                ));
                frame.containers.insert(container, target);
                frame.shared_cells.insert(
                    crate::sim::ir::shared_container_capture_name(container),
                    (format!("(*{owner})"), 0),
                );
            } else if storage.activation {
                frame.containers.insert(container, bound);
            } else if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                // Static subroutine storage keeps its own container; the
                // caller's fresh copy is consumed on entry.
                frame.line(format!(
                    "{}(&{}, &{bound});",
                    super::containers::copy_function(storage),
                    storage.c_name
                ));
            }
            continue;
        }
        if let Some(value) = formal.native_value {
            let parameter = format!("{}{index}", if formal.is_address() { "o" } else { "a" });
            // Companion containers bind like container formals (SIM-007).
            for (position, container) in formal.native_companions.iter().enumerate() {
                let storage = &ctx.model.containers[*container];
                let (ty, _, _) = super::super::containers::activation_storage(storage, "")?;
                let bound = format!(
                    "(*({ty}*){})",
                    super::super::model::native_companion_param(&parameter, position)
                );
                if storage.activation {
                    frame.containers.insert(*container, bound);
                } else if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                    frame.line(format!(
                        "{}(&{}, &{bound});",
                        super::containers::copy_function(storage),
                        storage.c_name
                    ));
                }
            }
            if ctx.model.native_values[value].activation {
                frame.native_values.insert(value, parameter);
            } else if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                let address = frame.native_value_address(value)?;
                frame.line(format!("llg_native_value_copy({address}, {parameter});"));
            }
            continue;
        }
        if let Some(array) = formal.real_array {
            let parameter = format!(
                "{}{index}",
                if formal.is_ref() {
                    "r"
                } else if formal.is_out {
                    "o"
                } else {
                    "a"
                }
            );
            if formal.is_ref() || ctx.model.array(array).activation {
                frame.fixed_arrays.insert(array, parameter);
            } else if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                // Static subroutine storage keeps its own cells; the caller's
                // fresh copy is consumed on entry.
                let total = ctx.model.array(array).total;
                frame.line(format!(
                    "memcpy({}, {parameter}, sizeof(double) * {total}ULL);",
                    ctx.model.array(array).c_name
                ));
            }
            continue;
        }
        if let Some(array) = formal.fixed_array {
            let parameter = format!(
                "{}{index}",
                if formal.is_ref() {
                    "r"
                } else if formal.is_out {
                    "o"
                } else {
                    "a"
                }
            );
            if formal.is_ref() || ctx.model.array(array).activation {
                frame.fixed_arrays.insert(array, parameter);
            } else if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                let address = frame.fixed_array_address(array)?;
                frame.line(format!(
                    "llg_fixed_array_copy({address}, {parameter}, {}, 0);",
                    u8::from(ctx.model.array(array).two_state)
                ));
            }
            continue;
        }

        if formal.event && formal.is_address() {
            // A `ref` event formal is the caller's handle; an output or inout
            // works on its own handle, assigned back at return.
            let handle = if formal.is_ref() {
                format!("r{index}")
            } else {
                let initial = if formal.mode == IrFormalMode::Inout {
                    format!("o{index} ? o{index}->object : NULL")
                } else {
                    "NULL".to_owned()
                };
                let local =
                    frame.declare("llg_event_t", "event_formal", format!("{{ {initial} }}"));
                format!("&{local}")
            };
            frame
                .event_bindings
                .first_mut()
                .expect("event scope")
                .insert(super::events::event_formal_binding(index), handle);
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
        } else if coroutine {
            frame.line(format!("sv4_copy({}, &{name});", binding.address));
        } else {
            // Plain functions borrow packed inputs by address.
            frame.line(format!("sv4_copy({}, {name});", binding.address));
        }
    }
    // Shared output cells are owned by the function scope, so they outlive
    // the body's scopes until the copy-back below.
    let mut shared_outputs = HashMap::new();
    for (index, formal) in function.formals.iter().enumerate() {
        if let Some(local) = formal.shared_local.as_ref().filter(|_| formal.string) {
            frame.statement(&IrStmt::SharedString {
                name: local.clone(),
                init: Some(IrStringExpr::FormalRead(index)),
            })?;
            continue;
        }
        if let Some(local) = formal
            .shared_local
            .as_ref()
            .filter(|_| formal.container.is_none())
        {
            let width = if formal.real { 0 } else { formal.width };
            let initial = IrExpr::new(IrExprKind::FormalRead(index), width, formal.signed, None);
            frame.shared_local(
                local,
                width,
                formal.signed,
                formal.two_state,
                Some(&initial),
            )?;
            let address = frame
                .lookup(local)
                .ok_or("shared output formal has no cell")?
                .address;
            shared_outputs.insert(index, address);
        }
    }
    if function.receiver_class.is_some() {
        // A method entered with a null receiver (reported above) returns at
        // once, like a `return`, rather than run on a null object.
        frame.line("if (!_this) goto _llg_return;");
    }
    frame.block(&function.body)?;
    frame.line("goto _llg_return;");
    frame.line("_llg_return: ;");
    for (index, formal) in function.formals.iter().enumerate() {
        if let Some(container) = formal
            .container
            .filter(|_| formal.shared_local.is_some() && formal.is_out)
        {
            let storage = &ctx.model.containers[container];
            let (ty, _, _) = super::super::containers::activation_storage(storage, "")?;
            let target = frame.container_name(container)?;
            frame.line(format!(
                "{}(({ty}*)o{index}, &{target});",
                super::containers::copy_function(storage)
            ));
        }
        if let Some(local) = formal.shared_local.as_ref().filter(|_| formal.string) {
            frame.string_assign(
                &format!("o{index}"),
                &IrStringExpr::LocalRead(local.clone()),
            )?;
        }
        if let Some(cell) = shared_outputs.get(&index) {
            frame.line(if formal.real {
                format!("*o{index} = *{cell};")
            } else {
                format!("sv4_copy(o{index}, {cell});")
            });
        }
        if formal.event && formal.is_out {
            let handle = frame
                .event_bindings
                .first()
                .and_then(|scope| scope.get(&super::events::event_formal_binding(index)))
                .cloned()
                .ok_or("event output formal has no handle")?;
            frame.line(format!(
                "if (o{index}) llg_event_assign(o{index}, {handle});"
            ));
        }
        if let Some(container) = formal
            .container
            .filter(|container| !ctx.model.containers[*container].activation && formal.is_out)
        {
            let storage = &ctx.model.containers[container];
            let (ty, _, _) = super::super::containers::activation_storage(storage, "")?;
            frame.line(format!(
                "{}(({ty}*)o{index}, &{});",
                super::containers::copy_function(storage),
                storage.c_name
            ));
        }
        if let Some(value) = formal
            .native_value
            .filter(|value| !ctx.model.native_values[*value].activation && formal.is_out)
        {
            let address = frame.native_value_address(value)?;
            frame.line(format!("llg_native_value_copy(o{index}, {address});"));
            for (position, container) in formal.native_companions.iter().enumerate() {
                let storage = &ctx.model.containers[*container];
                let (ty, _, _) = super::super::containers::activation_storage(storage, "")?;
                frame.line(format!(
                    "{}(({ty}*){}, &{});",
                    super::containers::copy_function(storage),
                    super::super::model::native_companion_param(&format!("o{index}"), position),
                    storage.c_name
                ));
            }
        }
        if let Some(array) = formal
            .real_array
            .filter(|array| !ctx.model.array(*array).activation && formal.is_out)
        {
            let total = ctx.model.array(array).total;
            frame.line(format!(
                "memcpy(o{index}, {}, sizeof(double) * {total}ULL);",
                ctx.model.array(array).c_name
            ));
        }
        if let Some(array) = formal
            .fixed_array
            .filter(|array| !ctx.model.array(*array).activation && formal.is_out)
        {
            let address = frame.fixed_array_address(array)?;
            frame.line(format!(
                "llg_fixed_array_copy(o{index}, {address}, {}, 0);",
                u8::from(ctx.model.array(array).two_state)
            ));
        }
    }

    if coroutine {
        if synchronous {
            if function.ret_string || function.ret_chandle {
                let kind = if function.ret_string {
                    NativeKind::String
                } else {
                    NativeKind::Chandle
                };
                let binding = frame.native_lookup("_ret", kind)?;
                frame.line(if function.ret_string {
                    format!("*(_llg_result) = llg_string_clone({});", binding.address)
                } else {
                    format!("*(_llg_result) = *({});", binding.address)
                });
            } else if let Some(ty) = function.ret {
                let binding = frame
                    .lookup("_ret")
                    .ok_or_else(|| "return owner was not created".to_owned())?;
                if ty.width() == 0 {
                    frame.line(format!(
                        "*(_llg_result) = {};",
                        round_shortreal(
                            format!("*({})", binding.address),
                            matches!(ty, IrType::Real { shortreal: true })
                        )
                    ));
                } else {
                    // As for a plain return: copy, a queued NBA may retain the cell.
                    frame.line(format!("sv4_copy(_llg_result, {});", binding.address));
                }
            }
        }
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
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            frame.line("return _llg_returned;");
        } else {
            // A queued selected NBA may still retain this return cell. Copy
            // instead of moving its payload before that NBA commits.
            frame.line(format!("sv4_copy(_llg_result, {});", binding.address));
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            frame.line("return;");
        }
    } else {
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        frame.line("return;");
    }
    let guard = if synchronous && super::super::model::owned_packed_result(function) {
        format!(
            "{} return LLG_CO_DONE;",
            super::super::destinations::assign("F->_llg_result", &function.ret_x())
        )
    } else if synchronous && !function.ret_string && !function.ret_chandle && function.ret.is_some()
    {
        format!(
            "*F->_llg_result = {}; return LLG_CO_DONE;",
            function.ret_x()
        )
    } else if coroutine {
        "return LLG_CO_DONE;".to_owned()
    } else if function.ret_string {
        "return (llg_string_t){0};".to_owned()
    } else if function.ret_chandle {
        "return NULL;".to_owned()
    } else if packed_result {
        format!(
            "{} return;",
            super::super::destinations::assign("_llg_result", &function.ret_x())
        )
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
    let sites = frame.recursive_sites();
    let (body, layout, cached_locals) = if coroutine {
        let finished = frame.into_coframe()?;
        (finished.body, Some(finished.layout), finished.cached_locals)
    } else {
        (frame.body().to_owned(), None, String::new())
    };
    let depth = if coroutine { "F->depth" } else { "depth" };
    let name = if synchronous {
        format!("{}_co", function.c_name)
    } else {
        function.c_name.clone()
    };
    let coroutine_prologue = if coroutine {
        format!("    {name}_frame_t* F = ({name}_frame_t*)co;\n{cached_locals}{dispatch}")
    } else {
        String::new()
    };
    let source = format!("static {return_type} {name}({signature}) {{\n{coroutine_prologue}    if ({depth} >= 256) {{ fprintf(stderr, \"llg: recursion limit exceeded\\n\"); {guard} }}\n{prologue}{body}\n}}\n{macro_epilogue}");
    Ok((source, layout, sites))
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
    frame.batch_owner = Some(process.c_name.clone());
    let (source, layout, net_batches) = render_process(ctx, process, execution, frame, true)?;
    Ok((
        source,
        layout.ok_or_else(|| "coroutine process has no frame layout".to_owned())?,
        net_batches,
    ))
}

pub(in crate::sim::emit_c) fn process(
    ctx: &RCtx<'_>,
    process: &IrProcess,
    execution: &ExecutionProcess,
) -> Result<String, String> {
    render_process(ctx, process, execution, Frame::new(ctx), false).map(|(source, _, _)| source)
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
    Ok((source, layout, net_batches))
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
