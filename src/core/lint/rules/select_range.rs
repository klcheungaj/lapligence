//! `out-of-range-select` — statically provable select bounds violations.
//!
//! The owned database keeps unpacked dimensions on their exact array node and
//! packed dimensions in an elaborated-instance/object projection.  This rule
//! uses those two projections only; it never infers a range from a width or
//! from another instance.  Unknown selectors, unresolved dimensions,
//! unsupported expressions and ambiguous multi-dimensional packed types are
//! consequently quiet.

#![allow(non_upper_case_globals)] // VPI operator constants use this style.

use std::collections::HashSet;

use crate::core::db::{Db, ExprKind, NodeId, NodeKind, Operation};
use crate::core::elab::{Bit, Val, Value};
use crate::core::lint::rules::analysis::{all_nodes, scope_path};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::ffi::vpi::{self, ValueData};

/// Flags bit, part and indexed-part selects whose known bounds are outside
/// the selected object's declared packed or unpacked dimensions.
pub struct OutOfRangeSelectRule;

impl LintRule for OutOfRangeSelectRule {
    fn id(&self) -> &'static str {
        "out-of-range-select"
    }

    fn description(&self) -> &'static str {
        "flags statically provable bit and part selects outside declared bounds"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let ids = all_nodes(db);
        let nested_array_selects = nested_array_element_selects(db, &ids);
        let mut visited = HashSet::new();
        let mut diagnostics_seen = HashSet::new();
        let mut out = Vec::new();

        for id in ids {
            if !visited.insert(id) || nested_array_selects.contains(&id) {
                continue;
            }
            let violation = match db.node_kind(id) {
                NodeKind::Expr(ExprKind::ArraySelect { base, indices, .. }) => {
                    check_array_select(db, *base, indices)
                }
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    check_bit_or_array_select(db, *base, *index)
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    check_part_select(db, *base, *left, *right)
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => check_indexed_part_select(db, *base, *base_expr, *width_expr, *neg),
                _ => None,
            };
            if let Some(violation) = violation {
                let node = db.node(id);
                let line = node.line.max(1);
                let col = node.col.max(1);
                let message = violation.message();
                // Include the rendered violation evidence in the key.  Two
                // parameterized instances can share a source location while
                // having different elaborated bounds; those findings must
                // remain visible to the caller.
                if !diagnostics_seen.insert((node.file.clone(), line, col, message.clone())) {
                    continue;
                }
                out.push(LintDiag {
                    rule: "out-of-range-select".to_string(),
                    severity: LintSeverity::Warning,
                    file: node.file.clone(),
                    line,
                    col,
                    message,
                });
            }
        }
        out
    }
}

/// Description of one bad selector.  Keeping the original declaration
/// orientation in the diagnostic makes ascending and descending ranges easy
/// to distinguish.
struct SelectViolation {
    kind: &'static str,
    requested: String,
    declared_left: i128,
    declared_right: i128,
}

impl SelectViolation {
    fn message(self) -> String {
        format!(
            "{kind} {requested} is outside declared bounds [{left}:{right}]",
            kind = self.kind,
            requested = self.requested,
            left = self.declared_left,
            right = self.declared_right,
        )
    }
}

/// Check a simple bit select.  A direct bit select whose base is an unpacked
/// array is the one-dimensional array-select shape emitted by some UHDM
/// objects; ordinary packed selects use the owned packed-range projection.
fn check_bit_or_array_select(db: &Db, base: NodeId, index: NodeId) -> Option<SelectViolation> {
    let object = declaration_object(db, base)?;
    if matches!(db.node_kind(object), NodeKind::Array { .. }) {
        if let Some(meta) = db.array_meta(object) {
            let bounds = meta.dims.first().copied().flatten()?;
            let value = eval_integer(db, index)?;
            if !within(value, bounds.0 as i128, bounds.1 as i128) {
                return Some(SelectViolation {
                    kind: "array select",
                    requested: format!("index {value}"),
                    declared_left: bounds.0 as i128,
                    declared_right: bounds.1 as i128,
                });
            }
            return None;
        }
    }
    let bounds = packed_bounds(db, object)?;
    let value = eval_integer(db, index)?;
    (!within(value, bounds.0, bounds.1)).then_some(SelectViolation {
        kind: "bit select",
        requested: format!("index {value}"),
        declared_left: bounds.0,
        declared_right: bounds.1,
    })
}

