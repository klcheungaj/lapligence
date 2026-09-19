//! Storage.

use super::*;

// ── Pass: unused_storage ──────────────────────────────────────────────────────

#[derive(Default)]
pub(super) struct Rw {
    pub(super) reads: HashSet<usize>,
    pub(super) writes: HashSet<usize>,
}

impl Rw {
    pub(super) fn read(&mut self, i: usize) {
        self.reads.insert(i);
    }
    fn write(&mut self, i: usize) {
        self.writes.insert(i);
    }
}

fn mark_dependency_read(dependency: &IrDependency, model: &IrModel, rw: &mut Rw) {
    if let IrDependency::PackedRange { storage, .. } = dependency {
        mark_dependency_read(storage, model, rw);
        return;
    }
    if let IrDependency::Scalar(name) | IrDependency::Real(name) = dependency {
        let alias_index = name
            .strip_prefix("llg_net_alias_")
            .and_then(|name| name.strip_suffix(".visible"))
            .and_then(|index| index.parse::<usize>().ok())
            .filter(|index| {
                model
                    .signals
                    .get(*index)
                    .is_some_and(|signal| !signal.net_alias.is_empty())
            });
        if let Some(i) = alias_index.or_else(|| {
            model
                .signals
                .iter()
                .position(|signal| signal.c_name == *name)
        }) {
            rw.read(i);
        }
    }
}

pub(super) fn mark_unused_storage(model: &mut IrModel, execution: Option<&[ExecutionProcess]>) {
    let mut rw = Rw::default();
    for access in &model.native_accesses {
        access
            .receiver
            .expressions(&mut |child| collect_expr_reads(child, model, &mut rw));
    }
    for allocation in &model.class_allocations {
        collect_stmts_rw(&allocation.body, model, &mut rw);
    }
    for object in &model.objects {
        if let Some(initial) = &object.initial {
            initial.expressions(&mut |child| collect_expr_reads(child, model, &mut rw));
        }
    }
    // Sensitivity lists and link sources are reads (plain globals only;
    // array-element addresses carry brackets and match nothing).
    let mut sens: Vec<IrDependency> = Vec::new();
    for p in &model.processes {
        if let crate::sim::ir::IrShape::SensLoop { reads } = &p.shape {
            sens.extend(reads.iter().cloned());
        }
        for x in &p.body {
            sens_lists_of(x, &mut sens);
        }
        for pre in &p.pre_fns {
            if let IrPreFn::Branch { body, .. } | IrPreFn::CapturedBranch { body, .. } = pre {
                for x in body {
                    sens_lists_of(x, &mut sens);
                }
            }
        }
    }
    if let Some(processes) = execution {
        for process in processes {
            for block in &process.blocks {
                for statement in &block.operations {
                    sens_lists_of(statement, &mut sens);
                }
                if let crate::sim::execution::ExecutionTerminator::Suspend {
                    trigger: TriggerPlan::Signals(signals),
                    ..
                } = &block.terminator
                {
                    sens.extend(signals.iter().cloned());
                }
            }
        }
    }
    for func in &model.funcs {
        for local in &func.locals {
            if let Some(initial) = &local.initial {
                collect_expr_reads(initial, model, &mut rw);
            }
        }
        collect_pre_fns_rw(&func.pre_fns, model, &mut rw);
        collect_stmts_rw(&func.body, model, &mut rw);
    }
    for p in &model.processes {
        collect_pre_fns_rw(&p.pre_fns, model, &mut rw);
        collect_stmts_rw(&p.body, model, &mut rw);
    }
    if let Some(processes) = execution {
        for process in processes {
            for block in &process.blocks {
                collect_stmts_rw(&block.operations, model, &mut rw);
            }
        }
    }
    for assertion in &model.assertions {
        rw.read(assertion.clock_signal);
        if let Some(disable) = assertion.disable_signal {
            rw.read(disable);
        }
        if let Some(condition) = &assertion.abort_condition {
            collect_expr_reads(condition, model, &mut rw);
        }
        if let Some(antecedent) = &assertion.antecedent {
            collect_expr_reads(antecedent, model, &mut rw);
        }
        if let Some(consequent) = &assertion.consequent {
            collect_expr_reads(consequent, model, &mut rw);
        }
        for sequence in [
            assertion.antecedent_sequence.as_ref(),
            assertion.consequent_sequence.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            for transition in &sequence.transitions {
                if let Some(clock) = transition.clock_signal {
                    rw.read(clock);
                }
            }
            for atom in &sequence.atoms {
                collect_expr_reads(atom, model, &mut rw);
            }
            for item in &sequence.match_items {
                collect_expr_reads(item, model, &mut rw);
            }
            for initializer in &sequence.initializers {
                collect_expr_reads(initializer, model, &mut rw);
            }
        }
    }
    for domain in &model.sampled_domains {
        rw.read(domain.clock_signal);
        collect_expr_reads(&domain.sample, model, &mut rw);
        if let Some(gate) = &domain.gate {
            collect_expr_reads(gate, model, &mut rw);
        }
    }
    for dependency in &sens {
        mark_dependency_read(dependency, model, &mut rw);
    }
    // A virtual-interface member is addressed through a runtime environment,
    // so its generated access has no textual signal dependency for this pass
    // to discover. Keep every descriptor member's concrete storage alive.
    for interface in &model.virtual_interfaces {
        for instance in &interface.instances {
            for signal in instance.members.iter().flatten() {
                rw.read(*signal);
            }
        }
    }
    // Declaration initializers count as writes and their RHSs count as reads.
    for step in &model.init_steps {
        match step {
            crate::sim::ir::IrInitStep::SetScalar { sig, .. } => rw.write(*sig),
            crate::sim::ir::IrInitStep::Initialize(initialization) => {
                collect_expr_reads(&initialization.value, model, &mut rw);
                if let crate::sim::ir::IrInitTarget::Signal(sig) = &initialization.target {
                    rw.write(*sig);
                }
            }
            _ => {}
        }
    }
    let waveform = model.waveform;
    let mut flags: Vec<bool> = model
        .signals
        .iter()
        .enumerate()
        .map(|(i, sig)| {
            let vpi_visible = model
                .vpi_objects
                .iter()
                .any(|object| object.signal == Some(i));
            !(waveform && sig.hdl_name.is_some())
                && !vpi_visible
                && !rw.reads.contains(&i)
                && !rw.writes.contains(&i)
        })
        .collect();
    // Alias source cells are retained for descriptor refreshes and debug/
    // waveform identity even when no generated expression directly reads the
    // raw storage.
    for (index, signal) in model.signals.iter().enumerate() {
        if !signal.net_alias.is_empty() {
            flags[index] = false;
        }
    }
    for (index, signal) in model.signals.iter().enumerate() {
        if !flags[index] {
            if let Some(target) = signal.alias {
                flags[target] = false;
            }
        }
    }
    for (sig, omit) in model.signals.iter_mut().zip(flags) {
        sig.omit = omit;
    }
}

