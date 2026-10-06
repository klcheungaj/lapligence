//! Records declared in procedural blocks.
//!
//! An unpacked record that is not a fixed packed value (it has string, real,
//! chandle, class-handle or queue/dynamic/associative leaves) and is declared
//! in an `initial`/`always`/`final` block uses the same declaration-owned leaf
//! storage as a module record, so member, whole-value, pattern, container and
//! comparison lowering apply unchanged. Each declaration of each instance owns
//! its leaves. Static records keep their value and initialize once in the
//! static schedule (SV 6.21, 10.5); automatic records are reset to their
//! default-uninitialized value and initialized at every block entry.
use super::record_defaults::LeafDefault;
use super::*;
use crate::core::db::JoinKind;
use crate::sim::ir::{IrContainerKind, IrContainerStmt};

impl Codegen<'_> {
    /// Collect the leaf storage of every procedural-block record of
    /// `process`, a process of the instance or generate scope at `path`.
    /// Runs after the scope's own declarations so no name-based lookup can
    /// mistake a block declaration for a scope member.
    pub(super) fn collect_block_records(
        &mut self,
        path: &str,
        process: NodeId,
    ) -> Result<(), String> {
        let mut found = BlockDeclarations::default();
        self.block_record_declarations(process, &mut found);
        for declaration in found.records {
            let statement = found.statements.get(&declaration).copied();
            self.collect_block_record(path, declaration, statement)?;
        }
        for declaration in found.natives {
            // Declarations the frontend creates without a declaration
            // statement (pattern variables) keep their own storage paths.
            if let Some(statement) = found.statements.get(&declaration).copied() {
                self.collect_block_native(path, declaration, statement)?;
            }
        }
        for declaration in found.events {
            if let Some(statement) = found.statements.get(&declaration).copied() {
                self.collect_block_event(path, declaration, statement)?;
            }
        }
        Ok(())
    }

    /// Block declarations hang off scope blocks that the statement tree also
    /// lists as children; their declaration statements carry the lexical
    /// position that fork analysis and per-entry initialization use.
    fn block_record_declarations(&self, node: NodeId, found: &mut BlockDeclarations) {
        if !found.visited.insert(node) {
            return;
        }
        if let NodeKind::Stmt(StmtKind::VariableDecl { declaration }) = self.kind(node) {
            found.statements.entry(*declaration).or_insert(node);
        }
        for child in &self.node(node).children {
            match self.kind(*child) {
                NodeKind::Var { .. } => {
                    if matches!(self.kind(node), NodeKind::Stmt(StmtKind::Begin))
                        && !found.records.contains(child)
                        && self
                            .query_descriptor(*child)
                            .is_some_and(Self::block_record_type)
                    {
                        found.records.push(*child);
                    } else if matches!(self.kind(node), NodeKind::Stmt(StmtKind::Begin))
                        && !found.natives.contains(child)
                        && self.block_native_type(*child).is_some()
                    {
                        found.natives.push(*child);
                    }
                }
                NodeKind::NamedEvent
                    if matches!(self.kind(node), NodeKind::Stmt(StmtKind::Begin))
                        && !found.events.contains(child)
                        && self.db.event_array_meta(*child).is_none() =>
                {
                    found.events.push(*child);
                }
                // Subroutine bodies keep their own storage; nested scopes
                // are collected with their own instance.
                NodeKind::FuncTask { .. }
                | NodeKind::ModuleInst { .. }
                | NodeKind::GenScope
                | NodeKind::GenScopeArray
                | NodeKind::ClassDef => {}
                _ => self.block_record_declarations(*child, found),
            }
        }
    }

    /// Records whose storage is leaf objects, signals and containers: an
    /// unpacked struct or tagged union with a leaf that has no packed
    /// representation. Fixed records stay one packed local and records
    /// beyond packed capacity use column layout (RTL-101).
    fn block_record_type(descriptor: &TypeDescriptor) -> bool {
        matches!(&descriptor.shape, TypeShape::Aggregate(layout)
            if matches!(layout.kind, AggregateKind::UnpackedStruct | AggregateKind::TaggedUnion))
            && Self::fixed_descriptor_width_bits(descriptor).is_none()
            && !super::record_columns::record_column_layout(descriptor)
    }

    fn collect_block_record(
        &mut self,
        path: &str,
        declaration: NodeId,
        statement: Option<NodeId>,
    ) -> Result<(), String> {
        if let Some(owner) = self.block_records.get(&declaration) {
            if owner != path {
                return Err(format!(
                    "procedural-block record `{}` is shared by `{owner}` and `{path}`",
                    self.node(declaration).name
                ));
            }
            return Ok(());
        }
        let name = self.node(declaration).name.clone();
        if self.query_descriptor(declaration).is_none() {
            return Err(format!(
                "procedural-block record `{name}` in `{path}` has no type"
            ));
        }
        let lifetime = self.db.variable_lifetime(declaration);
        if lifetime == VariableLifetime::Unavailable {
            return Err(format!(
                "resolved lifetime is unavailable for procedural-block record `{name}` in `{path}`"
            ));
        }
        if lifetime == VariableLifetime::Automatic {
            // One storage per declaration cannot represent a second live
            // activation of the same block, which only a fork can create.
            let statement = statement.ok_or_else(|| {
                format!("automatic record `{name}` in `{path}` has no declaration statement")
            })?;
            if self.concurrent_block_activation(statement, declaration) {
                return Err(format!(
                    "automatic record `{name}` in `{path}` can be live in two activations through a `join_any`/`join_none` fork that runs again; this is not supported"
                ));
            }
        }
        let scope = self.block_storage_scope(path, declaration);
        let first_signal = self.model.signals.len();
        let collected = self
            .collect_aggregate(&scope, declaration)
            .map_err(|error| error.replace(&scope, path))?;
        if !collected {
            return Err(format!(
                "procedural-block record `{name}` in `{path}` has no leaf storage"
            ));
        }
        // Block declarations have no waveform identity, like other
        // procedural locals.
        for signal in &mut self.model.signals[first_signal..] {
            signal.hdl_name = None;
        }
        self.block_records.insert(declaration, path.to_owned());
        if lifetime == VariableLifetime::Static {
            if let Some(initializer) = self.db.var_initializer(declaration) {
                self.reserve_initializer_order(declaration);
                self.record_statement_initializers.push((
                    declaration,
                    initializer,
                    path.to_owned(),
                ));
            }
        }
        Ok(())
    }

    /// A lookup key for the C names of one block declaration's leaves. Its
    /// components extend the scope's with an empty component and the
    /// declaration identity: no source identifier is empty, so the names
    /// cannot collide with a scope member or another block's declaration.
    /// The key itself contains U+001F, which no source path does.
    pub(in super::super) fn block_storage_scope(
        &mut self,
        path: &str,
        declaration: NodeId,
    ) -> String {
        let key = format!("{path}\u{1f}{}", declaration.index());
        let mut components = self
            .c_paths
            .get(path)
            .cloned()
            .unwrap_or_else(|| vec![path.to_owned()]);
        components.push(String::new());
        components.push(declaration.index().to_string());
        self.c_paths.insert(key.clone(), components);
        key
    }

    /// Whether one activation of an automatic declaration can still be in
    /// use when its block is entered again, which one storage per
    /// declaration cannot represent. Only a `join_any`/`join_none` fork
    /// keeps an activation alive past its block, so this holds when
    ///
    /// - a detached fork encloses the declaration and can start again, or
    /// - the declaring block can run again while a detached fork inside it
    ///   still references the declaration.
    ///
    /// A block that runs again is one inside a loop or an `always`
    /// procedure; automatic variables cannot be referenced hierarchically
    /// (SV 6.21), so references are lexical.
    pub(super) fn concurrent_block_activation(
        &self,
        statement: NodeId,
        declaration: NodeId,
    ) -> bool {
        let mut current = self.node(statement).parent;
        while let Some(node) = current {
            if matches!(self.kind(node), NodeKind::Process { .. }) {
                break;
            }
            if self.detached_fork(node) && self.may_run_again(node) {
                return true;
            }
            current = self.node(node).parent;
        }
        let Some(block) = self.node(statement).parent else {
            return false;
        };
        let mut pending = vec![block];
        let mut visited = HashSet::new();
        let mut referencing_fork = false;
        while let Some(node) = pending.pop() {
            if !visited.insert(node) {
                continue;
            }
            if self.detached_fork(node) && self.references_declaration(node, declaration) {
                referencing_fork = true;
                break;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        referencing_fork && self.may_run_again(block)
    }

    fn detached_fork(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::Stmt(StmtKind::Fork { join_kind, .. })
            if *join_kind != JoinKind::All)
    }

    /// Whether the statement `node` of a process body can execute more than
    /// once: it is inside a loop, or the process is not `initial`/`final`.
    /// Statements the lowering does not model count as loops.
    fn may_run_again(&self, node: NodeId) -> bool {
        let mut current = Some(node);
        while let Some(id) = current {
            match self.kind(id) {
                NodeKind::Process { kind } => {
                    return !matches!(kind, ProcessKind::Initial | ProcessKind::Final)
                }
                NodeKind::Stmt(
                    StmtKind::For { .. }
                    | StmtKind::Foreach { .. }
                    | StmtKind::While { .. }
                    | StmtKind::DoWhile { .. }
                    | StmtKind::Repeat { .. }
                    | StmtKind::Forever { .. }
                    | StmtKind::Unsupported { .. },
                ) if id != node => return true,
                _ => {}
            }
            current = self.node(id).parent;
        }
        true
    }

    /// Whether the subtree at `node` names `declaration`, by resolved
    /// reference or, for an unresolved reference, by name.
    pub(super) fn references_declaration(&self, node: NodeId, declaration: NodeId) -> bool {
        let name = &self.node(declaration).name;
        let mut pending = vec![node];
        let mut visited = HashSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            match self.kind(id) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) if *target == declaration => return true,
                NodeKind::Expr(ExprKind::Ref { target: None }) if &self.node(id).name == name => {
                    return true
                }
                NodeKind::Expr(ExprKind::HierPath { refs, .. })
                    if refs.contains(&Some(declaration)) =>
                {
                    return true
                }
                _ => {}
            }
            pending.extend(self.node(id).children.iter().copied());
        }
        false
    }

    /// The statements of a procedural-block record declaration, or `None`
    /// for any other declaration. Static storage initializes in the static
    /// schedule; automatic storage is reset and initialized at each entry.
    pub(in super::super) fn lower_block_record_declaration(
        &mut self,
        path: &str,
        declaration: NodeId,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        if !self.block_records.contains_key(&declaration) {
            return Ok(None);
        }
        if self.db.variable_lifetime(declaration) != VariableLifetime::Automatic {
            return Ok(Some(Vec::new()));
        }
        let mut statements = self.block_record_reset(path, declaration)?;
        if let Some(initializer) = self.db.var_initializer(declaration) {
            statements.push(self.block_record_assignment(path, declaration, initializer)?);
        }
        Ok(Some(statements))
    }

    /// Restore every leaf to its default-uninitialized value (SV Table 6-7):
    /// empty strings and containers, null handles, 0.0 reals and X (or 0
    /// for two-state) packed leaves.
    fn block_record_reset(
        &mut self,
        path: &str,
        declaration: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let aggregate = self
            .unpacked_aggregates
            .get(&declaration)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "procedural-block record `{}` in `{path}` has no storage",
                    self.node(declaration).name
                )
            })?;
        // Without an initializer the record starts from its member defaults
        // (SV 7.2.2); an initializer replaces the whole value.
        let descriptor = self
            .db
            .var_initializer(declaration)
            .is_none()
            .then(|| self.query_descriptor(declaration).cloned())
            .flatten();
        let mut statements = Vec::with_capacity(aggregate.leaves.len());
        let mut signals = HashSet::new();
        for leaf in &aggregate.leaves {
            let default = match &descriptor {
                Some(descriptor) => Self::record_member_default(descriptor, &leaf.path)?,
                None => None,
            };
            if let Some(signal) = &leaf.signal {
                // Untagged union members share one signal.
                if !signals.insert(signal.ir) {
                    continue;
                }
                let value = if let Some(LeafDefault::Real(value)) = default {
                    real_literal_expr(value)
                } else if signal.real {
                    real_literal_expr(0.0)
                } else if let Some(LeafDefault::Packed(constant)) = default {
                    IrExpr::new(
                        IrExprKind::Const(constant),
                        signal.width,
                        signal.signed,
                        None,
                    )
                } else {
                    let constant = Self::fixed_descriptor_uninitialized(&leaf.member.descriptor)
                        .ok_or_else(|| {
                            format!(
                                "record member `{}` in `{path}` has no default-uninitialized value",
                                aggregate_path_suffix(&leaf.path)
                            )
                        })?;
                    IrExpr::new(
                        IrExprKind::Const(constant),
                        signal.width,
                        signal.signed,
                        None,
                    )
                };
                statements.push(IrStmt::Assign {
                    lhs: self.reference_lhs(IrLhs::Whole(signal.ir))?,
                    rhs: value,
                    nba: false,
                });
            } else if let Some(object) = leaf.object {
                let object = self.reference_object(object);
                statements.push(IrStmt::Object(Box::new(
                    match self.model.objects[object].ty {
                        IrObjectType::String => IrObjectStmt::StringAssign(
                            object,
                            IrStringExpr::Literal(match default {
                                Some(LeafDefault::String(bytes)) => bytes,
                                _ => Vec::new(),
                            }),
                        ),
                        IrObjectType::Chandle => {
                            IrObjectStmt::ChandleAssign(object, IrChandleExpr::Null)
                        }
                        IrObjectType::Semaphore | IrObjectType::Process => {
                            return Err(format!(
                                "record member `{}` in `{path}` has no resettable storage",
                                aggregate_path_suffix(&leaf.path)
                            ))
                        }
                    },
                )));
            } else if let Some(container) = &leaf.container {
                statements.push(IrStmt::Container(Box::new(IrContainerStmt::Delete(
                    container.ir,
                ))));
                if matches!(
                    self.model.containers[container.ir].kind,
                    IrContainerKind::Associative { .. }
                ) {
                    statements.push(IrStmt::Container(Box::new(IrContainerStmt::ResetDefault(
                        container.ir,
                    ))));
                }
            } else {
                return Err(format!(
                    "record member `{}` in `{path}` has no leaf storage",
                    aggregate_path_suffix(&leaf.path)
                ));
            }
        }
        Ok(statements)
    }

    fn block_record_assignment(
        &mut self,
        path: &str,
        declaration: NodeId,
        initializer: NodeId,
    ) -> Result<IrStmt, String> {
        self.lower_unpacked_aggregate_assignment(
            path,
            declaration,
            initializer,
            false,
            Operation::Assignment,
        )?
        .ok_or_else(|| {
            format!(
                "initializer of record `{}` in `{path}` has no record assignment",
                self.node(declaration).name
            )
        })
    }

    /// Static block-record initializers, and module-record initializers
    /// that are not leaf-wise patterns, run once as record assignments in
    /// the static initialization schedule, after every body has lowered.
    pub(in super::super) fn emit_record_statement_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.record_statement_initializers);
        for (declaration, initializer, path) in initializers {
            let inst = self.owner_instance(declaration).ok_or_else(|| {
                format!(
                    "record initializer for `{}` has no owning instance",
                    self.node(declaration).name
                )
            })?;
            self.inst = inst;
            self.depth_arg = "0".into();
            let statement = self.block_record_assignment(&path, declaration, initializer)?;
            self.record_initializer_source(declaration, initializer);
            let identity = self.declaration_identity(declaration)?;
            self.declaration_statements.push((identity, statement));
        }
        Ok(())
    }
}

/// Record declarations of one process body and their declaration statements.
#[derive(Default)]
struct BlockDeclarations {
    visited: HashSet<NodeId>,
    records: Vec<NodeId>,
    /// String, chandle and class-handle declarations.
    natives: Vec<NodeId>,
    /// Scalar named events.
    events: Vec<NodeId>,
    statements: HashMap<NodeId, NodeId>,
}
