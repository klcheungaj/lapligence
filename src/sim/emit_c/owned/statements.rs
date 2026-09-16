//! Lexical owners, loop back-edges and explicit nonlocal cleanup.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn begin_block(&mut self, body: &[IrStmt]) {
        self.line("{");
        let marker = self.name("mark");
        self.line(format!("llg_value_scope_t* {marker} = llg_value_scope_mark();"));
        self.marks.push(marker);
        self.bindings.push(HashMap::new());
        self.temp_roots.push(self.slots.clone());
        self.labels.push(body.iter().filter_map(|stmt| match stmt {
            IrStmt::Label(name) => Some((name.clone(), false)), _ => None,
        }).collect());
    }
    pub(super) fn end_block(&mut self) {
        let marker = self.marks.pop().expect("matched lexical scope");
        self.line(format!("llg_value_scopes_end_since({marker});"));
        self.bindings.pop(); self.labels.pop(); self.temp_roots.pop();
        self.line("}");
    }
    pub(super) fn block(&mut self, body: &[IrStmt]) -> Result<(), String> {
        self.begin_block(body);
        for statement in body { self.statement(statement)?; }
        self.end_block();
        Ok(())
    }
    fn budget(&mut self) {
        self.line(format!("llg_budget_point({});", c_string_literal(self.ctx.func.map(|f| f.c_name.as_str()).unwrap_or(self.ctx.model.design_name()))));
    }
    fn condition(&mut self, expr: &IrExpr) -> Result<String, String> {
        let value = self.expression(expr)?;
        let result = self.scalar("int", value.truth());
        self.discard(value);
        Ok(result)
    }
    fn goto(&mut self, label: &str) -> Result<(), String> {
        let target = self.labels.iter().rposition(|labels| labels.contains_key(label))
            .ok_or_else(|| pending("jumps into another lexical scope"))?;
        if self.labels[target][label] { return Err(pending("backward unstructured jumps")); }
        let preserved = self.temp_roots[target].clone();
        for slot in 0..self.slots.len() {
            if self.slots[slot] && !preserved.get(slot).copied().unwrap_or(false) {
                // Compile-time liveness remains unchanged for fall-through
                // paths; only this control-flow edge releases these owners.
                self.line(format!("sv4_destroy(&_llg_t[{slot}]);"));
            }
        }
        if target + 1 < self.marks.len() {
            self.line(format!("llg_value_scopes_end_since({});", self.marks[target + 1]));
        }
        self.line(format!("goto {label};"));
        Ok(())
    }

    pub(super) fn statement(&mut self, statement: &IrStmt) -> Result<(), String> {
        match statement {
            IrStmt::Nop => self.line(";"),
            IrStmt::Block(body) => self.block(body)?,
            IrStmt::DeclLocal { name, width, signed, init, two_state } =>
                self.local(name, *width, *signed, *two_state, init.as_deref())?,
            IrStmt::Assign { lhs, rhs, nba } => {
                let value = self.expression(rhs)?;
                let target = self.target(lhs)?;
                self.store(&target, value, *nba, "0")?;
                self.release_target(target);
            }
            IrStmt::DelayedAssign { lhs, rhs, ticks } => {
                let value = self.expression(rhs)?;
                let target = self.target(lhs)?;
                let ticks = self.delay(ticks)?;
                self.store(&target, value, true, &ticks)?;
                self.release_target(target);
            }
            IrStmt::If { cond, then_, els, check } => {
                if !check.is_none() { return Err(pending("unique/priority diagnostics")); }
                let condition = self.condition(cond)?;
                self.line(format!("if ({condition})"));
                self.block(then_)?;
                if let Some(body) = els { self.line("else"); self.block(body)?; }
            }
            IrStmt::While { cond, body } => {
                self.line("for (;;) {"); self.budget();
                let condition = self.condition(cond)?;
                self.line(format!("if (!{condition}) break;"));
                self.block(body)?; self.line("}");
            }
            IrStmt::Forever { body } => {
                self.line("for (;;) {"); self.budget();
                self.block(body)?; self.line("}");
            }
            IrStmt::For { init, cond, incr, body } => {
                self.begin_block(init);
                for statement in init { self.statement(statement)?; }
                self.line("for (;;) {"); self.budget();
                let condition = self.condition(cond)?;
                self.line(format!("if (!{condition}) break;"));
                self.block(body)?; self.block(incr)?;
                self.line("}"); self.end_block();
            }
            IrStmt::Repeat { count, body } => {
                let count = self.expression(count)?;
                if count.width == 0 { return Err(pending("real-valued repeat counts")); }
                let code = format!("sv4_repeat_count({})", count.code);
                let width = count.width;
                let count = self.replace(count, code, width, false);
                self.line(format!("while ({}) {{", count.truth())); self.budget();
                self.block(body)?;
                let one = self.value(format!("sv4_from_u64(1, {}, 0)", count.width), count.width, false);
                self.line(format!("sv4_replace(&{}, sv4_sub({}, {}));", count.code, count.code, one.code));
                self.discard(one); self.line("}"); self.discard(count);
            }
            IrStmt::Case { sel, kind, items, check } => {
                if !check.is_none() { return Err(pending("qualified case diagnostics")); }
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
                            self.line(format!("{hit} = ({} == {});", selector.real(), value.real()));
                            self.discard(value);
                        } else {
                            let code = format!("{}({}, {})", kind.cmp_fn(), selector.code, value.code);
                            let result = self.replace(value, code, 1, false);
                            self.line(format!("{hit} = {};", result.truth())); self.discard(result);
                        }
                        self.line("}");
                    }
                    self.line(format!("if ({hit}) {{ {matched} = 1;"));
                    self.block(&item.body)?; self.line("}"); self.line("}");
                }
                if let Some(default) = items.iter().find(|item| item.exprs.is_empty()) {
                    self.line(format!("if (!{matched})")); self.block(&default.body)?;
                }
                self.discard(selector);
            }
            IrStmt::Delay { ticks } => {
                let ticks = self.delay(ticks)?; self.line(format!("llg_wait_time({ticks});"));
            }
            IrStmt::WaitAny { sens } => self.wait_any(sens, None)?,
            IrStmt::WaitCond { cond, sens, body } => {
                self.line("for (;;) {");
                let condition = self.condition(cond)?;
                self.line(format!("if ({condition}) break;"));
                self.wait_any(sens, None)?; self.line("}"); self.block(body)?;
            }
            IrStmt::WaitEvents { specs } => self.wait_events(specs)?,
            IrStmt::EventTrigger { ev } => {
                let event = self.event_address(ev)?; self.line(format!("llg_event_trigger({event});"));
            }
            IrStmt::NonblockingEventTrigger { ev, ticks } => {
                let event = self.event_address(ev)?;
                let delay = if let Some(ticks) = ticks { self.delay(ticks)? } else { "0".to_owned() };
                self.line(format!("llg_nba_event_after({event}, {delay});"));
            }
            IrStmt::EventAssign { target, source } => {
                let target = self.event_address(target)?;
                if let Some(source) = source {
                    let source = self.event_address(source)?; self.line(format!("llg_event_assign({target}, {source});"));
                } else { self.line(format!("llg_event_assign_null({target});")); }
            }
            IrStmt::WaitEventTriggered { event, body } => {
                let event = self.event_address(event)?;
                self.line(format!("llg_wait_event_triggered({event});")); self.block(body)?;
            }
            IrStmt::Fork { join_kind, branches, target } => {
                if target.is_some() { return Err(pending("named fork activation cleanup")); }
                let kind = match join_kind { IrJoinKind::Join => "LLG_JOIN", IrJoinKind::Any => "LLG_JOIN_ANY", IrJoinKind::None => "LLG_JOIN_NONE" };
                let group = self.scalar("llg_fork_group_t*", format!("llg_fork_group_new({kind})"));
                for (function, label) in branches {
                    self.line(format!("llg_fork({function}, {}, {group});", c_string_literal(label)));
                }
                self.line(format!("llg_join({group});"));
            }
            IrStmt::WaitFork => self.line("llg_wait_fork();"),
            IrStmt::DisableFork => self.line("llg_disable_fork();"),
            IrStmt::Display { fmt, args, newline, .. } => {
                let args = args.iter().map(|(value, real)| if *real { IrDisplayArg::Real(value.clone()) } else { IrDisplayArg::Packed(value.clone()) }).collect::<Vec<_>>();
                self.display(fmt, &args, "", *newline, None, self.ctx.model.precision_fs)?;
            }
            IrStmt::DisplayTyped { fmt, args, scope, newline, descriptor, time_unit_fs, .. } =>
                self.display(fmt, args, scope, *newline, descriptor.as_ref(), *time_unit_fs)?,
            IrStmt::Severity { level, fmt, args, scope, location, fatal_finish_number } => {
                let values = self.formatted_arguments(args, self.ctx.model.precision_fs)?;
                let (scope, location) = (c_string_literal(scope), c_string_literal(location));
                if *level == IrSeverityLevel::Fatal {
                    let number = fatal_finish_number.ok_or_else(|| "fatal task missing finish number".to_owned())?;
                    self.line(format!("llg_rt_fatal_typed({number}, {fmt}, {values}, {}, {scope}, {location});", args.len()));
                } else {
                    let level = match level { IrSeverityLevel::Info => "LLG_SEVERITY_INFO", IrSeverityLevel::Warning => "LLG_SEVERITY_WARNING", _ => "LLG_SEVERITY_ERROR" };
                    self.line(format!("llg_rt_severity_typed({level}, {fmt}, {values}, {}, {scope}, {location});", args.len()));
                }
            }
            IrStmt::PlusArg(expr) => { let value = self.expression(expr)?; self.discard(value); }
            IrStmt::Call(call) => self.call_statement(call)?,
            IrStmt::Return { value } => {
                if let Some(value) = value {
                    let address = self.return_address.clone().ok_or_else(|| "value return in a void procedure".to_owned())?;
                    let target = self.address(&address)?;
                    let value = self.expression(value)?;
                    let value = self.convert(value, target.width, target.signed, target.two_state, target.shortreal);
                    if target.width == 0 { self.line(format!("*({}) = {};", target.address, value.code)); }
                    else { self.line(format!("sv4_move({}, &{});", target.address, value.code)); }
                    self.discard(value);
                }
                self.line("goto _llg_return;");
            }
            IrStmt::Label(label) => {
                self.labels.last_mut().ok_or_else(|| "label outside a lexical block".to_owned())?.insert(label.clone(), true);
                self.line(format!("{label}: ;"));
            }
            IrStmt::Goto(label) => self.goto(label)?,
            IrStmt::WaveFile(_) | IrStmt::WaveDumpVars(_) | IrStmt::WaveLimit(_)
            | IrStmt::WaveOn | IrStmt::WaveOff | IrStmt::WaveDumpAll | IrStmt::WaveFlush => {
                if !self.ctx.model.waveform { return Err("wave control without waveform-enabled model".to_owned()); }
                match statement {
                    IrStmt::WaveFile(path) => self.line(format!("llg_wave_file({}, llg_time());", c_string_literal(path))),
                    IrStmt::WaveDumpVars(selection) => {
                        if selection.names().is_empty() {
                            self.line(format!("llg_wave_dumpvars_select(llg_time(), {}u, NULL, 0u);", selection.depth()));
                        } else {
                            let names = selection.names().iter().map(|name| c_string_literal(name)).collect::<Vec<_>>().join(", ");
                            let array = self.name("wave_names");
                            self.line(format!("const char* {array}[] = {{ {names} }};"));
                            self.line(format!("llg_wave_dumpvars_select(llg_time(), {}u, {array}, {}u);", selection.depth(), selection.names().len()));
                        }
                    }
                    IrStmt::WaveLimit(expr) => {
                        let value = self.expression(expr)?;
                        self.line(format!("llg_wave_limit(sv4_to_u64({}), llg_time());", value.code));
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
            IrStmt::FinishControl { verbosity, location } => self.line(format!("llg_rt_finish_with_level({verbosity}, {});", c_string_literal(location))),
            IrStmt::StopControl { verbosity, location } => self.line(format!("llg_rt_stop_with_level({verbosity}, {});", c_string_literal(location))),
            IrStmt::ProgramExit => self.line("llg_program_exit();"),
            _ => return Err(pending("this statement's capture/cleanup path")),
        }
        Ok(())
    }
}
