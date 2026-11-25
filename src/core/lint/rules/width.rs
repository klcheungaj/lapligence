//! `width-mismatch` — assignment and port-link width mismatches.
//!
//! For every procedural assignment, continuous assignment and port link, the
//! LHS width is compared with the RHS width.  Severity follows the LRM:
//! truncation (RHS wider than LHS) is a Warning, zero/sign extension (RHS
//! narrower) is allowed and reported as Info.  An element of an unpacked
//! array, queue or associative array and a structure member have the width of
//! their declared packed type, so reading one into a narrower target warns.
//! Widths that cannot be computed (unsized literals, unknown types, unpacked
//! slices and rows, function calls, …) are skipped, and so is an assignment
//! whose target is no signal or packed select of one (an element or a member).

use crate::core::db::{Db, Direction, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::{expr_width, object_width, signal_of_ref, ElementWidths};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns when an assignment's LHS and RHS widths differ.
pub struct WidthMismatchRule;

impl LintRule for WidthMismatchRule {
    fn id(&self) -> &'static str {
        "width-mismatch"
    }

    fn description(&self) -> &'static str {
        "warns on assignment width mismatch (truncation warns, extension is informational)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for &id in ctx.nodes() {
            match db.node_kind(id) {
                NodeKind::Stmt(StmtKind::Assign { .. }) | NodeKind::ContAssign { .. } => {
                    let node = db.node(id);
                    if let (Some(lhs), Some(rhs)) = (node.children.first(), node.children.get(1)) {
                        let Some(name) = signal_of_ref(db, *lhs).map(|s| db.node(s).name.clone())
                        else {
                            continue; // concat/other non-signal LHS: not named
                        };
                        if let Some(diag) = width_diag(
                            db,
                            id,
                            &name,
                            expr_width(db, *lhs, ElementWidths::Declared),
                            expr_width(db, *rhs, ElementWidths::Declared),
                        ) {
                            out.push(diag);
                        }
                    }
                }
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    high_expr,
                    ty,
                    ..
                } => {
                    // Port link: input ports are driven by the parent (the
                    // port is the LHS), output ports drive the parent (the
                    // actual is the LHS).  The actual's width is its own
                    // expression's (`.p(v[3])` is one bit), and the port's is
                    // its internal symbol's or port expression's.
                    if !matches!(direction, Direction::Input | Direction::Output) {
                        continue;
                    }
                    // A concatenated port is judged as a whole: its
                    // components each connect only their slice of the actual.
                    if db.multi_port_of(id).is_some() {
                        continue;
                    }
                    if !db.multi_port_components(id).is_empty() {
                        out.extend(multi_port_diag(db, id, *direction, ty.width));
                        continue;
                    }
                    let Some(actual) = high_expr.or(*high) else {
                        continue;
                    };
                    if low.is_some() && *low == *high {
                        continue;
                    }
                    let actual_width = if high_expr.is_some() {
                        expr_width(db, actual, ElementWidths::Declared)
                    } else {
                        object_width(db, actual)
                    };
                    let port_width = match low {
                        Some(low) => object_width(db, *low),
                        None => db
                            .port_internal_expression(id)
                            .and_then(|expr| expr_width(db, expr, ElementWidths::Declared))
                            .or(ty.width),
                    };
                    let port_name = low
                        .map_or_else(|| db.node(id).name.clone(), |low| db.node(low).name.clone());
                    let actual_name = high
                        .map(|high| db.node(high).name.clone())
                        .or_else(|| signal_of_ref(db, actual).map(|s| db.node(s).name.clone()));
                    let diag = if *direction == Direction::Input {
                        width_diag(db, id, &port_name, port_width, actual_width)
                    } else {
                        let Some(actual_name) = actual_name else {
                            continue; // concatenation actual: not named
                        };
                        width_diag(db, id, &actual_name, actual_width, port_width)
                    };
                    out.extend(diag);
                }
                _ => {}
            }
        }
        out
    }
}