/// Reads/writes of a function's or process's attached helpers: monitor/strobe
/// evaluators read their arguments; fork-branch coroutines run whole
/// statement trees.
fn collect_pre_fns_rw(pre_fns: &[IrPreFn], model: &IrModel, rw: &mut Rw) {
    for pre in pre_fns {
        match pre {
            IrPreFn::MonEval { args, context, .. } => {
                for e in args {
                    collect_expr_reads(e, model, rw);
                }
                if let Some(context) = context {
                    for capture in context.captures() {
                        collect_expr_reads(capture.initial(), model, rw);
                    }
                }
            }
            IrPreFn::EventAssign {
                captures, lhs, rhs, ..
            } => {
                for capture in captures {
                    collect_expr_reads(capture.initial(), model, rw);
                }
                collect_lhs_rw(lhs, model, rw);
                collect_expr_reads(rhs, model, rw);
            }
            IrPreFn::DeferredAssertion { captures, body, .. } => {
                for capture in captures {
                    collect_expr_reads(capture.initial(), model, rw);
                }
                collect_stmts_rw(body, model, rw);
            }
            IrPreFn::DisplayEval { args, .. } => {
                for arg in args {
                    arg.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                }
            }
            IrPreFn::RealEval { value, context, .. } => {
                collect_expr_reads(value, model, rw);
                if let Some(context) = context {
                    for capture in context.captures() {
                        collect_expr_reads(capture.initial(), model, rw);
                    }
                }
            }
            IrPreFn::ForceEval { value, .. } => collect_expr_reads(value, model, rw),
            IrPreFn::Branch { body, .. } => collect_stmts_rw(body, model, rw),
            IrPreFn::CapturedBranch { captures, body, .. } => {
                for capture in captures {
                    collect_expr_reads(capture.initial(), model, rw);
                }
                collect_stmts_rw(body, model, rw);
            }
        }
    }
}