/// Check an ordinary packed part select.  Both endpoints must be known and
/// within the numeric interval of the declaration.
fn check_part_select(
    db: &Db,
    base: NodeId,
    left: NodeId,
    right: NodeId,
) -> Option<SelectViolation> {
    let object = declaration_object(db, base)?;
    let bounds = packed_bounds(db, object)?;
    let left = eval_integer(db, left)?;
    let right = eval_integer(db, right)?;
    if within(left, bounds.0, bounds.1) && within(right, bounds.0, bounds.1) {
        return None;
    }
    Some(SelectViolation {
        kind: "part select",
        requested: format!("range [{left}:{right}]"),
        declared_left: bounds.0,
        declared_right: bounds.1,
    })
}

/// Check an indexed part select (`+:` or `-:`), including its computed end
/// point.  The direction of an indexed select is independent of the
/// declaration's ascending/descending spelling, so numeric interval checks
/// cover both declaration orientations.
fn check_indexed_part_select(
    db: &Db,
    base: NodeId,
    base_expr: NodeId,
    width_expr: NodeId,
    neg: bool,
) -> Option<SelectViolation> {
    let object = declaration_object(db, base)?;
    let bounds = packed_bounds(db, object)?;
    let base_value = eval_integer(db, base_expr)?;
    let width = eval_integer(db, width_expr)?;
    if width <= 0 {
        return None;
    }
    let offset = width.checked_sub(1)?;
    let end = if neg {
        base_value.checked_sub(offset)?
    } else {
        base_value.checked_add(offset)?
    };
    if within(base_value, bounds.0, bounds.1) && within(end, bounds.0, bounds.1) {
        return None;
    }
    let direction = if neg { "-:" } else { "+:" };
    Some(SelectViolation {
        kind: "indexed part select",
        requested: format!("range [{base_value}:{end}] ({base_value} {direction} width {width})"),
        declared_left: bounds.0,
        declared_right: bounds.1,
    })
}

/// Check every statically known array dimension and, when the shape is the
/// documented one-extra-index form, the packed select of the array element.
fn check_array_select(db: &Db, base: NodeId, indices: &[NodeId]) -> Option<SelectViolation> {
    let object = declaration_object(db, base)?;
    let meta = db.array_meta(object)?;

    for (dimension, bounds) in meta.dims.iter().enumerate() {
        let Some(index) = indices.get(dimension) else {
            break;
        };
        let Some((left, right)) = bounds else {
            continue;
        };
        let Some(value) = eval_integer(db, *index) else {
            continue;
        };
        if !within(value, *left as i128, *right as i128) {
            return Some(SelectViolation {
                kind: "array select",
                requested: format!("index {value} (dimension {})", dimension + 1),
                declared_left: *left as i128,
                declared_right: *right as i128,
            });
        }
    }

    // A var_select for `array[index][element_select]` carries one extra
    // trailing index.  Only consume exactly one extra index; anything more
    // has no unambiguous selector-to-dimension mapping in the current model.
    if indices.len() != meta.dims.len() + 1 {
        return None;
    }
    let extra = *indices.last()?;
    match db.node_kind(extra) {
        NodeKind::Expr(ExprKind::BitSelect { base, index })
            if same_declaration_object(db, *base, object) =>
        {
            check_packed_bit_select(db, object, *index)
        }
        NodeKind::Expr(ExprKind::PartSelect { base, left, right })
            if same_declaration_object(db, *base, object) =>
        {
            check_packed_part_select(db, object, *left, *right)
        }
        NodeKind::Expr(ExprKind::IndexedPartSelect {
            base,
            base_expr,
            width_expr,
            neg,
        }) if same_declaration_object(db, *base, object) => {
            check_packed_indexed_part_select(db, object, *base_expr, *width_expr, *neg)
        }
        _ => {
            // In the compact UHDM shape, the trailing element bit index is a
            // plain constant/ref rather than a nested bit_select.
            check_packed_bit_select(db, object, extra)
        }
    }
}

fn check_packed_bit_select(db: &Db, object: NodeId, index: NodeId) -> Option<SelectViolation> {
    let bounds = packed_bounds(db, object)?;
    let value = eval_integer(db, index)?;
    (!within(value, bounds.0, bounds.1)).then_some(SelectViolation {
        kind: "bit select",
        requested: format!("index {value}"),
        declared_left: bounds.0,
        declared_right: bounds.1,
    })
}

