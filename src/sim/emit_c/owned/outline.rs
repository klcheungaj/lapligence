//! Long statement runs of a body moved into separate plain C functions.
//!
//! C compilers optimize a function in time superlinear in its size, while a
//! process or subprogram body becomes one C function whose size grows
//! linearly with its statements. A statement sequence heavier than
//! [`LLG_OUTLINE_RUN_WEIGHT`] is therefore cut into runs of consecutive
//! statements of at most that weight, and each run of at least
//! [`LLG_OUTLINE_MIN_RUN_WEIGHT`] becomes a `static int` helper that the body
//! calls once. Lighter sequences, and bodies that instance sharing may merge,
//! are emitted exactly as before.
//!
//! A helper is an ordinary plain frame, so it can hold only what a plain
//! function can execute and only what needs no state of the body that calls
//! it. A statement joins a run only when, from the IR alone:
//!
//! - it cannot suspend, so every resume label stays in the coroutine that
//!   owns the frame: no wait, delay, fork, `$stop`, coroutine callee or
//!   dynamic dispatch;
//! - it is no cancellation point (`disable`, a callee that may disable), so
//!   the caller's activation checks are unchanged;
//! - every local, formal, label or activation it names is declared inside the
//!   statement itself; other names resolve to model storage, exactly as they
//!   do in the body (an automatic variable declared earlier in the body, a
//!   formal or `return` keeps its statement inline). A run is checked again
//!   against the bindings in scope just before it is emitted;
//! - it consists only of the statement and expression forms `Check` admits,
//!   whose rendering depends on nothing else of the frame.
//!
//! The helper takes the caller's temporary array (`_llg_t`) and allocates
//! its temporaries from the slots the caller has free at that point, so a
//! call costs no scope allocation; its own lexical scopes and stack cells are
//! unwound before it returns. A helper returns nonzero when it left through a
//! termination check (`$fatal` from a callee, a failed class access, the
//! process step limit): the caller then leaves exactly as the same check
//! inline would have. A flag that a statement only sets (`$finish`) is left
//! to the caller's next check, as inline.
//!
//! The weight counts IR statements, lvalues and expression nodes; each node
//! renders to a few C lines, so it bounds the C size of a helper while being
//! known before rendering. A statement heavier than one run is rendered in
//! place, and every sequence inside it is then split even when lighter than
//! a run, so its own runs leave the body too.
use super::*;
use crate::sim::ir::{LLG_OUTLINE_MIN_RUN_WEIGHT, LLG_OUTLINE_RUN_WEIGHT};
use std::collections::HashSet;
use std::ops::Range;

/// Outlined helpers of one body and their naming state, moved into each
/// helper frame while it renders.
pub(super) struct OutlineSink {
    owner: String,
    /// The body has a recursion `depth` (a subprogram or fork branch), which
    /// its calls pass on and a helper then takes as a parameter.
    depth: bool,
    next: usize,
    helpers: String,
    may_disable: HashMap<usize, bool>,
}

/// How a statement sequence is emitted.
pub(super) enum OutlinePlan {
    /// The body does not outline here.
    Off,
    /// No longer than one run: in place, and so is every sequence inside it.
    Light,
    Split(Vec<Segment>),
}

/// A part of a split statement sequence.
#[derive(Debug, PartialEq)]
pub(super) enum Segment {
    Inline(Range<usize>),
    /// A statement heavier than one run, rendered in place under pressure.
    Heavy(usize),
    Outline(Range<usize>),
}

