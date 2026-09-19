//! Captures.

use super::*;
use crate::sim::ir::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn capture_source(&self, target: NodeId) -> Option<CaptureSource> {
        if let Some(binding) = self.capture_binding(target) {
            let info = binding.local.clone();
            let initial = IrExpr::new(
                IrExprKind::LocalRead(Self::capture_local_name(binding.storage)),
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
        let function = self.func.as_ref()?;
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
    pub(in super::super) fn fork_capture_targets(&self, branch: NodeId) -> Vec<NodeId> {
        fn visit(cg: &Codegen<'_>, node: NodeId, branch: NodeId, out: &mut HashSet<NodeId>) {
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
                    && (cg.capture_source(target).is_some()
                        || cg.proc_string_local_name(target).is_some())
                {
                    out.insert(target);
                }
            }
            for child in &cg.node(node).children {
                visit(cg, *child, branch, out);
            }
        }

        let mut targets = HashSet::new();
        visit(self, branch, branch, &mut targets);
        let mut targets = targets.into_iter().collect::<Vec<_>>();
        targets.sort_by_key(|node| node.index());
        targets
    }
}