fn check_packed_part_select(
    db: &Db,
    object: NodeId,
    left: NodeId,
    right: NodeId,
) -> Option<SelectViolation> {
    let bounds = packed_bounds(db, object)?;
    let left = eval_integer(db, left)?;
    let right = eval_integer(db, right)?;
    if within(left, bounds.0, bounds.1) && within(right, bounds.0, bounds.1) {
        return None;
    }
    Some(SelectViolation {
        kind: "part select",
        requested: format!("range [{left}:{right}]"),
        declared_left: bounds.0,
        declared_right: bounds.1,
    })
}

fn check_packed_indexed_part_select(
    db: &Db,
    object: NodeId,
    base_expr: NodeId,
    width_expr: NodeId,
    neg: bool,
) -> Option<SelectViolation> {
    let bounds = packed_bounds(db, object)?;
    let base_value = eval_integer(db, base_expr)?;
    let width = eval_integer(db, width_expr)?;
    if width <= 0 {
        return None;
    }
    let offset = width.checked_sub(1)?;
    let end = if neg {
        base_value.checked_sub(offset)?
    } else {
        base_value.checked_add(offset)?
    };
    if within(base_value, bounds.0, bounds.1) && within(end, bounds.0, bounds.1) {
        return None;
    }
    let direction = if neg { "-:" } else { "+:" };
    Some(SelectViolation {
        kind: "indexed part select",
        requested: format!("range [{base_value}:{end}] ({base_value} {direction} width {width})"),
        declared_left: bounds.0,
        declared_right: bounds.1,
    })
}

/// Return all nested select nodes represented as the extra packed selector of
/// an array select.  They are checked through the outer ArraySelect so one
/// source select cannot produce two findings.
fn nested_array_element_selects(db: &Db, ids: &[NodeId]) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for id in ids {
        let NodeKind::Expr(ExprKind::ArraySelect { base, indices, .. }) = db.node_kind(*id) else {
            continue;
        };
        let Some(object) = declaration_object(db, *base) else {
            continue;
        };
        let Some(meta) = db.array_meta(object) else {
            continue;
        };
        if indices.len() != meta.dims.len() + 1 {
            continue;
        }
        let Some(extra) = indices.last() else {
            continue;
        };
        if is_select_node(db, *extra) && same_declaration_object(db, *extra, object) {
            out.insert(*extra);
        }
    }
    out
}

fn is_select_node(db: &Db, id: NodeId) -> bool {
    matches!(
        db.node_kind(id),
        NodeKind::Expr(ExprKind::BitSelect { .. })
            | NodeKind::Expr(ExprKind::PartSelect { .. })
            | NodeKind::Expr(ExprKind::IndexedPartSelect { .. })
    )
}

/// The declaration object behind a reference or a select base.  Port and
/// parameter nodes are included because the packed-range projection captures
/// their per-instance declarations too.
fn declaration_object(db: &Db, id: NodeId) -> Option<NodeId> {
    match db.node_kind(id) {
        NodeKind::Net { .. }
        | NodeKind::Var { .. }
        | NodeKind::Array { .. }
        | NodeKind::Port { .. }
        | NodeKind::Param { .. } => Some(id),
        NodeKind::Expr(ExprKind::Ref { target }) => {
            target.and_then(|target| is_declaration_object(db, target).then_some(target))
        }
        NodeKind::Expr(ExprKind::BitSelect { base, .. })
        | NodeKind::Expr(ExprKind::PartSelect { base, .. })
        | NodeKind::Expr(ExprKind::IndexedPartSelect { base, .. })
        | NodeKind::Expr(ExprKind::ArraySelect { base, .. }) => declaration_object(db, *base),
        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
            .iter()
            .rev()
            .flatten()
            .find_map(|target| is_declaration_object(db, *target).then_some(*target)),
        _ => None,
    }
}

fn is_declaration_object(db: &Db, id: NodeId) -> bool {
    matches!(
        db.node_kind(id),
        NodeKind::Net { .. }
            | NodeKind::Var { .. }
            | NodeKind::Array { .. }
            | NodeKind::Port { .. }
            | NodeKind::Param { .. }
    )
}

fn same_declaration_object(db: &Db, id: NodeId, expected: NodeId) -> bool {
    declaration_object(db, id) == Some(expected)
}

