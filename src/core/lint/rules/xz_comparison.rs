//! `xz-logical-equality` — logical equality against visible X/Z literals.
//!
//! Logical equality (`==`/`!=`) treats an X or Z bit as an unknown result,
//! while case equality (`===`/`!==`) compares the four-state values.  A
//! literal X/Z operand is therefore usually an accidental use of the logical
//! operator.  This rule deliberately inspects only the literal directly
//! present at either operand (through transparent casts); parameter values
//! and constant-expression trees are not followed.

use crate::core::db::{Db, ExprKind, NodeId, NodeKind, Operation};
use crate::core::lint::rules::analysis::all_nodes;
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::core::value::{ScalarValue, ValueData};

/// Flags `==`/`!=` whose direct operand visibly contains X, Z or `?`.
pub struct XzLogicalEqualityRule;

impl LintRule for XzLogicalEqualityRule {
    fn id(&self) -> &'static str {
        "xz-logical-equality"
    }

    fn description(&self) -> &'static str {
        "flags logical equality compared directly with an X/Z/? literal"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        let mut diagnostics_seen = std::collections::HashSet::new();
        for id in all_nodes(db) {
            let NodeKind::Expr(ExprKind::Operation { op, operands, .. }) = db.node_kind(id) else {
                continue;
            };
            if !matches!(op, Operation::Equal | Operation::NotEqual) {
                continue;
            }
            let (Some(left), Some(right)) = (operands.first(), operands.get(1)) else {
                continue;
            };
            if !has_visible_xz_literal(db, *left) && !has_visible_xz_literal(db, *right) {
                continue;
            }

            let node = db.node(id);
            let line = node.line.max(1);
            let col = node.col.max(1);
            let message =
                "logical equality compares a literal containing X/Z/?; use === or !== instead"
                    .to_string();
            if !diagnostics_seen.insert((node.file.clone(), line, col, message.clone())) {
                continue;
            }
            out.push(LintDiag {
                rule: "xz-logical-equality".to_string(),
                severity: LintSeverity::Warning,
                file: node.file.clone(),
                line,
                col,
                message,
            });
        }
        out
    }
}

/// Whether `id` is a literal containing a four-state unknown, after peeling
/// only transparent cast nodes.  References and operation trees are
/// intentionally not traversed: the rule is about a visible literal at the
/// comparison site, not the value eventually assigned to a parameter.
fn has_visible_xz_literal(db: &Db, id: NodeId) -> bool {
    match db.node_kind(id) {
        NodeKind::Expr(ExprKind::Cast { operand, .. }) => has_visible_xz_literal(db, *operand),
        NodeKind::Expr(ExprKind::Constant { value, .. }) => value_contains_xz(value),
        _ => false,
    }
}

/// Inspect the owned value representation without evaluating or
/// expanding it.  `bval` is nonzero for every unknown bit in a vector.
fn value_contains_xz(value: &ValueData) -> bool {
    match value {
        ValueData::Bin(digits)
        | ValueData::Oct(digits)
        | ValueData::Dec(digits)
        | ValueData::Hex(digits) => digits
            .chars()
            .any(|digit| matches!(digit, 'x' | 'X' | 'z' | 'Z' | '?')),
        ValueData::Scalar(scalar) => matches!(
            *scalar,
            ScalarValue::X | ScalarValue::Z | ScalarValue::DontCare
        ),
        ValueData::Vector { unknown_words, .. } => unknown_words.iter().any(|word| *word != 0),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::build_design;

    fn rule_diags(sv: &str, top: &str) -> Vec<LintDiag> {
        let (db, model) = build_design(sv, top);
        XzLogicalEqualityRule.check(&LintCtx {
            db: &db,
            model: &model,
        })
    }

    #[test]
    fn either_logical_operand_with_xz_warns_once() {
        let diags = rule_diags(
            "module xz1;\n\
             logic [3:0] a;\n\
             logic y;\n\
             assign y = a == 4'b1x00;\n\
             assign y = 1'bz != a;\n\
             endmodule\n",
            "xz1",
        );
        assert_eq!(diags.len(), 2, "one warning per comparison: {diags:?}");
        assert!(diags.iter().all(|d| d.severity == LintSeverity::Warning));
        assert!(diags.iter().all(|d| d.message.contains("=== or !==")));
    }

    #[test]
    fn cast_wrapped_and_unsized_literals_warn() {
        let diags = rule_diags(
            "module xz2;\n\
             logic [3:0] a;\n\
             logic y;\n\
             assign y = a == int'(4'bx);\n\
             assign y = a != 'x;\n\
             endmodule\n",
            "xz2",
        );
        assert_eq!(diags.len(), 2, "cast and unsized X are visible: {diags:?}");
    }

    #[test]
    fn decimal_xz_literals_warn() {
        let diags = rule_diags(
            "module xz_dec;\n\
             logic [3:0] a;\n\
             logic y;\n\
             assign y = a == 4'dx;\n\
             assign y = a != 4'dz;\n\
             endmodule\n",
            "xz_dec",
        );
        assert_eq!(
            diags.len(),
            2,
            "decimal X/Z literals are visible: {diags:?}"
        );
    }

    #[test]
    fn case_and_wildcard_equalities_are_quiet() {
        let diags = rule_diags(
            "module xz3;\n\
             logic [3:0] a;\n\
             logic y;\n\
             assign y = a === 4'bx;\n\
             assign y = a !== 4'bz;\n\
             assign y = a ==? 4'bx;\n\
             assign y = a !=? 4'bz;\n\
             endmodule\n",
            "xz3",
        );
        assert!(
            diags.is_empty(),
            "only logical ==/!= are checked: {diags:?}"
        );
    }

    #[test]
    fn known_literals_and_parameter_refs_are_quiet() {
        let diags = rule_diags(
            "module xz4;\n\
             parameter logic [3:0] P = 4'bxxxx;\n\
             logic [3:0] a;\n\
             logic y;\n\
             assign y = a == 4'b0101;\n\
             assign y = a != P;\n\
             endmodule\n",
            "xz4",
        );
        assert!(
            diags.is_empty(),
            "parameter values are not followed: {diags:?}"
        );
    }

    #[test]
    fn identical_instance_findings_collapse_to_one() {
        let diags = rule_diags(
            "module child(input logic a, output logic y);\n\
             assign y = a == 1'bx;\n\
             endmodule\n\
             module top; logic a, y0, y1;\n\
             child c0(.a(a), .y(y0));\n\
             child c1(.a(a), .y(y1));\n\
             endmodule\n",
            "top",
        );
        assert_eq!(
            diags.len(),
            1,
            "cloned source finding must deduplicate: {diags:?}"
        );
    }
}