/// Group `(weight, outlinable)` statements into segments.
fn segments(weights: &[(usize, bool)]) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut run = 0..0;
    let mut run_weight = 0;
    let close = |segments: &mut Vec<Segment>, run: &Range<usize>, weight: usize| {
        if run.is_empty() {
            return;
        }
        segments.push(if weight >= LLG_OUTLINE_MIN_RUN_WEIGHT {
            Segment::Outline(run.clone())
        } else {
            Segment::Inline(run.clone())
        });
    };
    for (index, &(weight, outlinable)) in weights.iter().enumerate() {
        if outlinable && weight <= LLG_OUTLINE_RUN_WEIGHT {
            if run_weight + weight > LLG_OUTLINE_RUN_WEIGHT {
                close(&mut segments, &run, run_weight);
                run = index..index;
                run_weight = 0;
            }
            if run.is_empty() {
                run = index..index;
            }
            run.end = index + 1;
            run_weight += weight;
            continue;
        }
        close(&mut segments, &run, run_weight);
        run = index + 1..index + 1;
        run_weight = 0;
        segments.push(if weight > LLG_OUTLINE_RUN_WEIGHT {
            Segment::Heavy(index)
        } else {
            Segment::Inline(index..index + 1)
        });
    }
    close(&mut segments, &run, run_weight);
    // Adjacent in-place parts render as one sequence, so net-contribution
    // and dense-cell folding still see all of them.
    let mut merged: Vec<Segment> = Vec::with_capacity(segments.len());
    for segment in segments {
        if let (Some(Segment::Inline(last)), Segment::Inline(next)) = (merged.last_mut(), &segment)
        {
            last.end = next.end;
            continue;
        }
        merged.push(segment);
    }
    merged
}

/// The local a statement declares for the rest of its sequence.
fn declared_name(statement: &IrStmt) -> Option<&str> {
    match statement {
        IrStmt::DeclLocal { name, .. }
        | IrStmt::SharedLocal { name, .. }
        | IrStmt::DeclString { name, .. }
        | IrStmt::SharedString { name, .. } => Some(name),
        _ => None,
    }
}

/// Statement lists nested in a statement, for weighing forms a run cannot
/// hold.
fn nested(statement: &IrStmt) -> Vec<&[IrStmt]> {
    match statement {
        IrStmt::Located { statement, .. } => nested(statement),
        IrStmt::Block(body)
        | IrStmt::While { body, .. }
        | IrStmt::Repeat { body, .. }
        | IrStmt::Forever { body }
        | IrStmt::ActivationScope { body, .. }
        | IrStmt::WaitCond { body, .. }
        | IrStmt::WaitEventTriggered { body, .. } => vec![body],
        IrStmt::If { then_, els, .. } => {
            let mut lists = vec![then_.as_slice()];
            lists.extend(els.as_deref());
            lists
        }
        IrStmt::Case { items, .. } => items.iter().map(|item| item.body.as_slice()).collect(),
        IrStmt::For {
            init, incr, body, ..
        } => vec![init, incr, body],
        IrStmt::WaitOrder {
            success, failure, ..
        } => vec![success, failure],
        IrStmt::ImmediateAssertion {
            if_true, if_false, ..
        } => if_true
            .as_deref()
            .into_iter()
            .chain(if_false.as_deref())
            .collect(),
        _ => Vec::new(),
    }
}

/// Labels declared anywhere inside a statement: the only targets a `goto`
/// in a run may have.
fn collect_labels(statement: &IrStmt, labels: &mut HashSet<String>) {
    if let IrStmt::Label(name) = statement.unlocated() {
        labels.insert(name.clone());
    }
    for list in nested(statement) {
        for statement in list {
            collect_labels(statement, labels);
        }
    }
}

/// Weighs one statement and decides whether a run may hold it.
struct Check<'c, 'a, 'm> {
    frame: &'c Frame<'a, 'm>,
    depth: bool,
    may_disable: &'c mut HashMap<usize, bool>,
    sequence_locals: &'c HashSet<String>,
    scopes: Vec<Vec<String>>,
    labels: HashSet<String>,
    weight: usize,
    ok: bool,
}

