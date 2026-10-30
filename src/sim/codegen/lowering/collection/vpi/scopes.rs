//! Statement blocks as VPI scopes (IEEE 1800-2009 37.12, decision A19-D27).
//!
//! The owned database keeps each block's declarations in a declaration
//! scope node (a `StatementBlock`) that the block statement lists as a
//! child it does not own. Named blocks always have one; an unnamed begin or
//! fork has one only when it declares an item, a `for` loop only when it
//! declares its loop variables (an implicit block holding the loop) and a
//! `foreach` loop always (the implicit block of its loop variables). Each
//! such declaration scope is a VPI scope: 37.12 details 1-3.

use super::*;

/// The statement blocks that are VPI scopes, built once per lowering.
#[derive(Debug, Default)]
pub(in crate::sim::codegen) struct VpiScopeIndex {
    /// The declaration scope of each block statement that has one.
    block_scope: HashMap<NodeId, NodeId>,
    /// The block statement of each declaration scope.
    scope_block: HashMap<NodeId, NodeId>,
    /// `unnamed$$_<n>` of each unnamed declaration scope of an instance,
    /// generate scope, task or function: `n` counts the unnamed scopes whose
    /// nearest named scope is the same, from 0 in source pre-order.
    names: HashMap<NodeId, String>,
}

/// The `vpiType` of an unnamed block that is a scope: the statement that
/// declares it (37.12 details 1-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScopeStatement {
    Begin,
    Fork,
    /// The loop of an implicit block that declares its loop variables.
    For(NodeId),
    /// The loop of the implicit block of its loop variables.
    Foreach(NodeId),
}

impl ScopeStatement {
    pub(super) const fn type_code(self) -> &'static str {
        match self {
            Self::Begin => "vpiBegin",
            Self::Fork => "vpiFork",
            Self::For(_) => "vpiFor",
            Self::Foreach(_) => "vpiForeachStmt",
        }
    }
}

