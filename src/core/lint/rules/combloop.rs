//! `combinational-loop` — combinational feedback through comb processes.
//!
//! Builds a signal dependency graph from `always_comb` / `always @*`
//! processes and continuous assignments: an edge read → write means a change
//! of the read signal can propagate to the written signal within the same
//! time step.  Strongly connected components (SCCs) of size ≥ 2, and
//! self-loops (a signal read and written by the same comb process), are
//! combinational loops.  Processes containing a delay or explicit edge
//! control break combinationality and are excluded.
//!
//! A process's inputs are its *exposed* reads: a variable the process reads
//! before it has definitely written it on every path. A value the process
//! writes and then reads back (`q = a; o = q;`) is not an input, while
//! `a = a ^ 1` still is. Branches define a variable only when every arm
//! does; loop bodies and other statements define nothing after them.
//! Variables declared inside a process (block locals, `for` loop variables)
//! are process state, not signals, and never form edges. Every `llg` run
//! stops on this rule's errors, so these exclusions keep ordinary
//! combinational code from being reported.

use std::collections::{HashMap, HashSet};

use crate::core::db::{Db, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::{
    all_nodes, collect_reads, collect_writes, has_timing_control, is_comb_process,
    process_local_vars,
};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Flags combinational feedback loops.
pub struct CombinationalLoopRule;

impl LintRule for CombinationalLoopRule {
    fn id(&self) -> &'static str {
        "combinational-loop"
    }

    fn description(&self) -> &'static str {
        "flags combinational feedback loops through always_comb / @* processes and continuous assigns"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut edges: Vec<(NodeId, NodeId)> = Vec::new();
        for id in all_nodes(db) {
            let comb = match db.node_kind(id) {
                NodeKind::Process { .. } => is_comb_process(db, id) && !has_timing_control(db, id),
                NodeKind::ContAssign { .. } => true,
                _ => continue,
            };
            if !comb {
                continue;
            }
            let is_process = matches!(db.node_kind(id), NodeKind::Process { .. });
            let mut writes = collect_writes(db, id);
            let mut reads = if is_process {
                let mut exposed = Vec::new();
                let mut defined = HashSet::new();
                for child in &db.node(id).children {
                    exposed_reads(db, *child, &mut defined, &mut exposed);
                }
                exposed
            } else {
                collect_reads(db, id)
            };
            if is_process {
                let local_vars = process_local_vars(db, id);
                let local = |signal: &NodeId| local_vars.contains(signal);
                writes.retain(|signal| !local(signal));
                reads.retain(|signal| !local(signal));
            }
            if reads.is_empty() || writes.is_empty() {
                continue;
            }
            for r in &reads {
                for w in &writes {
                    edges.push((*r, *w));
                }
            }
        }
        edges.sort_unstable_by_key(|(r, w)| (r.0, w.0));
        edges.dedup();

        let mut node_set: HashSet<NodeId> = HashSet::new();
        for (r, w) in &edges {
            node_set.insert(*r);
            node_set.insert(*w);
        }
        let mut nodes: Vec<NodeId> = node_set.into_iter().collect();
        nodes.sort_by_key(|n| n.0);

        let mut out = Vec::new();
        for scc in strongly_connected_components(&edges, &nodes) {
            let is_loop = scc.len() >= 2
                || (scc.len() == 1 && edges.iter().any(|(r, w)| *r == scc[0] && *w == scc[0]));
            if !is_loop {
                continue;
            }
            let mut members = scc.clone();
            members.sort_by_key(|m| m.0);
            let first = members[0];
            let names: Vec<String> = members.iter().map(|m| db.node(*m).name.clone()).collect();
            let listed = names
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<String>>()
                .join(", ");
            let node = db.node(first);
            out.push(LintDiag {
                rule: "combinational-loop".to_string(),
                logical: None,
                severity: LintSeverity::Error,
                file: node.file().map(str::to_owned),
                line: node.line,
                col: node.col,
                message: format!("combinational loop involving signals: {listed}"),
            });
        }
        out
    }
}

