//! Exercise the owned statement traversal without the native frontend.
use super::*;
use crate::core::db::{ConditionalPredicate, Node, PredicateClause, UniquePriorityCheck};

struct Ast {
    nodes: Vec<Node>,
}

impl Ast {
    fn new() -> Self {
        let mut ast = Self { nodes: Vec::new() };
        ast.add(
            NodeKind::FuncTask {
                is_task: false,
                automatic: false,
                is_static: false,
                is_virtual: false,
                is_pure: false,
                is_final: false,
                is_constructor: false,
                ret: Some(crate::core::model::TypeInfo::default()),
                body: None,
            },
            vec![],
        );
        ast
    }

    fn add(&mut self, kind: NodeKind, children: Vec<NodeId>) -> NodeId {
        let id = NodeId::from_index(self.nodes.len());
        self.nodes.push(Node {
            kind,
            children,
            parent: None,
            name: String::new(),
            full_name: String::new(),
            file: None,
            line: 0,
            col: 0,
            end_line: 0,
            end_col: 0,
        });
        id
    }

    fn literal(&mut self) -> NodeId {
        self.add(
            NodeKind::Expr(ExprKind::Constant {
                const_type: crate::core::db::ConstantType::Binary,
                value: ValueData::Bin("0".to_owned()),
                size: 1,
                source: crate::core::db::ConstantSource::NotCaptured,
                time_scale: None,
            }),
            vec![],
        )
    }

    fn assign(&mut self) -> NodeId {
        let lhs = self.add(
            NodeKind::Expr(ExprKind::Ref {
                target: Some(NodeId(0)),
            }),
            vec![],
        );
        let rhs = self.literal();
        self.add(
            NodeKind::Stmt(StmtKind::Assign {
                blocking: true,
                op: Operation::Assignment,
                delay: None,
            }),
            vec![lhs, rhs],
        )
    }

    fn block(&mut self, children: Vec<NodeId>) -> NodeId {
        self.add(NodeKind::Stmt(StmtKind::Begin), children)
    }

    fn conditional(&mut self, yes: NodeId) -> NodeId {
        let cond = self.literal();
        self.add(
            NodeKind::Stmt(StmtKind::IfElse {
                predicate: ConditionalPredicate {
                    clauses: vec![PredicateClause {
                        expression: cond,
                        pattern: None,
                    }],
                },
                if_true: yes,
                if_false: None,
                check: UniquePriorityCheck::None,
            }),
            vec![cond, yes],
        )
    }

    fn do_loop(&mut self, body: NodeId) -> NodeId {
        let cond = self.literal();
        self.add(
            NodeKind::Stmt(StmtKind::DoWhile { cond, body }),
            vec![body, cond],
        )
    }

    fn proof(mut self, body: NodeId) -> bool {
        let NodeKind::FuncTask { body: target, .. } = &mut self.nodes[0].kind else {
            unreachable!();
        };
        *target = Some(body);
        self.nodes[0].children = vec![body];
        let db = Db::from_test_nodes("return_flow", self.nodes, vec![], HashMap::new()).unwrap();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
        Codegen::new(&semantic).static_return_is_callback_independent(body, NodeId(0))
    }
}

#[test]
fn do_loop_jumps_do_not_reach_later_writes() {
    for is_break in [false, true] {
        for assignment_position in [0, 1, 2] {
            let mut ast = Ast::new();
            let jump = ast.add(
                NodeKind::Stmt(if is_break { StmtKind::Break } else { StmtKind::Continue }),
                vec![],
            );
            let branch = ast.conditional(jump);
            let write = ast.assign();
            let body = ast.block(vec![branch, write]);
            let loop_node = ast.do_loop(body);
            let outer_write = ast.assign();
            let root = ast.block(match assignment_position {
                0 => vec![loop_node],
                1 => vec![outer_write, loop_node],
                _ => vec![loop_node, outer_write],
            });
            assert_eq!(ast.proof(root), assignment_position != 0);
        }
    }
}

#[test]
fn nested_loop_jumps_are_consumed_by_their_lexical_loop() {
    for is_break in [false, true] {
        let mut ast = Ast::new();
        let jump = ast.add(
            NodeKind::Stmt(if is_break { StmtKind::Break } else { StmtKind::Continue }),
            vec![],
        );
        let inner = ast.do_loop(jump);
        let write = ast.assign();
        let outer_body = ast.block(vec![inner, write]);
        let outer = ast.do_loop(outer_body);
        assert!(ast.proof(outer));
    }
}

#[test]
fn an_outer_continue_bypasses_an_inner_loop_write() {
    let mut ast = Ast::new();
    let jump = ast.add(NodeKind::Stmt(StmtKind::Continue), vec![]);
    let branch = ast.conditional(jump);
    let write = ast.assign();
    let inner = ast.do_loop(write);
    let body = ast.block(vec![branch, inner]);
    let outer = ast.do_loop(body);
    assert!(!ast.proof(outer));
}

#[test]
fn zero_trip_loop_cannot_establish_a_result() {
    let mut ast = Ast::new();
    let write = ast.assign();
    let cond = ast.literal();
    let root = ast.add(NodeKind::Stmt(StmtKind::While { cond, body: write }), vec![cond, write]);
    assert!(!ast.proof(root));
}

#[test]
fn explicit_return_does_not_require_a_fallthrough_write() {
    let mut ast = Ast::new();
    let value = ast.literal();
    let ret = ast.add(NodeKind::Stmt(StmtKind::Return { value: Some(value) }), vec![value]);
    let root = ast.do_loop(ret);
    assert!(ast.proof(root));
}

#[test]
fn unassigned_return_cannot_be_repaired_by_a_later_write() {
    let mut ast = Ast::new();
    let ret = ast.add(NodeKind::Stmt(StmtKind::Return { value: None }), vec![]);
    let branch = ast.conditional(ret);
    let write = ast.assign();
    let root = ast.block(vec![branch, write]);
    assert!(!ast.proof(root));
}

#[test]
fn unmodelled_control_transfer_fails_closed() {
    let mut ast = Ast::new();
    let disable = ast.add(NodeKind::Stmt(StmtKind::Disable { target: Some(NodeId(0)) }), vec![]);
    let write = ast.assign();
    let root = ast.block(vec![disable, write]);
    assert!(!ast.proof(root));
}

#[test]
fn partial_result_assignment_is_not_a_whole_result_definition() {
    let mut ast = Ast::new();
    let base = ast.add(
        NodeKind::Expr(ExprKind::Ref { target: Some(NodeId(0)) }),
        vec![],
    );
    let index = ast.literal();
    let lhs = ast.add(NodeKind::Expr(ExprKind::BitSelect { base, index }), vec![base, index]);
    let rhs = ast.literal();
    let root = ast.add(
        NodeKind::Stmt(StmtKind::Assign {
            blocking: true,
            op: Operation::Assignment,
            delay: None,
        }),
        vec![lhs, rhs],
    );
    assert!(!ast.proof(root));
}

#[test]
fn persistent_result_reads_remain_conservatively_rejected() {
    let mut ast = Ast::new();
    let result = ast.add(
        NodeKind::Expr(ExprKind::Ref { target: Some(NodeId(0)) }),
        vec![],
    );
    let ret = ast.add(NodeKind::Stmt(StmtKind::Return { value: Some(result) }), vec![result]);
    let write = ast.assign();
    let root = ast.block(vec![write, ret]);
    assert!(!ast.proof(root));
}
