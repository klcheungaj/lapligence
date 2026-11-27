//! The order in which final procedures run (decision S09-D12).
//!
//! 9.2.3 leaves the order to the implementation; the rule is the S09-D12 row
//! of `docs/lrm_decisions.md`. One walk of the active instance tree per kind
//! of instance (interface, module, program) ranks the final procedures; the
//! lowered spawns are then sorted by rank. Process emission order is not
//! touched, and the walk stays linear in the instance count.

use super::*;

/// Which pass of the final order an instance's procedures belong to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FinalPhase {
    Interface,
    Module,
    Program,
}

impl Codegen<'_> {
    /// The lowered final procedures (`(process node, function name)` in
    /// emission order) in the order they run. A final procedure the walk does
    /// not reach is an internal error, never a silently dropped spawn.
    pub(in super::super) fn order_final_procedures(
        &self,
        tops: &[NodeId],
        lowered: Vec<(NodeId, String)>,
    ) -> Result<Vec<String>, String> {
        if lowered.is_empty() {
            return Ok(Vec::new());
        }
        let mut ranks = HashMap::new();
        for phase in [
            FinalPhase::Interface,
            FinalPhase::Module,
            FinalPhase::Program,
        ] {
            for &top in tops {
                self.rank_finals(top, phase, &mut ranks);
            }
        }
        let mut ranked = lowered
            .into_iter()
            .map(|(process, name)| match ranks.get(&process) {
                Some(&rank) => Ok((rank, name)),
                None => Err(format!(
                    "internal: final procedure `{name}` is outside the instance tree"
                )),
            })
            .collect::<Result<Vec<_>, String>>()?;
        ranked.sort_by_key(|(rank, _)| *rank);
        Ok(ranked.into_iter().map(|(_, name)| name).collect())
    }

    fn final_phase(&self, inst: NodeId) -> FinalPhase {
        if self.db.is_program_instance(inst) {
            FinalPhase::Program
        } else if matches!(
            self.kind(inst),
            NodeKind::ModuleInst {
                is_interface: true,
                ..
            }
        ) {
            FinalPhase::Interface
        } else {
            FinalPhase::Module
        }
    }

    fn rank_finals(&self, inst: NodeId, phase: FinalPhase, ranks: &mut HashMap<NodeId, usize>) {
        if self.final_phase(inst) == phase {
            self.rank_scope_finals(inst, ranks);
        }
        for child in self.final_child_order(inst) {
            self.rank_finals(child, phase, ranks);
        }
    }

    /// The scope's own final procedures, then those of its generate blocks.
    fn rank_scope_finals(&self, scope: NodeId, ranks: &mut HashMap<NodeId, usize>) {
        for &child in &self.node(scope).children {
            if matches!(
                self.kind(child),
                NodeKind::Process {
                    kind: ProcessKind::Final
                }
            ) {
                let next = ranks.len();
                ranks.insert(child, next);
            }
        }
        for block in self.generate_blocks(scope) {
            self.rank_scope_finals(block, ranks);
        }
    }

    /// The elaborated generate blocks directly inside `scope`, in source
    /// order, loop iterations in index order.
    fn generate_blocks(&self, scope: NodeId) -> Vec<NodeId> {
        let mut blocks = Vec::new();
        for &child in &self.node(scope).children {
            match self.kind(child) {
                NodeKind::GenScope => blocks.push(child),
                NodeKind::GenScopeArray => blocks.extend(
                    self.node(child)
                        .children
                        .iter()
                        .copied()
                        .filter(|&block| matches!(self.kind(block), NodeKind::GenScope)),
                ),
                _ => {}
            }
        }
        blocks
    }

    /// The instances written in `scope`, then those of its generate blocks
    /// (a block's own before those of its nested blocks).
    fn scope_instances(&self, scope: NodeId, instances: &mut Vec<NodeId>) {
        instances.extend(
            self.node(scope)
                .children
                .iter()
                .copied()
                .filter(|&child| matches!(self.kind(child), NodeKind::ModuleInst { .. })),
        );
        for block in self.generate_blocks(scope) {
            self.scope_instances(block, instances);
        }
    }

    fn contains_instance(&self, scope: NodeId) -> bool {
        self.node(scope).children.iter().any(|&child| {
            matches!(self.kind(child), NodeKind::ModuleInst { .. })
                || (matches!(
                    self.kind(child),
                    NodeKind::GenScope | NodeKind::GenScopeArray
                ) && self.contains_instance(child))
        })
    }

    /// The instances inside `inst` in the order their final procedures run:
    /// those containing no instance first, each part grouped by definition in
    /// first-appearance order.
    fn final_child_order(&self, inst: NodeId) -> Vec<NodeId> {
        let mut instances = Vec::new();
        self.scope_instances(inst, &mut instances);
        let mut groups: HashMap<(bool, &str), usize> = HashMap::new();
        let mut keyed = instances
            .into_iter()
            .map(|child| {
                let group = (
                    self.contains_instance(child),
                    match self.kind(child) {
                        NodeKind::ModuleInst { def_name, .. } => def_name.as_str(),
                        _ => "",
                    },
                );
                let next = groups.len();
                let index = *groups.entry(group).or_insert(next);
                ((group.0, index), child)
            })
            .collect::<Vec<_>>();
        // Stable: instances of one group keep their order.
        keyed.sort_by_key(|(key, _)| *key);
        keyed.into_iter().map(|(_, child)| child).collect()
    }
}