impl Codegen<'_> {
    pub(super) fn vpi_scopes(&self) -> &VpiScopeIndex {
        self.vpi_scopes.get_or_init(|| self.build_vpi_scopes())
    }

    fn is_declaration_scope(&self, id: NodeId) -> bool {
        matches!(self.kind(id), NodeKind::Stmt(StmtKind::Begin))
            && self.db.semantic_detail(id) == Some("StatementBlock")
    }

    /// Whether `id` keeps its own count of unnamed scopes: an instance, a
    /// generate scope, a task or function, or a named block.
    fn is_named_vpi_scope(&self, id: NodeId) -> bool {
        match self.kind(id) {
            NodeKind::ModuleInst { .. } | NodeKind::GenScope | NodeKind::FuncTask { .. } => true,
            _ => self.is_declaration_scope(id) && !self.node(id).name.is_empty(),
        }
    }

    /// The declaration scopes `id` owns, in capture (source) order.
    fn owned_declaration_scopes(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.node(id).children.iter().copied().filter(move |child| {
            self.node(*child).parent == Some(id) && self.is_declaration_scope(*child)
        })
    }

    /// Whether the unnamed declaration scope `id` is inside an instance or
    /// generate scope through blocks only, or in a task or function of
    /// one: the scopes the catalog holds (not a class method or a package
    /// subroutine).
    fn in_instance_scope(&self, id: NodeId) -> bool {
        let mut current = self.node(id).parent;
        while let Some(parent) = current {
            match self.kind(parent) {
                NodeKind::ModuleInst { .. } | NodeKind::GenScope => return true,
                NodeKind::FuncTask { .. } => return self.vpi_declared_in_instance(parent),
                _ if self.is_declaration_scope(parent) => current = self.node(parent).parent,
                _ => return false,
            }
        }
        false
    }

    fn build_vpi_scopes(&self) -> VpiScopeIndex {
        let mut index = VpiScopeIndex::default();
        for id in self.db.node_ids() {
            let block = matches!(
                self.kind(id),
                NodeKind::Stmt(StmtKind::Begin | StmtKind::Fork { .. })
            ) && self.db.semantic_detail(id) == Some("Block");
            if block {
                if let Some(scope) = self.node(id).children.iter().copied().find(|child| {
                    self.node(*child).parent != Some(id) && self.is_declaration_scope(*child)
                }) {
                    index.block_scope.insert(id, scope);
                    index.scope_block.insert(scope, id);
                }
            }
            if !self.is_named_vpi_scope(id) {
                continue;
            }
            // Number the unnamed scopes below this named scope in pre-order;
            // a named scope below keeps its own count.
            let mut count = 0usize;
            let mut stack = self.owned_declaration_scopes(id).collect::<Vec<_>>();
            stack.reverse();
            while let Some(scope) = stack.pop() {
                if !self.node(scope).name.is_empty() {
                    continue;
                }
                if self.in_instance_scope(scope) {
                    index.names.insert(scope, format!("unnamed$$_{count}"));
                }
                count += 1;
                let first = stack.len();
                stack.extend(self.owned_declaration_scopes(scope));
                stack[first..].reverse();
            }
        }
        index
    }

    /// The declaration scope of block statement `id`, if it has one.
    pub(super) fn vpi_block_scope(&self, id: NodeId) -> Option<NodeId> {
        self.vpi_scopes().block_scope.get(&id).copied()
    }

    /// The generated name of unnamed declaration scope `id`, if it is a
    /// catalog scope.
    pub(super) fn vpi_unnamed_scope_name(&self, id: NodeId) -> Option<&str> {
        self.vpi_scopes().names.get(&id).map(String::as_str)
    }

    /// The statement an unnamed catalog scope `scope` stands for.
    pub(super) fn vpi_scope_statement(&self, scope: NodeId) -> Option<(NodeId, ScopeStatement)> {
        self.vpi_unnamed_scope_name(scope)?;
        let block = *self.vpi_scopes().scope_block.get(&scope)?;
        let statement = match self.kind(block) {
            NodeKind::Stmt(StmtKind::Fork { .. }) => ScopeStatement::Fork,
            _ => {
                if let Some((found, _)) = self.vpi_for_declarations(block) {
                    ScopeStatement::For(found)
                } else if let Some(found) = self.vpi_foreach_block(block) {
                    ScopeStatement::Foreach(found)
                } else {
                    ScopeStatement::Begin
                }
            }
        };
        Some((block, statement))
    }

    /// The statements of a block or list, lists folded in, declarations
    /// and the block's declaration scope left out.
    pub(super) fn vpi_flatten(&self, id: NodeId, out: &mut Vec<NodeId>) {
        for child in &self.node(id).children {
            match (self.kind(*child), self.db.semantic_detail(*child)) {
                (NodeKind::Stmt(StmtKind::Begin), Some("List")) => self.vpi_flatten(*child, out),
                (NodeKind::Stmt(StmtKind::Begin), Some("StatementBlock"))
                | (NodeKind::Stmt(StmtKind::VariableDecl { .. }), _) => {}
                (
                    NodeKind::Stmt(_)
                    | NodeKind::FuncCall { .. }
                    | NodeKind::SysCall { .. }
                    | NodeKind::MethodCall { .. },
                    _,
                ) => out.push(*child),
                _ => {}
            }
        }
    }

    /// The foreach statement an implicit block holds only to declare its
    /// loop variables (37.12 detail 3 makes the foreach their scope), or
    /// `None` for any other block.
    pub(super) fn vpi_foreach_block(&self, id: NodeId) -> Option<NodeId> {
        let mut statements = Vec::new();
        self.vpi_flatten(id, &mut statements);
        let [found] = statements.as_slice() else {
            return None;
        };
        let NodeKind::Stmt(StmtKind::Foreach { vars, .. }) = self.kind(*found) else {
            return None;
        };
        // The loop variables live in the block's declaration scope.
        let declared_here = |var: NodeId| {
            self.node(var)
                .parent
                .is_some_and(|parent| parent == id || self.node(id).children.contains(&parent))
        };
        vars.iter()
            .flatten()
            .any(|var| declared_here(*var))
            .then_some(*found)
    }

    /// The variables an implicit block declares for the one `for` loop it
    /// holds (`for (int i = 0; ...)`), or `None` for any other block.
    pub(super) fn vpi_for_declarations(&self, id: NodeId) -> Option<(NodeId, Vec<NodeId>)> {
        let mut children = Vec::new();
        for child in &self.node(id).children {
            if self.db.semantic_detail(*child) == Some("List") {
                children.extend(self.node(*child).children.iter().copied());
            } else {
                children.push(*child);
            }
        }
        let mut declarations = Vec::new();
        let mut found = None;
        for child in children {
            match (self.kind(child), self.db.semantic_detail(child)) {
                (NodeKind::Stmt(StmtKind::VariableDecl { declaration }), _) => {
                    declarations.push(*declaration);
                }
                (NodeKind::Stmt(StmtKind::For { init, vars, .. }), _)
                    if found.is_none() && init.is_empty() && vars.is_empty() =>
                {
                    found = Some(child);
                }
                (NodeKind::Stmt(StmtKind::Begin), Some("StatementBlock")) => {}
                _ => return None,
            }
        }
        let found = found?;
        (!declarations.is_empty()).then_some((found, declarations))
    }
}