impl Check<'_, '_, '_> {
    fn reject(&mut self) {
        self.ok = false;
    }

    fn declare(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name.to_owned());
        }
    }

    /// A name read or written by the statement: its own local, or storage
    /// that resolves identically in a helper frame.
    fn name(&mut self, name: &str) {
        if self
            .scopes
            .iter()
            .any(|scope| scope.iter().any(|local| local == name))
        {
            return;
        }
        let frame = self.frame;
        if name.starts_with('*')
            || self.sequence_locals.contains(name)
            || frame.bindings.iter().any(|scope| scope.contains_key(name))
            || frame
                .native_bindings
                .iter()
                .any(|scope| scope.contains_key(name))
            || frame.shared_cells.contains_key(name)
            || frame
                .ctx
                .model
                .native_accesses
                .iter()
                .any(|access| access.name == name)
        {
            self.reject();
        }
    }

    fn address(&mut self, address: &str) {
        match address.strip_prefix('&') {
            Some(name) => self.name(name),
            None => self.reject(),
        }
    }

    fn body(&mut self, body: &[IrStmt]) {
        self.scopes.push(Vec::new());
        for statement in body {
            self.statement(statement);
        }
        self.scopes.pop();
    }

    fn statement(&mut self, statement: &IrStmt) {
        if let IrStmt::Located { statement, .. } = statement {
            return self.statement(statement);
        }
        self.weight += 1;
        match statement {
            IrStmt::Nop | IrStmt::Finish | IrStmt::FinishControl { .. } | IrStmt::Label(_) => {}
            IrStmt::Coverage(operation)
                if matches!(operation.as_ref(), IrCoverageStmt::StatementHit { .. }) => {}
            IrStmt::Goto(label) => {
                if !self.labels.contains(label) {
                    self.reject();
                }
            }
            IrStmt::Display { args, .. } => {
                for (value, _) in args {
                    self.expr(value);
                }
            }
            IrStmt::DisplayTyped {
                args, descriptor, ..
            } => {
                self.display_args(args);
                if let Some(descriptor) = descriptor {
                    self.expr(descriptor);
                }
            }
            IrStmt::Severity { args, .. } => self.display_args(args),
            IrStmt::Assign { lhs, rhs, .. } => {
                self.expr(rhs);
                self.lhs(lhs);
            }
            IrStmt::DeclLocal { name, init, .. } => {
                if let Some(init) = init {
                    self.expr(init);
                }
                self.declare(name);
            }
            IrStmt::Block(body) => self.body(body),
            IrStmt::If {
                cond, then_, els, ..
            } => {
                self.expr(cond);
                self.body(then_);
                if let Some(els) = els {
                    self.body(els);
                }
            }
            IrStmt::Case { sel, items, .. } => {
                self.expr(sel);
                for item in items {
                    for value in &item.exprs {
                        self.expr(value);
                    }
                    self.body(&item.body);
                }
            }
            IrStmt::While { cond, body } => {
                self.expr(cond);
                self.body(body);
            }
            IrStmt::Repeat { count, body } => {
                self.expr(count);
                self.body(body);
            }
            IrStmt::Forever { body } => self.body(body),
            IrStmt::For {
                init,
                cond,
                incr,
                body,
            } => {
                self.scopes.push(Vec::new());
                for statement in init {
                    self.statement(statement);
                }
                self.expr(cond);
                self.body(body);
                self.body(incr);
                self.scopes.pop();
            }
            IrStmt::ImmediateAssertion {
                condition,
                if_true,
                if_false,
                ..
            } => {
                self.expr(condition);
                for body in [if_true, if_false].into_iter().flatten() {
                    self.body(body);
                }
            }
            IrStmt::Call(call) => {
                self.scopes.push(Vec::new());
                for (name, _, init) in &call.temps {
                    if let Some(init) = init {
                        self.expr(init);
                    }
                    self.declare(name);
                }
                self.call(
                    call.f,
                    &call.args,
                    call.depth,
                    call.receiver.is_some()
                        || call.virtual_dispatch
                        || call.virtual_call.is_some()
                        || call.dpi_site.is_some(),
                );
                for (lhs, temp, _, _) in &call.copyouts {
                    self.name(temp);
                    self.lhs(lhs);
                }
                self.scopes.pop();
            }
            other => {
                self.reject();
                for list in nested(other) {
                    self.body(list);
                }
            }
        }
    }

    fn display_args(&mut self, args: &[IrDisplayArg]) {
        for arg in args {
            match arg {
                IrDisplayArg::Packed(value)
                | IrDisplayArg::Real(value)
                | IrDisplayArg::Strength(value) => self.expr(value),
                IrDisplayArg::String(value) | IrDisplayArg::Text(value) => self.string(value),
            }
        }
    }

    fn call(&mut self, function: usize, args: &[IrCallArg], depth: IrDepth, dynamic: bool) {
        let model = self.frame.ctx.model;
        let Some(callee) = model.funcs.get(function) else {
            return self.reject();
        };
        // Calls in a subprogram pass its `depth` on; a helper takes it from
        // its caller.
        if dynamic
            || (depth.is_function_based() && !self.depth)
            || callee.dpi.is_some()
            || callee.receiver_class.is_some()
            || callee.is_inline_expanded()
            || self.frame.coroutine_functions.contains(&function)
            || *self.may_disable.entry(function).or_insert_with(|| {
                crate::sim::execution::effects_for_function(model, function)
                    .iter()
                    .any(|effect| {
                        matches!(
                            effect,
                            crate::sim::execution::ExecutionEffect::Disable
                                | crate::sim::execution::ExecutionEffect::Suspend
                        )
                    })
            })
        {
            self.reject();
        }
        for arg in args {
            self.weight += 1;
            match arg {
                IrCallArg::Val(value) => self.expr(value),
                IrCallArg::OutAddr(address) => self.address(address),
                IrCallArg::OutTemp {
                    name,
                    init,
                    writeback,
                    storage_addr,
                    storage_lhs,
                    storage_read,
                    selector_inits,
                } => {
                    for (name, _, _, _, initial) in selector_inits {
                        self.expr(initial);
                        self.declare(name);
                    }
                    if let Some(init) = init {
                        self.expr(init);
                    }
                    self.declare(name);
                    if let Some(address) = storage_addr {
                        self.address(address);
                    }
                    if let Some(lhs) = storage_lhs {
                        self.lhs(lhs);
                    }
                    if let Some(read) = storage_read {
                        self.expr(read);
                    }
                    self.lhs(writeback);
                }
                _ => self.reject(),
            }
        }
    }

    fn array(&mut self, array: usize) {
        // An activation array lives in the body's frame.
        if self.frame.ctx.model.array(array).activation {
            self.reject();
        }
    }

    fn lhs(&mut self, lhs: &IrLhs) {
        self.weight += 1;
        match lhs {
            IrLhs::Whole(_) | IrLhs::Part(..) => {}
            IrLhs::Bit(_, index, _) => self.expr(index),
            IrLhs::IdxPart(_, base, width, ..) => {
                self.expr(base);
                self.expr(width);
            }
            IrLhs::PackedSelect { target, steps, .. } => {
                self.lhs(target);
                for step in steps {
                    self.expr(&step.base);
                }
            }
            IrLhs::WholeRef { addr, .. } => self.address(addr),
            IrLhs::Ref { addr, bit, .. } => {
                self.address(addr);
                if let Some(bit) = bit {
                    self.expr(bit);
                }
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                self.array(*arr);
                for index in indices {
                    self.expr(index);
                }
                elem_sel.expressions(&mut |child| self.expr(child));
            }
            IrLhs::TaggedSelect { .. } | IrLhs::Stream { .. } => self.reject(),
        }
    }

    fn string(&mut self, value: &IrStringExpr) {
        self.weight += 1;
        match value {
            IrStringExpr::Literal(_) | IrStringExpr::Read(_) => {}
            IrStringExpr::LocalRead(name) => self.name(name),
            IrStringExpr::Concat(parts) => {
                for part in parts {
                    self.string(part);
                }
            }
            IrStringExpr::Repeat(part, count) => {
                self.string(part);
                self.expr(count);
            }
            IrStringExpr::FromPacked(value) => self.expr(value),
            IrStringExpr::Case(part, _) => self.string(part),
            IrStringExpr::Substr(part, first, last) => {
                self.string(part);
                self.expr(first);
                self.expr(last);
            }
            _ => self.reject(),
        }
    }

    fn expr(&mut self, value: &IrExpr) {
        self.weight += 1;
        match &value.kind {
            IrExprKind::Const(_) | IrExprKind::SigRead(_) | IrExprKind::Fill(_) => {}
            IrExprKind::LocalRead(name) => self.name(name),
            IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
                self.expr(a);
                self.expr(b);
            }
            IrExprKind::Un { a, .. }
            | IrExprKind::RealUn { a, .. }
            | IrExprKind::CastToReal { a, .. }
            | IrExprKind::CastToPacked { a }
            | IrExprKind::Resize { a }
            | IrExprKind::Convert { a }
            | IrExprKind::ToTwoState { a }
            | IrExprKind::BitStreamCast { a, .. } => self.expr(a),
            IrExprKind::Mux { sel, a, b } => {
                self.expr(sel);
                self.expr(a);
                self.expr(b);
            }
            IrExprKind::Predicate { clauses: parts }
            | IrExprKind::Concat { parts }
            | IrExprKind::Replicate { parts, .. } => {
                for part in parts {
                    self.expr(part);
                }
            }
            IrExprKind::BitSel { base, idx } => {
                self.expr(base);
                self.expr(idx);
            }
            IrExprKind::PartSel { base, .. } => self.expr(base),
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                width_expr,
                ..
            } => {
                self.expr(base);
                self.expr(base_idx);
                self.expr(width_expr);
            }
            IrExprKind::ArrayRead {
                arr,
                indices,
                elem_sel,
            } => {
                self.array(*arr);
                for index in indices {
                    self.expr(index);
                }
                elem_sel.expressions(&mut |child| self.expr(child));
            }
            IrExprKind::Inside { value, items } => {
                self.expr(value);
                for item in items {
                    match item {
                        IrInsideItem::Value(value) => self.expr(value),
                        IrInsideItem::Range { low, high } => {
                            self.expr(low);
                            self.expr(high);
                        }
                        IrInsideItem::OpenRange { low, high } => {
                            for bound in [low, high].into_iter().flatten() {
                                self.expr(bound);
                            }
                        }
                        _ => self.reject(),
                    }
                }
            }
            IrExprKind::CallFn(call) => self.call(
                call.f,
                &call.args,
                call.depth,
                call.receiver.is_some()
                    || call.virtual_dispatch
                    || call.virtual_call.is_some()
                    || call.dpi_site.is_some(),
            ),
            IrExprKind::SysFunc(function) => self.system_function(function),
            _ => self.reject(),
        }
    }

    fn system_function(&mut self, function: &IrSysFunc) {
        match function {
            IrSysFunc::Time { .. } | IrSysFunc::Realtime { .. } => {}
            IrSysFunc::Clog2(value)
            | IrSysFunc::Bits(value)
            | IrSysFunc::Rtoi(value)
            | IrSysFunc::Itor(value)
            | IrSysFunc::RealToBits(value)
            | IrSysFunc::BitsToReal(value)
            | IrSysFunc::ShortRealToBits(value)
            | IrSysFunc::BitsToShortReal(value)
            | IrSysFunc::BitQuery { arg: value, .. } => self.expr(value),
            IrSysFunc::Math { args, .. } => {
                for arg in args {
                    self.expr(arg);
                }
            }
            IrSysFunc::Urandom { seed } => {
                if let Some(seed) = seed {
                    self.expr(seed);
                }
            }
            IrSysFunc::UrandomRange { max, min } => {
                self.expr(max);
                if let Some(min) = min {
                    self.expr(min);
                }
            }
            _ => self.reject(),
        }
    }
}

