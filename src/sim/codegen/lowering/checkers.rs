//! Checker restrictions that the frontend does not enforce for the IEEE
//! 1800-2009 checker forms (Clause 17).
//!
//! The frontend follows the later revision of Clause 17, whose checker
//! procedures are `always_comb`/`always_latch`/`always_ff`; it admits a 2009
//! `always` procedure without checking its statements. These checks run only
//! when the design instantiates a checker.

use super::*;
use std::collections::BTreeMap;

/// One assigned bit interval of a checker variable, in declared index
/// coordinates, with the assignment that writes it (17.7.1 SAR).
struct AssignedBits {
    low: i128,
    high: i128,
    assignment: NodeId,
}

impl Codegen<'_> {
    /// IEEE 1800-2009 17.5 and 17.7.1 restrictions on checker procedures.
    pub(super) fn validate_checker_constructs(&self) -> Result<(), String> {
        for checker in self.db.checker_instances() {
            let mut assigned: BTreeMap<u32, Vec<AssignedBits>> = BTreeMap::new();
            // Visit members in source order, so a Single Assignment Rule
            // violation is reported at the later assignment.
            let mut pending: Vec<NodeId> =
                self.node(checker).children.iter().rev().copied().collect();
            while let Some(member) = pending.pop() {
                match self.kind(member) {
                    // A nested checker validates its own procedures.
                    NodeKind::ModuleInst { .. } => {}
                    NodeKind::GenScope | NodeKind::GenScopeArray => {
                        pending.extend(self.node(member).children.iter().rev().copied());
                    }
                    // Free checker variables need assume-set solving before
                    // Observed (17.7.2), which is not implemented: reject them
                    // rather than simulate them as ordinary variables.
                    NodeKind::Var { .. }
                        if self.db.rand_qualifier(member)
                            != crate::core::db::RandQualifier::None =>
                    {
                        return Err(format!(
                            "{}: free checker variable `{}` is not supported: llg does not \
                             randomize free variables or solve assume sets (IEEE 1800-2009 \
                             17.7, 17.7.2)",
                            self.source_location(member),
                            self.node(member).name
                        ));
                    }
                    NodeKind::Process {
                        kind:
                            ProcessKind::Always {
                                always_type: AlwaysKind::Always,
                            },
                    } => {
                        let Some(body) = self.node(member).children.first().copied() else {
                            continue;
                        };
                        self.validate_checker_always(checker, body, &mut assigned)?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// A checker `always` procedure "may contain deferred and concurrent
    /// assertions, nonblocking variable assignments and a procedural timing
    /// control statement using an event control. All other statements shall
    /// not appear" (17.5); sequential blocks group them (17.3.1 example).
    fn validate_checker_always(
        &self,
        checker: NodeId,
        statement: NodeId,
        assigned: &mut BTreeMap<u32, Vec<AssignedBits>>,
    ) -> Result<(), String> {
        let illegal = |what: &str| {
            Err(format!(
                "{}: {what} is not allowed in a checker always procedure: it may contain only \
                 event controls, nonblocking assignments to checker variables and assertions \
                 (IEEE 1800-2009 17.5)",
                self.source_location(statement)
            ))
        };
        match self.kind(statement) {
            NodeKind::Stmt(StmtKind::Begin | StmtKind::Empty) => {
                for child in self.node(statement).children.clone() {
                    // A named block's declarations are not statements.
                    if matches!(
                        self.kind(child),
                        NodeKind::Stmt(_) | NodeKind::SysCall { .. } | NodeKind::FuncCall { .. }
                    ) || matches!(self.kind(child), NodeKind::MethodCall { .. })
                    {
                        self.validate_checker_always(checker, child, assigned)?;
                    }
                }
                Ok(())
            }
            NodeKind::Stmt(StmtKind::EventControl { body, .. }) => match body {
                Some(body) => self.validate_checker_always(checker, *body, assigned),
                None => Ok(()),
            },
            NodeKind::Stmt(
                StmtKind::ImmediateAssertion { deferred: true, .. }
                | StmtKind::ConcurrentAssertion { .. },
            ) => Ok(()),
            NodeKind::Stmt(StmtKind::ImmediateAssertion { .. }) => {
                illegal("a simple immediate assertion (only deferred and concurrent assertions are allowed)")
            }
            NodeKind::Stmt(StmtKind::Assign {
                blocking: false,
                delay: None,
                ..
            }) => self.record_checker_assignment(checker, statement, assigned),
            NodeKind::Stmt(StmtKind::Assign { blocking: true, .. }) => {
                illegal("a blocking assignment (17.7.1 allows only nonblocking assignments)")
            }
            NodeKind::Stmt(StmtKind::Assign { .. }) => {
                illegal("an assignment with an intra-assignment timing control")
            }
            NodeKind::Stmt(StmtKind::DelayControl { .. } | StmtKind::CycleDelayControl { .. }) => {
                illegal("a delay control (only event controls are allowed)")
            }
            NodeKind::SysCall { name } => illegal(&format!("the system task call `{name}`")),
            NodeKind::FuncCall { name, .. } => illegal(&format!("the call of `{name}`")),
            NodeKind::Stmt(StmtKind::IfElse { .. }) => illegal("an if statement"),
            NodeKind::Stmt(StmtKind::Case { .. } | StmtKind::PatternCase { .. }) => {
                illegal("a case statement")
            }
            NodeKind::Stmt(
                StmtKind::For { .. }
                | StmtKind::While { .. }
                | StmtKind::DoWhile { .. }
                | StmtKind::Repeat { .. }
                | StmtKind::Forever { .. }
                | StmtKind::Foreach { .. },
            ) => illegal("a loop statement"),
            NodeKind::Stmt(StmtKind::Wait { .. } | StmtKind::WaitOrder { .. }) => {
                illegal("a wait statement")
            }
            NodeKind::Stmt(StmtKind::Fork { .. }) => illegal("a fork-join block"),
            _ => illegal("this statement"),
        }
    }

    /// Checker variable assignment rules (17.7.1): the target is a checker
    /// variable, its select is its longest static prefix, and no bit is
    /// assigned twice (the Single Assignment Rule).
    fn record_checker_assignment(
        &self,
        checker: NodeId,
        assignment: NodeId,
        assigned: &mut BTreeMap<u32, Vec<AssignedBits>>,
    ) -> Result<(), String> {
        let Some(lhs) = self.node(assignment).children.first().copied() else {
            return Ok(());
        };
        let location = self.source_location(assignment);
        let (variable, bits) = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => (*target, None),
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let Some(variable) = self.ref_target(*base) else {
                    return Ok(());
                };
                let index = self.checker_static_index(*index, &location)?;
                (variable, Some((index, index)))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let Some(variable) = self.ref_target(*base) else {
                    return Ok(());
                };
                let left = self.checker_static_index(*left, &location)?;
                let right = self.checker_static_index(*right, &location)?;
                (variable, Some((left.min(right), left.max(right))))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                let Some(variable) = self.ref_target(*base) else {
                    return Ok(());
                };
                let start = self.checker_static_index(*base_expr, &location)?;
                let width = self.checker_static_index(*width_expr, &location)?;
                let (low, high) = if *neg {
                    (start - width + 1, start)
                } else {
                    (start, start + width - 1)
                };
                (variable, Some((low, high)))
            }
            // Other targets (nested selects, concatenations) keep only the
            // target check below.
            _ => match self.ref_target(lhs) {
                Some(variable) => (variable, None),
                None => return Ok(()),
            },
        };
        if self.owning_checker(variable) != Some(checker) {
            return Err(format!(
                "{location}: a checker procedure may assign only variables of its own checker, \
                 not `{}` (IEEE 1800-2009 17.7.1)",
                self.node(variable).name
            ));
        }
        let (low, high) = bits.unwrap_or((i128::MIN, i128::MAX));
        let intervals = assigned.entry(variable.0).or_default();
        if let Some(previous) = intervals
            .iter()
            .find(|interval| interval.low <= high && low <= interval.high)
        {
            return Err(format!(
                "{location}: checker variable `{}` is already assigned at {}; a bit of a checker \
                 variable may be assigned only once (Single Assignment Rule, IEEE 1800-2009 \
                 17.7.1)",
                self.node(variable).name,
                self.source_location(previous.assignment)
            ));
        }
        intervals.push(AssignedBits {
            low,
            high,
            assignment,
        });
        Ok(())
    }

    /// A select of a checker variable assignment target, which must be a
    /// constant: the target is its longest static prefix (17.7.1).
    fn checker_static_index(&self, index: NodeId, location: &str) -> Result<i128, String> {
        self.eval_bound_i128(index).map_err(|_| {
            format!(
                "{location}: the target of a checker variable assignment must be its longest \
                 static prefix; its select must be a constant expression (IEEE 1800-2009 \
                 17.7.1, 11.5.3)"
            )
        })
    }

    fn ref_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.ref_target(*base),
            _ => None,
        }
    }

    /// The innermost checker instance that declares `node`.
    pub(super) fn owning_checker(&self, node: NodeId) -> Option<NodeId> {
        let mut current = self.node(node).parent;
        while let Some(scope) = current {
            if matches!(self.kind(scope), NodeKind::ModuleInst { .. }) {
                return self.db.checker_instance(scope).map(|_| scope);
            }
            current = self.node(scope).parent;
        }
        None
    }
}
