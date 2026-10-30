//! Statement and toggle coverage (IEEE 1800-2009 40.2.2, 40.3.2).
//!
//! Instrumenting every statement and watching every signal costs run time,
//! so a model gets a kind only when something can ask for it (decision
//! S46-D13): a `$coverage_*` call whose type argument may name the kind
//! (a constant `SV_COV_STATEMENT`/`SV_COV_TOGGLE`, or a type that is not a
//! constant), or the VPI catalog of `--vpi`/`--dpi-lib`, whose applications
//! may ask through `vpi_control`. A `$coverage_*` request also starts the
//! kind at time 0 (40.3.2.1: coverage starts automatically for the
//! hierarchy enabled for it); the catalog alone makes it available but
//! stopped until a start request. Every other model is generated exactly
//! as before.
//!
//! A statement entity is a source statement node: lowering puts one
//! [`IrCoverageStmt::StatementHit`] in front of it, which the emitter turns
//! into one increment of the entity's counter. A toggle entity is a packed
//! net or variable of an instance (decision S46-D14); the runtime watches
//! its storage.
use super::*;
use crate::sim::ir::{IrStatementItem, IrToggleSignal};

/// The 40.3.1 coverage type values.
const SV_COV_STATEMENT: u64 = 22;
const SV_COV_TOGGLE: u64 = 23;

/// Statement and toggle coverage requested by the design and the catalog.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::sim::codegen::lowering) struct CodeCoveragePlan {
    pub(in crate::sim::codegen::lowering) statement: bool,
    pub(in crate::sim::codegen::lowering) toggle: bool,
    /// A `$coverage_*` call names the kind: it collects from time 0.
    pub(in crate::sim::codegen::lowering) statement_started: bool,
    pub(in crate::sim::codegen::lowering) toggle_started: bool,
}

