//! `unconnected-port` — instance ports left unconnected.
//!
//! A port of a module instantiation without a parent-side connection usually
//! means an unfinished or mistyped instantiation: unconnected inputs leave
//! the child logic undriven (the input floats to `x`/`z`), unconnected
//! outputs silently drop their results, and unconnected inouts break
//! bidirectional links.  The rule flags every port of a non-top instance
//! with no usable connection, covering all three source shapes:
//!
//! - ports omitted from the connection list entirely (`child u0 ();`),
//! - positional gaps (`child u0 (a, , c);`),
//! - explicitly-empty named connections (`.o()`).
//!
//! In the owned db these are exactly the ports where Surelog recorded no
//! `vpiHighConn` object at all, or only its explicit-empty marker (a
//! `vpiNullOp` operation without operands; see
//! [`crate::core::db::NodeKind::Port`] for the signature, recovered at
//! db-capture time because `high: None` alone is ambiguous).  Everything else
//! counts as connected: resolved signal refs, expression connections
//! (`.i(a & b)`), constant connections (`.v(4'd0)`) and the `` `.* `` /
//! `.name` shorthand forms — Surelog resolves those to ordinary signal refs,
//! so they need no special handling and cannot be distinguished from
//! explicit connections (nor do they need to be).
//!
//! Scope decisions: top instances are skipped (their ports have no
//! instantiation site); all port directions are flagged uniformly at Warning,
//! including intentionally-open inputs `.i()` (an undriven input is usually
//! NOT what the user wants); interface instances follow the same rule as
//! module instances.  Findings are positioned at the instantiation site (the
//! instance node), not at the child's port declaration.  Ports whose
//! declaration carries a default value that the instantiation leaves
//! omitted are NOT flagged: Surelog binds the default expression as the
//! port's high connection during elaboration, so they look connected here.
//!
//! Surelog also SYNTHESIZES one per-port copy of a connected interface into
//! the child module for every interface-typed port (named after the port).
//! Those copies are implementation views, not user-written instantiation
//! sites: their ports have no parent-side connection by construction, so
//! flagging them would only duplicate findings for instances nobody wrote —
//! they are skipped via
//! [`crate::core::lint::rules::analysis::iface_copy_instances`].
//! User-written interface instances (`bus u_bus ();`) are still linted like
//! any other instance.  Messages name the instance by its hierarchical
//! display path (`top.blk[0].u0`), and positions are clamped to the 1-based
//! [`LintDiag`] contract (some synthetic nodes carry a 0 column).

use crate::core::db::NodeKind;
use crate::core::lint::rules::analysis::{
    iface_copy_instances, iter_instances, port_unconnected, strip_lib,
};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Flags instance ports left unconnected.
pub struct UnconnectedPortRule;

