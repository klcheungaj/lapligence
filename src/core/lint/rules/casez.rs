//! `casez-misuse` — overlapping wildcard items and constant selectors in
//! `casez`/`casex` statements.
//!
//! In a `casez`/`casex` the first matching item wins.  Two items whose
//! wildcard patterns overlap (some selector value matches both) make the later
//! item unreachable, which is usually a mistake: the first item claims values
//! the writer meant for the second.  When the statement has no default item
//! the rule warns; with a default the overlap is left alone (the default
//! catches what the writer missed).  A compile-time constant selector
//! (`casez (4'b0000)`) collapses the whole statement to a single always-true
//! item and is flagged regardless.
//!
//! Only literal item patterns (`ExprKind::Constant` with binary/octal/hex
//! values) participate; `x`, `z` and `?` digits are wildcards.  Item pairs
//! where either side is not a literal are skipped.

use crate::core::db::{CaseKind, Db, ExprKind, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::all_nodes;
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::ffi::vpi::ValueData;

/// Warns about overlapping wildcard items and constant selectors in
/// `casez`/`casex` statements.
pub struct CasezMisuseRule;

impl LintRule for CasezMisuseRule {
    fn id(&self) -> &'static str {
        "casez-misuse"
    }

    fn description(&self) -> &'static str {
        "flags overlapping wildcard items and constant selectors in casez/casex statements"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            if !matches!(db.node_kind(id), NodeKind::Process { .. }) {
                continue;
            }
            let mut cases = Vec::new();
            collect_wildcard_cases(db, id, &mut cases);
            for case in cases {
                check_case(db, case, &mut out);
            }
        }
        out
    }
}

/// Every `casez`/`casex` statement in the tree rooted at `root`.
fn collect_wildcard_cases(db: &Db, root: NodeId, out: &mut Vec<NodeId>) {
    if let NodeKind::Stmt(StmtKind::Case { case_type, .. }) = db.node_kind(root) {
        if matches!(case_type, CaseKind::X | CaseKind::Z) {
            out.push(root);
        }
    }
    for c in &db.node(root).children {
        collect_wildcard_cases(db, *c, out);
    }
}

fn check_case(db: &Db, case: NodeId, out: &mut Vec<LintDiag>) {
    let node = db.node(case);
    let at = |message: String| LintDiag {
        rule: "casez-misuse".to_string(),
        severity: LintSeverity::Warning,
        file: node.file.clone(),
        line: node.line,
        col: node.col,
        message,
    };
    let items = match &node.kind {
        NodeKind::Stmt(StmtKind::Case { items, .. }) => items,
        _ => return,
    };
    if let Some(sel) = node.children.first() {
        if is_const_expr(db, *sel) {
            out.push(at("constant selector in casez/casex".to_string()));
        }
    }
    // Overlap is only flagged when no default item exists.
    if items.iter().any(|it| it.exprs.is_empty()) {
        return;
    }
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            if item_pair_overlaps(db, &items[i].exprs, &items[j].exprs) {
                out.push(at(
                    "casez/casex items overlap (first match may be unintended); \
                     add a default or refine wildcards"
                        .to_string(),
                ));
                return;
            }
        }
    }
}

/// True when some expression of item `a` and some expression of item `b` are
/// both constant patterns that overlap.  Non-constant expressions are skipped.
fn item_pair_overlaps(db: &Db, a: &[NodeId], b: &[NodeId]) -> bool {
    for ea in a {
        let Some(pa) = pattern_of(db, *ea) else {
            continue;
        };
        for eb in b {
            let Some(pb) = pattern_of(db, *eb) else {
                continue;
            };
            if patterns_overlap(&pa, &pb) {
                return true;
            }
        }
    }
    false
}

/// One bit of a constant item pattern.
#[derive(Clone, Copy, PartialEq)]
enum Bit {
    Known(bool),
    Wild,
}