impl Codegen<'_> {
    /// Decide which code coverage kinds the model carries before any body
    /// is lowered: `vpi` is the catalog option.
    pub(in crate::sim::codegen::lowering) fn plan_code_coverage(&mut self, vpi: bool) {
        let mut plan = CodeCoveragePlan::default();
        let roots = self
            .design_nodes()
            .into_iter()
            .chain(self.db.packages().iter().copied())
            .collect::<Vec<_>>();
        let mut stack = roots;
        let mut seen = HashSet::new();
        while let Some(node) = stack.pop() {
            if !seen.insert(node) {
                continue;
            }
            stack.extend(self.node(node).children.iter().copied());
            let NodeKind::SysCall { name } = self.kind(node) else {
                continue;
            };
            let type_argument = match name.as_str() {
                "$coverage_control" => 1,
                "$coverage_get_max" | "$coverage_get" | "$coverage_merge" | "$coverage_save" => 0,
                _ => continue,
            };
            match self
                .node(node)
                .children
                .get(type_argument)
                .and_then(|argument| self.coverage_type_constant(*argument))
            {
                Some(SV_COV_STATEMENT) => plan.statement_started = true,
                Some(SV_COV_TOGGLE) => plan.toggle_started = true,
                Some(_) => {}
                None => {
                    plan.statement_started = true;
                    plan.toggle_started = true;
                }
            }
        }
        plan.statement = plan.statement_started || vpi;
        plan.toggle = plan.toggle_started || vpi;
        // The tables that serve the requests are kept even when no call
        // site is lowered.
        if plan.statement_started || plan.toggle_started {
            self.coverage_api_used = true;
        }
        self.code_coverage = plan;
    }

    /// The value of a constant coverage type argument (a literal or a
    /// parameter), or `None` when it is not a known constant.
    fn coverage_type_constant(&self, argument: NodeId) -> Option<u64> {
        let argument = self.p30_unwrap_cast(argument);
        let value = match self.kind(argument) {
            NodeKind::Expr(ExprKind::Constant { value, size, .. }) => {
                val_from_value_data(value, *size).ok()?
            }
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.param_vals.get(target)?.clone(),
            _ => return None,
        };
        match value {
            Val::Bits(bits) => bits.to_u64(),
            _ => None,
        }
    }

    /// Whether statement `h` is a statement coverage entity (A.6.4
    /// `statement_item`): every statement, a written `begin` or `fork`
    /// block included; not a null statement, a declaration, nor the
    /// blocks the frontend adds (a statement list, an expression
    /// statement's wrapper, the scope of a `for` or `foreach` loop's
    /// variables). Expression statements (calls, increments) count. The
    /// `else if` of a conditional statement is part of it (A.6.6
    /// `conditional_statement`), not a statement of its own.
    fn statement_coverable(&self, h: NodeId) -> bool {
        match self.kind(h) {
            NodeKind::Stmt(StmtKind::IfElse { .. }) => !self.else_if_continuation(h),
            NodeKind::Stmt(StmtKind::Begin) => {
                self.db.semantic_detail(h) == Some("Block")
                    && self.vpi_for_declarations(h).is_none()
                    && self.vpi_foreach_block(h).is_none()
            }
            NodeKind::Stmt(StmtKind::Fork { .. }) => true,
            NodeKind::Stmt(
                StmtKind::Empty | StmtKind::VariableDecl { .. } | StmtKind::Unsupported { .. },
            ) => false,
            NodeKind::Stmt(_)
            | NodeKind::SysCall { .. }
            | NodeKind::FuncCall { .. }
            | NodeKind::MethodCall { .. }
            | NodeKind::Expr(_) => true,
            _ => false,
        }
    }

    /// Whether `h` is the unqualified `if` written as the `else if` of
    /// another `if` (A.6.6). Qualified branch chains are emitted only when
    /// their `else if` links stay single `if` statements, so these must
    /// carry no hit of their own; an `else unique if` or `else priority if`
    /// starts a conditional statement of its own.
    fn else_if_continuation(&self, h: NodeId) -> bool {
        let NodeKind::Stmt(StmtKind::IfElse { check, .. }) = self.kind(h) else {
            return false;
        };
        *check == crate::core::db::UniquePriorityCheck::None
            && self.node(h).parent().is_some_and(|parent| {
                matches!(
                    self.kind(parent),
                    NodeKind::Stmt(StmtKind::IfElse { if_false: Some(branch), .. }) if *branch == h
                )
            })
    }

    /// The hit that counts statement `h`, or `None` when `h` is no entity
    /// or the model has no statement coverage.
    pub(in crate::sim::codegen::lowering) fn statement_hit(&mut self, h: NodeId) -> Option<IrStmt> {
        if !self.code_coverage.statement
            || self.statement_hits_suppressed != 0
            || !self.statement_coverable(h)
        {
            return None;
        }
        let next = u32::try_from(self.statement_entities.len()).ok()?;
        let entity = *self.statement_entities.entry(h).or_insert(next);
        if entity == next {
            self.statement_entity_order.push(h);
        }
        Some(IrStmt::Coverage(Box::new(IrCoverageStmt::StatementHit {
            entity,
        })))
    }

    /// The statement entities, in entity order, with their instances and
    /// source positions.
    pub(in crate::sim::codegen::lowering) fn statement_coverage_items(
        &mut self,
    ) -> Vec<IrStatementItem> {
        let order = std::mem::take(&mut self.statement_entity_order);
        let mut items = Vec::with_capacity(order.len());
        for node in order {
            let instance = self
                .owning_inst(node)
                .and_then(|owner| self.coverage_instances().index.get(&owner).copied());
            let data = self.node(node);
            items.push(IrStatementItem {
                identity: node.index() as u64,
                instance,
                file: data.file().unwrap_or_default().to_owned(),
                line: data.line,
                column: data.col,
                code: None,
            });
        }
        items
    }

    /// The toggle coverage signals: every packed net and variable declared
    /// in an instance or one of its generate scopes, in declaration order
    /// (decision S46-D14).
    pub(in crate::sim::codegen::lowering) fn toggle_coverage_signals(
        &mut self,
        model: &IrModel,
    ) -> Vec<IrToggleSignal> {
        if !self.code_coverage.toggle {
            return Vec::new();
        }
        let mut candidates = self
            .sig_globals
            .iter()
            .filter(|(_, info)| !info.real && info.width > 0)
            .map(|(node, info)| (*node, info.ir, info.width))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(node, _, _)| node.index());
        let mut toggles = Vec::new();
        for (node, signal, width) in candidates {
            if !matches!(self.kind(node), NodeKind::Var { .. } | NodeKind::Net { .. }) {
                continue;
            }
            // Declared in an instance or a generate scope of one (a loop
            // generate's scopes sit in a scope array); not in a subroutine,
            // block, class or package.
            let mut owner = self.node(node).parent;
            while let Some(parent) = owner {
                if !matches!(
                    self.kind(parent),
                    NodeKind::GenScope | NodeKind::GenScopeArray
                ) {
                    break;
                }
                owner = self.node(parent).parent;
            }
            let Some(owner) =
                owner.filter(|owner| matches!(self.kind(*owner), NodeKind::ModuleInst { .. }))
            else {
                continue;
            };
            let Some(ir) = model.signals.get(signal) else {
                continue;
            };
            if !ir.net_alias.is_empty()
                || !matches!(ir.ty, IrType::Packed { width: w, .. } if w == width)
            {
                continue;
            }
            let Some(instance) = self.coverage_instances().index.get(&owner).copied() else {
                continue;
            };
            toggles.push(IrToggleSignal {
                signal,
                full_name: self.vpi_full_name(node),
                instance,
                width,
                object: None,
            });
        }
        toggles
    }

    /// Put the statement and toggle entities into the coverage tables.
    pub(in crate::sim::codegen::lowering) fn populate_code_coverage(
        &mut self,
        model: &mut IrModel,
    ) {
        if model.assertion_coverage.is_none() {
            return;
        }
        let statements = self.statement_coverage_items();
        let toggles = self.toggle_coverage_signals(model);
        let plan = self.code_coverage;
        if let Some(coverage) = model.assertion_coverage.as_mut() {
            coverage.statements = statements;
            coverage.toggles = toggles;
            coverage.statement_started = plan.statement_started;
            coverage.toggle_started = plan.toggle_started;
        }
    }

    /// Link each toggle signal to the catalog row of its net or variable
    /// (40.5.3 vpiCovered of a signal handle).
    pub(in crate::sim::codegen::lowering) fn link_toggle_objects(&self, model: &mut IrModel) {
        let Some(coverage) = model.assertion_coverage.as_mut() else {
            return;
        };
        if coverage.toggles.is_empty() {
            return;
        }
        let rows = model
            .vpi_objects
            .iter()
            .enumerate()
            .filter(|(_, object)| object.signal.is_some() && !object.member)
            .map(|(row, object)| (object.full_name.as_str(), row))
            .collect::<HashMap<_, _>>();
        for toggle in &mut coverage.toggles {
            toggle.object = rows.get(toggle.full_name.as_str()).copied();
        }
    }
}
