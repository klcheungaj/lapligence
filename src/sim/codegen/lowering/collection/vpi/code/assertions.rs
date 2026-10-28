//! Assertion statement objects of the VPI catalog (IEEE 1800-2009 37.45,
//! 37.46, 37.48, 37.51): the clocking event, disable condition, property
//! and action statements of each assertion item.
//!
//! An item's entry is not a handle of its own (the assertion handle is the
//! coverage item); it holds the relations of 37.46 and 37.51 as children.
//! Property and sequence operators are not decomposed: a property
//! expression other than a Boolean expression is an opaque operation.

use super::*;
use crate::core::db::AssertionExprKind;
use crate::sim::codegen::lowering::assertions::ProcedureClock;

/// Where the clocking event of an assertion comes from (37.46 detail 1: the
/// actual clocking event, explicit or inferred).
pub(super) enum ClockSource<'a> {
    /// An event list written in the source: an assertion clocking event or
    /// a default clocking block.
    Written(&'a [EventSpec], NodeId),
    /// The edge inferred from the enclosing procedure (16.15.6).
    Procedure(EventSpec, NodeId),
}

impl<'c, 'a> CodeTables<'c, 'a> {
    /// The top clocking event of property `node`: the one that clocks the
    /// whole property, reached through `disable iff`, asynchronous aborts
    /// and the body of a sequence or property instance. `None` when the
    /// clock is inherited from the context.
    fn top_clocking(&self, mut node: NodeId) -> Option<(&'a [EventSpec], NodeId)> {
        let cg = self.cg;
        loop {
            match cg.kind(node) {
                NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                    control,
                    signal,
                    specs,
                    ..
                }) => {
                    // `$inferred_clock` stands for the context clock.
                    if matches!(cg.kind(*signal), NodeKind::SysCall { .. }) {
                        return None;
                    }
                    return Some((specs.as_slice(), *control));
                }
                NodeKind::AssertionExpr(
                    AssertionExprKind::DisableIff { expr, .. }
                    | AssertionExprKind::Abort {
                        expr, sync: false, ..
                    },
                ) => node = *expr,
                _ => node = self.instance_body(node)?,
            }
        }
    }

    /// The `disable iff` written in the statement whose property is `node`
    /// (under its clocking event). A condition inside a named property or
    /// a `default disable iff` is not the statement's (decision A17-D4).
    fn statement_disable(&self, mut node: NodeId) -> Option<NodeId> {
        loop {
            match self.cg.kind(node) {
                NodeKind::AssertionExpr(AssertionExprKind::DisableIff { condition, .. }) => {
                    return Some(*condition)
                }
                NodeKind::AssertionExpr(AssertionExprKind::Clocking { expr, .. }) => node = *expr,
                _ => return None,
            }
        }
    }

    /// The instance a property expression consists of, if it is exactly
    /// one sequence or property instance.
    pub(super) fn sole_instance(&self, node: NodeId) -> Option<NodeId> {
        let cg = self.cg;
        let instance = match cg.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated: false,
                repetition: None,
            }) => *expr,
            _ => node,
        };
        matches!(
            cg.kind(instance),
            NodeKind::Expr(ExprKind::AssertionInstance { .. })
        )
        .then_some(instance)
    }

    /// The declaration an instance names and its kind (`"Sequence"` or
    /// `"Property"`, as the frontend details it; `""` otherwise).
    pub(super) fn instance_declaration(&self, instance: NodeId) -> (NodeId, &'a str) {
        let cg = self.cg;
        let target = match cg.kind(instance) {
            NodeKind::Expr(ExprKind::AssertionInstance { target, .. }) => *target,
            _ => instance,
        };
        (target, cg.db.semantic_detail(target).unwrap_or(""))
    }

    fn instance_body(&self, node: NodeId) -> Option<NodeId> {
        match self.cg.kind(self.sole_instance(node)?) {
            NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => Some(*body),
            _ => None,
        }
    }

    /// The actual clocking event of the concurrent assertion `statement`
    /// with property `property`, and whether it is inferred: written at the
    /// top of the property, else the procedure's edge, else the default
    /// clocking (16.15.6, 16.17).
    pub(super) fn assertion_clock(
        &self,
        statement: NodeId,
        property: NodeId,
    ) -> Option<(ClockSource<'a>, bool)> {
        let cg = self.cg;
        if let Some((specs, at)) = self.top_clocking(property) {
            return Some((ClockSource::Written(specs, at), false));
        }
        let mut current = cg.node(statement).parent;
        while let Some(node) = current {
            if matches!(cg.kind(node), NodeKind::Process { .. }) {
                let body = cg.node(node).children.first().copied();
                if let Some(ProcedureClock::Edge(spec)) =
                    body.and_then(|body| cg.infer_procedure_clock(body))
                {
                    return Some((ClockSource::Procedure(spec, statement), true));
                }
                break;
            }
            current = cg.node(node).parent;
        }
        let instance = cg.owning_inst(statement)?;
        let block = cg.default_clocking_block(instance)?;
        let info = cg.db.clocking_block(block)?;
        Some((ClockSource::Written(&info.event_specs, info.event), true))
    }

    /// The entry of assertion statement `id`: its type and the children of
    /// 37.46 (concurrent) or 37.51 (immediate).
    pub(super) fn assertion(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        // An empty action `;` is no action statement (A17-D4).
        let stmt = |this: &Self, role, id: Option<NodeId>| -> Option<(&'static str, Item<'a>)> {
            let id = id.filter(|id| !matches!(cg.kind(*id), NodeKind::Stmt(StmtKind::Empty)))?;
            Some((role, this.statement_item(id)?))
        };
        match cg.kind(id) {
            NodeKind::Stmt(StmtKind::ImmediateAssertion {
                kind,
                cond,
                if_true,
                if_false,
                ..
            }) => {
                let type_code = match kind {
                    crate::core::db::ImmediateAssertionKind::Assert => "vpiImmediateAssert",
                    crate::core::db::ImmediateAssertionKind::Assume => "vpiImmediateAssume",
                    crate::core::db::ImmediateAssertionKind::Cover => "vpiImmediateCover",
                };
                let code = self.located(type_code, id);
                let mut children = vec![("vpiExpr", Item::Expr(*cond))];
                children.extend(stmt(self, "vpiStmt", *if_true));
                children.extend(stmt(self, "vpiElseStmt", *if_false));
                (code, children)
            }
            NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                kind,
                property,
                if_true,
                if_false,
                ..
            }) => {
                use crate::core::db::ConcurrentAssertionKind as Kind;
                let type_code = match kind {
                    Kind::Assume => "vpiAssume",
                    Kind::Cover | Kind::CoverSequence => "vpiCover",
                    _ => "vpiAssert",
                };
                let mut code = self.located(type_code, id);
                let mut children = Vec::new();
                if let Some((clock, inferred)) = self.assertion_clock(id, *property) {
                    if inferred {
                        code.flags |= IrVpiCode::INFERRED;
                    }
                    children.push(("vpiClockingEvent", Item::Clock(clock)));
                }
                if let Some(condition) = self.statement_disable(*property) {
                    children.push(("vpiDisableCondition", Item::Expr(condition)));
                }
                children.push(("vpiProperty", Item::Property(*property, id)));
                children.extend(stmt(self, "vpiStmt", *if_true));
                children.extend(stmt(self, "vpiElseStmt", *if_false));
                (code, children)
            }
            _ => self.opaque("vpiAssert", id),
        }
    }

    /// The `vpiProperty` of a concurrent assertion: a property instance
    /// when the statement's property is exactly one, else a property
    /// specification (37.46) with the clocking event and disable condition
    /// written in the statement and the property expression (37.48).
    pub(super) fn property(
        &mut self,
        property: NodeId,
        statement: NodeId,
    ) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        if let Some(instance) = self
            .sole_instance(property)
            .filter(|instance| self.instance_declaration(*instance).1 == "Property")
        {
            return self.assertion_expression(instance);
        }
        let code = self.located("vpiPropertySpec", statement);
        let mut children: Children<'a> = Vec::new();
        let mut inner = property;
        loop {
            match cg.kind(inner) {
                NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                    control,
                    signal,
                    specs,
                    expr,
                    ..
                }) if !children.iter().any(|(role, _)| *role == "vpiClockingEvent") => {
                    if !matches!(cg.kind(*signal), NodeKind::SysCall { .. }) {
                        children.push((
                            "vpiClockingEvent",
                            Item::Clock(ClockSource::Written(specs, *control)),
                        ));
                    }
                    inner = *expr;
                }
                NodeKind::AssertionExpr(AssertionExprKind::DisableIff { condition, expr })
                    if !children
                        .iter()
                        .any(|(role, _)| *role == "vpiDisableCondition") =>
                {
                    children.push(("vpiDisableCondition", Item::Expr(*condition)));
                    inner = *expr;
                }
                _ => break,
            }
        }
        children.push(("vpiPropertyExpr", Item::AssertionExpr(inner)));
        (code, children)
    }

    /// A sequence or property expression (37.48, 37.50): a Boolean
    /// expression is an expression object, a named sequence or property
    /// an instance object; operators are not decomposed.
    pub(super) fn assertion_expression(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        if let Some(instance) = self.sole_instance(id) {
            return self.instance(instance);
        }
        match cg.kind(id) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated: false,
                repetition: None,
            }) => self.assertion_expression(*expr),
            NodeKind::AssertionExpr(_) => {
                let mut code = IrVpiCode::new("vpiOperation");
                code.flags |= IrVpiCode::OPAQUE;
                (code, Vec::new())
            }
            _ => self.expression(id),
        }
    }

    /// The event expression of a clocking event.
    pub(super) fn clock(
        &mut self,
        clock: ClockSource<'a>,
        scope: Option<usize>,
    ) -> (IrVpiCode, Children<'a>, Option<usize>) {
        match clock {
            ClockSource::Written(specs, at) => self.build(Item::Events(specs, at), scope),
            ClockSource::Procedure(spec, at) => self.build(Item::OwnedEvent(spec, at), scope),
        }
    }

    /// A sequence or property instance (37.50, 37.52): the name of its
    /// declaration, its actual arguments in formal order (defaults
    /// included) and a link to the declaration.
    fn instance(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let NodeKind::Expr(ExprKind::AssertionInstance { bindings, .. }) = cg.kind(id) else {
            return self.opaque("vpiOperation", id);
        };
        let (declaration, kind) = self.instance_declaration(id);
        let (type_code, relation) = match kind {
            "Sequence" => ("vpiSequenceInst", "vpiSequenceDecl"),
            "Property" => ("vpiPropertyInst", "vpiPropertyDecl"),
            _ => return self.opaque("vpiOperation", id),
        };
        let mut code = self.located(type_code, id);
        code.text = Some(cg.node(declaration).name.clone());
        let formals = &cg.node(declaration).children;
        let mut actuals: Vec<_> = bindings.iter().collect();
        actuals.sort_by_key(|binding| formals.iter().position(|formal| *formal == binding.formal));
        let mut children: Children<'a> = actuals
            .into_iter()
            .map(|binding| ("vpiArgument", Item::AssertionExpr(binding.actual)))
            .collect();
        if let Some(entry) = self.declarations.get(&declaration) {
            children.push((relation, Item::Link(*entry)));
        }
        (code, children)
    }

    /// Add the sequence or property declaration `id` declared in catalog
    /// scope `scope` as a root (37.47, 37.49), once.
    pub(in super::super) fn declaration_root(&mut self, id: NodeId, scope: Option<usize>) {
        if !self.declarations.contains_key(&id) {
            let entry = self.root(Item::Declaration(id), scope);
            self.declarations.insert(id, entry);
        }
    }

    /// A declaration: its name and its formals in declaration order.
    pub(super) fn declaration(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let (type_code, formal) = match cg.db.semantic_detail(id) {
            Some("Sequence") => ("vpiSequenceDecl", "vpiSeqFormalDecl"),
            _ => ("vpiPropertyDecl", "vpiPropFormalDecl"),
        };
        let mut code = self.located(type_code, id);
        code.text = Some(cg.node(id).name.clone());
        let children = cg
            .node(id)
            .children
            .iter()
            .filter(|child| cg.db.semantic_detail(**child) == Some("AssertionPort"))
            .map(|child| (formal, Item::AssertionFormal(*child, formal)))
            .collect();
        (code, children)
    }

    /// A formal argument (37.47 details 2 and 4, 37.49 details 2 and 4): its
    /// name, its direction (`vpiNoDirection` unless it is a local variable
    /// argument) and the typespec of a typed formal.
    pub(super) fn formal(&mut self, id: NodeId, type_code: &'static str) -> IrVpiCode {
        let cg = self.cg;
        let mut code = self.located(type_code, id);
        code.text = Some(cg.node(id).name.clone());
        let NodeKind::FuncArg { direction, ty, .. } = cg.kind(id) else {
            return code;
        };
        code.subtype = match direction {
            Direction::Input => "vpiInput",
            Direction::Output => "vpiOutput",
            Direction::Inout => "vpiInout",
            _ => "vpiNoDirection",
        };
        if ty.type_name.as_deref() != Some("untyped") {
            code.typespec = cg
                .db
                .type_descriptor(id)
                .and_then(|descriptor| cg.vpi_typespec(self.typespecs, descriptor, None, 0));
        }
        code
    }
}
