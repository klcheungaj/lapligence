//! Procedural checker instances (IEEE 1800-2009 17.3.1).
//!
//! "All contents of a checker instance other than static assertion
//! statements are considered to exist during every time step, regardless of
//! whether the checker is static or procedural", so the hierarchy walk
//! lowers a procedural instance's variables, procedures and procedural
//! assertions like a static one. Its static concurrent assertions, and those
//! of the static checker instances nested in it, are instead "added to the
//! pending procedural assertion queue for their process when the checker
//! instantiation is reached in process execution": the hierarchy walk skips
//! them, and the instantiation statement registers each one as a procedural
//! concurrent assertion of the enclosing procedure (16.15.6) and queues one
//! instance of each whenever it executes.

use super::*;
use crate::core::db::ConcurrentAssertionKind;

impl Codegen<'_> {
    /// Whether the static concurrent assertions of checker instance `inst`
    /// are queued by a procedural instantiation instead of monitored: `inst`
    /// is procedural, or statically nested in a procedural instance.
    pub(in crate::sim::codegen) fn queued_by_procedural_checker(&self, inst: NodeId) -> bool {
        let mut current = Some(inst);
        while let Some(checker) = current {
            let Some(info) = self.db.checker_instance(checker) else {
                return false;
            };
            if info.procedural {
                return true;
            }
            if !info.nested {
                return false;
            }
            current = self.owning_checker(checker);
        }
        false
    }

    /// The body of the procedure whose code instantiates procedural checker
    /// instance `checker`.
    pub(in crate::sim::codegen) fn procedural_checker_body(
        &self,
        checker: NodeId,
    ) -> Option<NodeId> {
        self.procedural_checker_bodies.get(&checker).copied()
    }

    /// Record the procedure body around each procedural checker statement,
    /// before any clock of a checker it instantiates is resolved.
    pub(in crate::sim::codegen) fn collect_procedural_checker_bodies(&mut self) {
        for statement in self.db.node_ids() {
            let NodeKind::Stmt(StmtKind::ProceduralChecker { instances }) = self.kind(statement)
            else {
                continue;
            };
            let mut current = self.node(statement).parent;
            while let Some(node) = current {
                if matches!(self.kind(node), NodeKind::Process { .. }) {
                    break;
                }
                current = self.node(node).parent;
            }
            let Some(body) =
                current.and_then(|process| self.node(process).children.first().copied())
            else {
                continue;
            };
            for &instance in instances {
                self.procedural_checker_bodies.insert(instance, body);
            }
        }
    }

    /// Lower a procedural checker statement: queue one instance of every
    /// static concurrent assertion of each checker instance it names. The
    /// first lowering registers them as procedural assertions of the
    /// enclosing procedure, whose inferred clock (16.15.6) clocks those
    /// without a clock of their own.
    pub(in crate::sim::codegen) fn lower_procedural_checkers(
        &mut self,
        instances: &[NodeId],
    ) -> Result<Vec<IrStmt>, String> {
        let mut queue = Vec::new();
        for &instance in instances {
            let indices = match self.procedural_checker_queues.get(&instance) {
                Some(indices) => indices.clone(),
                None => {
                    let saved = self.inst;
                    let mut indices = Vec::new();
                    let path = self.instance_path_of(instance);
                    let result = self.register_checker_static_assertions(
                        instance,
                        instance,
                        &path,
                        &mut indices,
                    );
                    self.inst = saved;
                    result?;
                    self.procedural_checker_queues
                        .insert(instance, indices.clone());
                    indices
                }
            };
            queue.extend(
                indices
                    .into_iter()
                    .map(|index| IrStmt::QueueProceduralAssertion { index }),
            );
        }
        Ok(queue)
    }

    /// Register one static assertion of a procedural checker as a procedural
    /// assertion of the instantiating procedure.
    fn queue_checker_assertion(
        &mut self,
        inst: NodeId,
        path: &str,
        assertion: NodeId,
        indices: &mut Vec<u32>,
    ) -> Result<(), String> {
        match self.emit_procedural_concurrent_assertion(inst, path, assertion)? {
            IrStmt::QueueProceduralAssertion { index } => {
                indices.push(index);
                Ok(())
            }
            _ => Err("procedural checker assertion was not queued".to_owned()),
        }
    }

    /// Register the static concurrent assertions of `scope` (a checker
    /// instance or one of its generate scopes) and of its nested static
    /// checker instances, in source order.
    fn register_checker_static_assertions(
        &mut self,
        inst: NodeId,
        scope: NodeId,
        path: &str,
        indices: &mut Vec<u32>,
    ) -> Result<(), String> {
        for child in self.node(scope).children.clone() {
            match self.kind(child) {
                // `restrict property` creates no assertion (16.15.4).
                NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                    kind: ConcurrentAssertionKind::Restrict,
                    ..
                }) => {}
                NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. }) => {
                    self.queue_checker_assertion(inst, path, child, indices)?;
                }
                NodeKind::Process { .. } => {
                    for assertion in self.assertion_member_assertions(child).unwrap_or_default() {
                        if !matches!(
                            self.kind(assertion),
                            NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                                kind: ConcurrentAssertionKind::Restrict,
                                ..
                            })
                        ) {
                            self.queue_checker_assertion(inst, path, assertion, indices)?;
                        }
                    }
                }
                NodeKind::GenScope => {
                    let gs_path = self
                        .gen_scope_paths
                        .get(&child)
                        .cloned()
                        .unwrap_or_else(|| path.to_owned());
                    self.register_checker_static_assertions(inst, child, &gs_path, indices)?;
                }
                NodeKind::GenScopeArray => {
                    for nested in self.node(child).children.clone() {
                        if matches!(self.kind(nested), NodeKind::GenScope) {
                            let gs_path = self
                                .gen_scope_paths
                                .get(&nested)
                                .cloned()
                                .unwrap_or_else(|| path.to_owned());
                            self.register_checker_static_assertions(
                                inst, nested, &gs_path, indices,
                            )?;
                        }
                    }
                }
                NodeKind::ModuleInst { .. } if self.db.checker_instance(child).is_some() => {
                    let child_path = self.instance_path_of(child);
                    self.register_checker_static_assertions(child, child, &child_path, indices)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