impl OutlineSink {
    pub(super) fn new(owner: &str, depth: bool) -> Self {
        Self {
            owner: owner.to_owned(),
            depth,
            next: 0,
            helpers: String::new(),
            may_disable: HashMap::new(),
        }
    }
}

impl Frame<'_, '_> {
    /// Outline long statement runs of this body into helpers named after
    /// `owner` (its C function name). `depth`: the body has the C variable
    /// `depth` its calls pass on. The caller emits
    /// [`Frame::outlined_helpers`] before the body's function.
    pub(super) fn enable_outlining(&mut self, owner: &str, depth: bool) {
        self.outline = Some(OutlineSink::new(owner, depth));
    }

    /// Definitions of the helpers this body calls, callees first.
    pub(super) fn outlined_helpers(&mut self) -> String {
        self.outline
            .as_mut()
            .map(|sink| std::mem::take(&mut sink.helpers))
            .unwrap_or_default()
    }

    /// Only ordinary procedural statements outline: callbacks, sampled or
    /// postponed evaluation, inlined callees and recursive coroutines keep
    /// state a helper frame would not have.
    fn outline_applies(&self) -> bool {
        self.outline.is_some()
            && !self.synchronous
            && !self.read_only_callback
            && !self.postponed_reader
            && !self.deferred_action
            && !self.sampled_reads
            && !self.item_callback
            && !self.quiet_receivers
            && self.allow_calls
            && self.formal_overrides.is_empty()
            && self.callback_signal_overrides.is_empty()
            && self.sequence_addresses.is_empty()
            && self.construction_stack.is_empty()
            && self.access_stack.is_empty()
    }

