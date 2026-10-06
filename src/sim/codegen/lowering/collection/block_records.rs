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
                    }
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
        let descriptor = self
            .query_descriptor(declaration)
            .cloned()
            .ok_or_else(|| format!("procedural-block record `{name}` in `{path}` has no type"))?;
        if let Some(member) = Self::member_with_initializer(&descriptor) {
            return Err(format!(
                "member default of `{member}` in procedural-block record `{name}` in `{path}` is not supported"
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
            if self.forked_block_activation(statement) {
                return Err(format!(
                    "automatic record `{name}` in `{path}` declared in or around a fork with `join_any` or `join_none` is not supported"
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
                self.block_record_initializers
                    .push((declaration, initializer, path.to_owned()));
            }
        }
        Ok(())
    }

    /// A lookup key for the C names of one block declaration's leaves. Its
    /// components extend the scope's with an empty component and the
    /// declaration identity: no source identifier is empty, so the names
    /// cannot collide with a scope member or another block's declaration.
    /// The key itself contains U+001F, which no source path does.
    fn block_storage_scope(&mut self, path: &str, declaration: NodeId) -> String {
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

    fn member_with_initializer(descriptor: &TypeDescriptor) -> Option<String> {
        match &descriptor.shape {
            TypeShape::Aggregate(layout) => layout.members.iter().find_map(|member| {
                if member.initializer.is_some() {
                    Some(member.name.clone())
                } else {
                    Self::member_with_initializer(&member.descriptor)
                }
            }),
            TypeShape::FixedArray { element, .. } => Self::member_with_initializer(element),
            _ => None,
        }
    }

    /// Whether a fork can keep one activation of a declaration's block live
    /// while the block is entered again: its declaration `statement` is
    /// inside a fork that its parent need not join, or its block starts one.
    fn forked_block_activation(&self, statement: NodeId) -> bool {
        let detached = |node: NodeId| {
            matches!(self.kind(node), NodeKind::Stmt(StmtKind::Fork { join_kind, .. })
                if *join_kind != JoinKind::All)
        };
        let mut current = self.node(statement).parent;
        while let Some(node) = current {
            if matches!(self.kind(node), NodeKind::Process { .. }) {
                break;
            }
            if detached(node) {
                return true;
            }
            current = self.node(node).parent;
        }
        let mut pending = self.node(statement).parent.into_iter().collect::<Vec<_>>();
        let mut visited = HashSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node) {
                continue;
            }
            if detached(node) {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
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
        let mut statements = Vec::with_capacity(aggregate.leaves.len());
        let mut signals = HashSet::new();
        for leaf in &aggregate.leaves {
            if let Some(signal) = &leaf.signal {
                // Untagged union members share one signal.
                if !signals.insert(signal.ir) {
                    continue;
                }
                let value = if signal.real {
                    real_literal_expr(0.0)
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
                        IrObjectType::String => {
                            IrObjectStmt::StringAssign(object, IrStringExpr::Literal(Vec::new()))
                        }
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
                "initializer of procedural-block record `{}` in `{path}` has no record assignment",
                self.node(declaration).name
            )
        })
    }

    /// Static block-record initializers run once in the static
    /// initialization schedule, after every body has lowered.
    pub(in super::super) fn emit_block_record_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.block_record_initializers);
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
    statements: HashMap<NodeId, NodeId>,
}