/// Append to `out` the signals `node` reads before they are in `defined`
/// (definitely written earlier on every path), then add the signals `node`
/// definitely writes to `defined`.
fn exposed_reads(db: &Db, node: NodeId, defined: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    let expose = |root: NodeId, defined: &HashSet<NodeId>, out: &mut Vec<NodeId>| {
        for signal in collect_reads(db, root) {
            if !defined.contains(&signal) && !out.contains(&signal) {
                out.push(signal);
            }
        }
    };
    let children = &db.node(node).children;
    match db.node_kind(node) {
        NodeKind::Stmt(StmtKind::Begin) => {
            for child in children {
                exposed_reads(db, *child, defined, out);
            }
        }
        NodeKind::Stmt(StmtKind::EventControl {
            implicit: true,
            body: Some(body),
            ..
        }) => exposed_reads(db, *body, defined, out),
        NodeKind::Stmt(StmtKind::Assign { .. }) => {
            expose(node, defined, out);
            defined.extend(collect_writes(db, node));
        }
        NodeKind::Stmt(StmtKind::IfElse {
            if_true, if_false, ..
        }) => {
            for child in children {
                if *child != *if_true && Some(*child) != *if_false {
                    expose(*child, defined, out);
                }
            }
            let mut taken = defined.clone();
            exposed_reads(db, *if_true, &mut taken, out);
            let mut other = defined.clone();
            if let Some(if_false) = if_false {
                exposed_reads(db, *if_false, &mut other, out);
            }
            *defined = taken.intersection(&other).copied().collect();
        }
        NodeKind::Stmt(StmtKind::Case { items, .. }) => {
            let bodies: Vec<NodeId> = items.iter().filter_map(|item| item.body).collect();
            for child in children {
                if !bodies.contains(child) {
                    expose(*child, defined, out);
                }
            }
            // Without a default item no arm may run, keeping `defined` as is.
            let mut merged: Option<HashSet<NodeId>> = items
                .iter()
                .all(|item| !item.exprs.is_empty())
                .then(|| defined.clone());
            for item in items {
                let mut arm = defined.clone();
                if let Some(body) = item.body {
                    exposed_reads(db, body, &mut arm, out);
                }
                merged = Some(match merged {
                    Some(previous) => previous.intersection(&arm).copied().collect(),
                    None => arm,
                });
            }
            if let Some(merged) = merged {
                *defined = merged;
            }
        }
        _ => expose(node, defined, out),
    }
}

