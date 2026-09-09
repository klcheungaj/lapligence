//! `mixed-assignments` — blocking and nonblocking assignments in one process.
//!
//! A process whose statement body contains BOTH blocking (`=`) and
//! non-blocking (`<=`) assignments mixes two scheduling semantics in one
//! place: the blocking update lands immediately while the non-blocking one
//! is deferred to the NBA region, which makes read-back values order-
//! dependent, hides races between processes sampling the same signals, and
//! defeats both synthesis-friendly coding styles.  One Error finding is
//! emitted per offending process, positioned at the process keyword (the
//! process node) rather than at an individual assignment — the defect is the
//! coexistence, not either assignment alone.
//!
//! Overlap policy: `blocking-in-always_ff` / edge-sensitive-already flags
//! each blocking assignment inside clocked processes, and `nba-in-always_comb`
//! flags each nonblocking assignment inside combinational processes; this
//! rule flags the MIXING itself for every block kind (always/initial/final,
//! any sensitivity), including plain level-sensitive `always` and
//! `initial`/`final` blocks where neither of the other two rules ever fires.
//! On clocked/combinational processes findings may legitimately overlap with
//! those rules (kind-mismatch vs mixing are different diagnoses); no
//! suppression is applied.
//!
//! Only [`StmtKind::Assign`] counts as an assignment flavour: procedural
//! continuous assignments (`assign`/`deassign`), `force`/`release` and
//! declaration initializers are different constructs and are ignored.
//! Nested begin/if/case/loop/fork bodies stay part of their enclosing
//! process; function/task bodies are separate nodes outside the process tree
//! and are not reached.

use crate::core::db::{Db, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::{all_nodes, scope_path};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Errors on processes mixing blocking and non-blocking assignments.
pub struct MixedAssignRule;

impl LintRule for MixedAssignRule {
    fn id(&self) -> &'static str {
        "mixed-assignments"
    }

    fn description(&self) -> &'static str {
        "errors on a process whose body mixes blocking (=) and non-blocking (<=) assignments"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            if !matches!(db.node_kind(id), NodeKind::Process { .. }) {
                continue;
            }
            let mut kinds = AssignKinds::default();
            collect_assign_kinds(db, id, &mut kinds);
            if !(kinds.blocking && kinds.nonblocking) {
                continue;
            }
            let node = db.node(id);
            let path = node.parent.map(|p| scope_path(db, p)).unwrap_or_default();
            out.push(LintDiag {
                rule: "mixed-assignments".to_string(),
                severity: LintSeverity::Error,
                file: node.file.clone(),
                line: node.line,
                col: node.col,
                message: format!(
                    "process `{path}` mixes blocking (`=`) and non-blocking (`<=`) assignments"
                ),
            });
        }
        out
    }
}

/// Which assignment flavours occur in one process body.
#[derive(Default)]
struct AssignKinds {
    blocking: bool,
    nonblocking: bool,
}

