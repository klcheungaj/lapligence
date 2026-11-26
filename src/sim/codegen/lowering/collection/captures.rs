//! Captures.

use super::*;
use crate::sim::ir::*;
use crate::IdSet;

impl<'a> Codegen<'a> {
    pub(in super::super) fn capture_source(&self, target: NodeId) -> Option<CaptureSource> {
        if let Some(binding) = self.capture_binding(target) {
            let info = binding.local.clone();
            // A shared container or record keeps its canonical capture name,
            // under which the branch binds it, so nested forks alias it too.
            let name = match binding.storage.kind() {
                StorageKind::Native => self
                    .native_roots
                    .get(&target)
                    .map(|value| shared_native_capture_name(*value)),
                StorageKind::Container => self
                    .container_globals
                    .get(&target)
                    .map(|container| shared_container_capture_name(container.ir)),
                _ => None,
            }
            .unwrap_or_else(|| Self::capture_local_name(binding.storage));
            let initial = IrExpr::new(
                if binding.storage.kind() == StorageKind::Event {
                    IrExprKind::ObjectQuery(Box::new(IrObjectQuery::EventCapture(
                        IrEventRef::Captured(name),
                    )))
                } else {
                    IrExprKind::LocalRead(name)
                },
                info.width,
                info.signed,
                None,
            );
            return Some(CaptureSource {
                info,
                initial,
                lifetime: binding.storage.lifetime(),
                kind: binding.storage.kind(),
            });
        }
        if let Some(info) = self.proc_local_info(target) {
            if info.static_signal.is_none() {
                return Some(CaptureSource {
                    info: info.clone(),
                    initial: IrExpr::new(
                        IrExprKind::LocalRead(info.c_name.clone()),
                        info.width,
                        info.signed,
                        None,
                    ),
                    lifetime: StorageLifetime::Automatic,
                    kind: super::super::storage_kind(info.width),
                });
            }
            return None;
        }
        if let Some(name) = self.proc_semaphore_local_name(target) {
            if self.db.variable_lifetime(target) == VariableLifetime::Automatic {
                return Some(CaptureSource {
                    info: ProcLocalInfo {
                        c_name: name.to_owned(),
                        width: 1,
                        signed: false,
                        two_state: true,
                        static_signal: None,
                    },
                    // A shared handle is aliased under its own local name.
                    initial: if self.shared_locals.contains(&target) {
                        IrExpr::new(IrExprKind::LocalRead(name.to_owned()), 0, false, None)
                    } else {
                        IrExpr::new(
                            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::HandleCapture(
                                IrChandleExpr::LocalRead(name.to_owned()),
                            ))),
                            1,
                            false,
                            None,
                        )
                    },
                    lifetime: StorageLifetime::Automatic,
                    kind: StorageKind::Opaque,
                });
            }
            return None;
        }
        // A block string live in two activations (see
        // `collect_block_native`) and a fork-shared string `foreach` key are
        // process string locals.
        if let Some(name) = self.proc_string_local_name(target).filter(|_| {
            self.is_block_native(target)
                || self.shared_locals.contains(&target) && self.is_foreach_iterator(target)
        }) {
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: name.to_owned(),
                    width: 0,
                    signed: false,
                    two_state: true,
                    static_signal: None,
                },
                initial: IrExpr::new(IrExprKind::LocalRead(name.to_owned()), 0, false, None),
                lifetime: StorageLifetime::Automatic,
                kind: StorageKind::String,
            });
        }
        // An automatic subroutine event is captured by object identity.
        if let Some(name) = self.local_event_handles.get(&target) {
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: String::new(),
                    width: 1,
                    signed: false,
                    two_state: true,
                    static_signal: None,
                },
                initial: IrExpr::new(
                    IrExprKind::ObjectQuery(Box::new(IrObjectQuery::EventCapture(
                        IrEventRef::Captured(name.clone()),
                    ))),
                    1,
                    false,
                    None,
                ),
                lifetime: StorageLifetime::Automatic,
                kind: StorageKind::Event,
            });
        }
        // A shared activation native record is aliased through its frame.
        if let Some(value) = self.native_roots.get(&target).copied().filter(|value| {
            self.model.native_values[*value].activation && self.shared_locals.contains(&target)
        }) {
            let name = crate::sim::ir::shared_native_capture_name(value);
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: name.clone(),
                    width: 0,
                    signed: false,
                    two_state: true,
                    static_signal: None,
                },
                initial: IrExpr::new(IrExprKind::LocalRead(name), 0, false, None),
                lifetime: StorageLifetime::Automatic,
                kind: StorageKind::Native,
            });
        }
        // A shared activation container is aliased through its frame slot.
        if let Some(container) = self
            .container_globals
            .get(&target)
            .map(|container| container.ir)
            .filter(|container| {
                self.model.containers[*container].activation && self.shared_locals.contains(&target)
            })
        {
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: crate::sim::ir::shared_container_capture_name(container),
                    width: 0,
                    signed: false,
                    two_state: true,
                    static_signal: None,
                },
                initial: IrExpr::new(
                    IrExprKind::LocalRead(crate::sim::ir::shared_container_capture_name(container)),
                    0,
                    false,
                    None,
                ),
                lifetime: StorageLifetime::Automatic,
                kind: StorageKind::Container,
            });
        }
        let function = self.func.as_ref()?;
        // A shared handle local of the subroutine (chandle, class, semaphore
        // or mailbox handle) is aliased under its own local name.
        if matches!(
            self.kind(target),
            NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind)
        ) && self.shared_locals.contains(&target)
        {
            if let Some(IrChandleExpr::LocalRead(name)) = function.chandle_read.get(&target) {
                return Some(CaptureSource {
                    info: ProcLocalInfo {
                        c_name: name.clone(),
                        width: 0,
                        signed: false,
                        two_state: true,
                        static_signal: None,
                    },
                    initial: IrExpr::new(IrExprKind::LocalRead(name.clone()), 0, false, None),
                    lifetime: StorageLifetime::Automatic,
                    kind: StorageKind::Opaque,
                });
            }
        }
        // An automatic string local of the subroutine is captured by value
        // (or shared, see `fork_shared`) under its own local name.
        let string_local = match self.kind(target) {
            NodeKind::Var { ty } => {
                ty.kind == "string"
                    && self.db.variable_lifetime(target) == VariableLifetime::Automatic
            }
            NodeKind::FuncArg { ty, .. } => {
                ty.kind == "string" && self.shared_locals.contains(&target)
            }
            _ => false,
        };
        if string_local {
            if let Some((c_name, ..)) = function.locals.get(&target) {
                return Some(CaptureSource {
                    info: ProcLocalInfo {
                        c_name: c_name.clone(),
                        width: 0,
                        signed: false,
                        two_state: true,
                        static_signal: None,
                    },
                    initial: IrExpr::new(IrExprKind::LocalRead(c_name.clone()), 0, false, None),
                    lifetime: StorageLifetime::Automatic,
                    kind: StorageKind::String,
                });
            }
        }
        if let Some(event) = function.event_args.get(&target) {
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: String::new(),
                    width: 1,
                    signed: false,
                    two_state: true,
                    static_signal: None,
                },
                initial: IrExpr::new(
                    IrExprKind::ObjectQuery(Box::new(IrObjectQuery::EventCapture(event.clone()))),
                    1,
                    false,
                    None,
                ),
                lifetime: StorageLifetime::Automatic,
                kind: StorageKind::Event,
            });
        }
        if let Some(storage) = function.persistent.get(&target) {
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: storage.global.clone(),
                    width: storage.width,
                    signed: storage.signed,
                    two_state: storage.two_state,
                    static_signal: Some(storage.clone()),
                },
                initial: sig_read_expr_full(storage),
                lifetime: StorageLifetime::Static,
                kind: super::super::storage_kind(storage.width),
            });
        }
        if let Some((c_name, width, signed, two_state, _shortreal)) = function.locals.get(&target) {
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: c_name.clone(),
                    width: *width,
                    signed: *signed,
                    two_state: *two_state,
                    static_signal: None,
                },
                initial: IrExpr::new(IrExprKind::LocalRead(c_name.clone()), *width, *signed, None),
                lifetime: if self.function_is_automatic(function) {
                    StorageLifetime::Automatic
                } else {
                    StorageLifetime::Static
                },
                kind: super::super::storage_kind(*width),
            });
        }
        if let Some(arg) = function.arg_read.get(&target) {
            let initial = function.arg_ir.get(&target)?.clone();
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: function
                        .arg_write
                        .get(&target)
                        .and_then(|address| address.strip_prefix('&'))
                        .unwrap_or_default()
                        .to_owned(),
                    width: arg.width,
                    signed: arg.signed,
                    two_state: arg.two_state,
                    static_signal: function.persistent.get(&target).cloned(),
                },
                initial,
                lifetime: if function.persistent.contains_key(&target) {
                    StorageLifetime::Static
                } else {
                    StorageLifetime::Automatic
                },
                kind: super::super::storage_kind(arg.width),
            });
        }
        if function.ret_node == Some(target) {
            let ret = function.ret.as_ref()?;
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: ret.c_name.clone(),
                    width: ret.width,
                    signed: ret.signed,
                    two_state: ret.two_state,
                    static_signal: None,
                },
                initial: IrExpr::new(
                    IrExprKind::LocalRead(ret.c_name.clone()),
                    ret.width,
                    ret.signed,
                    None,
                ),
                lifetime: if self.function_is_automatic(function) {
                    StorageLifetime::Automatic
                } else {
                    StorageLifetime::Static
                },
                kind: super::super::storage_kind(ret.width),
            });
        }
        None
    }

    pub(super) fn function_is_automatic(&self, function: &FuncCtx) -> bool {
        function.def_node.is_some_and(|definition| {
            matches!(
                self.kind(definition),
                NodeKind::FuncTask {
                    automatic: true,
                    ..
                }
            )
        }) || function.def_node.is_none()
    }

    pub(in super::super) fn capture_target(&self, node: NodeId) -> Option<NodeId> {
        if self.capture_locals.contains_key(&node) {
            return Some(node);
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            if self.capture_locals.contains_key(target) {
                return Some(*target);
            }
        }
        self.lexical_proc_local(node)
            .map(|(target, _)| target)
            .filter(|target| self.capture_locals.contains_key(target))
    }

    pub(in super::super) fn nested_capture_ref(&self, node: NodeId) -> Option<NodeId> {
        if let Some(target) = self.capture_target(node) {
            return Some(target);
        }
        self.node(node)
            .children
            .iter()
            .find_map(|child| self.nested_capture_ref(*child))
    }

    pub(in super::super) fn capture_local_name(storage: StorageRef) -> String {
        format!("_fc{}_{}", storage.frame().index(), storage.slot())
    }

    pub(super) fn node_is_within(&self, node: NodeId, scope: NodeId) -> bool {
        self.nesting().contains(scope, node)
    }

    /// Whether automatic variable `declaration` is shared with a `join_none`
    /// or `join_any` branch that names it outside the branch's own scope.
    /// Such a variable lives in a shared activation frame (SV 6.21, 9.3.2).
    pub(in super::super) fn fork_shared(&mut self, declaration: NodeId) -> bool {
        self.fork_sets().0.contains(&declaration)
    }

    /// Whether a branch of any fork names automatic variable `declaration`
    /// outside the branch's own scope, so another process can write it.
    pub(in super::super) fn fork_visible(&mut self, declaration: NodeId) -> bool {
        self.fork_sets().1.contains(&declaration)
    }

    /// Whether `node` holds a `join_none` or `join_any` fork.
    pub(in super::super) fn holds_detached_fork(&self, node: NodeId) -> bool {
        let mut pending = vec![node];
        while let Some(node) = pending.pop() {
            if matches!(
                self.kind(node),
                NodeKind::Stmt(StmtKind::Fork { join_kind, .. }) if *join_kind != DbJoinKind::All
            ) {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// Whether a fork branch that names automatic variable `target` shares
    /// it with the parent; `detached` for a `join_none`/`join_any` fork.
    fn fork_shares(&self, detached: bool, target: NodeId) -> bool {
        // Strings, containers and native records are shared with branches
        // of every fork; other variables only with detached ones, since a
        // join branch borrows the suspended parent's cell.
        detached
            || matches!(
                self.kind(target),
                NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. } if ty.kind == "string"
            )
            || self.subroutine_container_meta(target).is_some()
            || self.native_value_type(target).is_some()
            || matches!(
                self.kind(target),
                NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind)
            )
    }

    /// Whether `foreach` iterator `variable` is shared with a fork nested in
    /// its loop (SV 12.7.3: the loop owns the variable), as `fork_shares`
    /// decides for that fork. A loop that sits inside a fork branch is not
    /// enough: its variable belongs to that branch's process alone.
    pub(in super::super) fn foreach_fork_shared(&mut self, variable: NodeId) -> bool {
        if !self.fork_shared(variable) {
            return false;
        }
        let Some(statement) = self.foreach_statement(variable) else {
            return false;
        };
        let mut pending = vec![statement];
        while let Some(node) = pending.pop() {
            if let NodeKind::Stmt(StmtKind::Fork {
                join_kind,
                branches,
                ..
            }) = self.kind(node)
            {
                let detached = matches!(join_kind, DbJoinKind::None | DbJoinKind::Any);
                if self.fork_shares(detached, variable)
                    && branches.iter().any(|branch| {
                        !matches!(
                            self.kind(*branch),
                            NodeKind::Stmt(StmtKind::VariableDecl { .. })
                        ) && self.branch_references(*branch).targets.contains(&variable)
                    })
                {
                    return true;
                }
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// The fork-shared and fork-visible automatic variables of the design.
    fn fork_sets(&mut self) -> &(HashSet<NodeId>, HashSet<NodeId>) {
        if self.fork_sets.is_none() {
            let mut shared = HashSet::new();
            let mut visible = HashSet::new();
            for node in self.db.node_ids() {
                let NodeKind::Stmt(StmtKind::Fork {
                    join_kind,
                    branches,
                    ..
                }) = self.kind(node)
                else {
                    continue;
                };
                let detached = matches!(join_kind, DbJoinKind::None | DbJoinKind::Any);
                for branch in branches {
                    if matches!(
                        self.kind(*branch),
                        NodeKind::Stmt(StmtKind::VariableDecl { .. })
                    ) {
                        continue;
                    }
                    for &target in &self.branch_references(*branch).targets {
                        // A by-value formal is marked whatever the
                        // subroutine's lifetime; only an automatic
                        // activation's body consumes the mark.
                        let by_value_formal = matches!(
                            self.kind(target),
                            NodeKind::FuncArg { direction, .. } if *direction != DbDirection::Ref
                        );
                        if !by_value_formal
                            && !(matches!(
                                self.kind(target),
                                NodeKind::Var { .. } | NodeKind::Array { .. }
                            ) && self.db.variable_lifetime(target)
                                == VariableLifetime::Automatic)
                        {
                            continue;
                        }
                        visible.insert(target);
                        if self.fork_shares(detached, target) {
                            shared.insert(target);
                        }
                    }
                }
            }
            self.fork_sets = Some((shared, visible));
        }
        self.fork_sets.get_or_insert_with(Default::default)
    }

    /// Find automatic declarations referenced by a fork branch. Declarations
    /// inside the branch are owned by that branch and are not captures.
    pub(in super::super) fn fork_capture_targets(&self, branch: NodeId) -> Vec<NodeId> {
        let references = self.branch_references(branch);
        let mut targets = references
            .names
            .iter()
            .filter_map(|name| {
                self.lexical_proc_local(*name)
                    .map(|(target, _)| target)
                    .or_else(|| {
                        self.lexical_proc_string_local(*name)
                            .map(|(target, _)| target)
                    })
            })
            .filter(|target| !self.node_is_within(*target, branch))
            .chain(references.targets.iter().copied())
            // A block string, handle or event live in two activations is
            // collected before any body lowers; one whose declaration
            // statement is in the branch is the branch's own.
            .filter(|target| {
                !self
                    .block_activation_locals
                    .get(target)
                    .is_some_and(|statement| self.node_is_within(*statement, branch))
            })
            .filter(|target| {
                self.capture_source(*target)
                    .is_some_and(|source| source.info.static_signal.is_none())
                    || self.proc_string_local_name(*target).is_some()
                    // A `ref` formal of the enclosing subroutine reaches
                    // a branch by reference.
                    || self.is_ref_formal(*target)
                        && self.native_ref_formal_slots(*target).is_some()
                    || self.is_ref_formal(*target)
                        && self.func.as_ref().is_some_and(|function| {
                            function.arg_ir.contains_key(target)
                                || function.string_read.contains_key(target)
                                || function.event_args.contains_key(target)
                                || function.chandle_read.contains_key(target)
                        })
            })
            // `r.n` names a native record formal as a path.
            .chain(references.path_targets.iter().copied().filter(|target| {
                self.is_ref_formal(*target) && self.native_ref_formal_slots(*target).is_some()
            }))
            .collect::<Vec<_>>();
        targets.sort_by_key(|node| node.index());
        targets.dedup();
        targets
    }

    /// What fork branch `branch` names outside itself. A fork inside it counts
    /// with what its own branches name.
    fn branch_references(&self, branch: NodeId) -> std::rc::Rc<BranchReferences> {
        if let Some(references) = self.branch_references.borrow().get(&branch) {
            return references.clone();
        }
        let references = std::rc::Rc::new(self.collect_branch_references(branch));
        self.branch_references
            .borrow_mut()
            .insert(branch, references.clone());
        references
    }

    fn collect_branch_references(&self, branch: NodeId) -> BranchReferences {
        let mut targets = HashSet::new();
        let mut path_targets = HashSet::new();
        let mut names = HashMap::new();
        let mut pending = vec![branch];
        let mut visited = IdSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node) {
                continue;
            }
            crate::work::count(crate::work::Work::BranchNodes, 1);
            let mut reach_name = |name: NodeId| {
                if let Some(key) = self.lexical_key(name, branch) {
                    names.entry(key).or_insert(name);
                }
            };
            match self.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => {
                    targets.insert(*target);
                }
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                    path_targets.extend(refs.iter().flatten().copied());
                    reach_name(node);
                }
                NodeKind::Stmt(StmtKind::Fork { branches, .. }) => {
                    reach_name(node);
                    for nested in branches {
                        if !self.node_is_within(*nested, branch) || !visited.insert(*nested) {
                            continue;
                        }
                        let nested = self.branch_references(*nested);
                        targets.extend(nested.targets.iter().copied());
                        path_targets.extend(nested.path_targets.iter().copied());
                        nested.names.iter().copied().for_each(&mut reach_name);
                    }
                }
                _ => reach_name(node),
            }
            pending.extend(self.db.walk_children(node));
            // Timing-control operands (`#n`, `@(e)`, `wait (c)`) are statement
            // fields rather than structural children.
            if let NodeKind::Stmt(statement) = self.kind(node) {
                statement.referenced_nodes(&mut pending);
            }
        }
        let outside = |nodes: HashSet<NodeId>| {
            nodes
                .into_iter()
                .filter(|target| !self.node_is_within(*target, branch))
                .collect()
        };
        BranchReferences {
            targets: outside(targets),
            path_targets: outside(path_targets),
            names: names.into_values().collect(),
        }
    }
}

/// What a fork branch names outside itself, in no order.
pub(in super::super) struct BranchReferences {
    /// What its references resolve to.
    targets: Vec<NodeId>,
    /// What parts of its hierarchical paths resolve to.
    path_targets: Vec<NodeId>,
    /// One node of those other nodes whose name is looked up in the same
    /// blocks and loops.
    names: Vec<NodeId>,
}