/// Find one exact packed dimension for one exact elaborated scope/object
/// identity.  Multi-dimensional packed objects are intentionally skipped in
/// v1 because a standalone select does not preserve enough mapping metadata.
fn packed_bounds(db: &Db, object: NodeId) -> Option<(i128, i128)> {
    if !is_declaration_object(db, object) {
        return None;
    }
    let node = db.node(object);
    let parent = node.parent?;
    let instance = scope_path(db, parent);
    if instance.is_empty() {
        return None;
    }

    let matching = db
        .elaborated_type_ranges()
        .iter()
        .filter(|entry| entry.instance == instance && entry.name == node.name)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return None;
    }
    let entry = matching[0];
    if entry.packed_ranges.len() != 1 {
        return None;
    }
    entry
        .packed_ranges
        .first()
        .copied()
        .flatten()
        .map(|range| (range.left, range.right))
}

fn within(value: i128, left: i128, right: i128) -> bool {
    value >= left.min(right) && value <= left.max(right)
}

/// Small, deliberately conservative integer evaluator for selector bounds.
/// It accepts only literal values, transparent casts, unary +/- and refs to
/// parameters whose values were resolved into the owned db.
fn eval_integer(db: &Db, id: NodeId) -> Option<i128> {
    match db.node_kind(id) {
        NodeKind::Expr(ExprKind::Constant { value, size, .. }) if *size > 0 => value_to_i128(value),
        NodeKind::Expr(ExprKind::Cast { operand, .. }) => eval_integer(db, *operand),
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => match db.node_kind(*target) {
            NodeKind::Param {
                value: Some(Val::Bits(value)),
                ..
            } => bits_to_i128(value),
            _ => None,
        },
        NodeKind::Expr(ExprKind::Operation { op, operands, .. }) if operands.len() == 1 => {
            let value = eval_integer(db, operands[0])?;
            match op {
                Operation::UnaryPlus => Some(value),
                Operation::UnaryMinus => value.checked_neg(),
                _ => None,
            }
        }
        _ => None,
    }
}

fn value_to_i128(value: &ValueData) -> Option<i128> {
    match value {
        ValueData::Int(value) => Some(*value as i128),
        ValueData::UInt(value) => Some(i128::from(*value)),
        ValueData::Bin(digits) => parse_radix(digits, 2),
        ValueData::Oct(digits) => parse_radix(digits, 8),
        ValueData::Hex(digits) => parse_radix(digits, 16),
        ValueData::Dec(digits) => digits
            .chars()
            .filter(|digit| *digit != '_')
            .collect::<String>()
            .parse::<i128>()
            .ok(),
        ValueData::Scalar(scalar) => match *scalar {
            vpi::vpi0 | vpi::vpiL => Some(0),
            vpi::vpi1 | vpi::vpiH => Some(1),
            _ => None,
        },
        _ => None,
    }
}

fn parse_radix(digits: &str, radix: u32) -> Option<i128> {
    let mut value = 0i128;
    let mut any = false;
    for digit in digits.chars() {
        if digit == '_' {
            continue;
        }
        let digit = digit.to_digit(radix)?;
        value = value
            .checked_mul(i128::from(radix))?
            .checked_add(i128::from(digit))?;
        any = true;
    }
    any.then_some(value)
}

fn bits_to_i128(value: &Value) -> Option<i128> {
    if value.bits.is_empty() || value.bits.iter().any(|bit| matches!(bit, Bit::X | Bit::Z)) {
        return None;
    }
    let signed = value.signed;
    let negative = signed && value.bits.first() == Some(&Bit::One);
    let width = value.bits.len();

    if width <= 127 {
        let raw = bits_to_u128(&value.bits)?;
        let raw = i128::try_from(raw).ok()?;
        if negative {
            let modulus = 1i128.checked_shl(width as u32)?;
            raw.checked_sub(modulus)
        } else {
            Some(raw)
        }
    } else {
        let prefix_len = width.saturating_sub(128);
        let sign_bit = if negative { Bit::One } else { Bit::Zero };
        if value.bits[..prefix_len].iter().any(|bit| *bit != sign_bit) {
            return None;
        }
        let raw = bits_to_u128(&value.bits[prefix_len..])?;
        if signed {
            Some(raw as i128)
        } else {
            i128::try_from(raw).ok()
        }
    }
}