/// The width finding of a concatenated port `{a, b}` (SV 23.2.2.1), named
/// by the port (or its expression) and the whole actual: each component
/// connects its slice of the actual, so the actual width is the sum of the
/// slices; an actual wider than the port is not visible in them. An output
/// to a concatenation actual is not named, as for other ports.
fn multi_port_diag(
    db: &Db,
    port: NodeId,
    direction: Direction,
    port_width: Option<u32>,
) -> Option<LintDiag> {
    let components = db.multi_port_components(port);
    let mut connected = 0u32;
    let mut actual = None;
    let mut names = Vec::with_capacity(components.len());
    for component in components {
        let NodeKind::Port {
            high,
            low,
            high_expr,
            high_open,
            ..
        } = db.node_kind(*component)
        else {
            return None;
        };
        names.push(low.map_or_else(
            || db.node(*component).name.clone(),
            |low| db.node(low).name.clone(),
        ));
        if *high_open {
            continue;
        }
        let width = match (high_expr, high) {
            (Some(expr), _) => expr_width(db, *expr, ElementWidths::Declared)?,
            (None, Some(high)) => object_width(db, *high)?,
            (None, None) => continue,
        };
        connected = connected.checked_add(width)?;
        // The actual is named when every component connects to one signal.
        actual = match (actual, high) {
            (None, Some(high)) => Some(Some(*high)),
            (Some(Some(previous)), Some(high)) if previous == *high => Some(Some(previous)),
            _ => Some(None),
        };
    }
    let port_width = port_width.or_else(|| {
        components
            .iter()
            .try_fold(0u32, |sum, component| match db.node_kind(*component) {
                NodeKind::Port { ty, .. } => sum.checked_add(ty.width?),
                _ => None,
            })
    })?;
    if direction == Direction::Input {
        let name = &db.node(port).name;
        let port_name = if name.is_empty() {
            format!("{{{}}}", names.join(", "))
        } else {
            name.clone()
        };
        width_diag(db, port, &port_name, Some(port_width), Some(connected))
    } else {
        let actual = db.node(actual.flatten()?).name.clone();
        width_diag(db, port, &actual, Some(connected), Some(port_width))
    }
}

