//! Definite whole-result assignment for private static-function callbacks.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Jump {
    Break(NodeId),
    Continue(NodeId),
}

/// None is unreachable; Some(false) reaches an exit without a whole-result
/// write. Each jump carries its lexical loop, so an inner break cannot escape
/// an outer loop. This analysis never credits partial writes or header effects.
#[derive(Default)]
struct Flow {
    next: Option<bool>,
    returns: Option<bool>,
    jumps: HashMap<Jump, bool>,
    unknown: bool,
}

fn meet(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left && right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

impl Flow {
    fn falls(assigned: bool) -> Self {
        Self {
            next: Some(assigned),
            ..Self::default()
        }
    }

    fn merge(mut self, other: Self) -> Self {
        self.next = meet(self.next, other.next);
        self.returns = meet(self.returns, other.returns);
        self.unknown |= other.unknown;
        for (target, assigned) in other.jumps {
            self.jumps
                .entry(target)
                .and_modify(|old| *old &= assigned)
                .or_insert(assigned);
        }
        self
    }

    fn jump(target: Option<Jump>, assigned: bool) -> Self {
        let mut flow = Self::default();
        if let Some(target) = target {
            flow.jumps.insert(target, assigned);
        } else {
            flow.unknown = true;
        }
        flow
    }

    fn exit_loop(mut self, node: NodeId, entry: Option<bool>, post_test: bool) -> Self {
        let breaks = self.jumps.remove(&Jump::Break(node));
        let continues = self.jumps.remove(&Jump::Continue(node));
        // A do-loop can exit after its first body/continue path. A pre-test
        // loop may skip the body; a forever loop exits only through break.
        let condition_exit = if post_test {
            meet(self.next, continues)
        } else {
            entry
        };
        self.next = meet(condition_exit, breaks);
        self
    }
}

impl Codegen<'_> {
    pub(super) fn function_establishes_return_on_every_path(
        &self,
        root: NodeId,
        function: NodeId,
    ) -> bool {
        let flow = statement(self, root, function, false, None);
        !flow.unknown
            && flow.jumps.is_empty()
            && flow.next.unwrap_or(true)
            && flow.returns.unwrap_or(true)
    }
}

fn assigned_lhs(cg: &Codegen<'_>, lhs: NodeId, function: NodeId) -> bool {
    matches!(
        cg.kind(lhs),
        NodeKind::Expr(ExprKind::Ref { target: Some(target) })
            if cg.canonical_func_target(*target).unwrap_or(*target) == function
    )
}

fn sequence(
    cg: &Codegen<'_>,
    nodes: &[NodeId],
    function: NodeId,
    input: bool,
    loop_target: Option<NodeId>,
) -> Flow {
    let mut flow = Flow::falls(input);
    for node in nodes {
        let Some(assigned) = flow.next.take() else {
            break;
        };
        // Abrupt exits accumulated so far bypass all following statements.
        flow = flow.merge(statement(cg, *node, function, assigned, loop_target));
    }
    flow
}

fn statement(
    cg: &Codegen<'_>,
    node: NodeId,
    function: NodeId,
    input: bool,
    loop_target: Option<NodeId>,
) -> Flow {
    let branch = |body: Option<NodeId>| {
        body.map(|body| statement(cg, body, function, input, loop_target))
            .unwrap_or_else(|| Flow::falls(input))
    };
    match cg.kind(node) {
        NodeKind::Stmt(StmtKind::Begin) => {
            sequence(cg, &cg.node(node).children, function, input, loop_target)
        }
        NodeKind::Stmt(StmtKind::Assign {
            blocking: true,
            op: Operation::Assignment,
            delay: None,
        }) => Flow::falls(
            input
                || cg
                    .node(node)
                    .children
                    .first()
                    .is_some_and(|lhs| assigned_lhs(cg, *lhs, function)),
        ),
        NodeKind::Expr(ExprKind::Operation {
            op: Operation::Assignment,
            assignment: true,
            operands,
            ..
        }) => Flow::falls(
            input
                || operands
                    .first()
                    .is_some_and(|lhs| assigned_lhs(cg, *lhs, function)),
        ),
        NodeKind::Stmt(StmtKind::Return { value }) => Flow {
            returns: Some(value.is_some() || input),
            ..Flow::default()
        },
        NodeKind::Stmt(StmtKind::Break) => Flow::jump(loop_target.map(Jump::Break), input),
        NodeKind::Stmt(StmtKind::Continue) => Flow::jump(loop_target.map(Jump::Continue), input),
        NodeKind::Stmt(StmtKind::IfElse {
            if_true, if_false, ..
        }) => branch(Some(*if_true)).merge(branch(*if_false)),
        NodeKind::Stmt(StmtKind::Case { items, .. }) => {
            let mut flow = Flow::default();
            for item in items {
                flow = flow.merge(branch(item.body));
            }
            if !items.iter().any(|item| item.exprs.is_empty()) {
                flow = flow.merge(Flow::falls(input));
            }
            flow
        }
        NodeKind::Stmt(StmtKind::PatternCase { items, default, .. }) => {
            let mut flow = branch(*default);
            for item in items {
                flow = flow.merge(branch(Some(item.body)));
            }
            flow
        }
        NodeKind::Stmt(
            StmtKind::While { body, .. }
            | StmtKind::Repeat { body, .. }
            | StmtKind::For { body, .. }
            | StmtKind::Foreach { body, .. },
        ) => statement(cg, *body, function, input, Some(node)).exit_loop(node, Some(input), false),
        NodeKind::Stmt(StmtKind::DoWhile { body, .. }) => {
            statement(cg, *body, function, input, Some(node)).exit_loop(node, None, true)
        }
        NodeKind::Stmt(StmtKind::Forever { body }) => {
            statement(cg, *body, function, input, Some(node)).exit_loop(node, None, false)
        }
        // Effects and persistent reads have a separate conservative check.
        // These nodes do not transfer control out of this lexical statement.
        NodeKind::Expr(_)
        | NodeKind::FuncCall { .. }
        | NodeKind::SysCall { .. }
        | NodeKind::MethodCall { .. }
        | NodeKind::Var { .. }
        | NodeKind::Array { .. }
        | NodeKind::Stmt(
            StmtKind::Empty | StmtKind::VariableDecl { .. } | StmtKind::Assign { delay: None, .. },
        ) => Flow::falls(input),
        // In particular, an unmodelled named disable must not become ordinary
        // fallthrough: a later write cannot prove a value on a bypassed path.
        _ => Flow {
            unknown: true,
            ..Flow::default()
        },
    }
}

#[cfg(test)]
mod tests;
