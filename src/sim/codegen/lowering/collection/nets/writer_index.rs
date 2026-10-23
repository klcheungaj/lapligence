//! Candidate writer nodes per net.
//!
//! The driver scans that build resolved net groups test every design node
//! against one group's members. Run once per group, that costs groups x
//! nodes, which made gate-level netlists quadratic in their instance count.
//! The index records, in one pass, every node whose written or connected
//! expressions can reach a net; a scan then visits only its group's
//! candidates, in design order, and applies the unchanged per-group checks.
//! Candidates are a superset: every target the scans' member lookups can
//! reach is recorded, so the per-group results and the first reported error
//! do not change.

use super::*;

/// Design-order positions of the nodes that may write or drive each net.
pub(super) struct NetWriterIndex {
    by_target: HashMap<NodeId, Vec<usize>>,
    /// Continuous assignments with an unresolved hierarchical target. The
    /// wired-net scan matches such a target against each net's name in the
    /// source text, so these are candidates for every net.
    every_net: Vec<usize>,
}

impl NetWriterIndex {
    /// Ascending positions of the candidate writers of any of `members`.
    pub(super) fn candidates(&self, members: &[NodeId]) -> Vec<usize> {
        let mut positions = self.every_net.clone();
        for member in members {
            if let Some(list) = self.by_target.get(member) {
                positions.extend_from_slice(list);
            }
        }
        positions.sort_unstable();
        positions.dedup();
        positions
    }
}

impl Codegen<'_> {
    /// Index the candidate writers among `nodes` (the design node order).
    pub(super) fn net_writer_index(&self, nodes: &[NodeId]) -> NetWriterIndex {
        let mut index = NetWriterIndex {
            by_target: HashMap::new(),
            every_net: Vec::new(),
        };
        let mut roots = Vec::new();
        let mut targets = Vec::new();
        for (position, id) in nodes.iter().enumerate() {
            roots.clear();
            if self.writer_roots(*id, &mut roots) {
                index.every_net.push(position);
            }
            targets.clear();
            for root in &roots {
                self.nested_write_targets(*root, &mut targets);
            }
            targets.sort_unstable_by_key(|target| target.index());
            targets.dedup();
            for target in &targets {
                index.by_target.entry(*target).or_default().push(position);
            }
        }
        index
    }

    /// The expressions of `id` that the group driver scans resolve against
    /// members: clocking sources, assignment targets, gate outputs, port
    /// connections and call actuals. Returns whether `id` is a continuous
    /// assignment whose hierarchical target is resolved only by source text.
    fn writer_roots(&self, id: NodeId, roots: &mut Vec<NodeId>) -> bool {
        if let Some(var) = self.db.clocking_var(id) {
            roots.extend(var.source);
            roots.push(var.expression);
        }
        match self.kind(id) {
            NodeKind::ContAssign { .. } => {
                let Some(lhs) = self.node(id).children.first().copied() else {
                    return false;
                };
                roots.push(lhs);
                matches!(
                    self.kind(lhs),
                    NodeKind::Expr(ExprKind::HierPath { refs, .. })
                        if refs.iter().all(Option::is_none)
                ) && self.hier_path_signal(lhs).is_none()
            }
            NodeKind::Stmt(StmtKind::Assign { .. }) => {
                roots.extend(self.node(id).children.first().copied());
                false
            }
            NodeKind::Gate { .. } => {
                roots.extend(self.gate_driven_terms(id));
                false
            }
            NodeKind::Port {
                high,
                low,
                high_expr,
                ..
            } => {
                roots.extend(self.port_child_target(id, *low));
                roots.extend(*high_expr);
                roots.extend(*high);
                false
            }
            NodeKind::FuncCall { .. } => {
                roots.extend(self.node(id).children.iter().copied());
                false
            }
            _ => false,
        }
    }

    /// Every target `nested_member_target` can find in `node` for some
    /// member set: the union over all members, not the first match.
    fn nested_write_targets(&self, node: NodeId, out: &mut Vec<NodeId>) {
        self.write_targets(node, out);
        for child in &self.node(node).children {
            self.nested_write_targets(*child, out);
        }
    }

    /// Every target `member_write_base` can return for `node` for some
    /// member set.
    fn write_targets(&self, node: NodeId, out: &mut Vec<NodeId>) {
        if let Some(expression) = self.modport_expression_target(node) {
            self.nested_write_targets(expression, out);
            return;
        }
        match self.kind(node) {
            NodeKind::Net { .. } => out.push(node),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => {
                out.push(*target);
                self.write_targets(*target, out);
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(nodes) = self
                    .hier_path_signal(node)
                    .and_then(|signal| self.sig_global_by_ir.get(&signal.ir))
                {
                    out.extend(nodes.values().copied());
                }
            }
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.write_targets(*base, out),
            _ => {}
        }
    }
}