/// One width finding, when both widths are known and differ.
fn width_diag(
    db: &Db,
    at: NodeId,
    lhs_name: &str,
    lhs_width: Option<u32>,
    rhs_width: Option<u32>,
) -> Option<LintDiag> {
    let (lw, rw) = (lhs_width?, rhs_width?);
    if lw == rw {
        return None;
    }
    let (severity, qualifier) = if rw > lw {
        (LintSeverity::Warning, "truncation")
    } else {
        (LintSeverity::Info, "extension")
    };
    let node = db.node(at);
    Some(LintDiag {
        rule: "width-mismatch".to_string(),
        logical: None,
        severity,
        file: node.file().map(str::to_owned),
        line: node.line,
        col: node.col,
        message: format!(
            "assignment width mismatch: LHS `{lhs_name}` is {lw} bits, RHS is {rw} bits ({qualifier})"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn extension_is_informational() {
        let diags = lint_design(
            "module wm;\n  logic [3:0] x;\n  logic [7:0] y;\n  assign y = x;\nendmodule\n",
            "wm",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert_eq!(got.len(), 1, "one width finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Info, "extension is Info");
        assert!(
            d.message.contains("`y` is 8 bits, RHS is 4 bits"),
            "{}",
            d.message
        );
        assert!(d.message.contains("extension"), "{}", d.message);
    }

    #[test]
    fn indexed_select_of_packed_elements_has_the_elements_width() {
        let diags = lint_design(
            "module wm1;\n  logic [0:3][7:0] pk;\n  integer i;\n  logic [15:0] w;\n  \
             logic [7:0] b;\n  assign w = pk[i -: 2];\n  assign b = pk[i];\nendmodule\n",
            "wm1",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert!(got.is_empty(), "no width finding: {:?}", got);
    }

    #[test]
    fn unpacked_array_slices_are_quiet() {
        let diags = lint_design(
            "class base; int id; endclass\nmodule wu;\n  base f[3];\n  base a[];\n  \
             base g[2];\n  int fi[4];\n  int ai[2];\n  int di[];\n  logic [7:0] m[4];\n  \
             logic [7:0] n[2];\n  logic [7:0] w[];\n  initial begin\n    a = f[1:2];\n    \
             g = f[0:1];\n    ai = fi[2:3];\n    di = fi[1:3];\n    n = m[2:3];\n    \
             w = m[0:2];\n    n = m[1+:2];\n    n[0:1] = m[3-:2];\n  end\nendmodule\n",
            "wu",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert!(got.is_empty(), "no width finding: {:?}", got);
    }

    #[test]
    fn packed_selects_near_unpacked_slices_still_warn() {
        let diags = lint_design(
            "module wp;\n  logic [7:0] m[4];\n  logic [3:0][7:0] pk;\n  logic [3:0] s;\n  \
             logic [7:0] b;\n  logic [15:0] h;\n  initial begin\n    s = m[1][7:2];\n    \
             b = pk[1+:2];\n    h = pk[2:1];\n  end\nendmodule\n",
            "wp",
        );
        let mut got: Vec<_> = rule_diags(&diags, "width-mismatch")
            .iter()
            .map(|d| (d.severity, d.message.clone()))
            .collect();
        got.sort_by(|a, b| a.1.cmp(&b.1));
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got[0].0, LintSeverity::Warning);
        assert!(
            got[0]
                .1
                .contains("`b` is 8 bits, RHS is 16 bits (truncation)"),
            "{got:?}"
        );
        assert_eq!(got[1].0, LintSeverity::Warning);
        assert!(
            got[1]
                .1
                .contains("`s` is 4 bits, RHS is 6 bits (truncation)"),
            "{got:?}"
        );
    }

    /// The declarations the element-select tests share.
    const ELEMENTS: &str = "module we;\n\
        \x20 logic [7:0] m[4], m2[2][3], d[], dq[$][], b;\n  logic [15:0] aa[string], w;\n\
        \x20 logic [3:0] s;\n  int q[$];\n  logic [1:0][11:0] pk[2];\n\
        \x20 typedef struct { logic [7:0] f; logic [7:0] g[3]; real r; string t; } st;\n\
        \x20 typedef struct packed { logic [2:0] a; logic [8:0] b; } ps;\n\
        \x20 st sa[2], s1;\n  ps pa[2];\n  enum logic [4:0] {E0, E1} en[2];\n\
        \x20 real ra[2];\n  string sq[$];\n";

    /// The severity and message of each `width-mismatch` finding of
    /// `statement` inside the module of [`ELEMENTS`].
    fn element_findings(statement: &str) -> Vec<(LintSeverity, String)> {
        let source = format!("{ELEMENTS}  initial begin {statement} end\nendmodule\n");
        let diags = lint_design(&source, "we");
        rule_diags(&diags, "width-mismatch")
            .iter()
            .map(|d| (d.severity, d.message.clone()))
            .collect()
    }

    #[test]
    fn elements_and_members_have_their_declared_width() {
        for (statement, rhs) in [
            ("s = m[1];", 8),
            ("s = d[0];", 8),
            ("s = q[0];", 32),
            ("s = q[$];", 32),
            ("s = aa[\"x\"];", 16),
            ("s = m2[0][1];", 8),
            ("s = dq[0][1];", 8),
            ("s = pk[1];", 24),
            ("s = pk[1][0];", 12),
            ("s = en[0];", 5),
            ("s = pa[1];", 12),
            ("s = pa[1].b;", 9),
            ("s = sa[0].f;", 8),
            ("s = sa[0].g[1];", 8),
            ("s = s1.f;", 8),
            ("s = s1.g[w];", 8),
            ("s = sq[0][1];", 8),
            ("s = m[1] + m[2];", 8),
            ("s = {m[1], m[2]};", 16),
            ("s = 1'b1 ? m[1] : b;", 8),
        ] {
            assert_eq!(
                element_findings(statement),
                [(
                    LintSeverity::Warning,
                    format!(
                        "assignment width mismatch: LHS `s` is 4 bits, RHS is {rhs} bits \
                         (truncation)"
                    )
                )],
                "{statement}"
            );
        }
        assert_eq!(
            element_findings("w = m[1];"),
            [(
                LintSeverity::Info,
                "assignment width mismatch: LHS `w` is 16 bits, RHS is 8 bits (extension)"
                    .to_string()
            )]
        );
    }

    #[test]
    fn elements_without_a_packed_width_and_equal_widths_are_quiet() {
        for statement in [
            // Equal widths.
            "b = m[1];",
            "b = m2[1][2];",
            "b = sa[1].f;",
            "b = m[1] + m[2];",
            "w = {m[1], m[2]};",
            "w = aa[\"x\"];",
            "b = sq[0][1];",
            // Rows, records, containers and whole arrays are not packed.
            "m2[0] = m2[1];",
            "dq[0] = dq[1];",
            "d = dq[0];",
            "sa[0] = sa[1];",
            "s1 = sa[1];",
            "s1.g = sa[0].g;",
            "m = m;",
            "q = q[1:$];",
            "m2[0] = '{default: 0};",
            "m[0:1] = m[2:3];",
            // Reals and strings have no width.
            "s = ra[0];",
            "s = sa[0].r;",
            "sq[0] = sa[0].t;",
        ] {
            assert_eq!(element_findings(statement), [], "{statement}");
        }
    }

    #[test]
    fn element_port_actuals_have_their_declared_width() {
        let diags = lint_design(
            "module c(input wire [3:0] i, output wire [15:0] o);\n  assign o = {4{i}};\n\
             endmodule\nmodule top;\n  logic [7:0] m[2];\n  logic [3:0] n[2];\n\
             \x20 wire [15:0] x[2];\n  wire [7:0] y[2];\n\
             \x20 c u0(.i(m[0]), .o(x[0]));\n  c u1(.i(n[0]), .o(y[1]));\nendmodule\n",
            "top",
        );
        let got: Vec<_> = rule_diags(&diags, "width-mismatch")
            .iter()
            .map(|d| d.message.clone())
            .collect();
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[0].contains("`i` is 4 bits, RHS is 8 bits (truncation)"));
        assert!(got[1].contains("`y` is 8 bits, RHS is 16 bits (truncation)"));
    }

    #[test]
    fn truncation_is_a_warning() {
        let diags = lint_design(
            "module wm2;\n  logic [3:0] x;\n  assign x = 8'hff;\nendmodule\n",
            "wm2",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert_eq!(got.len(), 1, "one width finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning, "truncation is Warning");
        assert!(
            d.message.contains("`x` is 4 bits, RHS is 8 bits"),
            "{}",
            d.message
        );
        assert!(d.message.contains("truncation"), "{}", d.message);
    }

    /// Pins the WIDER→NARROWER direction between two signals specifically
    /// (`assign narrow = wide;`): truncation must stay a Warning when the
    /// RHS is a signal, not just a sized literal.
    #[test]
    fn wider_signal_truncated_into_narrower_warns() {
        let diags = lint_design(
            "module wm2b;\n  logic [7:0] wide;\n  logic [3:0] narrow;\n  assign narrow = wide;\nendmodule\n",
            "wm2b",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert_eq!(got.len(), 1, "one width finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning, "truncation is Warning");
        assert!(
            d.message.contains("`narrow` is 4 bits, RHS is 8 bits"),
            "{}",
            d.message
        );
        assert!(d.message.contains("truncation"), "{}", d.message);
    }

    #[test]
    fn matching_widths_are_quiet() {
        let diags = lint_design(
            "module wm3;\n  logic [3:0] x;\n  logic [3:0] y;\n  assign y = x;\nendmodule\n",
            "wm3",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert!(got.is_empty(), "no width findings: {:?}", diags);
    }

    #[test]
    fn port_link_width_mismatch_is_reported() {
        let diags = lint_design(
            "module child(input wire [3:0] a, output wire b);\n  assign b = a[0];\nendmodule\n\
             module top;\n  wire [7:0] in_a;\n  wire out_b;\n  child u0 (.a(in_a), .b(out_b));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "width-mismatch");
        // in_a is 8 bits driving a 4-bit input port → truncation Warning.
        let warn = got.iter().find(|d| d.severity == LintSeverity::Warning);
        assert!(
            warn.is_some(),
            "expected a Warning width finding for the port link: {:?}",
            diags
        );
        assert!(warn
            .unwrap()
            .message
            .contains("`a` is 4 bits, RHS is 8 bits"));
        assert!(warn.unwrap().message.contains("truncation"));
    }

    #[test]
    fn bit_select_connection_to_scalar_port_is_quiet() {
        let diags = lint_design(
            "module inv(input a, output y);\n  assign y = ~a;\nendmodule\n\
             module top;\n  wire [3:0] n;\n  logic [3:0] v;\n\
             \x20 assign n[0] = 1'b0;\n\
             \x20 inv g0(.a(n[0]), .y(n[1]));\n\
             \x20 inv g1(n[1], v[3]);\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert!(got.is_empty(), "one bit to one bit: {:?}", diags);
    }

    #[test]
    fn select_connection_wider_than_its_port_still_warns() {
        let diags = lint_design(
            "module buf1(input a, output y);\n  assign y = a;\nendmodule\n\
             module top;\n  wire [3:0] n;\n  wire y;\n\
             \x20 assign n = 4'b0;\n\
             \x20 buf1 g0(.a(n[2:0]), .y(y));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert_eq!(got.len(), 1, "3 bits into 1: {:?}", diags);
        assert_eq!(got[0].severity, LintSeverity::Warning);
        assert!(
            got[0].message.contains("`a` is 1 bits, RHS is 3 bits"),
            "{}",
            got[0].message
        );
    }

    #[test]
    fn port_expression_width_is_the_expression_width() {
        let module = "module m(.p({a, b}), .q(c[2:1]));\n  input [1:0] a;\n  input b;\n\
                      \x20 input [3:0] c;\nendmodule\n";
        let diags = lint_design(
            &format!(
                "{module}module top;\n  logic [2:0] d;\n  logic [1:0] e;\n\
                 \x20 m u(.p(d), .q(e));\nendmodule\n"
            ),
            "top",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert!(got.is_empty(), "3 and 2 bits match: {:?}", diags);

        let diags = lint_design(
            &format!(
                "{module}module top;\n  logic [1:0] d;\n  logic [1:0] e;\n\
                 \x20 m u(.p(d), .q(e));\nendmodule\n"
            ),
            "top",
        );
        // The concatenation is judged as a whole: the 2-bit actual against
        // the 3-bit port `p`, not its component `a` against its slice.
        let got = rule_diags(&diags, "width-mismatch");
        assert_eq!(
            got.len(),
            1,
            "2 bits into the 3-bit concatenation: {:?}",
            diags
        );
        assert!(
            got[0].message.contains("`p` is 3 bits, RHS is 2 bits"),
            "{}",
            got[0].message
        );
    }

    #[test]
    fn concatenated_port_widths_name_the_port_and_the_actual() {
        let module = "module mi({a, b});\n  input [1:0] a;\n  input b;\nendmodule\n\
                      module mo(.p({a, b}));\n  output [1:0] a;\n  output b;\n\
                      \x20 assign a = 2'b10;\n  assign b = 1'b1;\nendmodule\n";
        let diags = lint_design(
            &format!(
                "{module}module top;\n  logic d;\n  wire [1:0] f;\n  wire g;\n  wire [1:0] h;\n\
                 \x20 mi u(d);\n  mo v(.p(f));\n  mo w(.p({{g, h}}));\nendmodule\n"
            ),
            "top",
        );
        let mut got: Vec<_> = rule_diags(&diags, "width-mismatch")
            .iter()
            .map(|d| d.message.clone())
            .collect();
        got.sort();
        assert_eq!(got.len(), 2, "{:?}", diags);
        assert!(
            got[0].contains("`f` is 2 bits, RHS is 3 bits (truncation)"),
            "{got:?}"
        );
        assert!(
            got[1].contains("`{a, b}` is 3 bits, RHS is 1 bits (extension)"),
            "{got:?}"
        );
    }
}
