//! Captures.

use super::*;
use crate::sim::ir::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn capture_source(&self, target: NodeId) -> Option<CaptureSource> {
        if let Some(binding) = self.capture_binding(target) {
            let info = binding.local.clone();
            let name = Self::capture_local_name(binding.storage);
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
                    initial: IrExpr::new(
                        IrExprKind::ObjectQuery(Box::new(IrObjectQuery::HandleCapture(
                            IrChandleExpr::LocalRead(name.to_owned()),
                        ))),
                        1,
                        false,
                        None,
                    ),
                    lifetime: StorageLifetime::Automatic,
                    kind: StorageKind::Opaque,
                });
            }
            return None;
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
        let function = self.func.as_ref()?;
        // An automatic string local of the subroutine is captured by value
        // (or shared, see `fork_shared`) under its own local name.
        if matches!(self.kind(target), NodeKind::Var { ty } if ty.kind == "string")
            && self.db.variable_lifetime(target) == VariableLifetime::Automatic
        {
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
        let mut current = Some(node);
        while let Some(candidate) = current {
            if candidate == scope {
                return true;
            }
            current = self.node(candidate).parent;
        }
        false
    }

    /// Find automatic declarations referenced by a fork branch. Declarations
    /// inside the branch are owned by that branch and are not captures.
    /// Whether automatic variable `declaration` is shared with a `join_none`
    /// or `join_any` branch that names it outside the branch's own scope.
    /// Such a variable lives in a shared activation frame (SV 6.21, 9.3.2).
    pub(in super::super) fn fork_shared(&mut self, declaration: NodeId) -> bool {
        if self.fork_shared.is_none() {
            let mut shared = HashSet::new();
            for node in self.db.node_ids() {
                let NodeKind::Stmt(StmtKind::Fork {
                    join_kind: DbJoinKind::None | DbJoinKind::Any,
                    branches,
                    ..
                }) = self.kind(node)
                else {
                    continue;
                };
                for branch in branches {
                    if matches!(
                        self.kind(*branch),
                        NodeKind::Stmt(StmtKind::VariableDecl { .. })
                    ) {
                        continue;
                    }
                    let mut pending = vec![*branch];
                    let mut visited = HashSet::new();
                    while let Some(current) = pending.pop() {
                        if !visited.insert(current) {
                            continue;
                        }
                        if let NodeKind::Expr(ExprKind::Ref {
                            target: Some(target),
                        }) = self.kind(current)
                        {
                            if matches!(self.kind(*target), NodeKind::Var { .. })
                                && !self.node_is_within(*target, *branch)
                                && self.db.variable_lifetime(*target) == VariableLifetime::Automatic
                            {
                                shared.insert(*target);
                            }
                        }
                        pending.extend(self.node(current).children.iter().copied());
                        if let NodeKind::Stmt(statement) = self.kind(current) {
                            statement.referenced_nodes(&mut pending);
                        }
                    }
                }
            }
            self.fork_shared = Some(shared);
        }
        self.fork_shared
            .as_ref()
            .is_some_and(|shared| shared.contains(&declaration))
    }

    pub(in super::super) fn fork_capture_targets(&self, branch: NodeId) -> Vec<NodeId> {
        fn visit(
            cg: &Codegen<'_>,
            node: NodeId,
            branch: NodeId,
            visited: &mut HashSet<NodeId>,
            out: &mut HashSet<NodeId>,
        ) {
            if !visited.insert(node) {
                return;
            }
            let target = match cg.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => Some(*target),
                _ => cg
                    .lexical_proc_local(node)
                    .map(|(target, _)| target)
                    .or_else(|| cg.lexical_proc_string_local(node).map(|(target, _)| target)),
            };
            if let Some(target) = target {
                if !cg.node_is_within(target, branch)
                    && (cg
                        .capture_source(target)
                        .is_some_and(|source| source.info.static_signal.is_none())
                        || cg.proc_string_local_name(target).is_some())
                {
                    out.insert(target);
                }
            }
            for child in &cg.node(node).children {
                visit(cg, *child, branch, visited, out);
            }
            // Timing-control operands (`#n`, `@(e)`, `wait (c)`) are statement
            // fields rather than structural children.
            if let NodeKind::Stmt(statement) = cg.kind(node) {
                let mut operands = Vec::new();
                statement.referenced_nodes(&mut operands);
                for operand in operands {
                    visit(cg, operand, branch, visited, out);
                }
            }
        }

        let mut targets = HashSet::new();
        visit(self, branch, branch, &mut HashSet::new(), &mut targets);
        let mut targets = targets.into_iter().collect::<Vec<_>>();
        targets.sort_by_key(|node| node.index());
        targets
    }
}
