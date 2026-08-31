//! `implicit-net` — signals implicitly declared from an undeclared identifier.
//!
//! When an identifier is used where a net is expected but never declared,
//! Surelog's elaboration silently promotes it to a 1-bit wire (the
//! `` `default_nettype`` default).  Such a typo usually surfaces as a signal
//! that is stuck at `z`: it is connected but never driven by anything real.
//! The created `logic_net` carries **no typespec**, while every explicitly
//! declared net/variable gets one, so in the owned database the signature of
//! an implicitly-declared net is a [`NodeKind::Net`] whose [`TypeInfo`] kind
//! is `"other"` (verified against Surelog v1.86 elaboration output; the
//! object is positioned at its creating use site).
//!
//! Undeclared identifiers in other positions degrade differently and are not
//! seen here: procedural-assignment LHS uses are rejected by Surelog as
//! elaboration errors ("Illegal lhs of type wire"), plain expression uses
//! leave an unbound ref without a net object, and `` `default_nettype none``
//! makes Surelog report "Illegal implicit net" itself.

use crate::core::db::NodeKind;
use crate::core::lint::rules::analysis::{all_nodes, signal_scope_path};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Flags nets that were implicitly declared from an undeclared identifier.
pub struct ImplicitNetRule;

impl LintRule for ImplicitNetRule {
    fn id(&self) -> &'static str {
        "implicit-net"
    }

    fn description(&self) -> &'static str {
        "flags nets implicitly declared from undeclared identifiers (likely typos)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            let NodeKind::Net { ty, .. } = db.node_kind(id) else {
                continue;
            };
            if ty.kind != "other" {
                continue;
            }
            let node = db.node(id);
            out.push(LintDiag {
                rule: "implicit-net".to_string(),
                severity: LintSeverity::Warning,
                file: node.file.clone(),
                line: node.line,
                col: node.col,
                message: format!(
                    "net `{}` in `{}` is implicitly declared from an undeclared identifier \
                     (promoted to a wire by `default_nettype`; likely a misspelling) — \
                     declare it explicitly or fix the spelling",
                    node.name,
                    signal_scope_path(db, id),
                ),
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};
    use crate::core::lint::{lint_with_config, LintConfig, RuleConfig};

    #[test]
    fn undeclared_port_connection_is_reported() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  wire oo;\n  child u0 (.i(misspelled_in), .o(oo));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "implicit-net");
        assert_eq!(got.len(), 1, "one implicit-net finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning);
        assert!(
            d.message.contains("`misspelled_in`"),
            "message names the signal: {}",
            d.message
        );
        assert!(d.message.contains("implicitly declared"), "{}", d.message);
        // The implicit net is positioned at its creating use site (the port
        // connection on line 6).
        assert_eq!(d.line, 6);
    }

    #[test]
    fn both_sides_undeclared_are_reported() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  child u0 (.i(in_a), .o(out_b));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "implicit-net");
        assert_eq!(got.len(), 2, "two findings: {:?}", diags);
        let mut names: Vec<&str> = got
            .iter()
            .map(|d| d.message.split('`').nth(1).expect("quoted name in message"))
            .collect();
        names.sort_unstable();
        assert_eq!(names, vec!["in_a", "out_b"]);
    }

    #[test]
    fn declared_connections_are_quiet() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  logic in_a;\n  wire out_b;\n  child u0 (.i(in_a), .o(out_b));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "implicit-net");
        assert!(
            got.is_empty(),
            "no findings for declared signals: {:?}",
            diags
        );
    }

    /// Pins the db signature the rule relies on: a bare explicitly-declared
    /// 1-bit `wire` carries a typespec in the owned db (kind != "other"), so
    /// it must never fire.  If a Surelog/UHDM bump changes that signature,
    /// this test fails before every declared net starts being flagged.
    #[test]
    fn declared_bare_wire_is_quiet() {
        let diags = lint_design("module wq;\n  wire w;\nendmodule\n", "wq");
        assert!(
            rule_diags(&diags, "implicit-net").is_empty(),
            "an explicitly declared bare wire must not be flagged: {diags:?}"
        );
    }

    /// Config plumbing: disabling the rule removes its findings.
    #[test]
    fn config_disables_the_rule() {
        let sv = "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
                  module top;\n  child u0 (.i(misspelled_in), .o());\nendmodule\n";
        let diags = lint_design(sv, "top");
        assert_eq!(
            rule_diags(&diags, "implicit-net").len(),
            1,
            "sanity: enabled by default: {diags:?}"
        );
        let (db, model) = crate::core::lint::rules::tests::build_design(sv, "top");
        let mut cfg = LintConfig::new();
        cfg.set(
            "implicit-net",
            RuleConfig {
                enabled: false,
                severity: None,
            },
        );
        let filtered = lint_with_config(&db, &model, &cfg);
        assert!(
            rule_diags(&filtered, "implicit-net").is_empty(),
            "{filtered:?}"
        );
    }

    /// Config plumbing: a severity override replaces the rule's own severity.
    #[test]
    fn config_severity_override_applies() {
        let sv = "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
                  module top;\n  child u0 (.i(misspelled_in), .o());\nendmodule\n";
        let (db, model) = crate::core::lint::rules::tests::build_design(sv, "top");
        let mut cfg = LintConfig::new();
        cfg.set(
            "implicit-net",
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Error),
            },
        );
        let diags = lint_with_config(&db, &model, &cfg);
        let got = rule_diags(&diags, "implicit-net");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].severity, LintSeverity::Error);
    }
}
