//! Unused storage: a signal that no IR construct reads or writes gets
//! `omit = true` and the emitter skips its declaration.
//!
//! Omitting storage that generated code still names breaks the model, so the
//! walk must see every reference, including those that are not expression
//! reads: wait sources, event-handle indices, call receivers, C-address
//! actuals and runtime registrations. Every IR enum and the model and call
//! records are matched without a wildcard arm or rest pattern, so a new
//! statement, expression, wait source, call argument or table fails to
//! compile here until it is classified. Nested payloads are walked through
//! their IR-owned `expressions()` visitors.

use std::collections::HashMap;

use super::*;

// ── Pass: unused_storage ──────────────────────────────────────────────────────

/// Signals referenced so far, plus a name index: lowering spells some
/// references as C names (`&G_x` actuals, wait sources, dependencies), and a
/// linear search per reference made the pass quadratic in the signal count.
pub(super) struct Rw {
    reads: Vec<bool>,
    writes: Vec<bool>,
    names: SignalNames,
}

impl Rw {
    fn new(signals: &[crate::sim::ir::IrSignal]) -> Self {
        Self {
            reads: vec![false; signals.len()],
            writes: vec![false; signals.len()],
            names: SignalNames::new(signals),
        }
    }
    pub(super) fn read(&mut self, i: usize) {
        if let Some(read) = self.reads.get_mut(i) {
            *read = true;
        }
    }
    fn write(&mut self, i: usize) {
        if let Some(write) = self.writes.get_mut(i) {
            *write = true;
        }
    }
    fn referenced(&self, i: usize) -> bool {
        self.reads[i] || self.writes[i]
    }
    /// Count an address actual (`&G_sig` or `G_sig`) as a read and, unless it
    /// is read-only, a write. Function-scope names match no signal.
    fn address(&mut self, address: &str, write: bool) {
        if let Some(i) = self.names.get(address.trim_start_matches('&')) {
            self.read(i);
            if write {
                self.write(i);
            }
        }
    }
}

/// The first signal with each C name, like a front-to-back search.
struct SignalNames(HashMap<String, usize>);

impl SignalNames {
    fn new(signals: &[crate::sim::ir::IrSignal]) -> Self {
        let mut names = HashMap::with_capacity(signals.len());
        for (index, signal) in signals.iter().enumerate() {
            names.entry(signal.c_name.clone()).or_insert(index);
        }
        Self(names)
    }

    fn get(&self, name: &str) -> Option<usize> {
        self.0.get(name).copied()
    }

    /// The signal a scalar, real or packed-range dependency names.
    fn dependency(
        &self,
        dependency: &IrDependency,
        signals: &[crate::sim::ir::IrSignal],
    ) -> Option<usize> {
        match dependency {
            IrDependency::PackedRange { storage, .. } => self.dependency(storage, signals),
            IrDependency::Scalar(name) | IrDependency::Real(name) => name
                .strip_prefix("llg_net_alias_")
                .and_then(|name| name.strip_suffix(".visible"))
                .and_then(|index| index.parse::<usize>().ok())
                .filter(|index| {
                    signals
                        .get(*index)
                        .is_some_and(|signal| !signal.net_alias.is_empty())
                })
                .or_else(|| self.get(name)),
            // Arrays, containers, objects and activation storage are not
            // signals.
            IrDependency::ArrayElement { .. }
            | IrDependency::ArrayContents(_)
            | IrDependency::ContainerContents(_)
            | IrDependency::ContainerShape(_)
            | IrDependency::Object(_)
            | IrDependency::SharedCell { .. }
            | IrDependency::SharedNative { .. }
            | IrDependency::RefFormal { .. }
            | IrDependency::NativeRefFormal { .. }
            | IrDependency::NativeAccess(_)
            | IrDependency::InterfaceInstance { .. } => None,
        }
    }
}

fn mark_dependency_read(dependency: &IrDependency, model: &IrModel, rw: &mut Rw) {
    if let Some(i) = rw.names.dependency(dependency, &model.signals) {
        rw.read(i);
    }
}