    pub(super) fn outline_plan(&mut self, statements: &[IrStmt]) -> OutlinePlan {
        if !self.outline_applies() || (self.outline_light && !self.outline_pressure) {
            return OutlinePlan::Off;
        }
        let Some(mut sink) = self.outline.take() else {
            return OutlinePlan::Off;
        };
        let weights = self.statement_weights(statements, sink.depth, &mut sink.may_disable);
        self.outline = Some(sink);
        let total = weights.iter().map(|(weight, _)| weight).sum::<usize>();
        if !self.outline_pressure && total <= LLG_OUTLINE_RUN_WEIGHT {
            return OutlinePlan::Light;
        }
        OutlinePlan::Split(segments(&weights))
    }

    /// Weight of each statement and whether a run may hold it.
    fn statement_weights(
        &self,
        statements: &[IrStmt],
        depth: bool,
        may_disable: &mut HashMap<usize, bool>,
    ) -> Vec<(usize, bool)> {
        let mut sequence_locals = HashSet::new();
        let mut weights = Vec::with_capacity(statements.len());
        for statement in statements {
            let mut labels = HashSet::new();
            collect_labels(statement, &mut labels);
            let mut check = Check {
                frame: self,
                depth,
                may_disable: &mut *may_disable,
                sequence_locals: &sequence_locals,
                scopes: vec![Vec::new()],
                labels,
                weight: 0,
                ok: true,
            };
            check.statement(statement);
            let (weight, ok) = (check.weight, check.ok);
            let unlocated = statement.unlocated();
            // A declaration binds for the rest of the sequence and a label is
            // a target from it, so neither may leave the body.
            let fixed = declared_name(unlocated).is_some() || matches!(unlocated, IrStmt::Label(_));
            if let Some(name) = declared_name(unlocated) {
                sequence_locals.insert(name.to_owned());
            }
            weights.push((weight, ok && !fixed));
        }
        weights
    }