/// Collect the assignment flavours of the statement tree rooted at `root`,
/// depth-first.
fn collect_assign_kinds(db: &Db, root: NodeId, out: &mut AssignKinds) {
    if let NodeKind::Stmt(StmtKind::Assign { blocking, .. }) = db.node_kind(root) {
        if *blocking {
            out.blocking = true;
        } else {
            out.nonblocking = true;
        }
    }
    for c in &db.node(root).children {
        collect_assign_kinds(db, *c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{build_design, lint_design, rule_diags};
    use crate::core::lint::{lint_with_config, LintConfig, RuleConfig};

    #[test]
    fn mixed_always_ff_is_an_error() {
        let diags = lint_design(
            "module ff;\n  logic clk, d, q;\n  logic d2, q2;\n\
             always_ff @(posedge clk) begin\n    q <= d;\n    q2 = d2;\n  end\nendmodule\n",
            "ff",
        );
        let got = rule_diags(&diags, "mixed-assignments");
        assert_eq!(got.len(), 1, "one finding per process: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Error);
        assert!(
            d.message.contains("process `ff` mixes blocking"),
            "{}",
            d.message
        );
        // Positioned at the process keyword line (process nodes are anchored
        // at column 1 of their source line).
        assert_eq!(d.line, 4, "{diags:?}");
        assert_eq!(d.col, 1, "{diags:?}");
    }

    #[test]
    fn pure_nonblocking_process_is_quiet() {
        let diags = lint_design(
            "module ff2;\n  logic clk, d, q;\n  always_ff @(posedge clk) q <= d;\nendmodule\n",
            "ff2",
        );
        assert!(
            rule_diags(&diags, "mixed-assignments").is_empty(),
            "{diags:?}"
        );
    }

    /// Blocking-only in an edge-sensitive always belongs to
    /// `blocking-in-always_ff`; without mixing this rule stays quiet.
    #[test]
    fn pure_blocking_process_is_quiet() {
        let diags = lint_design(
            "module ff3;\n  logic clk, d, q;\n  always @(posedge clk) q = d;\nendmodule\n",
            "ff3",
        );
        assert!(
            rule_diags(&diags, "mixed-assignments").is_empty(),
            "{diags:?}"
        );
    }

    #[test]
    fn mixed_initial_block_is_reported() {
        let diags = lint_design(
            "module it;\n  logic a, b;\n  initial begin\n    a = 1'b0;\n    b <= 1'b1;\n  end\nendmodule\n",
            "it",
        );
        let got = rule_diags(&diags, "mixed-assignments");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert!(
            got[0].message.contains("process `it`"),
            "{}",
            got[0].message
        );
        assert_eq!(got[0].line, 3, "{diags:?}");
    }

    /// Assignments nested in begin/if bodies stay part of the same process.
    #[test]
    fn mixed_through_nested_blocks_is_one_finding() {
        let diags = lint_design(
            "module nb;\n  logic clk, s, x, y;\n  always @(posedge clk) begin\n\
             \x20   if (s) begin\n\x20     x <= 1'b0;\n\x20   end else begin\n\
             \x20     y = x;\n\x20   end\n\x20 end\nendmodule\n",
            "nb",
        );
        let got = rule_diags(&diags, "mixed-assignments");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].line, 3, "{diags:?}");
    }

    /// Case arms and fork branches are part of their enclosing process too:
    /// mixing across those bodies is still ONE finding for the ONE process.
    #[test]
    fn mixing_across_case_and_fork_bodies_is_one_finding() {
        let diags = lint_design(
            "module cf;\n  logic clk, s, x, y;\n  always @(posedge clk) begin\n\
             \x20   case (s)\n\x20     1'b0: x <= 1'b0;\n\
             \x20     default: begin\n\x20       fork\n\x20         y = x;\n\
             \x20       join\n\x20     end\n\x20   endcase\n\x20 end\nendmodule\n",
            "cf",
        );
        let got = rule_diags(&diags, "mixed-assignments");
        assert_eq!(
            got.len(),
            1,
            "one process, one finding despite case/fork nesting: {diags:?}"
        );
        assert_eq!(got[0].line, 3, "{diags:?}");
    }

    /// Function/task bodies are separate nodes OUTSIDE the process tree: a
    /// blocking assignment inside a task called from an otherwise-pure NBA
    /// process must not be charged to that process.
    #[test]
    fn task_body_assignments_do_not_count() {
        let diags = lint_design(
            "module tk;\n  logic clk, d, q;\n  task t(input logic v);\n    q = v;\n  endtask\n\
             always_ff @(posedge clk) begin\n    q <= d;\n    t(d);\n  end\nendmodule\n",
            "tk",
        );
        assert!(
            rule_diags(&diags, "mixed-assignments").is_empty(),
            "the task's blocking write belongs to the task, not the caller: {diags:?}"
        );
    }

    /// Mixing is judged per process: separate pure processes never interact.
    #[test]
    fn separate_pure_processes_are_quiet() {
        let diags = lint_design(
            "module sp;\n  logic clk, d, q;\n  logic e;\n\
             always_ff @(posedge clk) q <= d;\n\
             always_comb e = ~q;\nendmodule\n",
            "sp",
        );
        assert!(
            rule_diags(&diags, "mixed-assignments").is_empty(),
            "{diags:?}"
        );
    }

    #[test]
    fn mixed_comb_block_overlaps_with_nba_in_comb() {
        // Documented overlap: nba-in-always_comb fires per assignment, this
        // rule once for the mixing itself.
        let diags = lint_design(
            "module cb;\n  logic x, y;\n  always_comb begin\n    x = y;\n    y <= x;\n  end\nendmodule\n",
            "cb",
        );
        assert_eq!(
            rule_diags(&diags, "mixed-assignments").len(),
            1,
            "{diags:?}"
        );
        assert_eq!(
            rule_diags(&diags, "nba-in-always_comb").len(),
            1,
            "{diags:?}"
        );
    }

    #[test]
    fn config_disables_the_rule() {
        let sv = "module ff;\n  logic clk, d, q;\n  logic d2, q2;\n\
                  always_ff @(posedge clk) begin\n    q <= d;\n    q2 = d2;\n  end\nendmodule\n";
        let diags = lint_design(sv, "ff");
        assert_eq!(
            rule_diags(&diags, "mixed-assignments").len(),
            1,
            "sanity: enabled by default: {diags:?}"
        );

        let mut cfg_from_toml = LintConfig::new();
        cfg_from_toml
            .parse_toml("[rules.mixed-assignments]\nenabled = false\n")
            .expect("registered ids parse without an unknown-rule error");
        let (db, model) = build_design(sv, "ff");
        let filtered = lint_with_config(&db, &model, &cfg_from_toml);
        assert!(
            rule_diags(&filtered, "mixed-assignments").is_empty(),
            "{filtered:?}"
        );
    }

    #[test]
    fn config_severity_override_applies() {
        let sv = "module ff;\n  logic clk, d, q;\n  logic d2, q2;\n\
                  always_ff @(posedge clk) begin\n    q <= d;\n    q2 = d2;\n  end\nendmodule\n";
        let (db, model) = build_design(sv, "ff");
        let mut cfg = LintConfig::new();
        cfg.set(
            "mixed-assignments",
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Warning),
            },
        );
        let diags = lint_with_config(&db, &model, &cfg);
        let got = rule_diags(&diags, "mixed-assignments");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].severity, LintSeverity::Warning);
    }
}
