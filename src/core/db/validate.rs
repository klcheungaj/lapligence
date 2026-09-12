use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use super::{Db, DriverDelay, ExprKind, NodeId, NodeKind, StmtKind};

/// A structural invariant violation in an owned [`Db`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbValidationError {
    path: String,
    detail: String,
}

impl DbValidationError {
    fn new(path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            detail: detail.into(),
        }
    }

    /// Location of the invalid value within the database.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Human-readable description of the violated invariant.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for DbValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid owned semantic database at {}: {}",
            self.path, self.detail
        )
    }
}

impl Error for DbValidationError {}

impl Db {
    /// Validate arena links and side-table references after semantic import.
    pub fn validate(&self) -> Result<(), DbValidationError> {
        let validator = Validator { db: self };
        validator.validate()
    }
}

struct Validator<'db> {
    db: &'db Db,
}

impl Validator<'_> {
    fn validate(&self) -> Result<(), DbValidationError> {
        let (semantic_kinds, semantic_details) = self.db.semantic_metadata_lengths();
        if semantic_kinds != 0 && semantic_kinds != self.db.nodes().len() {
            return self.fail(
                "semantic_kinds",
                "native semantic category table does not match node arena",
            );
        }
        if semantic_details != 0 && semantic_details != self.db.nodes().len() {
            return self.fail(
                "semantic_details",
                "native semantic detail table does not match node arena",
            );
        }
        for (index, node) in self.db.nodes().iter().enumerate() {
            let path = format!("nodes[{index}]");
            if let Some(parent) = node.parent {
                self.node(parent, &format!("{path}.parent"))?;
            }
            for (child_index, child) in node.children.iter().copied().enumerate() {
                // `children` includes both owned descendants and resolved
                // cross-references (for example port connections), so only
                // arena validity is universal here; reciprocity is not.
                self.node(child, &format!("{path}.children[{child_index}]"))?;
            }
            self.validate_kind_refs(&node.kind, &format!("{path}.kind"))?;
        }
        self.validate_acyclic_links()?;

        self.validate_roots(self.db.tops(), "tops", |kind| {
            matches!(kind, NodeKind::ModuleInst { is_top: true, .. })
        })?;
        self.validate_roots(self.db.flat_modules(), "flat_modules", |kind| {
            matches!(kind, NodeKind::ModuleInst { .. })
        })?;
        self.validate_roots(self.db.packages(), "packages", |kind| {
            matches!(kind, NodeKind::Package)
        })?;
        self.validate_roots(self.db.classes(), "classes", |kind| {
            matches!(kind, NodeKind::ClassDef)
        })?;

        for (array, metadata) in self.db.arrays() {
            let node = self.node(*array, &format!("arrays[{}]", array.0))?;
            if !matches!(node.kind, NodeKind::Array { .. }) {
                return self.fail(
                    format!("arrays[{}]", array.0),
                    "metadata key is not an array node",
                );
            }
            if let Some(init) = metadata.init {
                self.node(init, &format!("arrays[{}].init", array.0))?;
            }
        }

        for (event, metadata) in self.db.event_arrays() {
            let node = self.node(*event, &format!("event_arrays[{}]", event.0))?;
            if !matches!(node.kind, NodeKind::NamedEvent) {
                return self.fail(
                    format!("event_arrays[{}]", event.0),
                    "metadata key is not a named-event declaration",
                );
            }
            if metadata.init.is_some() {
                return self.fail(
                    format!("event_arrays[{}].init", event.0),
                    "named-event arrays cannot have declaration initializers",
                );
            }
        }

        for (net, delay) in self.db.net_delays() {
            let path = format!("net_delays[{}]", net.0);
            let node = self.node(*net, &path)?;
            if !matches!(node.kind, NodeKind::Net { .. }) {
                return self.fail(path, "metadata key is not a net node");
            }
            let mut refs = Vec::new();
            driver_delay_refs(Some(*delay), &mut refs);
            for (index, reference) in refs.into_iter().enumerate() {
                self.node(
                    reference,
                    &format!("net_delays[{}].expressions[{index}]", net.0),
                )?;
            }
        }

        for (variable, init) in self.db.var_initializers() {
            let node = self.node(*variable, &format!("vars_init[{}]", variable.0))?;
            if !matches!(node.kind, NodeKind::Var { .. }) {
                return self.fail(
                    format!("vars_init[{}]", variable.0),
                    "initializer key is not a variable node",
                );
            }
            self.node(*init, &format!("vars_init[{}]", variable.0))?;
        }

        for call in self.db.method_calls_with_clause_nodes() {
            let node = self.node(*call, &format!("method_calls_with_clause[{}]", call.0))?;
            if !matches!(node.kind, NodeKind::MethodCall { .. }) {
                return self.fail(
                    format!("method_calls_with_clause[{}]", call.0),
                    "with-clause metadata key is not a method call",
                );
            }
        }

        for (call, iterator) in self.db.method_call_iterator_nodes() {
            let node = self.node(*call, &format!("method_call_iterators[{}]", call.0))?;
            if !matches!(node.kind, NodeKind::MethodCall { .. }) {
                return self.fail(
                    format!("method_call_iterators[{}]", call.0),
                    "iterator metadata key is not a method call",
                );
            }
            if !self.db.method_call_has_with_clause(*call) {
                return self.fail(
                    format!("method_call_iterators[{}]", call.0),
                    "iterator metadata requires with-clause metadata",
                );
            }
            self.node(
                *iterator,
                &format!("method_call_iterators[{}].iterator", call.0),
            )?;
        }

        for variable in self.db.variable_lifetime_nodes().keys() {
            let node = self.node(*variable, &format!("variable_lifetimes[{}]", variable.0))?;
            if !matches!(
                node.kind,
                NodeKind::Var { .. } | NodeKind::Array { .. } | NodeKind::NamedEvent
            ) {
                return self.fail(
                    format!("variable_lifetimes[{}]", variable.0),
                    "lifetime metadata key is not variable storage",
                );
            }
        }

        let mut range_keys = HashSet::new();
        for (index, entry) in self.db.elaborated_type_ranges().iter().enumerate() {
            let node = self.node(
                entry.declaration,
                &format!("elaborated_type_ranges[{index}].declaration"),
            )?;
            if !matches!(
                node.kind,
                NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
            ) || node.name != entry.name
            {
                return self.fail(
                    format!("elaborated_type_ranges[{index}]"),
                    "range entry must identify its named storage declaration",
                );
            }
            if entry.instance.is_empty() || entry.name.is_empty() {
                return self.fail(
                    format!("elaborated_type_ranges[{index}]"),
                    "instance and object names must be non-empty",
                );
            }
            if !range_keys.insert(entry.declaration) {
                return self.fail(
                    format!("elaborated_type_ranges[{index}]"),
                    "duplicate declaration range entry",
                );
            }
        }
        Ok(())
    }

    fn validate_kind_refs(&self, kind: &NodeKind, path: &str) -> Result<(), DbValidationError> {
        let mut refs = Vec::new();
        kind.append_references(&mut refs);
        for (index, id) in refs.into_iter().enumerate() {
            self.node(id, &format!("{path}.refs[{index}]"))?;
        }
        match kind {
            NodeKind::Expr(ExprKind::Streaming { streams, .. }) if streams.is_empty() => {
                return self.fail(
                    format!("{path}.streams"),
                    "streaming expression has no operands",
                );
            }
            NodeKind::FuncTask {
                body: Some(body), ..
            } if !matches!(self.db.node_kind(*body), NodeKind::Stmt(_)) => {
                return self.fail(format!("{path}.body"), "subroutine body is not a statement");
            }
            NodeKind::Port {
                high,
                high_expr,
                high_present,
                high_open: true,
                ..
            } if !high_present || high.is_some() || high_expr.is_some() => {
                return self.fail(
                    format!("{path}.high_open"),
                    "an open port must be present and have no actual expression",
                );
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_acyclic_links(&self) -> Result<(), DbValidationError> {
        self.validate_link_graph("parent", |node| node.parent.as_slice())?;
        self.validate_link_graph("children", |node| &node.children)
    }

    fn validate_link_graph(
        &self,
        field: &str,
        links: fn(&super::Node) -> &[NodeId],
    ) -> Result<(), DbValidationError> {
        // Explicit DFS bounds stack usage and visits each link once, including
        // deep generated hierarchies and shared expression subtrees.
        let mut state = vec![0_u8; self.db.nodes().len()];
        let mut pending = Vec::new();
        for root in self.db.node_ids() {
            if state[root.index()] != 0 {
                continue;
            }
            state[root.index()] = 1;
            pending.push((root, 0));
            while let Some((id, next)) = pending.last_mut() {
                let neighbors = links(self.db.node(*id));
                let Some(target) = neighbors.get(*next).copied() else {
                    state[id.index()] = 2;
                    pending.pop();
                    continue;
                };
                *next += 1;
                match state[target.index()] {
                    0 => {
                        state[target.index()] = 1;
                        pending.push((target, 0));
                    }
                    1 => {
                        return self.fail(
                            format!("nodes[{}].{field}", id.index()),
                            format!("{field} links contain a cycle"),
                        );
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn validate_roots(
        &self,
        roots: &[NodeId],
        path: &str,
        expected_kind: impl Fn(&NodeKind) -> bool,
    ) -> Result<(), DbValidationError> {
        let mut seen = HashSet::new();
        for (index, root) in roots.iter().copied().enumerate() {
            if !seen.insert(root) {
                return self.fail(
                    format!("{path}[{index}]"),
                    format!("node {} is listed more than once", root.0),
                );
            }
            let node = self.node(root, &format!("{path}[{index}]"))?;
            if node.parent.is_some() {
                return self.fail(format!("{path}[{index}]"), "root node has a parent");
            }
            if !expected_kind(&node.kind) {
                return self.fail(format!("{path}[{index}]"), "root has the wrong node kind");
            }
        }
        Ok(())
    }

    fn node(&self, id: NodeId, path: &str) -> Result<&super::Node, DbValidationError> {
        self.db.nodes().get(id.index()).ok_or_else(|| {
            DbValidationError::new(path, format!("node index {} is out of bounds", id.0))
        })
    }

    fn fail<T>(
        &self,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<T, DbValidationError> {
        Err(DbValidationError::new(path, detail))
    }
}

impl NodeKind {
    /// Append embedded semantic references, independently of structural children.
    pub(crate) fn append_references(&self, refs: &mut Vec<NodeId>) {
        match self {
            NodeKind::Port {
                high,
                low,
                high_expr,
                ..
            } => refs.extend([*high, *low, *high_expr].into_iter().flatten()),
            NodeKind::IoDecl { expr, .. } => refs.extend(*expr),
            NodeKind::IfaceConn { actual, .. } => refs.push(*actual),
            NodeKind::ContAssign { delay, .. } => driver_delay_refs(*delay, refs),
            NodeKind::Gate { delay, terms, .. } => {
                driver_delay_refs(*delay, refs);
                refs.extend(terms.iter().map(|term| term.expr));
            }
            NodeKind::MethodCall { receiver, .. } => refs.extend(*receiver),
            NodeKind::FuncCall { callee, .. } => refs.extend(*callee),
            NodeKind::FuncTask { body, .. } => refs.extend(*body),
            NodeKind::FuncArg { default, .. } => refs.extend(*default),
            NodeKind::Stmt(stmt) => statement_refs(stmt, refs),
            NodeKind::Expr(expr) => expression_refs(expr, refs),
            NodeKind::ModuleInst { .. }
            | NodeKind::InstanceArray
            | NodeKind::Package
            | NodeKind::ClassDef
            | NodeKind::ModPort
            | NodeKind::Net { .. }
            | NodeKind::Var { .. }
            | NodeKind::Genvar { .. }
            | NodeKind::Array { .. }
            | NodeKind::NamedEvent
            | NodeKind::Param { .. }
            | NodeKind::ParamAssign { .. }
            | NodeKind::GenScopeArray
            | NodeKind::GenScope
            | NodeKind::Process { .. }
            | NodeKind::SysCall { .. }
            | NodeKind::EnumConst { .. }
            | NodeKind::Other => {}
        }
    }
}

fn driver_delay_refs(delay: Option<DriverDelay>, refs: &mut Vec<NodeId>) {
    match delay {
        Some(DriverDelay::Single(delay)) => refs.push(delay),
        Some(DriverDelay::RiseFall(rise, fall)) => refs.extend([rise, fall]),
        Some(DriverDelay::RiseFallTurnOff(rise, fall, turn_off)) => {
            refs.extend([rise, fall, turn_off]);
        }
        None => {}
    }
}

fn statement_refs(statement: &StmtKind, refs: &mut Vec<NodeId>) {
    match statement {
        StmtKind::IfElse { cond, .. } | StmtKind::Wait { cond } => refs.push(*cond),
        StmtKind::WaitOrder {
            events,
            if_true,
            if_false,
        } => {
            refs.extend(events.iter().copied());
            refs.extend(if_true.iter().chain(if_false.iter()).copied());
        }
        StmtKind::Assign { delay, .. } => {
            if let Some(delay) = delay {
                delay.referenced_nodes(refs);
            }
        }
        StmtKind::DelayControl { delay } => refs.push(*delay),
        StmtKind::VariableDecl { declaration } => refs.push(*declaration),
        StmtKind::Case { items, .. } => {
            for item in items {
                refs.extend(item.exprs.iter().copied());
                refs.extend(item.body);
            }
        }
        StmtKind::For {
            vars,
            init,
            cond,
            incr,
            body,
        } => {
            refs.extend(vars.iter().copied());
            refs.extend(init.iter().copied());
            refs.push(*cond);
            refs.extend(incr.iter().copied());
            refs.push(*body);
        }
        StmtKind::While { cond, body }
        | StmtKind::DoWhile { cond, body }
        | StmtKind::Repeat { cond, body } => refs.extend([*cond, *body]),
        StmtKind::Forever { body } => refs.push(*body),
        StmtKind::EventControl { specs, body, .. } => {
            for spec in specs {
                spec.referenced_nodes(refs);
            }
            refs.extend(*body);
        }
        StmtKind::EventTrigger { target, timing, .. } => {
            refs.extend(*target);
            if let Some(timing) = timing {
                timing.referenced_nodes(refs);
            }
        }
        StmtKind::Disable { target } | StmtKind::Return { value: target } => refs.extend(*target),
        StmtKind::Force { lhs, rhs } | StmtKind::ProcContAssign { lhs, rhs } => {
            refs.extend([*lhs, *rhs])
        }
        StmtKind::Release { lhs } | StmtKind::Deassign { lhs } => refs.push(*lhs),
        StmtKind::Fork { branches, .. } => refs.extend(branches.iter().copied()),
        StmtKind::Foreach { array, vars, body } => {
            refs.extend(*array);
            refs.extend(vars.iter().flatten().copied());
            refs.push(*body);
        }
        StmtKind::Begin
        | StmtKind::Empty
        | StmtKind::WaitFork
        | StmtKind::DisableFork
        | StmtKind::Break
        | StmtKind::Continue
        | StmtKind::Unsupported { .. } => {}
    }
}

fn expression_refs(expression: &ExprKind, refs: &mut Vec<NodeId>) {
    match expression {
        ExprKind::ScopeRef { target } => refs.push(*target),
        ExprKind::Operation { operands, .. } => refs.extend(operands.iter().copied()),
        ExprKind::TaggedPattern { value, .. } => refs.extend(*value),
        ExprKind::Cast { operand, .. } => refs.push(*operand),
        ExprKind::NewArray { size, initializer } => {
            refs.push(*size);
            refs.extend(*initializer);
        }
        ExprKind::Streaming { streams, .. } => {
            for stream in streams {
                refs.push(stream.value);
                refs.extend(stream.with_expr);
            }
        }
        ExprKind::Ref { target } => refs.extend(*target),
        ExprKind::BitSelect { base, index } => refs.extend([*base, *index]),
        ExprKind::PartSelect { base, left, right } => refs.extend([*base, *left, *right]),
        ExprKind::IndexedPartSelect {
            base,
            base_expr,
            width_expr,
            ..
        } => refs.extend([*base, *base_expr, *width_expr]),
        ExprKind::ArraySelect { base, indices } => {
            refs.push(*base);
            refs.extend(indices.iter().copied());
        }
        ExprKind::HierPath { refs: parts, .. } => refs.extend(parts.iter().flatten().copied()),
        ExprKind::Constant { .. } | ExprKind::DataType | ExprKind::Unbounded | ExprKind::Other => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn node(kind: NodeKind) -> super::super::Node {
        super::super::Node {
            kind,
            children: Vec::new(),
            parent: None,
            name: String::new(),
            full_name: String::new(),
            file: None,
            line: 0,
            col: 0,
            end_line: 0,
            end_col: 0,
        }
    }

    fn from_nodes(nodes: Vec<super::super::Node>) -> Result<Db, super::super::DbError> {
        Db::from_test_nodes("test", nodes, Vec::new(), HashMap::new())
    }

    fn empty_db() -> Db {
        Db::empty_for_validation_test()
    }

    #[test]
    fn rejects_an_out_of_bounds_root() {
        let mut db = empty_db();
        db.push_top_for_validation_test(NodeId(0));

        let error = db.validate().expect_err("invalid root must fail");
        assert_eq!(error.path(), "tops[0]");
        assert!(error.detail().contains("out of bounds"));
    }

    #[test]
    fn accepts_an_empty_structural_snapshot() {
        empty_db()
            .validate()
            .expect("empty graph is structurally valid");
    }

    #[test]
    fn rejects_invalid_embedded_references_without_child_edges() {
        let error = from_nodes(vec![node(NodeKind::Expr(ExprKind::Ref {
            target: Some(NodeId(1)),
        }))])
        .expect_err("embedded reference must be checked");
        assert!(error.to_string().contains("nodes[0].kind.refs[0]"));
    }

    #[test]
    fn rejects_out_of_bounds_scope_reference_targets() {
        let error = from_nodes(vec![node(NodeKind::Expr(ExprKind::ScopeRef {
            target: NodeId(1),
        }))])
        .expect_err("metadata targets must still be validated");
        assert!(error.to_string().contains("nodes[0].kind.refs[0]"));
        assert!(error.to_string().contains("out of bounds"));
    }

    #[test]
    fn rejects_missing_statement_and_assignment_delay_expressions() {
        for statement in [
            StmtKind::DelayControl { delay: NodeId(1) },
            StmtKind::Assign {
                blocking: false,
                op: super::super::Operation::Assignment,
                delay: Some(super::super::IntraControl::Delay(NodeId(1))),
            },
        ] {
            let error = from_nodes(vec![node(NodeKind::Stmt(statement))])
                .expect_err("typed delay references must be checked");
            assert!(error.to_string().contains("nodes[0].kind.refs[0]"));
        }
    }

    #[test]
    fn rejects_missing_declarations_bodies_and_expression_operands() {
        for kind in [
            NodeKind::FuncTask {
                is_task: false,
                automatic: true,
                ret: None,
                body: Some(NodeId(1)),
            },
            NodeKind::Stmt(StmtKind::VariableDecl {
                declaration: NodeId(1),
            }),
            NodeKind::Expr(ExprKind::NewArray {
                size: NodeId(1),
                initializer: None,
            }),
            NodeKind::Expr(ExprKind::NewArray {
                size: NodeId(0),
                initializer: Some(NodeId(1)),
            }),
            NodeKind::Expr(ExprKind::Streaming {
                direction: super::super::StreamingDirection::LeftToRight,
                slice_size: 0,
                streams: vec![super::super::StreamOperand {
                    value: NodeId(1),
                    with_expr: None,
                }],
            }),
            NodeKind::Expr(ExprKind::Streaming {
                direction: super::super::StreamingDirection::RightToLeft,
                slice_size: 8,
                streams: vec![super::super::StreamOperand {
                    value: NodeId(0),
                    with_expr: Some(NodeId(1)),
                }],
            }),
        ] {
            let error = from_nodes(vec![node(kind)])
                .expect_err("all typed operands must belong to the owned arena");
            assert!(error.to_string().contains("out of bounds"));
        }
    }

    #[test]
    fn checks_every_driver_delay_reference() {
        let valid = NodeId(0);
        let invalid = NodeId(2);
        for (position, delay) in [
            DriverDelay::Single(invalid),
            DriverDelay::RiseFall(valid, invalid),
            DriverDelay::RiseFallTurnOff(valid, valid, invalid),
        ]
        .into_iter()
        .enumerate()
        {
            for kind in [
                NodeKind::ContAssign {
                    net_decl: false,
                    delay: Some(delay),
                    strength0: super::super::Strength::Unspecified,
                    strength1: super::super::Strength::Unspecified,
                },
                NodeKind::Gate {
                    class: super::super::PrimClass::Gate,
                    prim_type: super::super::PrimitiveType::Buf,
                    strength0: super::super::Strength::Unspecified,
                    strength1: super::super::Strength::Unspecified,
                    delay: Some(delay),
                    terms: Vec::new(),
                },
            ] {
                let error = from_nodes(vec![node(NodeKind::Other), node(kind)])
                    .expect_err("every transition delay reference must be checked");
                assert!(error
                    .to_string()
                    .contains(&format!("nodes[1].kind.refs[{position}]")));
            }
        }
    }

    #[test]
    fn rejects_empty_streaming_expressions() {
        for direction in [
            super::super::StreamingDirection::LeftToRight,
            super::super::StreamingDirection::RightToLeft,
        ] {
            let expression = node(NodeKind::Expr(ExprKind::Streaming {
                direction,
                slice_size: 8,
                streams: vec![],
            }));
            let error = from_nodes(vec![expression]).expect_err("stream operand is required");
            assert!(error
                .to_string()
                .contains("streaming expression has no operands"));
        }
    }

    #[test]
    fn rejects_storage_as_a_subroutine_body() {
        let function = node(NodeKind::FuncTask {
            is_task: true,
            automatic: false,
            ret: None,
            body: Some(NodeId(1)),
        });
        let storage = node(NodeKind::Var {
            ty: crate::core::model::TypeInfo {
                kind: "logic".into(),
                width: Some(1),
                signed: false,
                type_name: None,
            },
        });
        let error = from_nodes(vec![function, storage]).expect_err("body must be a statement");
        assert!(error
            .to_string()
            .contains("subroutine body is not a statement"));
    }

    #[test]
    fn rejects_contradictory_open_port_metadata() {
        for (present, open, high, expression, valid) in [
            (false, true, None, None, false),
            (true, true, Some(NodeId(1)), None, false),
            (true, true, None, Some(NodeId(1)), false),
            (true, true, None, None, true),
            (false, false, None, None, true),
            (true, false, Some(NodeId(1)), Some(NodeId(1)), true),
        ] {
            let port = node(NodeKind::Port {
                direction: super::super::Direction::Input,
                ty: crate::core::model::TypeInfo {
                    kind: "logic".into(),
                    width: Some(1),
                    signed: false,
                    type_name: None,
                },
                strength0: super::super::Strength::Unspecified,
                strength1: super::super::Strength::Unspecified,
                high,
                low: None,
                high_expr: expression,
                high_present: present,
                high_open: open,
            });
            let result = from_nodes(vec![port, node(NodeKind::Expr(ExprKind::Other))]);
            assert_eq!(result.is_ok(), valid);
            if let Err(error) = result {
                assert!(error.to_string().contains("an open port must be present"));
            }
        }
    }

    #[test]
    fn rejects_parent_and_child_cycles() {
        let mut parent = node(NodeKind::Other);
        parent.parent = Some(NodeId(0));
        assert!(from_nodes(vec![parent])
            .expect_err("parent cycle")
            .to_string()
            .contains("parent links contain a cycle"));

        let mut first = node(NodeKind::Other);
        let mut second = node(NodeKind::Other);
        first.children.push(NodeId(1));
        second.children.push(NodeId(0));
        assert!(from_nodes(vec![first, second])
            .expect_err("child cycle")
            .to_string()
            .contains("children links contain a cycle"));
    }

    #[test]
    fn accepts_shared_children_and_recursive_semantic_references() {
        let mut first = node(NodeKind::Expr(ExprKind::Ref {
            target: Some(NodeId(1)),
        }));
        let mut second = node(NodeKind::Expr(ExprKind::Ref {
            target: Some(NodeId(0)),
        }));
        first.children.push(NodeId(2));
        second.children.push(NodeId(2));
        from_nodes(vec![first, second, node(NodeKind::Other)])
            .expect("shared children and semantic reference cycles are valid");
    }

    #[test]
    fn validates_deep_hierarchy_without_recursive_traversal() {
        let count = 16_384;
        let nodes = (0..count)
            .map(|index| {
                let mut item = node(NodeKind::Other);
                if index > 0 {
                    item.parent = Some(NodeId::from_index(index - 1));
                }
                if index + 1 < count {
                    item.children.push(NodeId::from_index(index + 1));
                }
                item
            })
            .collect();
        from_nodes(nodes).expect("deep hierarchy must validate");
    }
}