/// Parse a case-item constant into a bit pattern, MSB first.  Binary/octal/
/// hex literals are understood (`x`, `z`, `?` digits are wildcards); the
/// pattern is zero-extended to the literal's declared width.  Anything else
/// yields `None`.
fn pattern_of(db: &Db, id: NodeId) -> Option<Vec<Bit>> {
    let (value, size) = match db.node_kind(id) {
        NodeKind::Expr(ExprKind::Constant { value, size, .. }) => (value, *size),
        _ => return None,
    };
    let (digits, bits_per_digit): (&str, u32) = match value {
        ValueData::Bin(s) => (s, 1),
        ValueData::Oct(s) => (s, 3),
        ValueData::Hex(s) => (s, 4),
        _ => return None,
    };
    let mut out = Vec::new();
    for c in digits.chars() {
        if c == '_' {
            continue;
        }
        if matches!(c, 'x' | 'X' | 'z' | 'Z' | '?') {
            out.extend(std::iter::repeat_n(Bit::Wild, bits_per_digit as usize));
            continue;
        }
        let v = c.to_digit(16)?;
        if v >= (1 << bits_per_digit) {
            return None; // not a valid digit for this radix (e.g. octal 8/9)
        }
        for i in (0..bits_per_digit).rev() {
            out.push(Bit::Known((v >> i) & 1 == 1));
        }
    }
    if out.is_empty() {
        return None;
    }
    // A narrower item compares against a wider selector after unsigned
    // zero-extension on the left.
    let declared = size.max(out.len() as i32) as usize;
    if out.len() < declared {
        let mut padded = Vec::with_capacity(declared);
        padded.extend(std::iter::repeat_n(Bit::Known(false), declared - out.len()));
        padded.extend(out);
        out = padded;
    }
    Some(out)
}

/// True when two patterns both match some common selector value, compared
/// over the union width (shorter patterns zero-extend on the left).
fn patterns_overlap(a: &[Bit], b: &[Bit]) -> bool {
    let width = a.len().max(b.len());
    for i in 0..width {
        let ba = a.get(i).copied().unwrap_or(Bit::Known(false));
        let bb = b.get(i).copied().unwrap_or(Bit::Known(false));
        let compatible = match (ba, bb) {
            (Bit::Wild, _) | (_, Bit::Wild) => true,
            (Bit::Known(x), Bit::Known(y)) => x == y,
        };
        if !compatible {
            return false;
        }
    }
    true
}

/// True when the expression is a compile-time constant: a literal, or a cast
/// of one.  Anything containing a reference or an operation is not.
fn is_const_expr(db: &Db, id: NodeId) -> bool {
    match db.node_kind(id) {
        NodeKind::Expr(ExprKind::Constant { .. }) => true,
        NodeKind::Expr(ExprKind::Cast { operand, .. }) => is_const_expr(db, *operand),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn overlapping_items_without_default_warn() {
        let diags = lint_design(
            "module cz;\n\
             \x20 logic [3:0] sel;\n\
             \x20 logic x, y, a, b;\n\
             \x20 always_comb begin\n\
             \x20   casez (sel)\n\
             \x20     4'b1???: x = a;\n\
             \x20     4'b11??: y = b;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "cz",
        );
        let got = rule_diags(&diags, "casez-misuse");
        assert_eq!(got.len(), 1, "one overlap finding: {:?}", diags);
        assert_eq!(got[0].severity, LintSeverity::Warning);
        assert!(got[0].message.contains("items overlap"));
        assert_eq!(got[0].line, 5, "positioned at the case node");
    }

    #[test]
    fn overlapping_items_with_default_are_quiet() {
        let diags = lint_design(
            "module cwd;\n\
             \x20 logic [3:0] sel;\n\
             \x20 logic x, y, a, b;\n\
             \x20 always_comb begin\n\
             \x20   casez (sel)\n\
             \x20     4'b1???: x = a;\n\
             \x20     4'b11??: y = b;\n\
             \x20     default: x = b;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "cwd",
        );
        let got = rule_diags(&diags, "casez-misuse");
        assert!(got.is_empty(), "no findings: {:?}", diags);
    }

    #[test]
    fn constant_selector_warns() {
        let diags = lint_design(
            "module csc;\n\
             \x20 logic x, a;\n\
             \x20 always_comb begin\n\
             \x20   casez (4'b0000)\n\
             \x20     4'b0000: x = a;\n\
             \x20     default: x = a;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "csc",
        );
        let got = rule_diags(&diags, "casez-misuse");
        assert_eq!(got.len(), 1, "one constant-selector finding: {:?}", diags);
        assert!(got[0].message.contains("constant selector"));
    }
}
