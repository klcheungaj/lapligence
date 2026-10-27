//! Free checker variables (IEEE 1800-2009 17.7, 17.7.2).
//!
//! Every free variable starts with an unconstrained random value unless its
//! declaration initializes it, drawn in the initialization schedule from its
//! checker instance's initialization stream. The active ones (not `const`
//! and assigned by no checker procedure) are read with their current value
//! wherever values are otherwise sampled, and each checker instance with
//! active variables gets one [`IrFreeSet`]: a solve callback the runtime
//! runs before the Observed region, at the clock events of the instance's
//! assume set, or in every time step when the set is empty. The callback
//! solves one constraint per enforced assume statement on the constraint
//! solver: for a predicate assumption `A |-> C` (or `C`), "A implies C" when
//! its clock event is due; for `A |=> C`, C when a pending attempt owes its
//! consequent at this event. Other property forms are in the set (their
//! clock events trigger solving) but add no constraint; "there is no
//! requirement that a solution be found if it exists", and the assumption
//! itself is still checked in the Observed region.

use super::*;
use crate::sim::codegen::lowering::randomization::FreeSolveVar;
use crate::sim::ir::{IrConcurrentAssertionKind, IrFreeSet, IrPreFn, IrRuntimeQuery, IrSysFunc};

/// The active free variables of one checker instance, planned before any
/// process or assertion is lowered.
pub(in crate::sim::codegen) struct FreeSetPlan {
    checker: NodeId,
    variables: Vec<FreeSolveVar>,
}

fn query(query: IrRuntimeQuery) -> IrExpr {
    IrExpr::new(IrExprKind::RuntimeQuery(query), 1, false, None)
}

fn not(value: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::LogNot,
            a: Box::new(value),
        },
        1,
        false,
        None,
    )
}

fn or(a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::Bin {
            op: IrBinOp::LogOr,
            a: Box::new(a),
            b: Box::new(b),
        },
        1,
        false,
        None,
    )
}

/// An unconstrained random value of `width` bits from the calling stream.
fn random_value(width: u32) -> IrExpr {
    let words = width.div_ceil(32).max(1);
    let parts = (0..words)
        .map(|_| {
            IrExpr::new(
                IrExprKind::SysFunc(Box::new(IrSysFunc::Urandom { seed: None })),
                32,
                false,
                None,
            )
        })
        .collect::<Vec<_>>();
    let value = if words == 1 {
        parts.into_iter().next().expect("one word")
    } else {
        IrExpr::new(IrExprKind::Concat { parts }, words * 32, false, None)
    };
    IrExpr::resize_to(value, width, false)
}

/// Every signal `expression` reads.
fn signal_reads(expression: &IrExpr, reads: &mut HashSet<usize>) {
    let mut copy = expression.clone();
    crate::sim::opt::walk_expr_mut(&mut copy, &mut |node| {
        if let IrExprKind::SigRead(signal) = node.kind {
            reads.insert(signal);
        }
    });
}

/// Every signal an assertion's expressions read.
fn assertion_reads(assertion: &crate::sim::ir::IrAssertion) -> HashSet<usize> {
    let mut reads = HashSet::new();
    for expression in assertion
        .antecedent
        .iter()
        .chain(&assertion.consequent)
        .chain(&assertion.abort_condition)
    {
        signal_reads(expression, &mut reads);
    }
    let sequences = assertion
        .antecedent_sequence
        .iter()
        .chain(&assertion.consequent_sequence)
        .chain(
            assertion
                .property
                .iter()
                .flat_map(|property| &property.sequences),
        );
    for sequence in sequences {
        for expression in sequence
            .atoms()
            .iter()
            .chain(sequence.match_items())
            .chain(sequence.initializers())
        {
            signal_reads(expression, &mut reads);
        }
    }
    if let Some(property) = &assertion.property {
        for expression in &property.atoms {
            signal_reads(expression, &mut reads);
        }
    }
    reads
}

