//! `naming-style` — identifier naming conventions.
//!
//! The rule uses one built-in convention (no config file yet): modules, interfaces
//! and packages must be `snake_case`, signals and ports must be `snake_case`,
//! and parameters/localparams must be `UPPER_SNAKE_CASE`.  The per-kind
//! matchers are stored as fields on [`StyleRule`] with a [`Default`] impl so
//! custom patterns can be plugged in later without changing the rule
//! structure.  Matching is a tiny hand-rolled character check — no regex
//! crate.
//!
//! Module and package names come from the design model (they carry the
//! declaration positions); signal/port/parameter names come from the db
//! instance tree so positions point at each declaration.  Surelog prefixes
//! library-qualified names with `<lib>@` (e.g. `work@MyMod_Bad`); the prefix
//! is stripped before matching.  Flat (not instantiated) module definitions
//! are name-checked only: their signals/parameters are not captured in the
//! db (flat module nodes carry no children), so those declarations are not checked.

use crate::core::db::NodeKind;
use crate::core::lint::rules::analysis::iter_instances;
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// True when `s` is `snake_case`: a lowercase letter followed by lowercase
/// letters, digits and underscores.
pub fn snake_ok(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// True when `s` is `UPPER_SNAKE_CASE`: an uppercase letter followed by
/// uppercase letters, digits and underscores.
pub fn upper_ok(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Naming-convention rule with overridable per-kind matchers.
pub struct StyleRule {
    /// Matcher for module/interface/package names.
    pub module_ok: fn(&str) -> bool,
    /// Matcher for signal/port names.
    pub signal_ok: fn(&str) -> bool,
    /// Matcher for parameter/localparam names.
    pub param_ok: fn(&str) -> bool,
}

impl Default for StyleRule {
    fn default() -> Self {
        StyleRule {
            module_ok: snake_ok,
            signal_ok: snake_ok,
            param_ok: upper_ok,
        }
    }
}

impl LintRule for StyleRule {
    fn id(&self) -> &'static str {
        "naming-style"
    }

    fn description(&self) -> &'static str {
        "flags identifiers that do not match the default naming conventions (snake_case / UPPER_SNAKE_CASE)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        // Module and package names from the model, which carries their
        // declaration positions.
        for m in &ctx.model.modules {
            let name = clean_name(&m.name);
            if name.is_empty() || (self.module_ok)(name) {
                continue;
            }
            out.push(diag(
                format!("module name `{name}` does not match snake_case"),
                m.file.clone(),
                m.line,
                m.col,
            ));
        }
        for p in &ctx.model.packages {
            let name = clean_name(&p.name);
            if name.is_empty() || (self.module_ok)(name) {
                continue;
            }
            out.push(diag(
                format!("module name `{name}` does not match snake_case"),
                p.file.clone(),
                p.line,
                p.col,
            ));
        }
        // Signals, ports and parameters of every instantiated scope.
        for (inst, _) in iter_instances(db) {
            for c in &db.node(inst).children {
                let name = db.node(*c).name.clone();
                if name.is_empty() {
                    continue;
                }
                let node = db.node(*c);
                let finding = match db.node_kind(*c) {
                    NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => {
                        (!(self.signal_ok)(&name)).then(|| {
                            diag(
                                format!("signal name `{name}` does not match snake_case"),
                                node.file.clone(),
                                node.line,
                                node.col,
                            )
                        })
                    }
                    NodeKind::Port { .. } => (!(self.signal_ok)(&name)).then(|| {
                        diag(
                            format!("signal name `{name}` does not match snake_case"),
                            node.file.clone(),
                            node.line,
                            node.col,
                        )
                    }),
                    NodeKind::Param { .. } => (!(self.param_ok)(&name)).then(|| {
                        diag(
                            format!("parameter name `{name}` does not match UPPER_SNAKE_CASE"),
                            node.file.clone(),
                            node.line,
                            node.col,
                        )
                    }),
                    _ => None,
                };
                out.extend(finding);
            }
        }
        out
    }
}

/// Strip the `<library>@` prefix Surelog puts on library-qualified names
/// (e.g. `work@MyMod_Bad` → `MyMod_Bad`).
fn clean_name(s: &str) -> &str {
    s.split_once('@').map(|(_, rest)| rest).unwrap_or(s)
}

fn diag(message: String, file: Option<String>, line: u32, col: u32) -> LintDiag {
    LintDiag {
        rule: "naming-style".to_string(),
        severity: LintSeverity::Info,
        file,
        line,
        col,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn bad_names_are_reported_with_declaration_positions() {
        let sv = r#"module MyMod_Bad(input logic my_port);
  parameter WIDTH = 8;
  parameter width = 4;
  logic BadName;
  logic [3:0] okay_name;
endmodule
module inst_mod;
  parameter WIDTH = 8;
  parameter width = 4;
  logic BadName;
  logic [3:0] okay_name;
endmodule
module top2;
  inst_mod u1 ();
endmodule
"#;
        let diags = lint_design(sv, "top2");
        let got = rule_diags(&diags, "naming-style");
        let msgs: Vec<&str> = got.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(got.len(), 3, "expected 3 findings: {:?}", diags);
        // `work@` prefix is stripped before matching, and the clean name is
        // what appears in the message.
        assert!(
            msgs.iter()
                .any(|m| m.contains("module name `MyMod_Bad`") && !m.contains("work@")),
            "module finding: {msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("parameter name `width`")),
            "parameter finding: {msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("signal name `BadName`")),
            "signal finding: {msgs:?}"
        );
        assert!(
            got.iter().all(|d| d.severity == LintSeverity::Info),
            "all Info: {:?}",
            diags
        );
        let module = got
            .iter()
            .find(|d| d.message.contains("MyMod_Bad"))
            .unwrap();
        assert_eq!((module.line, module.col), (1, 1));
        let param = got
            .iter()
            .find(|d| d.message.contains("parameter name `width`"))
            .unwrap();
        assert_eq!((param.line, param.col), (9, 13));
        let signal = got
            .iter()
            .find(|d| d.message.contains("signal name `BadName`"))
            .unwrap();
        assert_eq!((signal.line, signal.col), (10, 9));
    }

    #[test]
    fn good_names_are_quiet() {
        let sv = r#"module inst_mod;
  parameter WIDTH = 8;
  logic [3:0] okay_name;
endmodule
module top2;
  inst_mod u1 ();
endmodule
"#;
        let diags = lint_design(sv, "top2");
        let got = rule_diags(&diags, "naming-style");
        assert!(got.is_empty(), "no findings: {:?}", diags);
    }

    #[test]
    fn matchers_cover_boundary_cases() {
        assert!(snake_ok("a"));
        assert!(snake_ok("ab_cd9"));
        assert!(!snake_ok(""));
        assert!(!snake_ok("Bad"));
        assert!(!snake_ok("_x"));
        assert!(!snake_ok("9x"));
        assert!(!snake_ok("a-b"));
        assert!(upper_ok("WIDTH"));
        assert!(upper_ok("WIDTH_2"));
        assert!(!upper_ok("width"));
        assert!(!upper_ok("Width"));
        assert!(!upper_ok(""));
        assert!(!upper_ok("_X"));
    }
}
