//! Traversal.

use super::*;

// ── Expression walking (every slot, recursively into statements) ─────────────

pub(super) fn walk_lhs_mut(l: &mut IrLhs, f: &mut impl FnMut(&mut IrExpr)) {
    match l {
        IrLhs::Bit(_, idx, _) => walk_expr_mut(idx, f),
        IrLhs::IdxPart(_, base, width, _, _, _) => {
            walk_expr_mut(base, f);
            walk_expr_mut(width, f);
        }
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => {
            for i in indices {
                walk_expr_mut(i, f);
            }
            if let IrElemSel::Bit(idx) | IrElemSel::Indexed { base: idx, .. } = elem_sel {
                walk_expr_mut(idx, f);
            }
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                walk_lhs_mut(part, f);
            }
        }
        _ => {}
    }
}

fn walk_call_args_mut(args: &mut [IrCallArg], f: &mut impl FnMut(&mut IrExpr)) {
    for arg in args {
        match arg {
            IrCallArg::Val(e) => walk_expr_mut(e, f),
            IrCallArg::StringVal(value) => {
                value.expressions_mut(&mut |child| walk_expr_mut(child, f));
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
                    walk_expr_mut(init, f);
                }
                walk_lhs_mut(writeback, f);
                if let Some(storage_lhs) = storage_lhs {
                    walk_lhs_mut(storage_lhs, f);
                }
                if let Some(storage_read) = storage_read {
                    walk_expr_mut(storage_read, f);
                }
                for (_, _, _, _, init) in selector_inits {
                    walk_expr_mut(init, f);
                }
            }
            IrCallArg::OutAddr(_) => {}
            IrCallArg::StringOutAddr(_) | IrCallArg::StringRefAddr { .. } => {}
            IrCallArg::RefAddr { read, lhs, .. } => {
                walk_expr_mut(read, f);
                walk_lhs_mut(lhs, f);
            }
            IrCallArg::StringOutTemp {
                init, storage_read, ..
            } => {
                if let Some(init) = init {
                    init.expressions_mut(&mut |child| walk_expr_mut(child, f));
                }
                if let Some(read) = storage_read {
                    read.expressions_mut(&mut |child| walk_expr_mut(child, f));
                }
            }
            IrCallArg::ChandleVal(_) | IrCallArg::ChandleAddr(_) | IrCallArg::ChandleRefAddr(_) => {
            }
        }
    }
}

