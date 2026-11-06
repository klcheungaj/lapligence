//! Captured numeric branch and evaluator entry points with registered owners.
use super::*;

pub(super) fn render<'a, 'm>(
    ctx: &'a RCtx<'m>,
    pre: &IrPreFn,
    function: Option<&'m IrFunc>,
    names: &'a StorageNames<'m>,
) -> Result<String, String> {
    // A report carrying `this` renders its arguments as its method does.
    let method_ctx;
    let ctx = if matches!(
        pre,
        IrPreFn::DisplayEval { receiver: Some(receiver), .. }
            | IrPreFn::MonitorReads { receiver: Some(receiver), .. } if receiver == "_this"
    ) {
        method_ctx = RCtx {
            value_backend: ctx.value_backend,
            constants: ctx.constants,
            model: ctx.model,
            func: function,
            sampled: false,
            activation_label: None,
        };
        &method_ctx
    } else {
        ctx
    };
    let mut frame = Frame::new(ctx);
    frame.storage_names = Some(names);
    bind_branch_formals(&mut frame, pre, function)?;
    render_with_frame(ctx, pre, frame, false).map(|(source, _)| source)
}

/// A captured branch that names `ref` formals of its subroutine through
/// captured descriptors resolves them with that subroutine's formal table.
fn bind_branch_formals<'m>(
    frame: &mut Frame<'_, 'm>,
    pre: &IrPreFn,
    function: Option<&'m IrFunc>,
) -> Result<(), String> {
    let IrPreFn::CapturedBranch { captures, .. } = pre else {
        return Ok(());
    };
    if captures
        .iter()
        .any(|capture| matches!(capture.initial().kind(), IrExprKind::FormalRead(_)))
    {
        frame.branch_formals =
            Some(function.ok_or("a process fork branch cannot capture a `ref` formal")?);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn coroutine_branch<'a, 'm>(
    ctx: &'a RCtx<'m>,
    pre: &IrPreFn,
    function: Option<&'m IrFunc>,
    owner: crate::sim::execution::CoroutineId,
    analysis: &crate::sim::execution::ExecutionAnalysis,
    frame_upper_bounds: &std::collections::BTreeMap<usize, usize>,
    names: &'a StorageNames<'m>,
    outline: bool,
) -> Result<(String, super::super::super::frame_layout::FrameLayout), String> {
    let mut frame = Frame::new_coframe(ctx, analysis, owner, frame_upper_bounds)?;
    frame.storage_names = Some(names);
    bind_branch_formals(&mut frame, pre, function)?;
    if outline {
        if let IrPreFn::Branch { c_name, .. } | IrPreFn::CapturedBranch { c_name, .. } = pre {
            frame.enable_outlining(c_name, true);
        }
    }
    let (source, layout) = render_with_frame(ctx, pre, frame, true)?;
    Ok((
        source,
        layout.ok_or_else(|| "coroutine branch has no frame layout".to_owned())?,
    ))
}

fn render_with_frame(
    ctx: &RCtx<'_>,
    pre: &IrPreFn,
    mut frame: Frame<'_, '_>,
    coroutine: bool,
) -> Result<
    (
        String,
        Option<super::super::super::frame_layout::FrameLayout>,
    ),
    String,
> {
    // Helpers can be detached from the subprogram that defined them. They do
    // not inherit a C stack-local recursion-depth variable from that caller.
    if coroutine {
        let depth = frame.declare_named("int", "depth", "0".to_owned());
        frame.line(format!("(void){depth};"));
    } else {
        frame.line("int depth = 0; (void)depth;");
    }
    match pre {
        IrPreFn::Branch { c_name, body } | IrPreFn::CapturedBranch { c_name, body, .. } => {
            frame.cell_eligibility = frame_cells::CellEligibility::analyze(ctx, body);
            if let IrPreFn::CapturedBranch { captures, .. } = pre {
                // A handle `ref` formal's second capture is its change marker.
                let mut captured_formals = Vec::new();
                for capture in captures {
                    let storage = capture.storage();
                    if let IrExprKind::FormalRead(index) = capture.initial().kind() {
                        if storage.kind() == StorageKind::Opaque {
                            let marker = captured_formals.contains(index);
                            captured_formals.push(*index);
                            frame.bind_ref_formal_capture(
                                *index,
                                marker,
                                storage,
                                "llg_proc_frame(self)",
                            )?;
                            continue;
                        }
                    }
                    let name = format!("_fc{}_{}", storage.frame().index(), storage.slot());
                    frame.bind_capture(
                        &name,
                        storage,
                        capture.initial(),
                        "llg_proc_frame(self)",
                    )?;
                    if let Some(alias) = capture.alias() {
                        frame.alias_capture(alias, &name);
                    }
                }
            }
            // SV 9.7 / 9.6.2: a named block forming the whole branch has no
            // statement to resume at after `disable`, so the branch ends KILLED.
            // Statement coverage hits before it execute nothing else.
            let start = body
                .iter()
                .take_while(|statement| statement.is_statement_hit())
                .count();
            frame.terminal_activation = matches!(&body[start..], [IrStmt::ActivationScope { .. }]);
            frame.block(body)?;
            frame.terminal_activation = false;
            frame.line("goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            if coroutine {
                frame.line("return LLG_CO_DONE;");
            } else {
                frame.line("return;");
            }
            let frame_helpers = frame.outlined_helpers();
            let prologue = frame.prologue();
            let dispatch = frame.dispatch();
            let macro_epilogue = frame.macro_epilogue();
            let (body, layout, cached_locals) = if coroutine {
                let finished = frame.into_coframe()?;
                (finished.body, Some(finished.layout), finished.cached_locals)
            } else {
                (frame.body().to_owned(), None, String::new())
            };
            let frame_pointer = if coroutine {
                format!(
                    "    {c_name}_frame_t* F = ({c_name}_frame_t*)co;\n{cached_locals}{dispatch}"
                )
            } else {
                String::new()
            };
            let helpers = frame_helpers;
            let source = format!(
                "{helpers}static {} {c_name}({}) {{\n{frame_pointer}{prologue}{body}\n}}\n{macro_epilogue}",
                if coroutine { "llg_co_status_t" } else { "void" },
                if coroutine {
                    "llg_co_frame_t* co, llg_co_chain_t* ch"
                } else {
                    "llg_proc_t* self"
                }
            );
            Ok((source, layout))
        }
        IrPreFn::ForceEval {
            c_name,
            value,
            real,
        } => {
            frame.read_only_callback = true;
            // Inlined helper loops leave through `_llg_return`; keep every
            // expression temporary in a block that closes before that label.
            frame.line("{");
            let value = frame.expression(value)?;
            if *real {
                frame.line(format!("*out = {};", value.real()));
            } else {
                if value.width == 0 {
                    return Err("real value in a packed force evaluator".to_owned());
                }
                frame.line(format!("sv4_move(out, &{});", value.code));
            }
            frame.discard(value);
            frame.line("}");
            frame.line("if (0) goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            let ty = if *real { "double" } else { "sv4_t" };
            Ok((
                format!(
                    "static void {c_name}({ty}* out) {{\n{}{}\n}}\n",
                    frame.prologue(),
                    frame.body()
                ),
                None,
            ))
        }
        IrPreFn::DisplayEval {
            c_name,
            args,
            time_unit_fs,
            private_effects,
            prelude,
            receiver,
        } => {
            // Lowering proved that a private-effect helper stores only to its
            // own storage. The runtime evaluates display callbacks in a
            // private-evaluation context that applies those Postponed stores
            // without publication (SV 4.4.2.9 forbids any visible write).
            frame.read_only_callback = !*private_effects;
            frame.postponed_reader = true;
            frame.line("(void)out; (void)context;");
            bind_receiver(&mut frame, receiver.as_deref());
            frame.line("{");
            for statement in prelude {
                frame.statement(statement)?;
            }
            let values = frame.formatted_arguments(args, *time_unit_fs)?;
            // Arguments have all been evaluated before this ownership transfer;
            // no callback or user expression can abandon a partial out buffer.
            for index in 0..args.len() {
                frame.line(format!("out[{index}] = {values}[{index}];"));
                frame.line(format!("{values}[{index}] = (llg_fmt_arg_t){{0}};"));
            }
            frame.line("}");
            frame.line("if (0) goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok((
                format!(
                    "static void {c_name}(llg_fmt_arg_t* out, void* context) {{\n{}{}\n}}\n",
                    frame.prologue(),
                    frame.body()
                ),
                None,
            ))
        }
        IrPreFn::MonitorReads {
            c_name,
            reads,
            receiver,
        } => {
            frame.read_only_callback = true;
            frame.postponed_reader = true;
            frame.line("(void)out; (void)context;");
            bind_receiver(&mut frame, receiver.as_deref());
            frame.line("{");
            for (index, read) in reads.iter().enumerate() {
                let IrDependency::NativeAccess(name) = read else {
                    return Err("a monitor resolves only native accesses".to_owned());
                };
                let (real, address) = frame.native_access_storage(name)?;
                let kind = if real {
                    "LLG_FMT_REAL"
                } else {
                    "LLG_FMT_PACKED"
                };
                frame.line(format!(
                    "out[{index}] = (llg_display_read_t){{ {kind}, {address} }};"
                ));
            }
            frame.line("}");
            frame.line("if (0) goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok((
                format!(
                    "static void {c_name}(llg_display_read_t* out, void* context) {{\n{}{}\n}}\n",
                    frame.prologue(),
                    frame.body()
                ),
                None,
            ))
        }
        IrPreFn::MonEval {
            c_name,
            args,
            context,
            item,
            real_item,
        } => {
            frame.read_only_callback = true;
            frame.item_callback = *item;
            if *item {
                frame.line("(void)__llg_method_item; (void)__llg_method_index;");
            }
            frame.line("(void)out; (void)context;");
            bind_context(&mut frame, context.as_ref())?;
            frame.line("{");
            // Compute first, publish second: no unregistered partial output
            // survives a nonlocal exit while evaluating a later argument.
            let mut values = Vec::new();
            for expression in args {
                let value = frame.expression(expression)?;
                if value.width == 0 {
                    return Err(pending("real results in packed callbacks"));
                }
                values.push(value);
            }
            for (index, value) in values.into_iter().enumerate() {
                frame.line(format!("sv4_move(&out[{index}], &{});", value.code));
                frame.discard(value);
            }
            frame.line("}");
            frame.line("if (0) goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            let item_params = match (*item, *real_item) {
                (true, true) => "double __llg_method_item, sv4_t __llg_method_index, ",
                (true, false) => "sv4_t __llg_method_item, sv4_t __llg_method_index, ",
                (false, _) => "",
            };
            Ok((
                format!(
                    "static void {c_name}(sv4_t* out, {item_params}void* context) {{\n{}{}\n}}\n",
                    frame.prologue(),
                    frame.body()
                ),
                None,
            ))
        }
        IrPreFn::RealEval {
            c_name,
            value,
            context,
        } => {
            frame.read_only_callback = true;
            frame.line("(void)context;");
            bind_context(&mut frame, context.as_ref())?;
            frame.line("{");
            let value = frame.expression(value)?;
            frame.line(format!("*out = {};", value.real()));
            frame.discard(value);
            frame.line("}");
            frame.line("if (0) goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok((
                format!(
                    "static void {c_name}(double* out, void* context) {{\n{}{}\n}}\n",
                    frame.prologue(),
                    frame.body()
                ),
                None,
            ))
        }
        IrPreFn::DeferredAssertion {
            c_name,
            captures,
            body,
            ..
        } => {
            frame.deferred_action = true;
            frame.line("(void)frame;");
            for capture in captures {
                let storage = capture.storage();
                let name = format!("_fc{}_{}", storage.frame().index(), storage.slot());
                frame.bind_capture(&name, storage, capture.initial(), "frame")?;
            }
            frame.block(body)?;
            frame.line("goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok((
                format!(
                    "static void {c_name}(llg_frame_t* frame) {{\n{}{}\n}}\n",
                    frame.prologue(),
                    frame.body()
                ),
                None,
            ))
        }
        IrPreFn::EventAssign {
            c_name,
            captures,
            lhs,
            rhs,
            ..
        } => {
            frame.line("(void)frame;");
            for capture in captures {
                let storage = capture.storage();
                let name = format!("_fc{}_{}", storage.frame().index(), storage.slot());
                frame.bind_capture(&name, storage, capture.initial(), "frame")?;
            }
            let value = frame.expression(rhs)?;
            let writes = frame.prepare_assignment(lhs, value)?;
            for (target, value) in writes {
                frame.store(&target, value, true, "0")?;
                frame.release_target(target);
            }
            frame.line("if (0) goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok((
                format!(
                    "static void {c_name}(llg_frame_t* frame) {{\n{}{}\n}}\n",
                    frame.prologue(),
                    frame.body()
                ),
                None,
            ))
        }
    }
}

/// Bind the class-method receiver a deferred report carries in its context.
/// A fork branch names the receiver it captured; a method body names `this`.
fn bind_receiver(frame: &mut Frame<'_, '_>, receiver: Option<&str>) {
    let Some(receiver) = receiver else {
        return;
    };
    if receiver == "_this" {
        frame.line("void* _this = context; (void)_this;");
    } else {
        let binding = frame.native_local(receiver, NativeKind::Chandle);
        frame.line(format!("*({}) = context;", binding.address));
    }
}

fn bind_context(frame: &mut Frame<'_, '_>, context: Option<&IrEventContext>) -> Result<(), String> {
    if let Some(context) = context {
        for capture in context.captures() {
            // Aliased cells and strings are bound through their slot
            // addresses; packed and real copies are only read.
            let source = if capture.storage().ownership() == StorageOwnership::Owned
                && matches!(
                    capture.storage().kind(),
                    StorageKind::Packed | StorageKind::Real
                ) {
                "(const llg_frame_t*)context"
            } else {
                "(llg_frame_t*)context"
            };
            frame.bind_capture(
                capture.local(),
                capture.storage(),
                capture.initial(),
                source,
            )?;
        }
    }
    Ok(())
}