fn bits_to_u128(bits: &[Bit]) -> Option<u128> {
    let mut value = 0u128;
    for bit in bits {
        value = value.checked_mul(2)?;
        if *bit == Bit::One {
            value = value.checked_add(1)?;
        }
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::build_design;

    fn rule_diags(sv: &str, top: &str) -> Vec<LintDiag> {
        let (db, model) = build_design(sv, top);
        OutOfRangeSelectRule.check(&LintCtx {
            db: &db,
            model: &model,
        })
    }

    #[test]
    fn descending_packed_bounds_and_part_selects() {
        let diags = rule_diags(
            "module sr1;\n\
             logic [15:8] a;\n\
             logic y;\n\
             assign y = a[8];\n\
             assign y = a[15];\n\
             assign y = a[7];\n\
             assign y = a[16];\n\
             assign y = a[15:8];\n\
             assign y = a[16:8];\n\
             assign y = a[15:7];\n\
             endmodule\n",
            "sr1",
        );
        assert_eq!(diags.len(), 4, "only outside endpoints warn: {diags:?}");
        assert!(diags.iter().all(|d| d.severity == LintSeverity::Warning));
        assert!(diags.iter().all(|d| d.message.contains("[15:8]")));
    }

    #[test]
    fn ascending_packed_and_indexed_boundaries() {
        let diags = rule_diags(
            "module sr2;\n\
             logic [0:7] a;\n\
             logic y;\n\
             assign y = a[0];\n\
             assign y = a[7];\n\
             assign y = a[8];\n\
             assign y = a[0 +: 8];\n\
             assign y = a[7 -: 8];\n\
             assign y = a[1 +: 8];\n\
             assign y = a[6 -: 8];\n\
             endmodule\n",
            "sr2",
        );
        assert_eq!(
            diags.len(),
            3,
            "indexed endpoints use numeric bounds: {diags:?}"
        );
        assert!(diags.iter().any(|d| d.message.contains("index 8")));
    }

    #[test]
    fn nonzero_unpacked_bounds_are_checked() {
        let diags = rule_diags(
            "module sr3;\n\
             logic [7:0] mem [3:5];\n\
             logic y;\n\
             assign y = mem[3][0];\n\
             assign y = mem[5][7];\n\
             assign y = mem[2][0];\n\
             assign y = mem[6][0];\n\
             endmodule\n",
            "sr3",
        );
        assert_eq!(
            diags.len(),
            2,
            "only array indices 2 and 6 are outside: {diags:?}"
        );
        assert!(diags
            .iter()
            .all(|d| d.message.contains("[3:5]") || d.message.contains("[7:0]")));
    }

    #[test]
    fn dynamic_unknown_and_overflowing_selectors_are_quiet() {
        let diags = rule_diags(
            "module sr4;\n\
             logic [7:0] a;\n\
             logic [3:0] index;\n\
             logic y;\n\
             assign y = a[index];\n\
             assign y = a[1'bx];\n\
             assign y = a[1 + index];\n\
             assign y = a[128'hffffffffffffffffffffffffffffffff];\n\
             endmodule\n",
            "sr4",
        );
        assert!(
            diags.is_empty(),
            "uncertain/overflowing values are skipped: {diags:?}"
        );
    }

    #[test]
    fn parameterized_instance_ranges_are_not_crossed() {
        let diags = rule_diags(
            "module sr_child #(parameter W = 8) (output logic out);\n\
             logic [W-1:0] data;\n\
             assign out = data[W-1];\n\
             assign out = data[W];\n\
             endmodule\n\
             module sr_top;\n\
             logic a, b;\n\
             sr_child #(.W(8)) u8 (.out(a));\n\
             sr_child #(.W(4)) u4 (.out(b));\n\
             endmodule\n",
            "sr_top",
        );
        assert_eq!(
            diags.len(),
            2,
            "one W endpoint per exact instance range: {diags:?}"
        );
        assert!(diags.iter().all(|d| d.message.contains("is outside")));
    }

    #[test]
    fn identical_parameterized_instance_findings_collapse_to_one() {
        let diags = rule_diags(
            "module sr_child #(parameter W = 8) (output logic out);\n\
             logic [W-1:0] data;\n\
             assign out = data[W];\n\
             endmodule\n\
             module sr_top; logic a, b;\n\
             sr_child #(.W(8)) u0 (.out(a));\n\
             sr_child #(.W(8)) u1 (.out(b));\n\
             endmodule\n",
            "sr_top",
        );
        assert_eq!(
            diags.len(),
            1,
            "identical source/bounds must deduplicate: {diags:?}"
        );
    }
}