/// SCCs of the graph given by `edges` (read → write), over `nodes`, via
/// Kosaraju's algorithm with iterative DFS (no recursion-depth limits).
fn strongly_connected_components(edges: &[(NodeId, NodeId)], nodes: &[NodeId]) -> Vec<Vec<NodeId>> {
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut rev: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for (r, w) in edges {
        adj.entry(*r).or_default().push(*w);
        rev.entry(*w).or_default().push(*r);
    }
    for ns in adj.values_mut() {
        ns.sort_by_key(|n| n.0);
        ns.dedup();
    }
    for ns in rev.values_mut() {
        ns.sort_by_key(|n| n.0);
        ns.dedup();
    }

    // First pass: finish order on the forward graph.
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut order: Vec<NodeId> = Vec::new();
    for &start in nodes {
        if !visited.insert(start) {
            continue;
        }
        let mut stack: Vec<(NodeId, Vec<NodeId>)> =
            vec![(start, adj.get(&start).cloned().unwrap_or_default())];
        while let Some((node, neighbors)) = stack.last_mut() {
            match neighbors.pop() {
                Some(next) => {
                    if visited.insert(next) {
                        stack.push((next, adj.get(&next).cloned().unwrap_or_default()));
                    }
                }
                None => {
                    let done = *node;
                    stack.pop();
                    order.push(done);
                }
            }
        }
    }

    // Second pass: reverse finish order on the reversed graph.
    let mut visited2: HashSet<NodeId> = HashSet::new();
    let mut sccs = Vec::new();
    for &start in order.iter().rev() {
        if !visited2.insert(start) {
            continue;
        }
        let mut scc = Vec::new();
        let mut stack = vec![start];
        while let Some(n) = stack.pop() {
            scc.push(n);
            if let Some(ns) = rev.get(&n) {
                for m in ns {
                    if visited2.insert(*m) {
                        stack.push(*m);
                    }
                }
            }
        }
        sccs.push(scc);
    }
    sccs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn three_signal_cycle_is_reported_once() {
        let diags = lint_design(
            "module cl;\n\
             \x20 logic a, b, c, d;\n\
             \x20 always_comb begin a = b; end\n\
             \x20 always_comb begin b = c; end\n\
             \x20 always_comb begin c = a ^ d; end\n\
             endmodule\n",
            "cl",
        );
        let got = rule_diags(&diags, "combinational-loop");
        assert_eq!(got.len(), 1, "one loop finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Error);
        assert!(d.message.contains("a"), "lists a: {}", d.message);
        assert!(d.message.contains("b"), "lists b: {}", d.message);
        assert!(d.message.contains("c"), "lists c: {}", d.message);
        assert!(
            !d.message.contains("d"),
            "d is not in the loop: {}",
            d.message
        );
    }

    #[test]
    fn self_loop_is_reported() {
        let diags = lint_design(
            "module sl;\n  logic a;\n  always_comb begin a = a ^ 1'b1; end\nendmodule\n",
            "sl",
        );
        let got = rule_diags(&diags, "combinational-loop");
        assert_eq!(got.len(), 1, "one self-loop finding: {:?}", diags);
        assert!(got[0].message.contains("a"));
    }

    /// Values written before they are read back, process locals and loop
    /// variables are not feedback; reading before writing still is.
    #[test]
    fn written_before_read_values_and_locals_are_not_loops() {
        let diags = lint_design(
            "module wb;\n\
             \x20 logic a, s;\n\
             \x20 logic [3:0] q, o, p, r, t, u;\n\
             \x20 logic [3:0] arr [4];\n\
             \x20 always_comb begin q = {3'b0, a}; o = q; end\n\
             \x20 always_comb begin if (s) p = 4'd1; else p = 4'd2; r = p; end\n\
             \x20 always_comb begin case (s) 1'b0: t = 4'd1; default: t = 4'd2; endcase u = t; end\n\
             \x20 always @* begin for (int k = 0; k < 4; k++) arr[k] = {3'b0, a}; end\n\
             \x20 always_comb begin logic [3:0] tmp; tmp = q; tmp = tmp + 4'd1; end\n\
             endmodule\n",
            "wb",
        );
        let got = rule_diags(&diags, "combinational-loop");
        assert!(got.is_empty(), "no loop: {got:?}");
    }

    /// A one-armed `if` does not define its target on every path, so a later
    /// read of the previous value is real feedback.
    #[test]
    fn read_before_definite_write_is_still_a_loop() {
        let diags = lint_design(
            "module rb;\n  logic s;\n  logic [3:0] p, r;\n\
             \x20 always_comb begin if (s) p = 4'd1; r = p; p = r + 4'd1; end\nendmodule\n",
            "rb",
        );
        let got = rule_diags(&diags, "combinational-loop");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert!(got[0].message.contains("`p`"), "{}", got[0].message);
    }

    #[test]
    fn acyclic_design_is_quiet() {
        let diags = lint_design(
            "module ac;\n\
             \x20 logic a, b, d;\n\
             \x20 always_comb begin a = b; end\n\
             \x20 always_comb begin b = d; end\n\
             endmodule\n",
            "ac",
        );
        let got = rule_diags(&diags, "combinational-loop");
        assert!(got.is_empty(), "no loop: {:?}", diags);
    }

    #[test]
    fn edge_triggered_processes_are_not_combinational() {
        let diags = lint_design(
            "module et;\n  logic clk, a, b;\n\
             \x20 always @(posedge clk) a <= b;\n\
             \x20 always @(posedge clk) b <= a;\n\
             endmodule\n",
            "et",
        );
        let got = rule_diags(&diags, "combinational-loop");
        assert!(
            got.is_empty(),
            "no comb loop through registers: {:?}",
            diags
        );
    }
}