    pub(super) fn outlined_statements(
        &mut self,
        statements: &[IrStmt],
        plan: Vec<Segment>,
    ) -> Result<(), String> {
        for segment in plan {
            match segment {
                Segment::Inline(range) => self.inline_statements(&statements[range])?,
                Segment::Heavy(index) => {
                    let pressure = std::mem::replace(&mut self.outline_pressure, true);
                    let rendered = self.inline_statements(&statements[index..=index]);
                    self.outline_pressure = pressure;
                    rendered?;
                }
                Segment::Outline(range) => {
                    let run = &statements[range];
                    if self.run_still_outlines(run) {
                        self.outline_run(run)?;
                    } else {
                        self.inline_statements(run)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// The plan judged a run before the statements ahead of it were emitted.
    /// One of those may have bound a name the run reads (only a top-level
    /// declaration is recognized in advance), so check the run again against
    /// the bindings now in scope.
    fn run_still_outlines(&mut self, run: &[IrStmt]) -> bool {
        let Some(mut sink) = self.outline.take() else {
            return false;
        };
        let ok = self
            .statement_weights(run, sink.depth, &mut sink.may_disable)
            .iter()
            .all(|(_, ok)| *ok);
        self.outline = Some(sink);
        ok
    }

    /// Render `run` as a helper and call it here.
    fn outline_run(&mut self, run: &[IrStmt]) -> Result<(), String> {
        let mut sink = self
            .outline
            .take()
            .ok_or_else(|| "outlined run without an outlining body".to_owned())?;
        let name = format!("{}_llg_run_{}", sink.owner, sink.next);
        let depth = sink.depth;
        sink.next += 1;
        let mut helper = Frame::new(self.ctx);
        helper.storage_names = self.storage_names;
        helper.cell_eligibility = self.cell_eligibility.clone();
        helper.slots = self.slots.clone();
        helper.batch_owner = self.batch_owner.as_ref().map(|_| name.clone());
        helper.outline = Some(sink);
        // A run is no heavier than one run, so nothing inside it splits.
        helper.outline_light = true;
        helper.begin_block(run);
        let rendered = helper.inline_statements(run);
        let sink = helper.outline.take();
        self.outline = sink;
        rendered?;
        helper.end_block();
        if let Some(error) = helper
            .declaration_error
            .take()
            .or(helper.structural_error.take())
        {
            return Err(error);
        }
        if !helper.brace_kinds.is_empty() {
            return Err(format!("outlined run `{name}` has unclosed C braces"));
        }
        if helper.slots.len() > self.slots.len() {
            self.slots.resize(helper.slots.len(), false);
        }
        let tables = std::mem::take(&mut helper.net_batches);
        self.net_batches.batches.extend(tables.batches);
        self.net_batches.statics.extend(tables.statics);
        let (parameters, arguments, unused) = if depth {
            (
                "sv4_t* _llg_t, int depth",
                "_llg_t, depth",
                "    (void)_llg_t;\n    (void)depth;\n",
            )
        } else {
            ("sv4_t* _llg_t", "_llg_t", "    (void)_llg_t;\n")
        };
        let source = format!(
            "static int {name}({parameters}) {{\n{}    int _llg_exit = 1;\n    llg_value_scope_t* _llg_frame_base = llg_value_scope_mark();\n{unused}{}    _llg_exit = 0;\n    if (0) goto _llg_return;\n    _llg_return: ;\n    llg_value_scopes_end_since(_llg_frame_base);\n    return _llg_exit;\n}}\n",
            helper.stack_cells,
            helper.body()
        );
        if let Some(sink) = self.outline.as_mut() {
            sink.helpers.push_str(&source);
        }
        if self.plain_exits() {
            self.line(format!(
                "if (LLG_CO_UNLIKELY({name}({arguments}))) goto _llg_return;"
            ));
        } else {
            self.line(format!(
                "if (LLG_CO_UNLIKELY({name}({arguments}))) return LLG_CO_EXIT;"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: usize = LLG_OUTLINE_RUN_WEIGHT;
    const MIN: usize = LLG_OUTLINE_MIN_RUN_WEIGHT;

    #[test]
    fn runs_never_exceed_the_run_weight() {
        let weights = vec![(RUN / 3, true); 7];
        assert_eq!(
            segments(&weights),
            vec![
                Segment::Outline(0..3),
                Segment::Outline(3..6),
                Segment::Outline(6..7),
            ]
        );
    }

    #[test]
    fn fixed_statements_split_runs_and_short_runs_stay_inline() {
        let weights = vec![
            (MIN, true),
            (1, false),
            (MIN - 1, true),
            (2, false),
            (RUN + 1, true),
            (MIN, true),
        ];
        assert_eq!(
            segments(&weights),
            vec![
                Segment::Outline(0..1),
                Segment::Inline(1..4),
                Segment::Heavy(4),
                Segment::Outline(5..6),
            ]
        );
    }
}