impl Codegen<'_> {
    /// Classify the free variables, give the uninitialized ones their random
    /// initial values and record the active ones, before any read of them
    /// is lowered.
    pub(in crate::sim::codegen) fn plan_free_variables(&mut self) -> Result<(), String> {
        let checkers = std::mem::take(&mut self.checker_free);
        let mut active_signals = Vec::new();
        let mut plans = Vec::new();
        for entry in &checkers {
            let path = self.node(entry.checker).full_name.to_string();
            let mut variables = Vec::new();
            for &variable in &entry.variables {
                let name = self.node(variable).name.clone();
                let location = self.source_location(variable);
                let unsupported = |what: &str| {
                    format!(
                        "{location}: free checker variable `{name}` {what} is not supported: \
                         llg randomizes packed integral free variables (IEEE 1800-2009 17.7.2)"
                    )
                };
                let Some(info) = self.sig_globals.get(&variable).cloned() else {
                    return Err(unsupported("of an unpacked type"));
                };
                if info.real || info.net_driver.is_some() {
                    return Err(unsupported("of a real type"));
                }
                if self.enum_metadata_for_expr(variable).is_some() {
                    return Err(unsupported("of an enum type"));
                }
                let initialized = self.db.var_initializer(variable).is_some();
                if !initialized {
                    // "All free checker variables, both const and non-const,
                    // active and inactive, are initialized with unconstrained
                    // random values unless explicitly initialized in their
                    // declaration."
                    let identity = self.declaration_identity(variable)?;
                    let value = ir_to_storage(
                        random_value(info.width),
                        info.width,
                        info.signed,
                        info.two_state,
                    )?;
                    self.declaration_statements.push((
                        identity,
                        IrStmt::Assign {
                            lhs: IrLhs::Whole(info.ir),
                            rhs: value,
                            nba: false,
                        },
                    ));
                    self.initializer_random_scopes
                        .insert(identity, format!("{path}.initializer"));
                    self.model.random.threads = true;
                }
                let active =
                    !self.db.is_const_variable(variable) && !entry.assigned.contains(&variable);
                if !active {
                    continue;
                }
                active_signals.push(info.ir);
                self.free_variable_names.insert(info.ir, name.clone());
                variables.push(FreeSolveVar {
                    signal: info.ir,
                    name,
                    width: info.width,
                    signed: info.signed,
                    two_state: info.two_state,
                    // An initialized variable keeps its initial value in the
                    // first time step (17.7, "except the first one where its
                    // value is defined by the initialization").
                    active: if initialized {
                        not(query(IrRuntimeQuery::InitialTimeStep))
                    } else {
                        IrExpr::resize_to(lhs_integer_expr(1), 1, false)
                    },
                });
            }
            if !variables.is_empty() {
                plans.push(FreeSetPlan {
                    checker: entry.checker,
                    variables,
                });
            }
        }
        active_signals.sort_unstable();
        active_signals.dedup();
        self.model.free_variables = active_signals;
        self.free_set_plans = plans;
        self.checker_free = checkers;
        Ok(())
    }

    /// One solve callback and [`IrFreeSet`] per checker instance with active
    /// free variables, after every assertion is lowered.
    pub(in crate::sim::codegen) fn emit_free_variable_sets(&mut self) -> Result<(), String> {
        for plan in std::mem::take(&mut self.free_set_plans) {
            let path = self.node(plan.checker).full_name.to_string();
            let nested = format!("{path}.");
            let signals = plan
                .variables
                .iter()
                .map(|variable| variable.signal)
                .collect::<HashSet<_>>();
            // "The assume set of a checker instance is formed from the
            // checker assume statements and child checker assume statements
            // ... those that reference active free variables of the checker
            // are included in the assume set."
            let mut assumes = Vec::new();
            let mut constraints = Vec::new();
            for assertion in &self.model.assertions {
                if assertion.kind != IrConcurrentAssertionKind::Assume
                    || assertion.endpoint.is_some()
                    || (*assertion.scope != *path && !assertion.scope.starts_with(&nested))
                    || assertion_reads(assertion).is_disjoint(&signals)
                {
                    continue;
                }
                let index = u32::try_from(assumes.len())
                    .map_err(|_| format!("checker `{path}` has too many assume statements"))?;
                assumes.push(assertion.identity);
                let predicate = assertion.property.is_none()
                    && assertion.antecedent_sequence.is_none()
                    && assertion.consequent_sequence.is_none()
                    && assertion.abort_condition.is_none()
                    && assertion.future.is_none();
                let Some(consequent) = assertion.consequent.clone().filter(|_| predicate) else {
                    continue;
                };
                // The values must keep every assumption from failing in this
                // time step: an overlapping attempt begun at this event
                // checks its consequent now, a `|=>` attempt begun earlier
                // checks it at this event.
                let constraint = if assertion.overlapped || assertion.antecedent.is_none() {
                    let event = query(IrRuntimeQuery::CheckerAssume {
                        assume: index,
                        due: false,
                    });
                    let holds = match assertion.antecedent.clone() {
                        Some(antecedent) => or(not(antecedent), consequent),
                        None => consequent,
                    };
                    or(not(event), holds)
                } else {
                    let due = query(IrRuntimeQuery::CheckerAssume {
                        assume: index,
                        due: true,
                    });
                    or(not(due), consequent)
                };
                constraints.push(constraint);
            }
            let label = format!("{path}.free");
            let solve = self.new_fn_name(&label, "free_solve");
            let site = format!("free variables of checker instance {path}");
            let (body, _) = self.free_variable_solve(
                &format!("_llg_free_{}", plan.checker.index()),
                &site,
                &plan.variables,
                &constraints,
            )?;
            let frame = self.new_frame_id()?;
            let host = self.new_fn_name(&label, "free_host");
            self.model.processes.push(
                IrProcess::new_with_kind_and_writes(
                    host.clone(),
                    label.clone(),
                    IrProcessKind::Synthetic,
                    IrShape::RunOnce,
                    Vec::new(),
                    vec![IrPreFn::DeferredAssertion {
                        c_name: solve.clone(),
                        frame,
                        captures: Vec::new(),
                        body,
                    }],
                    Vec::new(),
                    self.origin(plan.checker),
                )
                .into_callback_host(),
            );
            self.assertion_action_procs.insert(host);
            self.model
                .free_sets
                .push(IrFreeSet::new(label, solve, assumes));
        }
        Ok(())
    }
}
