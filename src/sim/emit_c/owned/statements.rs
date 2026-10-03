//! Lexical owners, loop back-edges and explicit nonlocal cleanup.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn begin_block(&mut self, body: &[IrStmt]) {
        self.line("{");
        let marker = self.declare(
            "llg_value_scope_t*",
            "mark",
            "llg_value_scope_mark()".to_owned(),
        );
        self.marks.push(marker);
        self.mark_blocks.push(self.layout.current_block());
        self.bindings.push(HashMap::new());
        self.event_bindings.push(HashMap::new());
        self.native_bindings.push(HashMap::new());
        self.temp_roots.push(self.slots.clone());
        self.labels.push(
            body.iter()
                .filter_map(|stmt| match stmt {
                    IrStmt::Label(name) => Some((name.clone(), false)),
                    _ => None,
                })
                .collect(),
        );
    }
    pub(super) fn end_block(&mut self) {
        let marker = self.marks.pop().expect("matched lexical scope");
        self.mark_blocks.pop();
        self.line(format!("llg_value_scopes_end_since({marker});"));
        self.bindings.pop();
        self.event_bindings.pop();
        self.native_bindings.pop();
        self.labels.pop();
        self.temp_roots.pop();
        self.line("}");
    }
    pub(super) fn block(&mut self, body: &[IrStmt]) -> Result<(), String> {
        self.begin_block(body);
        self.statements(body)?;
        self.end_block();
        Ok(())
    }
    fn budget(&mut self) {
        let call = format!(
            "llg_budget_point({})",
            c_string_literal(
                self.ctx
                    .func
                    .map(|f| f.diagnostic_name())
                    .unwrap_or(self.ctx.model.design_name())
            )
        );
        if self.plain_exits() {
            self.line(format!("if (LLG_CO_UNLIKELY({call})) goto _llg_return;"));
        } else {
            self.line(format!("if (LLG_CO_UNLIKELY({call})) return LLG_CO_EXIT;"));
        }
    }
    pub(super) fn condition(&mut self, expr: &IrExpr) -> Result<String, String> {
        let value = self.operand(expr)?;
        let result = self.scalar("int", value.truth());
        self.discard(value);
        Ok(result)
    }
    pub(super) fn goto(&mut self, label: &str) -> Result<(), String> {
        let target = self
            .labels
            .iter()
            .rposition(|labels| labels.contains_key(label))
            .ok_or_else(|| pending("jumps into another lexical scope"))?;
        if self.labels[target][label] {
            return Err(pending("backward unstructured jumps"));
        }
        self.leave_activations(Some(target));
        let preserved = self.temp_roots[target].clone();
        for slot in 0..self.slots.len() {
            if self.slots[slot] && !preserved.get(slot).copied().unwrap_or(false) {
                // Compile-time liveness remains unchanged for fall-through
                // paths; only this control-flow edge releases these owners.
                self.line(format!("sv4_destroy(&_llg_t[{slot}]);"));
            }
        }
        if target + 1 < self.marks.len() {
            self.line(format!(
                "llg_value_scopes_end_since({});",
                self.marks[target + 1]
            ));
        }
        for block in self.layout.exited_blocks(self.mark_blocks[target]) {
            self.poison_block(block);
        }
        self.line(format!("goto {label};"));
        Ok(())
    }

    pub(super) fn statement(&mut self, statement: &IrStmt) -> Result<(), String> {
        let cancellation_mark = self.cancellation_mark();
        match statement {
            IrStmt::Located { statement, .. } => return self.statement(statement),
            IrStmt::Nop => self.line(";"),
            IrStmt::FixedArrayDeclare(index) => {
                let pointer = self.new_fixed_array(*index)?;
                self.fixed_arrays.insert(*index, pointer);
            }
            IrStmt::FixedValueAssign { dst, src, nba } => {
                let source = self.fixed_value(src, dst.array, dst.total)?;
                let target = self.fixed_view(dst)?;
                self.line(format!(
                    "llg_fixed_array_copy({target}, {source}, {}, {});",
                    u8::from(self.ctx.model.array(dst.array).two_state),
                    u8::from(*nba)
                ));
            }
            IrStmt::FixedArrayFill { array, value, nba } => {
                let address = self.fixed_array_address(*array)?;
                let two_state = self.ctx.model.array(*array).two_state;
                let value = self.expression(value)?;
                self.line(format!(
                    "llg_fixed_array_fill({address}, {}, {}, {});",
                    value.code,
                    u8::from(two_state),
                    u8::from(*nba)
                ));
                self.discard(value);
            }
            IrStmt::FixedArrayCopy {
                dst,
                src,
                nba,
                slice,
            } => {
                let dst_address = self.fixed_array_address(*dst)?;
                let src_address = self.fixed_array_address(*src)?;
                let dst = self.ctx.model.array(*dst);
                self.line(format!(
                    "llg_fixed_array_stream_copy({dst_address}, {src_address}, {}, {}, {slice}u);",
                    u8::from(dst.two_state),
                    u8::from(*nba)
                ));
            }
            IrStmt::Container(operation) => self.container_statement(operation)?,
            IrStmt::StreamAssign {
                source,
                slice,
                direction,
                targets,
            } => self.stream_assignment(source, *slice, *direction, targets)?,
            IrStmt::Object(operation) => self.object_statement(operation)?,
            IrStmt::DeclString { name, init } => {
                let binding = self.native_local(name, super::native::NativeKind::String);
                if let Some(value) = init {
                    self.string_assign(&binding.address, value)?;
                }
            }
            IrStmt::DelayedStringAssign { target, rhs, ticks } => {
                let binding = self.native_lookup(target, super::native::NativeKind::String)?;
                if binding.automatic {
                    return Err(pending("delayed writes to automatic string storage"));
                }
                let value = self.string(rhs)?;
                let ticks = self.delay(ticks)?;
                self.line(format!(
                    "llg_string_nba_after({}, {}, {ticks});",
                    binding.address,
                    value.take_string()
                ));
                self.native_discard(value);
            }
            IrStmt::RandomSeed { seed } => {
                let seed = self.expression(seed)?;
                self.line(format!("llg_process_srandom({});", seed.code));
                self.discard(seed);
            }
            IrStmt::RandomStateSet { state } => {
                let state = self.string(state)?;
                self.line(format!(
                    "(void)llg_process_set_randstate({});",
                    state.take_string()
                ));
                self.native_discard(state);
            }
            IrStmt::VpiCall { site, name, args } => {
                self.vpi_call(*site, name, args, None)?;
            }
            IrStmt::System(command) => {
                let value = self.system_command(command.as_ref())?;
                self.discard(value);
            }
            IrStmt::Stochastic(op) => self.stochastic(op)?,
            IrStmt::PrintTimescale {
                unit_fs,
                precision_fs,
                label,
            } => self.line(format!(
                "printf(\"%s: timescale is {}/{}\\n\", {});",
                super::super::constants::fs_to_timescale_str(*unit_fs),
                super::super::constants::fs_to_timescale_str(*precision_fs),
                c_string_literal(label)
            )),
            IrStmt::TimeFormat {
                units,
                precision,
                suffix,
                minimum_field_width,
            } => self.time_format(units, precision, suffix, minimum_field_width)?,
            IrStmt::Block(body) => self.block(body)?,
            IrStmt::EventCapture { name, source } => {
                // Snapshot object identity, not the address of an alias that
                // may be rebound while the inline task is suspended.
                let address = self.event_address(source)?;
                let pointer = self.scalar("llg_event_t*", address);
                let local = self.declare(
                    "llg_event_t",
                    "event_capture",
                    format!("{{ {pointer} ? {pointer}->object : NULL }}"),
                );
                self.event_bindings
                    .last_mut()
                    .expect("event scope")
                    .insert(name.clone(), format!("&{local}"));
            }
            IrStmt::DeclLocal {
                name,
                width,
                signed,
                init,
                two_state,
            } => self.local(name, *width, *signed, *two_state, init.as_deref())?,
            IrStmt::InertialAssign { lhs, rhs, delay } => self.inertial_assign(lhs, rhs, *delay)?,
            IrStmt::PcaAssign { .. } | IrStmt::PcaDrive { .. } => self.pca_task(statement)?,
            IrStmt::PcaDeassign { sig } => {
                let signal = self.ctx.model.signal(*sig);
                let suffix = if signal.ty.width() == 0 { "_d" } else { "" };
                self.line(format!("llg_pca_deassign{suffix}(&{});", signal.c_name));
            }
            IrStmt::Force {
                lhs,
                eval,
                reads,
                dependencies,
                ..
            } => self.force_task(lhs, Some(eval), reads, dependencies)?,
            IrStmt::Release { lhs } => self.force_task(lhs, None, &[], &[])?,
            IrStmt::Memory { .. } => self.memory_task(statement)?,
            IrStmt::MonitorSet { .. } => self.monitor_task(statement)?,
            IrStmt::MonitorEnable(enabled) => {
                self.line(format!("llg_monitor_set({});", u8::from(*enabled)))
            }
            IrStmt::FileControl { op, descriptor } => {
                self.file_control(*op, descriptor.as_ref())?
            }
            IrStmt::Assign { lhs, rhs, nba } => {
                let value = if matches!(lhs, IrLhs::Whole(_) | IrLhs::WholeRef { .. }) {
                    self.operand(rhs)?
                } else {
                    self.expression(rhs)?
                };
                let writes = self.prepare_assignment(lhs, value)?;
                for (target, value) in writes {
                    self.store(&target, value, *nba, "0")?;
                    self.release_target(target);
                }
            }
            IrStmt::DelayedAssign { lhs, rhs, ticks } => {
                let value = self.expression(rhs)?;
                let writes = self.prepare_assignment(lhs, value)?;
                let ticks = self.delay(ticks)?;
                for (target, value) in writes {
                    self.store(&target, value, true, &ticks)?;
                    self.release_target(target);
                }
            }
            IrStmt::If {
                cond,
                then_,
                els,
                check,
            } => {
                if !check.is_none() {
                    self.qualified_if(cond, then_, els.as_deref(), check)?;
                    self.cancellation_check_since(cancellation_mark)?;
                    return Ok(());
                }
                let condition = self.condition(cond)?;
                self.line(format!("if ({condition})"));
                self.block(then_)?;
                if let Some(body) = els {
                    self.line("else");
                    self.block(body)?;
                }
            }
            IrStmt::While { cond, body } => {
                self.line("for (;;) {");
                let condition = self.condition(cond)?;
                self.poison_loop_exit(&format!("!{condition}"));
                self.line(format!("if (!{condition}) break;"));
                self.budget();
                self.block(body)?;
                self.line("}");
            }
            IrStmt::Forever { body } => {
                self.line("for (;;) {");
                self.budget();
                self.block(body)?;
                self.line("}");
            }
            IrStmt::For {
                init,
                cond,
                incr,
                body,
            } => {
                self.begin_block(init);
                self.statements(init)?;
                self.line("for (;;) {");
                let condition = self.condition(cond)?;
                self.poison_loop_exit(&format!("!{condition}"));
                self.line(format!("if (!{condition}) break;"));
                self.budget();
                self.block(body)?;
                self.block(incr)?;
                self.line("}");
                self.end_block();
            }
            IrStmt::Repeat { count, body } => {
                let count = self.expression(count)?;
                if count.width == 0 {
                    return Err(pending("real-valued repeat counts"));
                }
                let code = format!("sv4_repeat_count({})", count.code);
                let width = count.width;
                let count = self.replace(count, code, width, false);
                self.line(format!("while ({}) {{", count.truth()));
                self.budget();
                self.block(body)?;
                let one = self.value(
                    format!("sv4_from_u64(1, {}, 0)", count.width),
                    count.width,
                    false,
                );
                self.assign(
                    &format!("&{}", count.code),
                    &format!("sv4_sub({}, {})", count.code, one.code),
                );
                self.discard(one);
                self.line("}");
                self.discard(count);
            }
            IrStmt::Case {
                sel,
                kind,
                items,
                check,
            } => {
                if !check.is_none() {
                    self.qualified_case(sel, *kind, items, check)?;
                    self.cancellation_check_since(cancellation_mark)?;
                    return Ok(());
                }
                let selector = self.expression(sel)?;
                let matched = self.scalar("int", "0".to_owned());
                for item in items.iter().filter(|item| !item.exprs.is_empty()) {
                    self.line(format!("if (!{matched}) {{"));
                    let hit = self.scalar("int", "0".to_owned());
                    for expression in &item.exprs {
                        self.line(format!("if (!{hit}) {{"));
                        let value = self.expression(expression)?;
                        if *kind == IrCaseKind::Inside {
                            self.line(format!("{hit} = {};", value.truth()));
                            self.discard(value);
                        } else if selector.width == 0 || value.width == 0 {
                            self.line(format!(
                                "{hit} = ({} == {});",
                                selector.real(),
                                value.real()
                            ));
                            self.discard(value);
                        } else {
                            let code =
                                format!("{}({}, {})", kind.cmp_fn(), selector.code, value.code);
                            let result = self.replace(value, code, 1, false);
                            self.line(format!("{hit} = {};", result.truth()));
                            self.discard(result);
                        }
                        self.line("}");
                    }
                    self.line(format!("if ({hit}) {{ {matched} = 1;"));
                    self.block(&item.body)?;
                    self.line("}");
                    self.line("}");
                }
                if let Some(default) = items.iter().find(|item| item.exprs.is_empty()) {
                    self.line(format!("if (!{matched})"));
                    self.block(&default.body)?;
                }
                self.discard(selector);
            }
            IrStmt::Delay { ticks } => {
                let ticks = self.delay(ticks)?;
                self.await_arm(
                    SuspensionOperation::Delay,
                    format!("llg_arm_time(self, {ticks})"),
                )?;
            }
            IrStmt::WaitAny { sens } => {
                self.wait_any(sens, None, SuspensionOperation::EventWait)?
            }
            IrStmt::WaitCond { cond, sens, body } => {
                self.line("for (;;) {");
                let condition = self.condition(cond)?;
                self.poison_loop_exit(&condition);
                self.line(format!("if ({condition}) break;"));
                self.wait_any(sens, None, SuspensionOperation::ConditionWait)?;
                self.line("}");
                self.block(body)?;
            }
            IrStmt::WaitEvents { specs } => self.wait_events(specs)?,
            IrStmt::ClockingSample {
                source,
                sample,
                mode,
            } => {
                let source = self.canonical_signal(*source);
                let sample = self.canonical_signal(*sample);
                self.line(match mode {
                    IrClockingSampleMode::OneStep => {
                        format!("(void)llg_sampled_copy({source}, {sample});")
                    }
                    IrClockingSampleMode::Observed => {
                        format!("(void)llg_clocking_sample_observed({source}, {sample});")
                    }
                    IrClockingSampleMode::History(ticks) => format!(
                        "(void)llg_clocking_sample_history({source}, {sample}, {ticks}ULL);"
                    ),
                });
            }
            IrStmt::ClockingEventTrigger { ev } => {
                let event = self.event_address(ev)?;
                self.line(format!("(void)llg_clocking_event_observed({event});"));
            }
            IrStmt::ClockingDrive {
                lhs,
                rhs,
                ticks,
                specs,
            } => self.clocking_drive(lhs, rhs, ticks, specs)?,
            IrStmt::ClockingCycleWait { count, specs } => {
                let count = self.expression(count)?;
                if count.width == 0 {
                    return Err("clocking cycle count must be integral".to_owned());
                }
                let sources = self.clocking_source_entries(specs)?;
                let remaining =
                    self.scalar("uint64_t", format!("llg_repeat_count({})", count.code));
                self.discard(count);
                self.line(format!("if ({remaining} == 0) {{"));
                let array =
                    self.arm_array("llg_wait_src_t", "clocking_sources", specs.len(), &sources);
                self.await_arm(
                    SuspensionOperation::ClockingCycle,
                    format!("llg_arm_clocking_cycle(self, {array}, {}, 1)", specs.len()),
                )?;
                self.line("} else {");
                self.line(format!("while ({remaining} != 0) {{"));
                let array =
                    self.arm_array("llg_wait_src_t", "clocking_sources", specs.len(), &sources);
                self.await_arm(
                    SuspensionOperation::ClockingCycle,
                    format!("llg_arm_clocking_cycle(self, {array}, {}, 0)", specs.len()),
                )?;
                self.line(format!("--{remaining};"));
                self.line("}");
                self.line("}");
            }
            IrStmt::NonblockingEventTriggerWhen { ev, specs, repeat } => {
                let target = self.event_address(ev)?;
                let target = self.scalar("llg_event_t*", target);
                let handle = self.declare(
                    "llg_event_t",
                    "event_target",
                    format!("{{ ({target}) ? ({target})->object : NULL }}"),
                );
                let count = self.event_repeat(repeat.as_ref())?;
                let sources = self.event_specs(specs, false)?;
                self.line(format!(
                    "llg_nba_event_when({sources}, {}, &{handle}, {count});",
                    specs.len()
                ));
            }
            IrStmt::NonblockingEventAssignWhen {
                specs,
                repeat,
                action,
                captures,
                ..
            } => {
                // An intra-assignment control captures its RHS and destination
                // selectors before evaluating the repeat/delay control.
                let captures =
                    self.prepare_captures(captures.iter().map(|c| (c.storage(), c.initial())))?;
                let count = self.event_repeat(repeat.as_ref())?;
                // Context expressions can exit nonlocally. Keep action values
                // in registered slots until they, too, have finished.
                let sources = self.event_specs(specs, false)?;
                let frame = self.name("event_action");
                let frame = self.publish_captures(&frame, captures);
                self.line(format!(
                    "llg_nba_event_assign_when({sources}, {}, {count}, {action}, {frame});",
                    specs.len()
                ));
            }
            IrStmt::WaitOrder {
                events,
                success,
                failure,
            } => {
                if events.is_empty() {
                    return Err("wait_order requires at least one event".to_owned());
                }
                let mut addresses = Vec::new();
                for event in events {
                    addresses.push(self.event_address(event)?);
                }
                let list = if addresses.is_empty() {
                    "NULL".to_owned()
                } else {
                    self.arm_array(
                        "const llg_event_t*",
                        "ordered_events",
                        addresses.len(),
                        &addresses.join(", "),
                    )
                };
                let success_flag = self.scalar("int", "0".to_owned());
                let await_mark = self.cancellation_mark();
                self.await_arm(
                    SuspensionOperation::WaitOrder,
                    format!(
                        "llg_arm_order(self, {list}, {}, &{success_flag})",
                        events.len()
                    ),
                )?;
                self.cancellation_check_covering(await_mark)?;
                self.line(format!("if ({success_flag} > 0)"));
                self.block(success)?;
                self.line(format!("else if ({success_flag} < 0)"));
                self.block(failure)?;
            }
            IrStmt::EventTrigger { ev } => {
                let event = self.event_address(ev)?;
                self.line(format!("llg_event_trigger({event});"));
            }
            IrStmt::NonblockingEventTrigger { ev, ticks } => {
                let event = self.event_address(ev)?;
                let delay = if let Some(ticks) = ticks {
                    self.delay(ticks)?
                } else {
                    "0".to_owned()
                };
                self.line(format!("llg_nba_event_after({event}, {delay});"));
            }
            IrStmt::EventAssign { target, source } => {
                let target = self.event_address(target)?;
                if let Some(source) = source {
                    let source = self.event_address(source)?;
                    self.line(format!("llg_event_assign({target}, {source});"));
                } else {
                    self.line(format!("llg_event_assign_null({target});"));
                }
            }
            IrStmt::WaitEventTriggered { event, body } => {
                let event = self.event_address(event)?;
                let await_mark = self.cancellation_mark();
                self.await_arm(
                    SuspensionOperation::EventTriggeredWait,
                    format!("llg_arm_event_triggered(self, {event})"),
                )?;
                self.cancellation_check_covering(await_mark)?;
                self.block(body)?;
            }
            IrStmt::Fork {
                join_kind,
                branches,
                target,
            } => {
                let kind = match join_kind {
                    IrJoinKind::Join => "LLG_JOIN",
                    IrJoinKind::Any => "LLG_JOIN_ANY",
                    IrJoinKind::None => "LLG_JOIN_NONE",
                };
                let group = self.fork_group(kind, *target);
                for (function, label) in branches {
                    self.line(format!(
                        "llg_fork(&{function}_desc, {}, {group});",
                        c_string_literal(label)
                    ));
                }
                if !branches.is_empty() && *join_kind != IrJoinKind::None {
                    self.await_arm(
                        SuspensionOperation::ForkJoin,
                        format!("llg_arm_join(self, {group})"),
                    )?;
                }
            }
            IrStmt::CapturedFork {
                join_kind,
                branches,
                target,
            } => {
                self.captured_fork(*join_kind, branches, *target)?;
            }
            IrStmt::ActivationScope { target, exit, body } => {
                self.activation_scope(*target, exit, body)?
            }
            IrStmt::DisableTarget { target } => {
                self.line(format!(
                    "llg_disable_target(self, {}u, {}u);",
                    target.declaration(),
                    target.instance()
                ));
                self.cancellation_point();
            }
            IrStmt::WaitFork => {
                self.await_arm(SuspensionOperation::WaitFork, "llg_arm_wait_fork(self)")?
            }
            IrStmt::DisableFork => self.line("llg_disable_fork(self);"),
            IrStmt::Display {
                fmt, args, newline, ..
            } => {
                let args = args
                    .iter()
                    .map(|(value, real)| {
                        if *real {
                            IrDisplayArg::Real(value.clone())
                        } else {
                            IrDisplayArg::Packed(value.clone())
                        }
                    })
                    .collect::<Vec<_>>();
                self.display(fmt, &args, "", *newline, None, self.ctx.model.precision_fs)?;
            }
            IrStmt::DisplayTyped {
                fmt,
                args,
                scope,
                newline,
                descriptor,
                time_unit_fs,
                ..
            } => self.display(
                fmt,
                args,
                scope,
                *newline,
                descriptor.as_ref(),
                *time_unit_fs,
            )?,
            IrStmt::AssertionControl { kind, args, scopes } => {
                self.assertion_control(*kind, args, scopes)?
            }
            IrStmt::Expect { identity } => {
                self.line(format!(
                    "if (!llg_assertion_expect_start({identity}ULL)) {{"
                ));
                self.leave_activations(None);
                self.line("goto _llg_return; }");
                self.await_arm(
                    SuspensionOperation::Expect,
                    format!("llg_arm_assertion(self, {identity}ULL)"),
                )?;
            }
            IrStmt::DeferredImmediateAssertion { .. } => self.deferred_assertion(statement)?,
            IrStmt::ImmediateAssertion {
                kind,
                condition,
                if_true,
                if_false,
                label,
                location,
                identity,
            } => {
                let condition = self.condition(condition)?;
                let label = c_string_literal(label);
                let location = c_string_literal(location);
                self.line(format!("if ({condition}) {{"));
                if *kind == IrImmediateAssertionKind::Cover {
                    self.line(format!(
                        "llg_assertion_cover({identity}ULL, {label}, {location});"
                    ));
                }
                if let Some(body) = if_true {
                    self.block(body)?;
                }
                self.line("} else {");
                if let Some(body) = if_false {
                    self.block(body)?;
                } else if *kind != IrImmediateAssertionKind::Cover {
                    let kind = if *kind == IrImmediateAssertionKind::Assert {
                        "LLG_ASSERTION_ASSERT"
                    } else {
                        "LLG_ASSERTION_ASSUME"
                    };
                    self.line(format!(
                        "llg_assertion_failure({kind}, {identity}ULL, {label}, {location});"
                    ));
                }
                self.line("}");
            }
            IrStmt::Severity {
                level,
                fmt,
                args,
                scope,
                location,
                fatal_finish_number,
                runtime_failure,
            } => {
                let values = self.formatted_arguments(args, self.ctx.model.precision_fs)?;
                let (scope, location) = (c_string_literal(scope), c_string_literal(location));
                if *level == IrSeverityLevel::Fatal {
                    let number = fatal_finish_number
                        .ok_or_else(|| "fatal task missing finish number".to_owned())?;
                    if *runtime_failure {
                        self.line("llg_rt_mark_failed();");
                    }
                    self.line(format!(
                        "llg_rt_fatal_typed({number}, {fmt}, {values}, {}, {scope}, {location});",
                        args.len()
                    ));
                } else {
                    let level = match level {
                        IrSeverityLevel::Info => "LLG_SEVERITY_INFO",
                        IrSeverityLevel::Warning => "LLG_SEVERITY_WARNING",
                        _ => "LLG_SEVERITY_ERROR",
                    };
                    self.line(format!(
                        "llg_rt_severity_typed({level}, {fmt}, {values}, {}, {scope}, {location});",
                        args.len()
                    ));
                }
            }
            IrStmt::PlusArg(expr) => {
                let value = self.expression(expr)?;
                self.discard(value);
            }
            IrStmt::Call(call) => self.call_statement(call)?,
            IrStmt::Return { value } => {
                if let Some(value) = value {
                    let address = self
                        .return_address
                        .clone()
                        .ok_or_else(|| "value return in a void procedure".to_owned())?;
                    let target = self.address(&address)?;
                    let value = self.expression(value)?;
                    let value = self.convert(
                        value,
                        target.width,
                        target.signed,
                        target.two_state,
                        target.shortreal,
                    );
                    if target.width == 0 {
                        self.line(format!("*({}) = {};", target.address, value.code));
                    } else {
                        self.line(format!("sv4_move({}, &{});", target.address, value.code));
                    }
                    self.discard(value);
                }
                self.leave_activations(None);
                self.line("goto _llg_return;");
            }
            IrStmt::Label(label) => {
                self.labels
                    .last_mut()
                    .ok_or_else(|| "label outside a lexical block".to_owned())?
                    .insert(label.clone(), true);
                self.line(format!("{label}: ;"));
            }
            IrStmt::Goto(label) => self.goto(label)?,
            IrStmt::WaveFile(_)
            | IrStmt::WaveDumpVars(_)
            | IrStmt::WaveLimit(_)
            | IrStmt::WaveOn
            | IrStmt::WaveOff
            | IrStmt::WaveDumpAll
            | IrStmt::WaveFlush => {
                if !self.ctx.model.waveform {
                    return Err("wave control without waveform-enabled model".to_owned());
                }
                match statement {
                    IrStmt::WaveFile(path) => self.line(format!(
                        "llg_wave_file({}, llg_time());",
                        c_string_literal(path)
                    )),
                    IrStmt::WaveDumpVars(selection) => {
                        if selection.names().is_empty() {
                            self.line(format!(
                                "llg_wave_dumpvars_select(llg_time(), {}u, NULL, 0u);",
                                selection.depth()
                            ));
                        } else {
                            let names = selection
                                .names()
                                .iter()
                                .map(|name| c_string_literal(name))
                                .collect::<Vec<_>>()
                                .join(", ");
                            let array = self.declare_array_init(
                                "const char*",
                                "wave_names",
                                selection.names().len(),
                                &names,
                            );
                            self.line(format!(
                                "llg_wave_dumpvars_select(llg_time(), {}u, {array}, {}u);",
                                selection.depth(),
                                selection.names().len()
                            ));
                        }
                    }
                    IrStmt::WaveLimit(expr) => {
                        let value = self.expression(expr)?;
                        self.line(format!(
                            "llg_wave_limit(sv4_to_u64({}), llg_time());",
                            value.code
                        ));
                        self.discard(value);
                    }
                    IrStmt::WaveOn => self.line("llg_wave_on(llg_time());"),
                    IrStmt::WaveOff => self.line("llg_wave_off(llg_time());"),
                    IrStmt::WaveDumpAll => self.line("llg_wave_dumpall(llg_time());"),
                    IrStmt::WaveFlush => self.line("llg_wave_flush(llg_time());"),
                    _ => unreachable!("waveform control classification"),
                }
            }
            IrStmt::Finish => self.line("llg_rt_finish();"),
            IrStmt::FinishControl {
                verbosity,
                location,
            } => self.line(format!(
                "llg_rt_finish_with_level({verbosity}, {});",
                c_string_literal(location)
            )),
            IrStmt::StopControl {
                verbosity,
                location,
            } => {
                let function_stop = self.ctx.func.is_some_and(|function| !function.is_task());
                if function_stop {
                    self.line(format!(
                        "llg_rt_request_stop({verbosity}, {});",
                        c_string_literal(location)
                    ));
                } else {
                    self.await_arm(
                        SuspensionOperation::Stop,
                        format!(
                            "llg_arm_stop(self, {verbosity}, {})",
                            c_string_literal(location)
                        ),
                    )?;
                }
            }
            IrStmt::ProgramExit => self.line("llg_program_exit(self);"),
        }
        if crate::sim::execution::effects_for_statements(
            self.ctx.model,
            std::slice::from_ref(statement),
        )
        .contains(&crate::sim::execution::ExecutionEffect::Terminate)
        {
            if self.plain_exits() {
                self.line("if (LLG_CO_UNLIKELY(llg_rt_exiting())) goto _llg_return;");
            } else {
                self.line("LLG_CO_EXIT_CHECK(ch);");
            }
        }
        if straight_line_statement(statement) {
            self.cancellation_check_covering(cancellation_mark)
        } else {
            self.cancellation_check_since(cancellation_mark)
        }
    }
}

/// Statements that can hold a cancellation point but no nested statements
/// (fork branches are separate coroutines):
/// no `break`, `continue` or `return` can leave them before their final
/// cancellation check, so that check covers their points for every
/// enclosing construct. Compound statements keep their points visible.
fn straight_line_statement(statement: &IrStmt) -> bool {
    matches!(
        statement,
        IrStmt::Delay { .. }
            | IrStmt::ClockingCycleWait { .. }
            | IrStmt::WaitEvents { .. }
            | IrStmt::WaitAny { .. }
            | IrStmt::WaitFork
            | IrStmt::Fork { .. }
            | IrStmt::CapturedFork { .. }
            | IrStmt::Expect { .. }
            | IrStmt::DisableTarget { .. }
            | IrStmt::Assign { .. }
            | IrStmt::Call(_)
            | IrStmt::Object(_)
            | IrStmt::StopControl { .. }
    )
}