impl LintRule for UnconnectedPortRule {
    fn id(&self) -> &'static str {
        "unconnected-port"
    }

    fn description(&self) -> &'static str {
        "flags instance ports left unconnected (omitted, positional gap or empty `.p()`)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let copies = iface_copy_instances(db);
        let mut out = Vec::new();
        for (inst_id, inst_path) in iter_instances(db) {
            let NodeKind::ModuleInst {
                def_name, is_top, ..
            } = db.node_kind(inst_id)
            else {
                continue;
            };
            if *is_top || copies.contains(&inst_id) {
                continue;
            }
            let inst = db.node(inst_id);
            let module = strip_lib(def_name);
            for c in &inst.children {
                if !matches!(db.node_kind(*c), NodeKind::Port { .. }) {
                    continue;
                }
                if !port_unconnected(db, *c) {
                    continue;
                }
                out.push(LintDiag {
                    rule: "unconnected-port".to_string(),
                    severity: LintSeverity::Warning,
                    file: inst.file.clone(),
                    line: inst.line.max(1),
                    col: inst.col.max(1),
                    message: format!(
                        "port `{}` of instance `{}` (`{}`) is not connected",
                        db.node(*c).name,
                        inst_path,
                        module
                    ),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{build_design, lint_design, rule_diags};
    use crate::core::lint::{lint_with_config, LintConfig, RuleConfig};

    #[test]
    fn omitted_port_is_reported() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  wire w1;\n  child u0 (.o(w1));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unconnected-port");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning);
        assert!(
            d.message
                .contains("port `i` of instance `top.u0` (`child`)"),
            "message names module, hierarchical instance path and port: {}",
            d.message
        );
        // Positioned at the instantiation site, not the child declaration.
        assert_eq!((d.line, d.col), (6, 3), "{diags:?}");
    }

    #[test]
    fn empty_named_connection_is_reported() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  wire w1;\n  child u0 (.i(), .o(w1));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unconnected-port");
        assert_eq!(got.len(), 1, "only `.i()` flagged: {:?}", diags);
        assert!(
            got[0].message.contains("port `i` of instance `top.u0`"),
            "{}",
            got[0].message
        );
    }

    #[test]
    fn fully_omitted_connection_list_reports_every_port() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  child u0 ();\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unconnected-port");
        assert_eq!(got.len(), 2, "both ports flagged: {:?}", diags);
        let mut names: Vec<&str> = got
            .iter()
            .map(|d| d.message.split('`').nth(1).expect("quoted name in message"))
            .collect();
        names.sort_unstable();
        assert_eq!(names, vec!["i", "o"]);
    }

    #[test]
    fn positional_gap_is_reported() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  wire w1;\n  child u0 (w1, );\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unconnected-port");
        assert_eq!(got.len(), 1, "the gapped output is flagged: {:?}", diags);
        assert!(
            got[0].message.contains("port `o` of instance `top.u0`"),
            "{}",
            got[0].message
        );
    }

    #[test]
    fn connected_ports_are_quiet() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  logic a;\n  wire w1;\n  child full (.i(a), .o(w1));\n\
             child expr (.i(a & w1), .o(w1));\n\
             child konst (.i(1'b0), .o(w1));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unconnected-port");
        assert!(
            got.is_empty(),
            "signal/expression/constant connections are connected: {:?}",
            diags
        );
    }

    /// `` `.* `` and `.name` shorthand resolve to ordinary refs in the db;
    /// they must never be flagged.
    #[test]
    fn dot_star_and_shorthand_are_quiet() {
        let diags = lint_design(
            "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
             module top;\n  wire i;\n  wire o;\n  child ds (.*);\n  child sh (.i, .o(o));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unconnected-port");
        assert!(got.is_empty(), "shorthand forms are connected: {:?}", diags);
    }

    /// Top instances have no instantiation site; their ports carry no high
    /// connection by construction and must stay quiet.
    #[test]
    fn top_ports_are_quiet() {
        let diags = lint_design("module t(input wire a, output wire b);\nendmodule\n", "t");
        assert!(
            rule_diags(&diags, "unconnected-port").is_empty(),
            "top ports are not findings: {diags:?}"
        );
    }

    /// Interface designs: Surelog synthesizes per-port COPY interface
    /// instances into every child with an interface-typed port.  Those copies
    /// have no parent-side connection by construction and must never be
    /// flagged — only user-written instances are linted.
    #[test]
    fn interface_port_left_empty_is_reported_once() {
        let diags = lint_design(
            "interface bus_if2(input wire clk); logic req; endinterface\n\
             module consumer2(bus_if2 b); endmodule\n\
             module top2; logic clk = 0; bus_if2 u_bus (); consumer2 u_cons (.b(u_bus)); endmodule\n",
            "top2",
        );
        let got = rule_diags(&diags, "unconnected-port");
        assert_eq!(got.len(), 1, "only the user-written instance: {:?}", diags);
        let d = got[0];
        assert!(
            d.message.contains("port `clk` of instance `top2.u_bus`"),
            "{}",
            d.message
        );
        // Every finding stays 1-based even when a node has no source column.
        for d in &diags {
            assert!(d.line >= 1 && d.col >= 1, "0-based position: {d:?}");
        }
    }

    /// A fully-connected interface port produces zero findings: the actual
    /// interface instance's own ports are connected AND the synthesized
    /// per-port copies stay silent.
    #[test]
    fn fully_connected_interface_port_is_quiet() {
        let diags = lint_design(
            "interface bus_if(input wire clk); logic req; endinterface\n\
             module consumer(bus_if b); endmodule\n\
             module top; logic clk = 0; bus_if u_bus (.clk(clk)); consumer u_cons (.b(u_bus)); endmodule\n",
            "top",
        );
        assert!(
            rule_diags(&diags, "unconnected-port").is_empty(),
            "{diags:?}"
        );
    }

    /// Config plumbing via `llg-lint.toml`-style keys: the new id is known
    /// to the parser (ids are discovered from the registry) and disabling it
    /// removes its findings.
    #[test]
    fn config_disables_the_rule() {
        let sv = "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
                  module top;\n  wire w1;\n  child u0 (.o(w1));\nendmodule\n";
        let diags = lint_design(sv, "top");
        assert_eq!(
            rule_diags(&diags, "unconnected-port").len(),
            1,
            "sanity: enabled by default: {diags:?}"
        );

        let mut cfg_from_toml = LintConfig::new();
        cfg_from_toml
            .parse_toml("[rules.unconnected-port]\nenabled = false\n")
            .expect("registered ids parse without an unknown-rule error");
        let (db, model) = build_design(sv, "top");
        let filtered = lint_with_config(&db, &model, &cfg_from_toml);
        assert!(
            rule_diags(&filtered, "unconnected-port").is_empty(),
            "{filtered:?}"
        );
    }

    /// Config plumbing: a severity override replaces the rule's own severity.
    #[test]
    fn config_severity_override_applies() {
        let sv = "module child(input wire i, output wire o);\n  assign o = i;\nendmodule\n\
                  module top;\n  wire w1;\n  child u0 (.o(w1));\nendmodule\n";
        let (db, model) = build_design(sv, "top");
        let mut cfg = LintConfig::new();
        cfg.set(
            "unconnected-port",
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Error),
            },
        );
        let diags = lint_with_config(&db, &model, &cfg);
        let got = rule_diags(&diags, "unconnected-port");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].severity, LintSeverity::Error);
    }
}