/// Sensitivity lists nested inside statements (`wait (cond)`/event-control
/// wait sources).  Every statement position is visited: blocks, if/else,
/// loops, case items + default, and wait bodies.  Named-event entries are
/// skipped here (they are not signal storage).
fn sens_lists_of(s: &IrStmt, out: &mut Vec<IrDependency>) {
    match s {
        IrStmt::WaitCond { sens, body, .. } => {
            out.extend(sens.iter().cloned());
            for x in body {
                sens_lists_of(x, out);
            }
        }
        IrStmt::WaitEvents { specs } | IrStmt::ClockingCycleWait { specs, .. } => {
            for (src, _) in specs {
                match src {
                    IrWaitSrc::Sig(name) => out.push(IrDependency::scalar(name)),
                    IrWaitSrc::Real(name) => out.push(IrDependency::real(name)),
                    IrWaitSrc::Evaluated { reads, .. } | IrWaitSrc::EvaluatedReal { reads, .. } => {
                        out.extend(reads.iter().cloned())
                    }
                    _ => {}
                }
            }
        }
        IrStmt::WaitAny { sens } => out.extend(sens.iter().cloned()),
        IrStmt::Block(b)
        | IrStmt::ActivationScope { body: b, .. }
        | IrStmt::While { body: b, .. }
        | IrStmt::Repeat { body: b, .. }
        | IrStmt::Forever { body: b } => {
            for x in b {
                sens_lists_of(x, out);
            }
        }
        IrStmt::If { then_, els, .. } => {
            for x in then_ {
                sens_lists_of(x, out);
            }
            if let Some(els) = els {
                for x in els {
                    sens_lists_of(x, out);
                }
            }
        }
        IrStmt::ImmediateAssertion {
            if_true, if_false, ..
        } => {
            if let Some(if_true) = if_true {
                for x in if_true {
                    sens_lists_of(x, out);
                }
            }
            if let Some(if_false) = if_false {
                for x in if_false {
                    sens_lists_of(x, out);
                }
            }
        }
        IrStmt::DeferredImmediateAssertion { .. } => {}
        IrStmt::For {
            init, incr, body, ..
        } => {
            for x in init.iter().chain(incr.iter()).chain(body.iter()) {
                sens_lists_of(x, out);
            }
        }
        IrStmt::Case { items, .. } => {
            for item in items {
                for x in &item.body {
                    sens_lists_of(x, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_stmts_rw(stmts: &[IrStmt], model: &IrModel, rw: &mut Rw) {
    for s in stmts {
        collect_stmt_rw(s, model, rw);
    }
}

fn collect_stmt_rw(s: &IrStmt, model: &IrModel, rw: &mut Rw) {
    if let Some(value) = s.delay_expression() {
        collect_expr_reads(value, model, rw);
    }
    match s {
        IrStmt::VpiCall { args, .. } => {
            for arg in args {
                collect_expr_reads(arg, model, rw);
            }
        }
        IrStmt::Memory {
            path,
            start,
            finish,
            ..
        } => {
            path.expressions(&mut |child| collect_expr_reads(child, model, rw));
            if let Some(start) = start {
                collect_expr_reads(start, model, rw);
            }
            if let Some(finish) = finish {
                collect_expr_reads(finish, model, rw);
            }
        }
        IrStmt::Container(operation) => {
            operation.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrStmt::StreamAssign {
            source, targets, ..
        } => {
            collect_expr_reads(source, model, rw);
            for target in targets {
                match target {
                    IrStreamTarget::Packed { lhs, .. } => collect_lhs_rw(lhs, model, rw),
                    IrStreamTarget::Container { selector, .. } => {
                        if let Some(selector) = selector {
                            collect_stream_selector_reads(selector, model, rw);
                        }
                    }
                    IrStreamTarget::FixedSelector { selector, .. } => {
                        collect_stream_selector_reads(selector, model, rw);
                    }
                }
            }
        }
        IrStmt::Object(operation) => {
            operation.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrStmt::PlusArg(expression) => collect_expr_reads(expression, model, rw),
        IrStmt::Block(b) | IrStmt::ActivationScope { body: b, .. } => {
            collect_stmts_rw(b, model, rw)
        }
        IrStmt::DeclLocal {
            init: Some(init), ..
        } => collect_expr_reads(init, model, rw),
        IrStmt::Assign { lhs, rhs, .. }
        | IrStmt::DelayedAssign { lhs, rhs, .. }
        | IrStmt::ClockingDrive { lhs, rhs, .. }
        | IrStmt::InertialAssign { lhs, rhs, .. } => {
            collect_lhs_rw(lhs, model, rw);
            collect_expr_reads(rhs, model, rw);
        }
        IrStmt::Stochastic(operation) => match operation.as_ref() {
            IrStochasticStmt::Initialize {
                q_id,
                q_type,
                max_length,
                status,
            } => {
                collect_expr_reads(q_id, model, rw);
                collect_expr_reads(q_type, model, rw);
                collect_expr_reads(max_length, model, rw);
                collect_lhs_rw(status, model, rw);
            }
            IrStochasticStmt::Add {
                q_id,
                job_id,
                inform_id,
                status,
            } => {
                collect_expr_reads(q_id, model, rw);
                collect_expr_reads(job_id, model, rw);
                collect_expr_reads(inform_id, model, rw);
                collect_lhs_rw(status, model, rw);
            }
            IrStochasticStmt::Remove {
                q_id,
                job_id,
                inform_id,
                status,
            } => {
                collect_expr_reads(q_id, model, rw);
                collect_lhs_rw(job_id, model, rw);
                collect_lhs_rw(inform_id, model, rw);
                collect_lhs_rw(status, model, rw);
            }
            IrStochasticStmt::Exam {
                q_id,
                stat_code,
                stat_value,
                status,
            } => {
                collect_expr_reads(q_id, model, rw);
                collect_expr_reads(stat_code, model, rw);
                collect_lhs_rw(stat_value, model, rw);
                collect_lhs_rw(status, model, rw);
            }
        },
        IrStmt::NonblockingEventAssignWhen {
            lhs,
            rhs,
            repeat,
            captures,
            ..
        } => {
            collect_lhs_rw(lhs, model, rw);
            collect_expr_reads(rhs, model, rw);
            if let Some(repeat) = repeat {
                collect_expr_reads(repeat, model, rw);
            }
            for capture in captures {
                collect_expr_reads(capture.initial(), model, rw);
            }
        }
        IrStmt::DelayedStringAssign { rhs, .. } => {
            rhs.expressions(&mut |expr| collect_expr_reads(expr, model, rw));
        }
        IrStmt::PcaAssign {
            sig, enable, value, ..
        }
        | IrStmt::PcaDrive {
            sig, enable, value, ..
        } => {
            rw.write(*sig);
            rw.write(*enable);
            collect_expr_reads(value, model, rw);
        }
        IrStmt::PcaDeassign { sig } => rw.write(*sig),
        IrStmt::ClockingSample { source, sample, .. } => {
            rw.read(*source);
            rw.write(*sample);
        }
        IrStmt::If {
            cond, then_, els, ..
        } => {
            collect_expr_reads(cond, model, rw);
            collect_stmts_rw(then_, model, rw);
            if let Some(els) = els {
                collect_stmts_rw(els, model, rw);
            }
        }
        IrStmt::ImmediateAssertion {
            condition,
            if_true,
            if_false,
            ..
        } => {
            collect_expr_reads(condition, model, rw);
            if let Some(if_true) = if_true {
                collect_stmts_rw(if_true, model, rw);
            }
            if let Some(if_false) = if_false {
                collect_stmts_rw(if_false, model, rw);
            }
        }
        IrStmt::DeferredImmediateAssertion {
            condition,
            if_true,
            if_false,
            ..
        } => {
            collect_expr_reads(condition, model, rw);
            for action in if_true.iter().chain(if_false.iter()) {
                for capture in action.captures() {
                    collect_expr_reads(capture.initial(), model, rw);
                }
            }
        }
        IrStmt::While { cond, body } | IrStmt::Repeat { count: cond, body } => {
            collect_expr_reads(cond, model, rw);
            collect_stmts_rw(body, model, rw);
        }
        IrStmt::Forever { body } => collect_stmts_rw(body, model, rw),
        IrStmt::For {
            init,
            cond,
            incr,
            body,
        } => {
            collect_stmts_rw(init, model, rw);
            collect_expr_reads(cond, model, rw);
            collect_stmts_rw(incr, model, rw);
            collect_stmts_rw(body, model, rw);
        }
        IrStmt::Case { sel, items, .. } => {
            collect_expr_reads(sel, model, rw);
            for item in items {
                for e in &item.exprs {
                    collect_expr_reads(e, model, rw);
                }
                collect_stmts_rw(&item.body, model, rw);
            }
        }
        IrStmt::WaitCond { cond, sens, body } => {
            collect_expr_reads(cond, model, rw);
            for dependency in sens {
                mark_dependency_read(dependency, model, rw);
            }
            collect_stmts_rw(body, model, rw);
        }
        IrStmt::WaitEventTriggered { body, .. } => collect_stmts_rw(body, model, rw),
        IrStmt::WaitOrder {
            success, failure, ..
        } => {
            collect_stmts_rw(success, model, rw);
            collect_stmts_rw(failure, model, rw);
        }
        // Event-control and combinational wait sources are reads of the
        // named signal globals (lowering emits their addresses into the wait
        // arrays; a signal referenced only here still needs its storage).
        // Named-event entries (`IrWaitSrc::Event`) are NOT storage — the
        // event globals are always emitted, so they are simply skipped.
        IrStmt::WaitEvents { specs } => {
            for (src, _) in specs {
                match src {
                    IrWaitSrc::Sig(name) => {
                        mark_dependency_read(&IrDependency::scalar(name), model, rw)
                    }
                    IrWaitSrc::Real(name) => {
                        mark_dependency_read(&IrDependency::real(name), model, rw)
                    }
                    IrWaitSrc::Evaluated { reads, .. } | IrWaitSrc::EvaluatedReal { reads, .. } => {
                        for dependency in reads {
                            mark_dependency_read(dependency, model, rw);
                        }
                    }
                    _ => {}
                }
            }
        }
        IrStmt::ClockingCycleWait { count, specs } => {
            collect_expr_reads(count, model, rw);
            for (src, _) in specs {
                match src {
                    IrWaitSrc::Sig(name) => {
                        mark_dependency_read(&IrDependency::scalar(name), model, rw)
                    }
                    IrWaitSrc::Real(name) => {
                        mark_dependency_read(&IrDependency::real(name), model, rw)
                    }
                    IrWaitSrc::Evaluated { reads, .. } | IrWaitSrc::EvaluatedReal { reads, .. } => {
                        for dependency in reads {
                            mark_dependency_read(dependency, model, rw);
                        }
                    }
                    _ => {}
                }
            }
        }
        IrStmt::WaitAny { sens } => {
            for dependency in sens {
                mark_dependency_read(dependency, model, rw);
            }
        }
        IrStmt::Force { lhs, value, .. } => {
            collect_lhs_rw(lhs, model, rw);
            collect_expr_reads(value, model, rw);
        }
        IrStmt::Release { lhs } => collect_lhs_rw(lhs, model, rw),
        IrStmt::Display { args, .. } => {
            for (e, _) in args {
                collect_expr_reads(e, model, rw);
            }
        }
        IrStmt::DisplayTyped {
            args, descriptor, ..
        } => {
            for arg in args {
                arg.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
            }
            if let Some(descriptor) = descriptor {
                collect_expr_reads(descriptor, model, rw);
            }
        }
        IrStmt::Severity { args, .. } => {
            for arg in args {
                arg.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
            }
        }
        IrStmt::AssertionControl { args, .. } => {
            for arg in args {
                collect_expr_reads(arg, model, rw);
            }
        }
        IrStmt::MonitorSet {
            descriptor: Some(descriptor),
            ..
        }
        | IrStmt::FileControl {
            descriptor: Some(descriptor),
            ..
        } => collect_expr_reads(descriptor, model, rw),
        IrStmt::TimeFormat {
            units,
            precision,
            suffix,
            minimum_field_width,
        } => {
            collect_expr_reads(units, model, rw);
            collect_expr_reads(precision, model, rw);
            collect_expr_reads(minimum_field_width, model, rw);
            suffix.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
        }
        IrStmt::WaveLimit(limit) => collect_expr_reads(limit, model, rw),
        IrStmt::Call(call) => collect_call_rw(call, model, rw),
        IrStmt::Return { value: Some(value) } => collect_expr_reads(value, model, rw),
        // Task-position `$system` and process RNG statements carry owned string
        // or packed expressions whose signal reads must keep their storage.
        IrStmt::System(Some(command)) => {
            command.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrStmt::RandomSeed { seed } => collect_expr_reads(seed, model, rw),
        IrStmt::RandomStateSet { state } => {
            state.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrStmt::DeclString {
            init: Some(init), ..
        } => init.expressions(&mut |child| collect_expr_reads(child, model, rw)),
        // Captures are evaluated at the fork site in the parent process, before
        // any detached branch runs; their initializers are ordinary reads.
        IrStmt::CapturedFork { branches, .. } => {
            for branch in branches {
                for capture in &branch.captures {
                    collect_expr_reads(capture.initial(), model, rw);
                }
            }
        }
        _ => {}
    }
}

fn collect_stream_selector_reads(selector: &IrStreamSelector, model: &IrModel, rw: &mut Rw) {
    match selector {
        IrStreamSelector::Index(index) => collect_expr_reads(index, model, rw),
        IrStreamSelector::Range { left, right } => {
            collect_expr_reads(left, model, rw);
            collect_expr_reads(right, model, rw);
        }
        IrStreamSelector::Indexed { base, width, .. } => {
            collect_expr_reads(base, model, rw);
            collect_expr_reads(width, model, rw);
        }
    }
}

fn collect_call_rw(call: &crate::sim::ir::IrCall, model: &IrModel, rw: &mut Rw) {
    if let Some(virtual_call) = &call.virtual_call {
        virtual_call
            .receiver
            .expressions(&mut |child| collect_expr_reads(child, model, rw));
    }
    for (index, arg) in call.args.iter().enumerate() {
        match arg {
            IrCallArg::Val(e) => collect_expr_reads(e, model, rw),
            IrCallArg::StringVal(value) => {
                value.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
            }
            IrCallArg::OutAddr(addr) => {
                // `&G_sig` / `G_sig`: a passed output/inout actual both reads
                // and writes its target (function-scope names never match).
                let name = addr.trim_start_matches('&');
                if let Some(i) = model.signals.iter().position(|sg| sg.c_name == name) {
                    rw.read(i);
                    rw.write(i);
                }
            }
            IrCallArg::RefAddr { lhs, read, .. } => {
                collect_expr_reads(read, model, rw);
                if call_formal(model, call.function_index(), index)
                    .is_none_or(|formal| !formal.is_const_ref())
                {
                    collect_lhs_rw(lhs, model, rw);
                }
            }
            IrCallArg::StringOutAddr(addr) | IrCallArg::StringRefAddr { addr, .. } => {
                let name = addr.trim_start_matches('&');
                if let Some(i) = model.signals.iter().position(|sg| sg.c_name == name) {
                    rw.read(i);
                    if call_formal(model, call.function_index(), index)
                        .is_none_or(|formal| !formal.is_const_ref())
                    {
                        rw.write(i);
                    }
                }
            }
            IrCallArg::OutTemp {
                init,
                writeback,
                storage_lhs,
                storage_read,
                selector_inits,
                ..
            } => {
                if let Some(init) = init {
                    collect_expr_reads(init, model, rw);
                }
                collect_lhs_rw(writeback, model, rw);
                if let Some(storage_lhs) = storage_lhs {
                    collect_lhs_rw(storage_lhs, model, rw);
                }
                if let Some(storage_read) = storage_read {
                    collect_expr_reads(storage_read, model, rw);
                }
                for (_, _, _, _, init) in selector_inits {
                    collect_expr_reads(init, model, rw);
                }
            }
            IrCallArg::StringOutTemp {
                init, storage_read, ..
            } => {
                if let Some(init) = init {
                    init.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                }
                if let Some(read) = storage_read {
                    read.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                }
            }
            IrCallArg::ChandleVal(_) | IrCallArg::ChandleAddr(_) | IrCallArg::ChandleRefAddr(_) => {
            }
        }
    }
    // Caller-side output/inout temps live here, not in `args`: their
    // initializers read the actual's current value.
    for (_, _, init) in &call.temps {
        if let Some(init) = init {
            collect_expr_reads(init, model, rw);
        }
    }
    // Copy-out assignments after the call write the actual targets (their
    // select indices are reads).
    for (lh, ..) in &call.copyouts {
        collect_lhs_rw(lh, model, rw);
    }
}

fn collect_lhs_rw(l: &IrLhs, model: &IrModel, rw: &mut Rw) {
    match l {
        IrLhs::PackedSelect { target, steps, .. } => {
            collect_lhs_rw(target, model, rw);
            for step in steps { collect_expr_reads(&step.base, model, rw); }
        }
        IrLhs::Whole(i) => rw.write(*i),
        IrLhs::WholeRef { .. } => {}
        IrLhs::Ref { bit, .. } => {
            if let Some(index) = bit {
                collect_expr_reads(index, model, rw);
            }
        }
        IrLhs::Bit(i, idx, _) => {
            rw.write(*i);
            collect_expr_reads(idx, model, rw);
        }
        IrLhs::Part(i, ..) => rw.write(*i),
        IrLhs::IdxPart(i, base, width, ..) => {
            rw.write(*i);
            collect_expr_reads(base, model, rw);
            collect_expr_reads(width, model, rw);
        }
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => {
            for idx in indices {
                collect_expr_reads(idx, model, rw);
            }
            elem_sel.expressions(&mut |idx| collect_expr_reads(idx, model, rw));
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                collect_lhs_rw(part, model, rw);
            }
        }
    }
}

fn collect_lhs_read(l: &IrLhs, model: &IrModel, rw: &mut Rw) {
    match l {
        IrLhs::PackedSelect { target, steps, .. } => {
            collect_lhs_read(target, model, rw);
            for step in steps { collect_expr_reads(&step.base, model, rw); }
        }
        IrLhs::Whole(i) | IrLhs::Bit(i, ..) | IrLhs::Part(i, ..) | IrLhs::IdxPart(i, ..) => {
            rw.read(*i);
            match l {
                IrLhs::Bit(_, index, _) => collect_expr_reads(index, model, rw),
                IrLhs::IdxPart(_, base, width, ..) => {
                    collect_expr_reads(base, model, rw);
                    collect_expr_reads(width, model, rw);
                }
                IrLhs::Whole(_) | IrLhs::Part(..) => {}
                _ => unreachable!(),
            }
        }
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => {
            for index in indices {
                collect_expr_reads(index, model, rw);
            }
            elem_sel.expressions(&mut |index| collect_expr_reads(index, model, rw));
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                collect_lhs_read(part, model, rw);
            }
        }
        IrLhs::Ref { bit, .. } => {
            if let Some(index) = bit {
                collect_expr_reads(index, model, rw);
            }
        }
        IrLhs::WholeRef { .. } => {}
    }
}

fn collect_expr_reads(e: &IrExpr, model: &IrModel, rw: &mut Rw) {
    match &e.kind {
        IrExprKind::SigRead(i) => rw.read(*i),
        IrExprKind::LocalRead(_) | IrExprKind::FormalRead(_) => {}
        IrExprKind::Verbatim { .. } => {}
        _ => {
            // Recurse structurally via a read-only walker.
            collect_children_reads(e, model, rw);
        }
    }
}

fn collect_children_reads(e: &IrExpr, model: &IrModel, rw: &mut Rw) {
    match &e.kind {
        IrExprKind::Container(operation) => {
            if let Some(signal) = operation.traversal_signal() {
                rw.read(signal);
            }
            operation.expressions(&mut |child| collect_expr_reads(child, model, rw));
        }
        IrExprKind::ObjectQuery(query) => {
            query.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrExprKind::EnumMethod(query) => {
            query.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            collect_expr_reads(a, model, rw);
            collect_expr_reads(b, model, rw);
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::RealUn { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::BitStreamCast { a, .. }
        | IrExprKind::ToTwoState { a }
        | IrExprKind::CastToReal { a, .. } => collect_expr_reads(a, model, rw),
        IrExprKind::Mux { sel, a, b } => {
            collect_expr_reads(sel, model, rw);
            collect_expr_reads(a, model, rw);
            collect_expr_reads(b, model, rw);
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            for p in parts {
                collect_expr_reads(p, model, rw);
            }
        }
        IrExprKind::Stream { value, .. } => collect_expr_reads(value, model, rw),
        IrExprKind::FixedStream { selector, .. } => {
            collect_stream_selector_reads(selector, model, rw)
        }
        IrExprKind::Inside { value, items } => {
            collect_expr_reads(value, model, rw);
            for item in items {
                match item {
                    IrInsideItem::Value(item) => collect_expr_reads(item, model, rw),
                    IrInsideItem::Range { low, high } => {
                        collect_expr_reads(low, model, rw);
                        collect_expr_reads(high, model, rw);
                    }
                    IrInsideItem::OpenRange { low, high } => {
                        if let Some(low) = low {
                            collect_expr_reads(low, model, rw);
                        }
                        if let Some(high) = high {
                            collect_expr_reads(high, model, rw);
                        }
                    }
                    IrInsideItem::Container { .. } => {}
                }
            }
        }
        IrExprKind::BitSel { base, idx } => {
            collect_expr_reads(base, model, rw);
            collect_expr_reads(idx, model, rw);
        }
        IrExprKind::Mutation(mutation) => {
            collect_lhs_rw(&mutation.lhs, model, rw);
            collect_expr_reads(&mutation.value, model, rw);
        }
        IrExprKind::DynamicCast(cast) => {
            collect_lhs_rw(&cast.lhs, model, rw);
            collect_expr_reads(&cast.rhs, model, rw);
            if let Some(source) = &cast.class_source {
                source.expressions(&mut |child| collect_expr_reads(child, model, rw));
            }
            for value in &cast.valid_values {
                collect_expr_reads(value, model, rw);
            }
        }
        IrExprKind::PartSel { base, .. } => collect_expr_reads(base, model, rw),
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => {
            collect_expr_reads(base, model, rw);
            collect_expr_reads(base_idx, model, rw);
            collect_expr_reads(width_expr, model, rw);
        }
        IrExprKind::ArrayRead {
            arr,
            indices,
            elem_sel,
        } => {
            let _ = arr;
            for i in indices {
                collect_expr_reads(i, model, rw);
            }
            elem_sel.expressions(&mut |idx| collect_expr_reads(idx, model, rw));
        }
        IrExprKind::CallFn(call) => {
            collect_call_rw_readonly(call.function_index(), &call.args, model, rw);
            if let Some(virtual_call) = &call.virtual_call {
                virtual_call
                    .receiver
                    .expressions(&mut |child| collect_expr_reads(child, model, rw));
            }
        }
        IrExprKind::SysFunc(sf) => match sf {
            IrSysFunc::TestPlusArgs { pattern } => {
                pattern.expressions(&mut |expression| collect_expr_reads(expression, model, rw))
            }
            IrSysFunc::ValuePlusArgs { format, target } => {
                format.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                match target {
                    crate::sim::ir::IrPlusArgTarget::Packed { lhs, .. }
                    | crate::sim::ir::IrPlusArgTarget::Real { lhs, .. } => {
                        collect_lhs_rw(lhs, model, rw)
                    }
                    crate::sim::ir::IrPlusArgTarget::String { .. } => {}
                }
            }
            IrSysFunc::System(Some(command)) => {
                command.expressions(&mut |child| collect_expr_reads(child, model, rw));
            }
            IrSysFunc::System(None) => {}
            IrSysFunc::VpiCall { args, .. } => {
                for arg in args {
                    collect_expr_reads(arg, model, rw);
                }
            }
            IrSysFunc::LegacyRandom { seed, args, .. } => {
                if let Some(seed) = seed {
                    collect_lhs_rw(seed, model, rw);
                    collect_lhs_read(seed, model, rw);
                }
                for arg in args {
                    collect_expr_reads(arg, model, rw);
                }
            }
            IrSysFunc::Urandom { seed } => {
                if let Some(seed) = seed {
                    collect_expr_reads(seed, model, rw);
                }
            }
            IrSysFunc::UrandomRange { max, min } => {
                collect_expr_reads(max, model, rw);
                if let Some(min) = min {
                    collect_expr_reads(min, model, rw);
                }
            }
            IrSysFunc::Clog2(a)
            | IrSysFunc::Bits(a)
            | IrSysFunc::BitQuery { arg: a, .. }
            | IrSysFunc::Rtoi(a)
            | IrSysFunc::Itor(a)
            | IrSysFunc::RealToBits(a)
            | IrSysFunc::BitsToReal(a)
            | IrSysFunc::ShortRealToBits(a)
            | IrSysFunc::BitsToShortReal(a) => collect_expr_reads(a, model, rw),
            IrSysFunc::Math { args, .. } => {
                for arg in args {
                    collect_expr_reads(arg, model, rw);
                }
            }
            IrSysFunc::QFull { q_id, status } => {
                collect_expr_reads(q_id, model, rw);
                collect_lhs_rw(status, model, rw);
            }
            IrSysFunc::Time { .. } | IrSysFunc::Realtime { .. } => {}
            IrSysFunc::FileOpen { path, mode } => {
                path.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                if let Some(mode) = mode {
                    mode.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                }
            }
            IrSysFunc::FileTell(descriptor) | IrSysFunc::FileEof(descriptor) => {
                collect_expr_reads(descriptor, model, rw)
            }
            IrSysFunc::FileSeek {
                descriptor,
                offset,
                operation,
            } => {
                collect_expr_reads(descriptor, model, rw);
                collect_expr_reads(offset, model, rw);
                collect_expr_reads(operation, model, rw);
            }
            IrSysFunc::FileError { descriptor, .. } => collect_expr_reads(descriptor, model, rw),
            IrSysFunc::FileInput(input) => {
                input.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                match input {
                    crate::sim::ir::IrFileInput::ScanFile { targets, .. }
                    | crate::sim::ir::IrFileInput::ScanString { targets, .. } => {
                        for target in targets {
                            match target {
                                crate::sim::ir::IrFileInputTarget::Packed { lhs, .. }
                                | crate::sim::ir::IrFileInputTarget::Real { lhs, .. } => {
                                    collect_lhs_rw(lhs, model, rw)
                                }
                                crate::sim::ir::IrFileInputTarget::String { .. } => {}
                            }
                        }
                    }
                    crate::sim::ir::IrFileInput::Read { target, .. } => {
                        if let crate::sim::ir::IrFileReadTarget::Packed { lhs, .. } = target {
                            collect_lhs_rw(lhs, model, rw);
                        }
                    }
                    crate::sim::ir::IrFileInput::Gets { target, .. } => match target {
                        crate::sim::ir::IrFileInputTarget::Packed { lhs, .. }
                        | crate::sim::ir::IrFileInputTarget::Real { lhs, .. } => {
                            collect_lhs_rw(lhs, model, rw);
                        }
                        crate::sim::ir::IrFileInputTarget::String { .. } => {}
                    },
                    crate::sim::ir::IrFileInput::Getc { .. }
                    | crate::sim::ir::IrFileInput::Ungetc { .. } => {}
                }
            }
            IrSysFunc::Sampled(call) => collect_expr_reads(&call.argument, model, rw),
        },
        _ => {}
    }
}

fn collect_call_rw_readonly(function: usize, args: &[IrCallArg], model: &IrModel, rw: &mut Rw) {
    for (index, arg) in args.iter().enumerate() {
        match arg {
            IrCallArg::Val(e) => collect_expr_reads(e, model, rw),
            IrCallArg::StringVal(value) => {
                value.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
            }
            IrCallArg::OutAddr(addr) => {
                let name = addr.trim_start_matches('&');
                if let Some(i) = model.signals.iter().position(|sg| sg.c_name == name) {
                    rw.read(i);
                }
            }
            IrCallArg::RefAddr { lhs, read, .. } => {
                collect_expr_reads(read, model, rw);
                if call_formal(model, function, index).is_none_or(|formal| !formal.is_const_ref()) {
                    collect_lhs_rw(lhs, model, rw);
                }
            }
            IrCallArg::StringOutAddr(addr) | IrCallArg::StringRefAddr { addr, .. } => {
                let name = addr.trim_start_matches('&');
                if let Some(i) = model.signals.iter().position(|sg| sg.c_name == name) {
                    rw.read(i);
                }
            }
            IrCallArg::OutTemp {
                init,
                writeback,
                storage_lhs,
                storage_read,
                selector_inits,
                ..
            } => {
                if let Some(init) = init {
                    collect_expr_reads(init, model, rw);
                }
                // Writebacks inside expression calls copy temps back into the
                // actuals: count as writes.
                collect_lhs_rw(writeback, model, rw);
                if let Some(storage_lhs) = storage_lhs {
                    collect_lhs_rw(storage_lhs, model, rw);
                }
                if let Some(storage_read) = storage_read {
                    collect_expr_reads(storage_read, model, rw);
                }
                for (_, _, _, _, init) in selector_inits {
                    collect_expr_reads(init, model, rw);
                }
            }
            IrCallArg::StringOutTemp {
                init, storage_read, ..
            } => {
                if let Some(init) = init {
                    init.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                }
                if let Some(read) = storage_read {
                    read.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                }
            }
            IrCallArg::ChandleVal(_) | IrCallArg::ChandleAddr(_) | IrCallArg::ChandleRefAddr(_) => {
            }
        }
    }
}

fn call_formal(model: &IrModel, function: usize, index: usize) -> Option<&IrFormal> {
    let callee = model.funcs.get(function)?;
    callee
        .formals
        .iter()
        .filter(|formal| formal.is_address())
        .chain(callee.formals.iter().filter(|formal| !formal.is_address()))
        .nth(index)
}