pub(super) fn mark_unused_storage(model: &mut IrModel, execution: Option<&[ExecutionProcess]>) {
    let mut rw = Rw::new(&model.signals);
    // Every table is named, without a rest pattern, so a new table fails to
    // compile here until it is classified as signal-referencing or not.
    let IrModel {
        design_name: _,
        precision_fs: _,
        // Waveform models keep every HDL-visible signal (below).
        waveform,
        wave_start: _,
        signals: _,
        net_groups: _,
        arrays,
        containers,
        objects,
        classes: _,
        native_accesses,
        native_types: _,
        native_values,
        pattern_types: _,
        pattern_classes: _,
        class_allocations,
        virtual_interfaces,
        udp_tables: _,
        udp_states: _,
        module_paths,
        // Switch nodes name net groups, never pruned signals.
        switch_network,
        path_pulse: _,
        timing_checks,
        timing_check_options: _,
        timing_doorbell,
        sdf_annotations: _,
        sdf_startup: _,
        sdf_gate_delays: _,
        sdf_loads: _,
        events: _,
        funcs,
        assertions,
        sampled_clocks,
        sampled_domains,
        processes,
        init_steps,
        spawns: _,
        final_spawns: _,
        vpi_objects,
        vpi_typespecs: _,
        vpi_compile_calls: _,
        vpi_builtin_calls: _,
        vpi_catalog: _,
        vpi_delays_writable: _,
        vpi_delay_select: _,
        random: _,
        rand_problems: _,
        // Free variables are signals the solve callbacks write and the
        // assertions read; both are walked with their owners.
        free_variables: _,
        free_sets: _,
        // Coverage metadata names no storage; samples read through statements.
        covergroups: _,
        coverage_types: _,
        assertion_coverage: _,
        dpi_scopes: _,
        dpi_sites: _,
    } = &*model;
    for access in native_accesses {
        access
            .receiver
            .expressions(&mut |child| collect_expr_reads(child, model, &mut rw));
    }
    // A class-member container or native value is reached through its
    // receiver handle expression.
    for receiver in containers
        .iter()
        .filter_map(|container| container.receiver.as_ref())
        .chain(
            native_values
                .iter()
                .filter_map(|value| value.receiver.as_ref()),
        )
    {
        receiver.expressions(&mut |child| collect_expr_reads(child, model, &mut rw));
    }
    // The runtime reads module path sources through their storage address
    // and evaluates the path conditions when it schedules a destination.
    for set in module_paths {
        for &signal in &set.sources {
            rw.read(signal);
        }
        for condition in &set.conditions {
            collect_expr_reads(condition, model, &mut rw);
        }
    }
    // The runtime reads direct switch controls through their storage address.
    for control in &switch_network.controls {
        rw.read(control.signal);
    }
    // The runtime reads timing check terminals through their storage address
    // and evaluates the event conditions when a step needs them. It writes
    // the notifiers, the delayed signals and the doorbell of the timing
    // process.
    for set in timing_checks {
        for terminal in &set.terminals {
            rw.read(terminal.signal);
        }
        for condition in &set.conditions {
            collect_expr_reads(condition, model, &mut rw);
        }
        for &notifier in &set.notifiers {
            rw.read(notifier);
            rw.write(notifier);
        }
        for line in &set.lines {
            if let Some(target) = set.terminals.get(line.target as usize) {
                rw.write(target.signal);
            }
        }
    }
    if let Some(doorbell) = *timing_doorbell {
        rw.read(doorbell);
        rw.write(doorbell);
    }
    // A net-array cell observes its canonical resolved net signal.
    for (_, signal) in arrays.iter().flat_map(|array| &array.net_elements) {
        rw.read(*signal);
    }
    for allocation in class_allocations {
        collect_stmts_rw(&allocation.body, model, &mut rw);
    }
    for object in objects {
        if let Some(initial) = &object.initial {
            initial.expressions(&mut |child| collect_expr_reads(child, model, &mut rw));
        }
    }
    // Sensitivity loops and execution trigger plans are reads; waits inside
    // statements are counted by the statement walk.
    for process in processes {
        match &process.shape {
            crate::sim::ir::IrShape::SensLoop { reads } => {
                for dependency in reads {
                    mark_dependency_read(dependency, model, &mut rw);
                }
            }
            crate::sim::ir::IrShape::RunOnce | crate::sim::ir::IrShape::Loop => {}
        }
    }
    if let Some(processes) = execution {
        for process in processes {
            for block in &process.blocks {
                if let crate::sim::execution::ExecutionTerminator::Suspend {
                    trigger: TriggerPlan::Signals(signals),
                    ..
                } = &block.terminator
                {
                    for dependency in signals {
                        mark_dependency_read(dependency, model, &mut rw);
                    }
                }
            }
        }
    }
    for func in funcs {
        // A lowered static result signal is read by the generated function
        // epilogue even when no IR expression directly names that return
        // value. Keep its storage alive through unused-storage pruning.
        if let Some(signal) = func.return_signal {
            rw.read(signal);
        }
        for local in &func.locals {
            if let Some(initial) = &local.initial {
                collect_expr_reads(initial, model, &mut rw);
            }
        }
        collect_pre_fns_rw(&func.pre_fns, model, &mut rw);
        collect_stmts_rw(&func.body, model, &mut rw);
    }
    for p in processes {
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
    for assertion in assertions {
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
        if let Some(property) = &assertion.property {
            for atom in &property.atoms {
                collect_expr_reads(atom, model, &mut rw);
            }
            for clock in &property.clocks {
                rw.read(clock.signal);
            }
        }
        for sequence in [
            assertion.antecedent_sequence.as_ref(),
            assertion.consequent_sequence.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(
            assertion
                .property
                .iter()
                .flat_map(|property| property.sequences.iter()),
        ) {
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
    for clock in sampled_clocks {
        match clock.kind {
            crate::sim::ir::IrSampledClockKind::Edge { signal, .. } => rw.read(signal),
            crate::sim::ir::IrSampledClockKind::Event => {}
        }
        if let Some(gate) = &clock.gate {
            collect_expr_reads(gate, model, &mut rw);
        }
    }
    for domain in sampled_domains {
        if domain.history_ticks != 0 {
            collect_expr_reads(&domain.sample, model, &mut rw);
        }
    }
    // A virtual-interface member is addressed through a runtime environment,
    // so its generated access has no textual signal dependency for this pass
    // to discover. Keep every descriptor member's concrete storage alive.
    for interface in virtual_interfaces {
        for instance in &interface.instances {
            for signal in instance.members.iter().flatten() {
                rw.read(*signal);
            }
        }
    }
    // Declaration initializers count as writes and their RHSs count as reads.
    for step in init_steps {
        match step {
            crate::sim::ir::IrInitStep::SetScalar { sig, .. } => rw.write(*sig),
            crate::sim::ir::IrInitStep::Initialize(initialization) => {
                collect_expr_reads(&initialization.value, model, &mut rw);
                match &initialization.target {
                    crate::sim::ir::IrInitTarget::Signal(sig) => rw.write(*sig),
                    crate::sim::ir::IrInitTarget::Fixed(lhs) => collect_lhs_rw(lhs, model, &mut rw),
                    crate::sim::ir::IrInitTarget::StaticLocal { .. } => {}
                }
            }
            crate::sim::ir::IrInitStep::Execute { body, .. } => {
                collect_stmt_rw(body, model, &mut rw);
            }
            // Sampled-value registration takes the signal's address.
            crate::sim::ir::IrInitStep::RegisterSampled(sig)
            | crate::sim::ir::IrInitStep::RegisterSampledHistory { sig, .. }
            | crate::sim::ir::IrInitStep::RegisterSampledValue(sig) => rw.read(*sig),
            // Arrays and net groups are not signal storage.
            crate::sim::ir::IrInitStep::FillArrayX(_)
            | crate::sim::ir::IrInitStep::FillArrayZ(_)
            | crate::sim::ir::IrInitStep::SetArrayElem { .. }
            | crate::sim::ir::IrInitStep::WriteNet { .. } => {}
        }
    }
    let waveform = *waveform;
    // One pass over the catalog: every signal is VPI-visible in a model with
    // a full catalog, so a per-signal scan would be quadratic.
    let mut vpi_visible = vec![false; model.signals.len()];
    for object in vpi_objects {
        if let Some(visible) = object.signal.and_then(|signal| vpi_visible.get_mut(signal)) {
            *visible = true;
        }
    }
    let mut flags: Vec<bool> = model
        .signals
        .iter()
        .enumerate()
        .map(|(i, sig)| {
            !(waveform && sig.hdl_name.is_some()) && !vpi_visible[i] && !rw.referenced(i)
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
    // A writer analysis may name leaves no statement touches (a record bound
    // whole to a native `ref` formal whose callee writes only some members,
    // SIM-008); omitted storage leaves the process write lists.
    let names = rw.names;
    let signals = &model.signals;
    for process in &mut model.processes {
        process.writes.retain(|write| {
            names
                .dependency(write, signals)
                .is_none_or(|index| !signals[index].omit)
        });
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

fn collect_stmts_rw(stmts: &[IrStmt], model: &IrModel, rw: &mut Rw) {
    for s in stmts {
        collect_stmt_rw(s, model, rw);
    }
}

fn collect_stmt_rw(s: &IrStmt, model: &IrModel, rw: &mut Rw) {
    for value in s.delay_expressions() {
        collect_expr_reads(value, model, rw);
    }
    match s.unlocated() {
        IrStmt::VpiCall { args, .. } | IrStmt::UdpStep { inputs: args, .. } => {
            for arg in args {
                collect_expr_reads(arg, model, rw);
            }
        }
        IrStmt::SwitchControl { state, .. } => collect_expr_reads(state, model, rw),
        IrStmt::MemoryAccess(task) => {
            // A load reads the cells outside its range and stores the root.
            if let Some(lhs) = task.image_target() {
                collect_lhs_rw(lhs, model, rw);
                collect_lhs_read(lhs, model, rw);
            }
            task.expressions(&mut |child| collect_expr_reads(child, model, rw));
        }
        IrStmt::Memory {
            path,
            text,
            view,
            start,
            finish,
            ..
        } => {
            path.expressions(&mut |child| collect_expr_reads(child, model, rw));
            for selector in &view.selectors {
                collect_expr_reads(&selector.value, model, rw);
            }
            if let Some(start) = start {
                collect_expr_reads(start, model, rw);
            }
            if let Some(finish) = finish {
                collect_expr_reads(finish, model, rw);
            }
            if let Some(text) = text {
                text.expressions(&mut |child| collect_expr_reads(child, model, rw));
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
                    IrStreamTarget::FixedImageSelector {
                        target, selector, ..
                    } => {
                        collect_lhs_rw(target, model, rw);
                        collect_stream_selector_reads(selector, model, rw);
                    }
                }
            }
        }
        IrStmt::Object(operation) => {
            if let Some(target) = operation.mailbox_target() {
                collect_mailbox_target_rw(target, rw);
            }
            operation.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrStmt::PlusArg(expression) => collect_expr_reads(expression, model, rw),
        IrStmt::Block(b) | IrStmt::ActivationScope { body: b, .. } => {
            collect_stmts_rw(b, model, rw)
        }
        IrStmt::DeclLocal {
            init: Some(init), ..
        }
        | IrStmt::SharedLocal {
            init: Some(init), ..
        } => collect_expr_reads(init, model, rw),
        IrStmt::ClockingDrive {
            lhs,
            rhs,
            specs,
            cycles,
            ..
        } => {
            collect_lhs_rw(lhs, model, rw);
            collect_expr_reads(rhs, model, rw);
            collect_wait_specs_rw(specs, model, rw);
            if let Some(cycles) = cycles {
                collect_expr_reads(cycles, model, rw);
            }
        }
        IrStmt::Assign { lhs, rhs, .. }
        | IrStmt::DelayedAssign { lhs, rhs, .. }
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
            specs,
            repeat,
            captures,
            ..
        } => {
            collect_lhs_rw(lhs, model, rw);
            collect_expr_reads(rhs, model, rw);
            collect_wait_specs_rw(specs, model, rw);
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
        IrStmt::DelayedChandleAssign { rhs, .. } => {
            rhs.expressions(&mut |expr| collect_expr_reads(expr, model, rw));
        }
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
        IrStmt::WaitEventTriggered { event, body } => {
            collect_event_reads(event, model, rw);
            collect_stmts_rw(body, model, rw);
        }
        IrStmt::WaitOrder {
            events,
            success,
            failure,
        } => {
            for event in events {
                collect_event_reads(event, model, rw);
            }
            collect_stmts_rw(success, model, rw);
            collect_stmts_rw(failure, model, rw);
        }
        // An event-array index or handle expression is evaluated where the
        // operation is issued.
        IrStmt::EventTrigger { ev }
        | IrStmt::ClockingEventTrigger { ev }
        | IrStmt::NonblockingEventTrigger { ev, .. }
        | IrStmt::EventCapture { source: ev, .. } => collect_event_reads(ev, model, rw),
        // `->> repeat(n) @(posedge c) ev` registers its wait sources and
        // evaluates the count when it is issued.
        IrStmt::NonblockingEventTriggerWhen { ev, specs, repeat } => {
            collect_event_reads(ev, model, rw);
            collect_wait_specs_rw(specs, model, rw);
            if let Some(repeat) = repeat {
                collect_expr_reads(repeat, model, rw);
            }
        }
        IrStmt::EventAssign { target, source } => {
            collect_event_reads(target, model, rw);
            if let Some(source) = source {
                collect_event_reads(source, model, rw);
            }
        }
        IrStmt::WaitEvents { specs, .. } => collect_wait_specs_rw(specs, model, rw),
        IrStmt::ClockingCycleWait { count, specs } => {
            collect_expr_reads(count, model, rw);
            collect_wait_specs_rw(specs, model, rw);
        }
        IrStmt::WaitAny { sens, .. } => {
            for dependency in sens {
                mark_dependency_read(dependency, model, rw);
            }
        }
        IrStmt::Force {
            lhs,
            value,
            reads,
            dependencies,
            ..
        } => {
            collect_lhs_rw(lhs, model, rw);
            collect_expr_reads(value, model, rw);
            // The force binding re-evaluates on these sources; validation
            // requires them to name emitted storage.
            for read in reads {
                rw.read(*read);
            }
            for dependency in dependencies {
                mark_dependency_read(dependency, model, rw);
            }
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
        // The monitor evaluator (`IrPreFn::MonEval`) reads the arguments; the
        // runtime watches the dependency addresses.
        IrStmt::MonitorSet {
            reads, descriptor, ..
        } => {
            for dependency in reads {
                mark_dependency_read(dependency, model, rw);
            }
            if let Some(descriptor) = descriptor {
                collect_expr_reads(descriptor, model, rw);
            }
        }
        IrStmt::FileControl { descriptor, .. } => {
            if let Some(descriptor) = descriptor {
                collect_expr_reads(descriptor, model, rw);
            }
        }
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
        IrStmt::Randomize(operation) => {
            for expression in operation.expressions() {
                collect_expr_reads(expression, model, rw);
            }
            for handle in operation.chandles() {
                handle.expressions(&mut |child| collect_expr_reads(child, model, rw));
            }
        }
        // Coverpoint and guard values and option expressions are reads.
        IrStmt::Coverage(operation) => {
            for expression in operation.expressions() {
                collect_expr_reads(expression, model, rw);
            }
            for value in operation.strings() {
                value.expressions(&mut |child| collect_expr_reads(child, model, rw));
            }
            for handle in operation.chandles() {
                handle.expressions(&mut |child| collect_expr_reads(child, model, rw));
            }
        }
        IrStmt::RandomStateSet { state } => {
            state.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrStmt::FixedArrayOrder(order) => {
            order.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrStmt::RealArrayOrder(order) => {
            order.expressions(&mut |child| collect_expr_reads(child, model, rw))
        }
        IrStmt::FixedValueAssign { dst, src, .. } => {
            src.expressions(&mut |child| collect_expr_reads(child, model, rw));
            // Descriptor operand calls can write through address formals.
            src.calls(&mut |call| collect_call_rw(call, model, rw));
            for selector in &dst.selectors {
                collect_expr_reads(&selector.value, model, rw);
            }
        }
        IrStmt::DeclString {
            init: Some(init), ..
        }
        | IrStmt::SharedString {
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
        IrStmt::FixedArrayFill { value, .. } => collect_expr_reads(value, model, rw),
        // The pulse writes its clock signal, which assertions read.
        IrStmt::AssertionClockPulse { signal } => {
            rw.read(*signal);
            rw.write(*signal);
        }
        // Delays are read through `IrStmt::delay_expression` above.
        IrStmt::Delay { .. } => {}
        // Fixed arrays, native values, events, activations and labels are
        // not signal storage; fork branches and spawned processes are
        // visited as their own processes and functions.
        IrStmt::Located { .. }
        | IrStmt::System(None)
        | IrStmt::FixedArrayCopy { .. }
        | IrStmt::FixedArrayDeclare(_)
        | IrStmt::NativeValueDeclare(_)
        | IrStmt::NativeValueSharedDeclare(_)
        | IrStmt::NativeValueCopy { .. }
        | IrStmt::DeclLocal { init: None, .. }
        | IrStmt::SharedLocal { init: None, .. }
        | IrStmt::DeclString { init: None, .. }
        | IrStmt::SharedString { init: None, .. }
        | IrStmt::EventDeclare { .. }
        | IrStmt::SampledClockTick { .. }
        | IrStmt::Fork { .. }
        | IrStmt::DisableTarget { .. }
        | IrStmt::DeferredAssertionDisable { .. }
        | IrStmt::QueueProceduralAssertion { .. }
        | IrStmt::WaitFork
        | IrStmt::DisableFork
        | IrStmt::Expect { .. }
        | IrStmt::MonitorEnable(_)
        | IrStmt::WaveFile(_)
        | IrStmt::WaveOn
        | IrStmt::WaveOff
        | IrStmt::WaveDumpAll
        | IrStmt::WaveFlush
        | IrStmt::Finish
        | IrStmt::FinishControl { .. }
        | IrStmt::ProgramExit
        | IrStmt::StopControl { .. }
        | IrStmt::PrintTimescale { .. }
        | IrStmt::SdfAnnotate { .. }
        | IrStmt::Return { value: None }
        | IrStmt::Label(_)
        | IrStmt::Goto(_)
        // Its delay expressions are read with every statement's.
        | IrStmt::SetNetDelay { .. }
        // Its sources are read through `IrModel::module_paths` (below).
        | IrStmt::ModulePathStamp { .. }
        // Its terminals are read through `IrModel::timing_checks` (below).
        | IrStmt::TimingCheckStep
        | IrStmt::Nop => {}
        // `$dumpvars` names scopes; waveform models keep every HDL-visible
        // signal (`model.waveform`).
        IrStmt::WaveDumpVars(_) => {}
    }
}

/// A mailbox retrieval writes through a C address (`&G_sig`); like an
/// output argument it counts as a read and a write of the named signal, so
/// the storage stays even when nothing else reads it (function-scope names
/// never match).
fn collect_mailbox_target_rw(target: &crate::sim::ir::IrMailboxTarget, rw: &mut Rw) {
    if let Some(addr) = target.address() {
        rw.address(addr, true);
    }
}

/// Event-control wait sources are reads of the named signal globals:
/// lowering emits their addresses into the runtime wait arrays, so a signal
/// referenced only here still needs its storage. A named event is not signal
/// storage, but an event-array index or handle expression is evaluated when
/// the wait is armed.
fn collect_wait_specs_rw(
    specs: &[(IrWaitSrc, crate::sim::ir::IrEdge)],
    model: &IrModel,
    rw: &mut Rw,
) {
    for (src, _) in specs {
        match src {
            IrWaitSrc::Sig(name) => mark_dependency_read(&IrDependency::scalar(name), model, rw),
            IrWaitSrc::Real(name) => mark_dependency_read(&IrDependency::real(name), model, rw),
            IrWaitSrc::Evaluated { reads, .. } | IrWaitSrc::EvaluatedReal { reads, .. } => {
                for dependency in reads {
                    mark_dependency_read(dependency, model, rw);
                }
            }
            IrWaitSrc::Event(event) | IrWaitSrc::FilteredEvent { event, .. } => {
                collect_event_reads(event, model, rw)
            }
            // Formal and native-local sources name activation storage.
            IrWaitSrc::RefFormal { .. }
            | IrWaitSrc::NativeRefFormal { .. }
            | IrWaitSrc::SharedNative { .. } => {}
        }
    }
}

fn collect_event_reads(event: &crate::sim::ir::IrEventRef, model: &IrModel, rw: &mut Rw) {
    event.expressions(&mut |child| collect_expr_reads(child, model, rw));
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
    let crate::sim::ir::IrCall {
        f,
        args,
        depth: _,
        receiver,
        virtual_dispatch: _,
        virtual_call,
        temps,
        copyouts,
        dpi_site: _,
    } = call;
    collect_call_target_reads(receiver.as_ref(), virtual_call.as_ref(), model, rw);
    collect_call_args_rw(*f, args, model, rw);
    // Caller-side output/inout temps live here, not in `args`: their
    // initializers read the actual's current value.
    for (_, _, init) in temps {
        if let Some(init) = init {
            collect_expr_reads(init, model, rw);
        }
    }
    // Copy-out assignments after the call write the actual targets (their
    // select indices are reads).
    for (lh, ..) in copyouts {
        collect_lhs_rw(lh, model, rw);
    }
}

fn collect_call_expr_rw(call: &crate::sim::ir::IrCallExpr, model: &IrModel, rw: &mut Rw) {
    let crate::sim::ir::IrCallExpr {
        f,
        args,
        depth: _,
        receiver,
        virtual_dispatch: _,
        virtual_call,
        void_x: _,
        dpi_site: _,
    } = call;
    collect_call_target_reads(receiver.as_ref(), virtual_call.as_ref(), model, rw);
    collect_call_args_rw(*f, args, model, rw);
}

/// A method receiver such as `q[i].f()` evaluates its selectors at the call
/// site.
fn collect_call_target_reads(
    receiver: Option<&crate::sim::ir::IrChandleExpr>,
    virtual_call: Option<&crate::sim::ir::IrVirtualCall>,
    model: &IrModel,
    rw: &mut Rw,
) {
    if let Some(receiver) = receiver {
        receiver.expressions(&mut |child| collect_expr_reads(child, model, rw));
    }
    if let Some(virtual_call) = virtual_call {
        virtual_call
            .receiver
            .expressions(&mut |child| collect_expr_reads(child, model, rw));
    }
}

/// Arguments of statement and expression calls alike: an output, inout or
/// non-const `ref` actual is written through its address by the callee.
fn collect_call_args_rw(function: usize, args: &[IrCallArg], model: &IrModel, rw: &mut Rw) {
    for (index, arg) in args.iter().enumerate() {
        match arg {
            IrCallArg::FixedValue(value) => {
                value.expressions(&mut |child| collect_expr_reads(child, model, rw));
                value.calls(&mut |call| collect_call_rw(call, model, rw));
            }
            IrCallArg::Val(e) => collect_expr_reads(e, model, rw),
            IrCallArg::StringVal(value) => {
                value.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
            }
            IrCallArg::NativeLeaves { leaves, .. } => {
                for leaf in leaves {
                    leaf.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                }
            }
            IrCallArg::NativeCall { call, .. } | IrCallArg::RealArrayCall { call, .. } => {
                collect_call_rw(call, model, rw)
            }
            IrCallArg::OutAddr(addr) => rw.address(addr, true),
            IrCallArg::RefAddr { lhs, read, .. } => {
                collect_expr_reads(read, model, rw);
                if call_formal(model, function, index).is_none_or(|formal| !formal.is_const_ref()) {
                    collect_lhs_rw(lhs, model, rw);
                }
            }
            IrCallArg::StringOutAddr(addr) | IrCallArg::StringRefAddr { addr, .. } => {
                let write =
                    call_formal(model, function, index).is_none_or(|formal| !formal.is_const_ref());
                rw.address(addr, write);
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
            IrCallArg::RealArrayValues(values) | IrCallArg::ContainerValues { values, .. } => {
                for value in values {
                    collect_expr_reads(value, model, rw);
                }
            }
            IrCallArg::EventVal(event) | IrCallArg::EventAddr(event) => {
                collect_event_reads(event, model, rw)
            }
            // A handle argument such as `q[i]` evaluates its selectors at the
            // call site.
            IrCallArg::ChandleVal(value) => {
                value.expressions(&mut |child| collect_expr_reads(child, model, rw))
            }
            IrCallArg::FixedArray(_)
            | IrCallArg::RealArray(_)
            | IrCallArg::NativeValue(_)
            | IrCallArg::NativeRefBound
            | IrCallArg::Container(_)
            | IrCallArg::ChandleAddr(_)
            | IrCallArg::ChandleRefAddr(_) => {}
        }
    }
}

fn collect_lhs_rw(l: &IrLhs, model: &IrModel, rw: &mut Rw) {
    match l {
        IrLhs::PackedSelect { target, steps, .. } => {
            collect_lhs_rw(target, model, rw);
            for step in steps {
                collect_expr_reads(&step.base, model, rw);
            }
        }
        IrLhs::TaggedSelect { target, steps, .. } => {
            collect_lhs_rw(target, model, rw);
            for step in steps {
                collect_expr_reads(&step.selection.base, model, rw);
            }
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
            for step in steps {
                collect_expr_reads(&step.base, model, rw);
            }
        }
        IrLhs::TaggedSelect { target, steps, .. } => {
            collect_lhs_read(target, model, rw);
            for step in steps {
                collect_expr_reads(&step.selection.base, model, rw);
            }
        }
        IrLhs::Whole(i) | IrLhs::Part(i, ..) => rw.read(*i),
        IrLhs::Bit(i, index, _) => {
            rw.read(*i);
            collect_expr_reads(index, model, rw);
        }
        IrLhs::IdxPart(i, base, width, ..) => {
            rw.read(*i);
            collect_expr_reads(base, model, rw);
            collect_expr_reads(width, model, rw);
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
        // Activation locals and formals, constants, fixed-array storage and
        // scheduler facts are not signal storage. `Verbatim` is rejected by
        // the owned emitter.
        IrExprKind::LocalRead(_)
        | IrExprKind::FormalRead(_)
        | IrExprKind::Const(_)
        | IrExprKind::Fill(_)
        | IrExprKind::FixedArrayCompare { .. }
        | IrExprKind::RuntimeQuery(_)
        | IrExprKind::Verbatim { .. } => {}
        // Sequential UDP state is model-owned emitted storage, not a signal.
        IrExprKind::UdpState(_) => {}
        // A match-item call copies its by-value arguments into the call frame
        // when the item is evaluated.
        IrExprKind::MatchCall(action) => {
            for capture in action.captures() {
                collect_expr_reads(capture.initial(), model, rw);
            }
        }
        IrExprKind::FixedValueCompare { left, right, .. } => {
            left.expressions(&mut |child| collect_expr_reads(child, model, rw));
            right.expressions(&mut |child| collect_expr_reads(child, model, rw));
            for value in [left, right] {
                value.calls(&mut |call| collect_call_rw(call, model, rw));
            }
        }
        IrExprKind::FixedArrayReduce(reduction) => {
            reduction.expressions(&mut |child| collect_expr_reads(child, model, rw));
        }
        IrExprKind::Container(operation) => {
            if let Some(signal) = operation.traversal_signal() {
                rw.read(signal);
            }
            operation.expressions(&mut |child| collect_expr_reads(child, model, rw));
        }
        IrExprKind::ObjectQuery(query) => {
            if let Some(target) = query.mailbox_target() {
                collect_mailbox_target_rw(target, rw);
            }
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
        | IrExprKind::StreamToFixed { a, .. }
        | IrExprKind::CastToReal { a, .. } => collect_expr_reads(a, model, rw),
        IrExprKind::Mux { sel, a, b }
        | IrExprKind::ArrayMux { sel, a, b, .. }
        | IrExprKind::StructMux { sel, a, b, .. } => {
            collect_expr_reads(sel, model, rw);
            collect_expr_reads(a, model, rw);
            collect_expr_reads(b, model, rw);
        }
        IrExprKind::UdpEval { inputs: parts, .. }
        | IrExprKind::Predicate { clauses: parts }
        | IrExprKind::Concat { parts }
        | IrExprKind::Replicate { parts, .. } => {
            for p in parts {
                collect_expr_reads(p, model, rw);
            }
        }
        IrExprKind::Pattern(pattern) => {
            collect_expr_reads(&pattern.value, model, rw);
            if let Some(constant) = &pattern.constant {
                collect_expr_reads(constant, model, rw);
            }
            if let Some(binding) = &pattern.binding {
                collect_lhs_rw(binding, model, rw);
            }
            for check in &pattern.checks {
                if let Some(constant) = &check.constant {
                    collect_expr_reads(constant, model, rw);
                }
                if let Some(binding) = &check.binding {
                    collect_lhs_rw(binding, model, rw);
                }
            }
            for binding in &pattern.native_bindings {
                binding.expressions(&mut |child| collect_expr_reads(child, model, rw));
                if let crate::sim::ir::IrNativeBinding::Value { lhs, .. } = binding {
                    collect_lhs_rw(lhs, model, rw);
                }
            }
        }
        IrExprKind::Stream { value, .. } => collect_expr_reads(value, model, rw),
        IrExprKind::FixedStream { selector, .. } => {
            collect_stream_selector_reads(selector, model, rw)
        }
        IrExprKind::FixedImageStream {
            image,
            fallback,
            selector,
            ..
        } => {
            collect_expr_reads(image, model, rw);
            collect_expr_reads(fallback, model, rw);
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
                    IrInsideItem::Cells(cells) => {
                        cells.expressions(&mut |child| collect_expr_reads(child, model, rw))
                    }
                    IrInsideItem::FixedArray { value, .. } => collect_expr_reads(value, model, rw),
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
        IrExprKind::Sequence(sequence) => {
            collect_stmts_rw(&sequence.statements, model, rw);
            collect_expr_reads(&sequence.value, model, rw);
        }
        IrExprKind::TaggedSelect { base, steps, .. } => {
            collect_expr_reads(base, model, rw);
            for step in steps {
                collect_expr_reads(&step.selection.base, model, rw);
            }
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
        IrExprKind::EventTriggered(event) => collect_event_reads(event, model, rw),
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
        IrExprKind::CallFn(call) => collect_call_expr_rw(call, model, rw),
        IrExprKind::SysFunc(sf) => match &**sf {
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
                    crate::sim::ir::IrPlusArgTarget::Element { read, .. } => {
                        collect_expr_reads(read, model, rw)
                    }
                }
            }
            IrSysFunc::CoverageCounts {
                query,
                covered,
                total,
            } => {
                if let Some(record) = query.chandle() {
                    record.expressions(&mut |child| collect_expr_reads(child, model, rw));
                }
                for target in [covered, total] {
                    match target {
                        crate::sim::ir::IrPlusArgTarget::Packed { lhs, .. }
                        | crate::sim::ir::IrPlusArgTarget::Real { lhs, .. } => {
                            collect_lhs_rw(lhs, model, rw)
                        }
                        crate::sim::ir::IrPlusArgTarget::String { .. } => {}
                        crate::sim::ir::IrPlusArgTarget::Element { read, .. } => {
                            collect_expr_reads(read, model, rw)
                        }
                    }
                }
            }
            IrSysFunc::System(Some(command)) => {
                command.expressions(&mut |child| collect_expr_reads(child, model, rw));
            }
            IrSysFunc::CoverageApi(call) => {
                call.expressions(&mut |child| collect_expr_reads(child, model, rw));
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
            IrSysFunc::RandJoinPick { lengths, ratio } => {
                for length in lengths {
                    collect_expr_reads(length, model, rw);
                }
                collect_expr_reads(ratio, model, rw);
            }
            IrSysFunc::Randcase { weights, .. } => {
                for weight in weights {
                    collect_expr_reads(weight, model, rw);
                }
            }
            IrSysFunc::RandStep { stream, .. } => {
                if let Some(stream) = stream {
                    stream.expressions(&mut |expression| collect_expr_reads(expression, model, rw));
                }
            }
            IrSysFunc::RandCallReady { .. }
            | IrSysFunc::RandValue { .. }
            | IrSysFunc::RandCyclic { .. } => {}
            IrSysFunc::RandInst { inst, index, .. } => {
                for operand in inst.iter().chain(index.iter()) {
                    collect_expr_reads(operand, model, rw);
                }
            }
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
                                crate::sim::ir::IrFileInputTarget::String { .. }
                                | crate::sim::ir::IrFileInputTarget::Element { .. } => {}
                            }
                        }
                    }
                    crate::sim::ir::IrFileInput::Read { target, .. } => match target {
                        crate::sim::ir::IrFileReadTarget::Packed { lhs, .. } => {
                            collect_lhs_rw(lhs, model, rw)
                        }
                        crate::sim::ir::IrFileReadTarget::Element { .. }
                        | crate::sim::ir::IrFileReadTarget::Array { .. }
                        | crate::sim::ir::IrFileReadTarget::Container { .. } => {}
                    },
                    crate::sim::ir::IrFileInput::Gets { target, .. } => match target {
                        crate::sim::ir::IrFileInputTarget::Packed { lhs, .. }
                        | crate::sim::ir::IrFileInputTarget::Real { lhs, .. } => {
                            collect_lhs_rw(lhs, model, rw);
                        }
                        crate::sim::ir::IrFileInputTarget::String { .. }
                        | crate::sim::ir::IrFileInputTarget::Element { .. } => {}
                    },
                    crate::sim::ir::IrFileInput::Getc { .. }
                    | crate::sim::ir::IrFileInput::Ungetc { .. } => {}
                }
            }
            IrSysFunc::Sampled(call) => collect_expr_reads(&call.argument, model, rw),
        },
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