fn walk_expr_mut(e: &mut IrExpr, f: &mut impl FnMut(&mut IrExpr)) {
    match &mut e.kind {
        IrExprKind::Container(operation) => {
            operation.expressions_mut(&mut |child| walk_expr_mut(child, f))
        }
        IrExprKind::ObjectQuery(query) => {
            query.expressions_mut(&mut |child| walk_expr_mut(child, f))
        }
        IrExprKind::EnumMethod(query) => {
            query.expressions_mut(&mut |child| walk_expr_mut(child, f))
        }
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            walk_expr_mut(a, f);
            walk_expr_mut(b, f);
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::RealUn { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::BitStreamCast { a, .. }
        | IrExprKind::ToTwoState { a } => walk_expr_mut(a, f),
        IrExprKind::CastToReal { a, .. } => walk_expr_mut(a, f),
        IrExprKind::Mux { sel, a, b } => {
            walk_expr_mut(sel, f);
            walk_expr_mut(a, f);
            walk_expr_mut(b, f);
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            for p in parts {
                walk_expr_mut(p, f);
            }
        }
        IrExprKind::Stream { value, .. } => walk_expr_mut(value, f),
        IrExprKind::Inside { value, items } => {
            walk_expr_mut(value, f);
            for item in items {
                match item {
                    IrInsideItem::Value(item) => walk_expr_mut(item, f),
                    IrInsideItem::Range { low, high } => {
                        walk_expr_mut(low, f);
                        walk_expr_mut(high, f);
                    }
                    IrInsideItem::OpenRange { low, high } => {
                        if let Some(low) = low {
                            walk_expr_mut(low, f);
                        }
                        if let Some(high) = high {
                            walk_expr_mut(high, f);
                        }
                    }
                    IrInsideItem::Container { .. } => {}
                }
            }
        }
        IrExprKind::BitSel { base, idx } => {
            walk_expr_mut(base, f);
            walk_expr_mut(idx, f);
        }
        IrExprKind::PartSel { base, .. } => walk_expr_mut(base, f),
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => {
            walk_expr_mut(base, f);
            walk_expr_mut(base_idx, f);
            walk_expr_mut(width_expr, f);
        }
        IrExprKind::ArrayRead {
            indices, elem_sel, ..
        } => {
            for i in indices {
                walk_expr_mut(i, f);
            }
            if let IrElemSel::Bit(idx) | IrElemSel::Indexed { base: idx, .. } = elem_sel {
                walk_expr_mut(idx, f);
            }
        }
        IrExprKind::CallFn(call) => {
            walk_call_args_mut(&mut call.args, f);
            if let Some(virtual_call) = &mut call.virtual_call {
                virtual_call
                    .receiver
                    .expressions_mut(&mut |child| walk_expr_mut(child, f));
            }
        }
        IrExprKind::Mutation(mutation) => {
            walk_lhs_mut(&mut mutation.lhs, f);
            walk_expr_mut(&mut mutation.value, f);
        }
        IrExprKind::DynamicCast(cast) => {
            walk_lhs_mut(&mut cast.lhs, f);
            walk_expr_mut(&mut cast.rhs, f);
            if let Some(source) = &mut cast.class_source {
                source.expressions_mut(&mut |child| walk_expr_mut(child, f));
            }
            for value in &mut cast.valid_values {
                walk_expr_mut(value, f);
            }
        }
        IrExprKind::SysFunc(sf) => match sf {
            IrSysFunc::TestPlusArgs { pattern } => {
                pattern.expressions_mut(&mut |expression| walk_expr_mut(expression, f))
            }
            IrSysFunc::ValuePlusArgs { format, target } => {
                format.expressions_mut(&mut |expression| walk_expr_mut(expression, f));
                match target {
                    crate::sim::ir::IrPlusArgTarget::Packed { lhs, .. }
                    | crate::sim::ir::IrPlusArgTarget::Real { lhs, .. } => walk_lhs_mut(lhs, f),
                    crate::sim::ir::IrPlusArgTarget::String { .. } => {}
                }
            }
            IrSysFunc::System(Some(command)) => {
                command.expressions_mut(&mut |child| walk_expr_mut(child, f));
            }
            IrSysFunc::System(None) => {}
            IrSysFunc::VpiCall { args, .. } => {
                for arg in args {
                    walk_expr_mut(arg, f);
                }
            }
            IrSysFunc::LegacyRandom { seed, args, .. } => {
                if let Some(seed) = seed {
                    walk_lhs_mut(seed, f);
                }
                for arg in args {
                    walk_expr_mut(arg, f);
                }
            }
            IrSysFunc::Urandom { seed } => {
                if let Some(seed) = seed {
                    walk_expr_mut(seed, f);
                }
            }
            IrSysFunc::UrandomRange { max, min } => {
                walk_expr_mut(max, f);
                if let Some(min) = min {
                    walk_expr_mut(min, f);
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
            | IrSysFunc::BitsToShortReal(a) => walk_expr_mut(a, f),
            IrSysFunc::Math { args, .. } => {
                for arg in args {
                    walk_expr_mut(arg, f);
                }
            }
            IrSysFunc::QFull { q_id, status } => {
                walk_expr_mut(q_id, f);
                walk_lhs_mut(status, f);
            }
            IrSysFunc::Time { .. } | IrSysFunc::Realtime { .. } => {}
            IrSysFunc::FileOpen { path, mode } => {
                path.expressions_mut(&mut |expression| walk_expr_mut(expression, f));
                if let Some(mode) = mode {
                    mode.expressions_mut(&mut |expression| walk_expr_mut(expression, f));
                }
            }
            IrSysFunc::FileTell(descriptor) | IrSysFunc::FileEof(descriptor) => {
                walk_expr_mut(descriptor, f)
            }
            IrSysFunc::FileSeek {
                descriptor,
                offset,
                operation,
            } => {
                walk_expr_mut(descriptor, f);
                walk_expr_mut(offset, f);
                walk_expr_mut(operation, f);
            }
            IrSysFunc::FileError { descriptor, .. } => walk_expr_mut(descriptor, f),
            IrSysFunc::FileInput(input) => {
                input.expressions_mut(&mut |expression| walk_expr_mut(expression, f));
            }
            IrSysFunc::Sampled(call) => walk_expr_mut(&mut call.argument, f),
        },
        _ => {}
    }
    f(e);
}

/// Visit every expression slot of one statement (recursively).
fn walk_stmt_mut(s: &mut IrStmt, f: &mut impl FnMut(&mut IrExpr)) {
    if let Some(value) = s.delay_expression_mut() {
        walk_expr_mut(value, f);
    }
    match s {
        IrStmt::System(Some(command)) => {
            command.expressions_mut(&mut |child| walk_expr_mut(child, f));
        }
        IrStmt::VpiCall { args, .. } => {
            for arg in args {
                walk_expr_mut(arg, f);
            }
        }
        IrStmt::Memory {
            path,
            start,
            finish,
            ..
        } => {
            path.expressions_mut(&mut |child| walk_expr_mut(child, f));
            if let Some(start) = start {
                walk_expr_mut(start, f);
            }
            if let Some(finish) = finish {
                walk_expr_mut(finish, f);
            }
        }
        IrStmt::Container(operation) => {
            operation.expressions_mut(&mut |child| walk_expr_mut(child, f))
        }
        IrStmt::StreamAssign {
            source, targets, ..
        } => {
            walk_expr_mut(source, f);
            for target in targets {
                match target {
                    IrStreamTarget::Packed { lhs, .. } => walk_lhs_mut(lhs, f),
                    IrStreamTarget::Container { selector, .. } => {
                        if let Some(selector) = selector {
                            walk_stream_selector_mut(selector, f);
                        }
                    }
                }
            }
        }
        IrStmt::Object(operation) => {
            operation.expressions_mut(&mut |child| walk_expr_mut(child, f))
        }
        IrStmt::PlusArg(expression) => walk_expr_mut(expression, f),
        IrStmt::RandomSeed { seed } => walk_expr_mut(seed, f),
        IrStmt::RandomStateSet { state } => {
            state.expressions_mut(&mut |child| walk_expr_mut(child, f));
        }
        IrStmt::Block(b)
        | IrStmt::Forever { body: b }
        | IrStmt::ActivationScope { body: b, .. } => walk_stmts_mut(b, f),
        IrStmt::Repeat { count, body } => {
            walk_expr_mut(count, f);
            walk_stmts_mut(body, f);
        }
        IrStmt::ClockingCycleWait { count, .. } => walk_expr_mut(count, f),
        IrStmt::DeclLocal {
            init: Some(init), ..
        } => walk_expr_mut(init, f),
        IrStmt::Assign { lhs, rhs, .. }
        | IrStmt::DelayedAssign { lhs, rhs, .. }
        | IrStmt::ClockingDrive { lhs, rhs, .. }
        | IrStmt::InertialAssign { lhs, rhs, .. } => {
            walk_lhs_mut(lhs, f);
            walk_expr_mut(rhs, f);
        }
        IrStmt::Stochastic(operation) => match operation.as_mut() {
            IrStochasticStmt::Initialize {
                q_id,
                q_type,
                max_length,
                status,
            } => {
                walk_expr_mut(q_id, f);
                walk_expr_mut(q_type, f);
                walk_expr_mut(max_length, f);
                walk_lhs_mut(status, f);
            }
            IrStochasticStmt::Add {
                q_id,
                job_id,
                inform_id,
                status,
            } => {
                walk_expr_mut(q_id, f);
                walk_expr_mut(job_id, f);
                walk_expr_mut(inform_id, f);
                walk_lhs_mut(status, f);
            }
            IrStochasticStmt::Remove {
                q_id,
                job_id,
                inform_id,
                status,
            } => {
                walk_expr_mut(q_id, f);
                walk_lhs_mut(job_id, f);
                walk_lhs_mut(inform_id, f);
                walk_lhs_mut(status, f);
            }
            IrStochasticStmt::Exam {
                q_id,
                stat_code,
                stat_value,
                status,
            } => {
                walk_expr_mut(q_id, f);
                walk_expr_mut(stat_code, f);
                walk_lhs_mut(stat_value, f);
                walk_lhs_mut(status, f);
            }
        },
        IrStmt::NonblockingEventAssignWhen {
            lhs,
            rhs,
            repeat,
            captures,
            ..
        } => {
            walk_lhs_mut(lhs, f);
            walk_expr_mut(rhs, f);
            if let Some(repeat) = repeat {
                walk_expr_mut(repeat, f);
            }
            for capture in captures {
                walk_expr_mut(capture.initial_mut(), f);
            }
        }
        IrStmt::DelayedStringAssign { rhs, .. } => {
            rhs.expressions_mut(&mut |expr| walk_expr_mut(expr, f));
        }
        IrStmt::PcaAssign { value, .. } | IrStmt::PcaDrive { value, .. } => {
            walk_expr_mut(value, f);
        }
        IrStmt::If {
            cond, then_, els, ..
        } => {
            walk_expr_mut(cond, f);
            walk_stmts_mut(then_, f);
            if let Some(els) = els {
                walk_stmts_mut(els, f);
            }
        }
        IrStmt::ImmediateAssertion {
            condition,
            if_true,
            if_false,
            ..
        } => {
            walk_expr_mut(condition, f);
            if let Some(if_true) = if_true {
                walk_stmts_mut(if_true, f);
            }
            if let Some(if_false) = if_false {
                walk_stmts_mut(if_false, f);
            }
        }
        IrStmt::DeferredImmediateAssertion {
            condition,
            if_true,
            if_false,
            ..
        } => {
            walk_expr_mut(condition, f);
            for action in if_true.iter_mut().chain(if_false.iter_mut()) {
                for capture in action.captures_mut() {
                    walk_expr_mut(capture.initial_mut(), f);
                }
            }
        }
        IrStmt::While { cond, body } => {
            walk_expr_mut(cond, f);
            walk_stmts_mut(body, f);
        }
        IrStmt::WaitCond { cond, body, .. } => {
            walk_expr_mut(cond, f);
            walk_stmts_mut(body, f);
        }
        IrStmt::WaitEventTriggered { body, .. } => walk_stmts_mut(body, f),
        IrStmt::WaitOrder {
            success, failure, ..
        } => {
            walk_stmts_mut(success, f);
            walk_stmts_mut(failure, f);
        }
        IrStmt::For {
            init,
            cond,
            incr,
            body,
        } => {
            walk_stmts_mut(init, f);
            walk_expr_mut(cond, f);
            walk_stmts_mut(incr, f);
            walk_stmts_mut(body, f);
        }
        IrStmt::Case { sel, items, .. } => {
            walk_expr_mut(sel, f);
            for item in items {
                for e in &mut item.exprs {
                    walk_expr_mut(e, f);
                }
                walk_stmts_mut(&mut item.body, f);
            }
        }
        IrStmt::CapturedFork { branches, .. } => {
            for branch in branches {
                for capture in &mut branch.captures {
                    walk_expr_mut(capture.initial_mut(), f);
                }
            }
        }
        IrStmt::Force { lhs, value, .. } => {
            walk_lhs_mut(lhs, f);
            walk_expr_mut(value, f);
        }
        IrStmt::Display { args, .. } => {
            for (e, _) in args {
                walk_expr_mut(e, f);
            }
        }
        IrStmt::DisplayTyped {
            args, descriptor, ..
        } => {
            for arg in args {
                arg.expressions_mut(&mut |expression| walk_expr_mut(expression, f));
            }
            if let Some(descriptor) = descriptor {
                walk_expr_mut(descriptor, f);
            }
        }
        IrStmt::Severity { args, .. } => {
            for arg in args {
                arg.expressions_mut(&mut |expression| walk_expr_mut(expression, f));
            }
        }
        IrStmt::AssertionControl { args, .. } => {
            for arg in args {
                walk_expr_mut(arg, f);
            }
        }
        IrStmt::MonitorSet {
            descriptor: Some(descriptor),
            ..
        }
        | IrStmt::FileControl {
            descriptor: Some(descriptor),
            ..
        } => walk_expr_mut(descriptor, f),
        IrStmt::TimeFormat {
            units,
            precision,
            suffix,
            minimum_field_width,
        } => {
            walk_expr_mut(units, f);
            walk_expr_mut(precision, f);
            walk_expr_mut(minimum_field_width, f);
            suffix.expressions_mut(&mut |expression| walk_expr_mut(expression, f));
        }
        IrStmt::WaveLimit(limit) => walk_expr_mut(limit, f),
        IrStmt::Call(call) => {
            walk_call_args_mut(&mut call.args, f);
            if let Some(virtual_call) = &mut call.virtual_call {
                virtual_call
                    .receiver
                    .expressions_mut(&mut |child| walk_expr_mut(child, f));
            }
            // Temp initializers and copy-out select indices are expression
            // slots of the call node.
            for (_, _, init) in &mut call.temps {
                if let Some(init) = init {
                    walk_expr_mut(init, f);
                }
            }
            for (lh, ..) in &mut call.copyouts {
                walk_lhs_mut(lh, f);
            }
        }
        IrStmt::Return { value: Some(value) } => walk_expr_mut(value, f),
        _ => {}
    }
}

fn walk_stream_selector_mut(selector: &mut IrStreamSelector, f: &mut impl FnMut(&mut IrExpr)) {
    match selector {
        IrStreamSelector::Index(index) => walk_expr_mut(index, f),
        IrStreamSelector::Range { left, right } => {
            walk_expr_mut(left, f);
            walk_expr_mut(right, f);
        }
        IrStreamSelector::Indexed { base, width, .. } => {
            walk_expr_mut(base, f);
            walk_expr_mut(width, f);
        }
    }
}

fn walk_stmts_mut(stmts: &mut [IrStmt], f: &mut impl FnMut(&mut IrExpr)) {
    for s in stmts {
        walk_stmt_mut(s, f);
    }
}

fn walk_pre_fn_mut(pre: &mut IrPreFn, f: &mut impl FnMut(&mut IrExpr)) {
    match pre {
        IrPreFn::Branch { body, .. } => walk_stmts_mut(body, f),
        IrPreFn::CapturedBranch { captures, body, .. } => {
            for capture in captures {
                walk_expr_mut(capture.initial_mut(), f);
            }
            walk_stmts_mut(body, f);
        }
        IrPreFn::MonEval { args, context, .. } => {
            for e in args {
                walk_expr_mut(e, f);
            }
            if let Some(context) = context {
                for capture in context.captures_mut() {
                    walk_expr_mut(capture.initial_mut(), f);
                }
            }
        }
        IrPreFn::EventAssign {
            captures, lhs, rhs, ..
        } => {
            for capture in captures {
                walk_expr_mut(capture.initial_mut(), f);
            }
            walk_lhs_mut(lhs, f);
            walk_expr_mut(rhs, f);
        }
        IrPreFn::DeferredAssertion { captures, body, .. } => {
            for capture in captures {
                walk_expr_mut(capture.initial_mut(), f);
            }
            walk_stmts_mut(body, f);
        }
        IrPreFn::DisplayEval { args, .. } => {
            for arg in args {
                arg.expressions_mut(&mut |expression| walk_expr_mut(expression, f));
            }
        }
        IrPreFn::RealEval { value, context, .. } => {
            walk_expr_mut(value, f);
            if let Some(context) = context {
                for capture in context.captures_mut() {
                    walk_expr_mut(capture.initial_mut(), f);
                }
            }
        }
        IrPreFn::ForceEval { value, .. } => walk_expr_mut(value, f),
    }
}

pub(super) fn walk_model_exprs_mut(model: &mut IrModel, f: &mut impl FnMut(&mut IrExpr)) {
    for func in &mut model.funcs {
        for local in &mut func.locals {
            if let Some(initial) = &mut local.initial {
                walk_expr_mut(initial, f);
            }
        }
        for pre in &mut func.pre_fns {
            walk_pre_fn_mut(pre, f);
        }
        walk_stmts_mut(&mut func.body, f);
    }
    for p in &mut model.processes {
        for pre in &mut p.pre_fns {
            walk_pre_fn_mut(pre, f);
        }
        walk_stmts_mut(&mut p.body, f);
    }
    for step in &mut model.init_steps {
        if let crate::sim::ir::IrInitStep::Initialize(initialization) = step {
            walk_expr_mut(&mut initialization.value, f);
        }
    }
    for assertion in &mut model.assertions {
        if let Some(condition) = &mut assertion.abort_condition {
            walk_expr_mut(condition, f);
        }
        if let Some(antecedent) = &mut assertion.antecedent {
            walk_expr_mut(antecedent, f);
        }
        if let Some(consequent) = &mut assertion.consequent {
            walk_expr_mut(consequent, f);
        }
        if let Some(sequence) = &mut assertion.antecedent_sequence {
            for atom in &mut sequence.atoms {
                walk_expr_mut(atom, f);
            }
            for item in &mut sequence.match_items {
                walk_expr_mut(item, f);
            }
            for initializer in &mut sequence.initializers {
                walk_expr_mut(initializer, f);
            }
        }
        if let Some(sequence) = &mut assertion.consequent_sequence {
            for atom in &mut sequence.atoms {
                walk_expr_mut(atom, f);
            }
            for item in &mut sequence.match_items {
                walk_expr_mut(item, f);
            }
            for initializer in &mut sequence.initializers {
                walk_expr_mut(initializer, f);
            }
        }
    }
    for domain in &mut model.sampled_domains {
        walk_expr_mut(&mut domain.sample, f);
        if let Some(gate) = &mut domain.gate {
            walk_expr_mut(gate, f);
        }
    }
}

pub(super) fn walk_execution_exprs_mut(processes: &mut [ExecutionProcess], f: &mut impl FnMut(&mut IrExpr)) {
    for process in processes {
        for block in &mut process.blocks {
            walk_stmts_mut(&mut block.operations, f);
        }
    }
}
