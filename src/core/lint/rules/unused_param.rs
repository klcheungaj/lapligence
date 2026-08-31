//! `unused-parameter` — parameters that are never read.
//!
//! Per instance (module instance or gen scope), every non-local parameter is
//! checked against every `Ref` target in the design: a parameter is used when
//! an expression resolves to it.  Reads anywhere in the design count — the
//! per-instance elaboration re-binds every ref to the instance's own
//! parameter node, so a read in any process, continuous assignment, or
//! function/task body of the design is the use of exactly that instance's
//! parameter.  Localparams are skipped (a localparam's only legal "use" is
//! its own default value, which elaboration folds away).
//!
//! v1 limitation: Surelog v1.86 folds constant uses of parameters during
//! elaboration, so parameters used only in ranges (`logic [W-1:0] x`), in
//! other parameters' defaults (`localparam X = UNUSED + 1`), in generate
//! conditions, or in fully-constant continuous assigns leave no `Ref` node in
//! the db and are reported as unused even when the source reads them.  Only
//! reads that survive elaboration as expression refs (process bodies,
//! non-constant continuous assigns) are detected.
//!
//! Type parameters (`parameter type T = …`) are captured as ordinary `Param`
//! nodes with the default type's `TypeInfo` and cannot be distinguished, so
//! they are checked like any other parameter.  Flat (never-instantiated)
//! module definitions carry no captured children in the db, so their
//! parameters are not checked in v1.

use std::collections::HashSet;

use crate::core::db::{ExprKind, NodeId, NodeKind};
use crate::core::lint::rules::analysis::{all_nodes, iter_instances};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Flags parameters that are never read anywhere in the design.
pub struct UnusedParameterRule;

impl LintRule for UnusedParameterRule {
    fn id(&self) -> &'static str {
        "unused-parameter"
    }

    fn description(&self) -> &'static str {
        "flags parameters that are never read"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let read: HashSet<NodeId> = all_nodes(db)
            .iter()
            .filter_map(|id| match db.node_kind(*id) {
                NodeKind::Expr(ExprKind::Ref { target }) => *target,
                _ => None,
            })
            .filter(|t| matches!(db.node_kind(*t), NodeKind::Param { .. }))
            .collect();
        let mut out = Vec::new();
        for (inst, path) in iter_instances(db) {
            for c in &db.node(inst).children {
                let NodeKind::Param { local, .. } = db.node_kind(*c) else {
                    continue;
                };
                if *local || read.contains(c) {
                    continue;
                }
                let node = db.node(*c);
                out.push(LintDiag {
                    rule: "unused-parameter".to_string(),
                    severity: LintSeverity::Info,
                    file: node.file.clone(),
                    line: node.line,
                    col: node.col,
                    message: format!("parameter `{}` in `{}` is never read", node.name, path),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn unreferenced_parameter_is_reported() {
        let diags = lint_design(
            "module up;\n  parameter W = 8;\n  parameter UNUSED = 3;\n  logic x;\n\
             \x20 always_comb x = W;\nendmodule\n",
            "up",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Info);
        assert!(d.message.contains("`UNUSED`"), "message: {}", d.message);
        assert!(d.message.contains("`up`"), "message: {}", d.message);
        assert!(
            d.message.contains("is never read"),
            "message: {}",
            d.message
        );
        assert_eq!(d.line, 3, "declaration line of UNUSED");
    }

    #[test]
    fn param_read_in_process_is_quiet() {
        let diags = lint_design(
            "module up2;\n  parameter W = 8;\n  logic x;\n  always_comb x = W;\nendmodule\n",
            "up2",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert!(got.is_empty(), "W is read: {:?}", diags);
    }

    #[test]
    fn localparam_is_skipped() {
        let diags = lint_design("module up3;\n  localparam X = 3;\nendmodule\n", "up3");
        let got = rule_diags(&diags, "unused-parameter");
        assert!(got.is_empty(), "localparams are skipped: {:?}", diags);
    }

    /// v1 limitation: a param read only in another param's default is folded
    /// to a constant by elaboration, so the read leaves no `Ref` node and the
    /// parameter is reported.  (Documented in the module docs.)
    #[test]
    fn param_read_only_in_another_params_default_is_reported_known_limitation() {
        let diags = lint_design(
            "module up4;\n  parameter UNUSED = 3;\n  localparam X = UNUSED + 1;\nendmodule\n",
            "up4",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert_eq!(got.len(), 1, "UNUSED flagged, X skipped: {:?}", diags);
        assert!(
            got[0].message.contains("`UNUSED`"),
            "message: {}",
            got[0].message
        );
    }

    /// v1 limitation: a param read only in a range is folded to a constant by
    /// elaboration, so the read leaves no `Ref` node and the parameter is
    /// reported.  (Documented in the module docs.)
    #[test]
    fn param_read_only_in_range_is_reported_known_limitation() {
        let diags = lint_design(
            "module up5;\n  parameter W = 8;\n  logic [W-1:0] x;\nendmodule\n",
            "up5",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert_eq!(got.len(), 1, "W flagged: {:?}", diags);
        assert!(
            got[0].message.contains("`W`"),
            "message: {}",
            got[0].message
        );
    }

    #[test]
    fn gen_scope_param_read_in_own_body_is_quiet() {
        let diags = lint_design(
            "module up6;\n  genvar i;\n\
             \x20 for (i = 0; i < 2; i = i + 1) begin : g\n\
             \x20   parameter P = 2;\n\
             \x20   logic z;\n\
             \x20   always_comb z = P;\n\
             \x20 end\nendmodule\n",
            "up6",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert!(got.is_empty(), "P is read in the gen scope: {:?}", diags);
    }

    #[test]
    fn unused_gen_scope_param_is_reported_per_unrolled_scope() {
        let diags = lint_design(
            "module up7;\n  genvar i;\n\
             \x20 for (i = 0; i < 2; i = i + 1) begin : g\n\
             \x20   parameter P2 = 3;\n\
             \x20 end\nendmodule\n",
            "up7",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert_eq!(got.len(), 2, "one finding per unrolled scope: {:?}", diags);
        let msgs: Vec<&str> = got.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("`P2`") && m.contains("`up7.g[0]`")),
            "gen scope path: {msgs:?}"
        );
        assert!(
            msgs.iter()
                .any(|m| m.contains("`P2`") && m.contains("`up7.g[1]`")),
            "gen scope path: {msgs:?}"
        );
    }
}
