//! `wide-index` — element, bit and part selects whose index is wider than 32
//! bits, and constant indices that no dynamically sized container accepts.
//!
//! A simulator never truncates an index: a value that does not fit in 32
//! bits, or a large unsigned value that a 32-bit signed reading would turn
//! negative, is out of bounds. A read then returns X (the type's default for
//! 2-state, string and real elements) and a write is ignored (IEEE 1800-2009
//! 7.4.6, 7.10.1, 11.5.1). A `logic [63:0] addr` index into a memory is the
//! realistic shape: the rule warns at the index, names its self-determined
//! width and says what the access does when the value is out of range.
//!
//! Checked positions: the element indices of unpacked arrays, queues, dynamic
//! arrays, strings and packed vectors; queue slice bounds; the base of an
//! indexed part select (`[base +: width]`). Associative-array keys are not
//! indices and are skipped, as are indices of unknown width (calls, unsized
//! literals), constants that fit in 32 bits, and bases whose type the owned
//! database does not carry.
//!
//! Constant indices: `out-of-range-select` and the frontend already report a
//! constant outside a fixed array or packed vector, so this rule leaves those
//! alone. It reports the constants nothing else can: a negative constant index
//! of a queue, dynamic array or string (always invalid) and a constant beyond
//! 32 bits there.
//!
//! Index arguments of the built-in methods that take one are checked too:
//! `insert` and `delete` of a queue, `getc`, `putc` and `substr` of a string.
//! Their formal is an `int`, but a wider actual is not converted: the call is
//! invalid (it changes nothing or returns the default), like the select form.

use std::collections::HashSet;

use crate::core::db::{ArrayKind, Db, ExprKind, NodeId, NodeKind, TypeShape};
use crate::core::lint::rules::analysis::{all_nodes, expr_width};
use crate::core::lint::rules::select_range::eval_integer;
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Widest index that is never narrowed.
const INDEX_BITS: u32 = 32;

/// Flags indices wider than 32 bits and always-invalid constant indices.
pub struct WideIndexRule;

/// What an index selects from, for the message and the constant policy.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    FixedArray,
    Queue,
    DynamicArray,
    Str,
    Packed,
    /// An indexed part select base (`[base +: width]`) of a packed value or
    /// a container.
    PartBase,
}

impl Target {
    fn noun(self) -> &'static str {
        match self {
            Target::FixedArray => "an unpacked array",
            Target::Queue => "a queue",
            Target::DynamicArray => "a dynamic array",
            Target::Str => "a string",
            Target::Packed => "a packed value",
            Target::PartBase => "an indexed part select",
        }
    }

    /// Whether nothing else reports a constant out of bounds: the size of a
    /// queue, dynamic array or string is not known statically.
    fn constant_unreported(self) -> bool {
        matches!(self, Target::Queue | Target::DynamicArray | Target::Str)
    }
}

impl LintRule for WideIndexRule {
    fn id(&self) -> &'static str {
        "wide-index"
    }

    fn description(&self) -> &'static str {
        "flags select indices wider than 32 bits, which are never truncated, and always-invalid constant indices"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        let mut seen: HashSet<(Option<String>, u32, u32, String)> = HashSet::new();
        let mut report = |node: NodeId, message: String| {
            let n = db.node(node);
            let (line, col) = (n.line.max(1), n.col.max(1));
            if seen.insert((n.file().map(str::to_owned), line, col, message.clone())) {
                out.push(LintDiag {
                    rule: "wide-index".to_string(),
                    logical: None,
                    severity: LintSeverity::Warning,
                    file: n.file().map(str::to_owned),
                    line,
                    col,
                    message,
                });
            }
        };
        for id in all_nodes(db) {
            match db.node_kind(id) {
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    for (index, target) in array_select_targets(db, *base, indices) {
                        check_index(db, index, target, None, &mut report);
                    }
                }
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    if let Some(target) = select_target(db, *base) {
                        check_index(db, *index, target, None, &mut report);
                    }
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    // Packed ranges are constant expressions; only a queue
                    // slice has run-time bounds.
                    if select_target(db, *base) == Some(Target::Queue) {
                        check_index(db, *left, Target::Queue, None, &mut report);
                        check_index(db, *right, Target::Queue, None, &mut report);
                    }
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect { base_expr, .. }) => {
                    check_index(db, *base_expr, Target::PartBase, None, &mut report);
                }
                NodeKind::MethodCall {
                    name,
                    receiver: Some(receiver),
                    ..
                } => {
                    let Some(target) = select_target(db, *receiver) else {
                        continue;
                    };
                    let args: Vec<NodeId> = db
                        .node(id)
                        .children
                        .iter()
                        .copied()
                        .filter(|child| child != receiver)
                        .collect();
                    let indices: &[NodeId] = match (name.as_str(), target, args.len()) {
                        ("insert", Target::Queue, 2) | ("delete", Target::Queue, 1) => &args[..1],
                        ("getc", Target::Str, 1) | ("putc", Target::Str, 2) => &args[..1],
                        ("substr", Target::Str, 2) => &args[..2],
                        _ => &[],
                    };
                    for index in indices {
                        check_index(db, *index, target, Some(name), &mut report);
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// Report `index` when it is wider than 32 bits or a constant no container
/// accepts. `report` receives the node to position at and the message.
fn check_index(
    db: &Db,
    index: NodeId,
    target: Target,
    method: Option<&str>,
    report: &mut impl FnMut(NodeId, String),
) {
    let index = strip_implicit_conversions(db, index);
    if let Some(value) = eval_integer(db, index) {
        if !target.constant_unreported() || (0..=i128::from(u32::MAX)).contains(&value) {
            return;
        }
        let why = if value < 0 {
            "is negative"
        } else {
            "does not fit in 32 bits"
        };
        report(
            index,
            match method {
                Some(method) => format!(
                    "constant index argument {value} of `{method}` on {} {why}: the call is \
                     always invalid (it changes nothing or returns the default)",
                    target.noun()
                ),
                None => format!(
                    "constant index {value} of {} {why}: the access is always invalid \
                     (a read returns X or the element default, a write is ignored)",
                    target.noun()
                ),
            },
        );
        return;
    }
    let Some(width) = expr_width(db, index) else {
        return;
    };
    if width <= INDEX_BITS {
        return;
    }
    if let Some(method) = method {
        report(
            index,
            format!(
                "index argument of `{method}` on {} is {width} bits wide: a value beyond 32 \
                 bits is not truncated, and an out-of-range index makes the call invalid (it \
                 changes nothing or returns the default)",
                target.noun()
            ),
        );
        return;
    }
    let subject = if target == Target::PartBase {
        "base of an indexed part select".to_owned()
    } else {
        format!("index of {}", target.noun())
    };
    report(
        index,
        format!(
            "{subject} is {width} bits wide: a value beyond 32 bits is not truncated, and an \
             out-of-range index makes the access invalid (a read returns X or the element \
             default, a write is ignored)"
        ),
    );
}

/// The index expression below any implicit conversion the frontend inserted.
fn strip_implicit_conversions(db: &Db, mut id: NodeId) -> NodeId {
    while db.is_implicit_conversion(id) {
        match db.node_kind(id) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => id = *operand,
            _ => break,
        }
    }
    id
}

/// The kind of value a single bit or element select indexes.
fn select_target(db: &Db, base: NodeId) -> Option<Target> {
    target_of(&db.type_descriptor(base)?.shape)
}

fn target_of(shape: &TypeShape) -> Option<Target> {
    match shape {
        TypeShape::PackedAtom { .. } => Some(Target::Packed),
        TypeShape::String => Some(Target::Str),
        TypeShape::FixedArray { .. } => Some(Target::FixedArray),
        TypeShape::Container { array, .. } => match array {
            ArrayKind::Queue { .. } => Some(Target::Queue),
            ArrayKind::Dynamic => Some(Target::DynamicArray),
            // An associative key is converted to the key type, not narrowed
            // to an index.
            ArrayKind::Associative(_) | ArrayKind::Static => None,
        },
        _ => None,
    }
}

/// Pair each index of a flattened array select with what it indexes, by
/// walking the base's type: a fixed unpacked array consumes one index per
/// dimension, a container one, and the remaining indices select within a
/// packed value or string. Associative keys are left out.
fn array_select_targets(db: &Db, base: NodeId, indices: &[NodeId]) -> Vec<(NodeId, Target)> {
    let mut out = Vec::new();
    let Some(mut descriptor) = db.type_descriptor(base) else {
        return out;
    };
    let mut next = 0;
    while next < indices.len() {
        match &descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                for _ in 0..dimensions.len() {
                    if let Some(index) = indices.get(next) {
                        out.push((*index, Target::FixedArray));
                        next += 1;
                    }
                }
                descriptor = element;
            }
            TypeShape::Container { array, element, .. } => {
                if let Some(target) = target_of(&descriptor.shape) {
                    out.push((indices[next], target));
                } else if !matches!(array, ArrayKind::Associative(_)) {
                    return out;
                }
                next += 1;
                descriptor = element;
            }
            TypeShape::PackedAtom { .. } | TypeShape::String => {
                let target = target_of(&descriptor.shape).unwrap_or(Target::Packed);
                out.extend(indices[next..].iter().map(|index| (*index, target)));
                break;
            }
            _ => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{build_design, lint_design, rule_diags};
    use crate::core::lint::{lint_with_config, LintConfig, RuleConfig};

    /// The findings of this rule for a module `t` whose body is `body`.
    fn findings(body: &str) -> Vec<String> {
        let sv = format!("module t;\n{body}\nendmodule\n");
        let diags = lint_design(&sv, "t");
        rule_diags(&diags, "wide-index")
            .into_iter()
            .map(|d| format!("{}:{} {}", d.line, d.col, d.message))
            .collect()
    }

    const DECLS: &str = "  logic [7:0] mem [0:15];\n  logic [7:0] mem2 [0:3][0:3];\n  \
        int q[$];\n  int d[];\n  string s;\n  logic [7:0] v;\n  logic [32:0] w33;\n  \
        logic [63:0] a64;\n  logic [64:0] w65;\n  int i;\n  int unsigned u;\n  int x;\n";

    #[test]
    fn wide_index_of_each_target_kind_warns_with_its_width() {
        for (stmt, noun, bits) in [
            ("x = mem[w33];", "an unpacked array", 33),
            ("x = mem[a64];", "an unpacked array", 64),
            ("x = q[a64];", "a queue", 64),
            ("x = d[w65];", "a dynamic array", 65),
            ("x = s[w33];", "a string", 33),
            ("x = v[a64];", "a packed value", 64),
            ("q[w33] = 1;", "a queue", 33),
            ("d[a64] = 1;", "a dynamic array", 64),
            ("v[a64] = 1'b1;", "a packed value", 64),
        ] {
            let got = findings(&format!("{DECLS}  initial begin\n    {stmt}\n  end"));
            assert_eq!(got.len(), 1, "{stmt}: {got:?}");
            assert!(
                got[0].contains(&format!("index of {noun} is {bits} bits wide")),
                "{stmt}: {}",
                got[0]
            );
            assert!(got[0].contains("is not truncated"), "{}", got[0]);
            assert!(got[0].contains("makes the access invalid"), "{}", got[0]);
        }
    }

    #[test]
    fn indexed_part_select_base_and_queue_slice_bounds_warn() {
        let got = findings(&format!(
            "{DECLS}  int r[$];\n  initial begin\n    v = a64[a64 +: 8];\n    r = q[0:a64];\n  end"
        ));
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(
            got.iter()
                .any(|m| m.contains("base of an indexed part select is 64 bits wide")),
            "{got:?}"
        );
        assert!(
            got.iter()
                .any(|m| m.contains("index of a queue is 64 bits wide")),
            "{got:?}"
        );
    }

    #[test]
    fn every_wide_index_of_a_multidimensional_array_warns() {
        let got = findings(&format!(
            "{DECLS}  initial begin\n    v = mem2[w33][a64];\n  end"
        ));
        assert_eq!(got.len(), 2, "{got:?}");
    }

    #[test]
    fn indices_of_32_bits_or_fewer_are_quiet() {
        let got = findings(&format!(
            "{DECLS}  initial begin\n    x = mem[i];\n    x = mem[u];\n    x = q[i + 1];\n    \
             x = d[v];\n    x = s[w33[4:0]];\n    x = mem[a64[3:0]];\n    x = mem[a64[31:0]];\n    \
             v = a64[i +: 8];\n    x = mem2[i][u];\n  end"
        ));
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn a_wide_expression_is_judged_by_its_self_determined_width() {
        let got = findings(&format!(
            "{DECLS}  initial begin\n    x = mem[a64 >> 4];\n    x = mem[{{1'b0, i}}];\n    \
             x = mem[(a64 >> 4) & 4'hf];\n  end"
        ));
        // A mask does not narrow: the operands extend to the widest (64 bits).
        assert_eq!(got.len(), 3, "{got:?}");
        assert_eq!(
            got.iter().filter(|m| m.contains("64 bits wide")).count(),
            2,
            "{got:?}"
        );
        assert!(got.iter().any(|m| m.contains("33 bits wide")), "{got:?}");
    }

    #[test]
    fn associative_array_keys_are_not_indices() {
        let got = findings(&format!(
            "{DECLS}  int aa [bit [63:0]];\n  int ab [logic [64:0]];\n  int ac [string];\n  \
             int ad [int][4];\n  int ae [string][$];\n  \
             initial begin\n    x = aa[a64];\n    x = ab[w65];\n    x = ac[\"k\"];\n    \
             x = ad[a64][1];\n    x = ae[\"k\"][i];\n  end"
        ));
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn an_index_below_an_associative_key_is_still_checked() {
        let got = findings(&format!(
            "{DECLS}  int ad [int][4];\n  int ae [string][$];\n  \
             initial begin\n    x = ad[i][a64];\n    x = ae[\"k\"][w33];\n  end"
        ));
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(
            got.iter()
                .any(|m| m.contains("index of an unpacked array is 64 bits wide")),
            "{got:?}"
        );
        assert!(
            got.iter()
                .any(|m| m.contains("index of a queue is 33 bits wide")),
            "{got:?}"
        );
    }

    #[test]
    fn a_constant_that_fits_is_quiet_whatever_its_literal_width() {
        let got = findings(&format!(
            "{DECLS}  localparam longint P = 64'd5;\n  initial begin\n    x = mem[64'd3];\n    \
             x = q[40'd7];\n    x = d[P];\n    x = s[32'hFFFF_FFFF];\n  end"
        ));
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn constants_no_dynamic_container_accepts_warn() {
        let got = findings(&format!(
            "{DECLS}  initial begin\n    x = q[-1];\n    x = d[-2];\n    x = s[-1];\n    \
             x = q[40'h1_0000_0003];\n    x = d[40'h1_0000_0003];\n  end"
        ));
        assert_eq!(got.len(), 5, "{got:?}");
        assert!(
            got.iter()
                .any(|m| m.contains("constant index -1 of a queue is negative")),
            "{got:?}"
        );
        assert!(
            got.iter()
                .any(|m| m.contains("constant index -2 of a dynamic array is negative")),
            "{got:?}"
        );
        assert!(
            got.iter()
                .any(|m| m.contains("constant index -1 of a string is negative")),
            "{got:?}"
        );
        assert!(
            got.iter().any(|m| m
                .contains("constant index 4294967299 of a queue does not fit in 32 bits")),
            "{got:?}"
        );
    }

    /// A constant outside a fixed array or packed vector is already reported
    /// by the frontend and `out-of-range-select`; this rule adds nothing.
    #[test]
    fn constants_outside_fixed_or_packed_objects_are_left_to_other_diagnostics() {
        let sv = format!(
            "module t;\n{DECLS}  initial begin\n    x = mem[16];\n    x = mem[-1];\n    \
             x = mem[40'h1_0000_0003];\n    x = v[8];\n    x = v[40'h1_0000_0003];\n  end\nendmodule\n"
        );
        let diags = lint_design(&sv, "t");
        assert!(rule_diags(&diags, "wide-index").is_empty(), "{diags:?}");
        assert!(
            !rule_diags(&diags, "out-of-range-select").is_empty(),
            "the existing rule still reports: {diags:?}"
        );
    }

    #[test]
    fn index_arguments_of_queue_and_string_methods_warn() {
        let got = findings(&format!(
            "{DECLS}  initial begin\n    q.insert(w33, 1);\n    q.delete(a64);\n    x = s.getc(w33);\n    \
             s.putc(a64, 8'h41);\n    s = s.substr(0, w65);\n    s = s.substr(a64, 3);\n  end"
        ));
        assert_eq!(got.len(), 6, "{got:?}");
        for needle in [
            "index argument of `insert` on a queue is 33 bits wide",
            "index argument of `delete` on a queue is 64 bits wide",
            "index argument of `getc` on a string is 33 bits wide",
            "index argument of `putc` on a string is 64 bits wide",
            "index argument of `substr` on a string is 65 bits wide",
            "index argument of `substr` on a string is 64 bits wide",
        ] {
            assert!(got.iter().any(|m| m.contains(needle)), "{needle}: {got:?}");
        }
        assert!(
            got.iter().all(|m| m.contains("makes the call invalid")),
            "{got:?}"
        );
    }

    #[test]
    fn method_arguments_that_fit_other_methods_and_unknown_widths_are_quiet() {
        let got = findings(&format!(
            "{DECLS}  function automatic longint wide_fn(); return 64'd1; endfunction\n  \
             int aa [bit [63:0]];\n  \
             initial begin\n    q.insert(i, 1);\n    q.delete(u);\n    x = s.getc(i);\n    \
             s = s.substr(0, 3);\n    q.delete();\n    aa.delete(a64);\n    x = aa.exists(a64);\n    \
             q.push_back(a64);\n    q.insert(64'd1, 1);\n    x = mem[wide_fn()];\n    x = mem['1];\n  end"
        ));
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn constant_method_index_arguments_no_container_accepts_warn() {
        let got = findings(&format!(
            "{DECLS}  initial begin\n    q.delete(-1);\n    x = s.getc(-2);\n    \
             q.insert(40'h1_0000_0003, 1);\n  end"
        ));
        assert_eq!(got.len(), 3, "{got:?}");
        assert!(
            got.iter().any(
                |m| m.contains("constant index argument -1 of `delete` on a queue is negative")
            ),
            "{got:?}"
        );
    }

    #[test]
    fn class_and_record_members_are_checked_by_their_container_kind() {
        let got = findings(&format!(
            "{DECLS}  typedef struct {{ int arr[4]; int q[$]; int aa[longint]; }} rec_t;\n  \
             rec_t r;\n  class C; int p[4]; int dq[$]; endclass\n  C c;\n  \
             initial begin\n    x = r.arr[a64];\n    x = r.q[w33];\n    x = r.aa[a64];\n    \
             c = new;\n    x = c.p[w33];\n    x = c.dq[a64];\n  end"
        ));
        assert_eq!(got.len(), 4, "{got:?}");
    }

    #[test]
    fn rule_is_registered_and_configurable() {
        let sv = format!("module t;\n{DECLS}  initial x = q[a64];\nendmodule\n");
        let diags = lint_design(&sv, "t");
        let got = rule_diags(&diags, "wide-index");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].severity, LintSeverity::Warning);

        let (db, model) = build_design(&sv, "t");
        let mut cfg = LintConfig::new();
        cfg.set(
            "wide-index",
            RuleConfig {
                enabled: false,
                severity: None,
            },
        );
        assert!(
            rule_diags(&lint_with_config(&db, &model, &cfg), "wide-index").is_empty(),
            "disabled rule reported"
        );
        let mut cfg = LintConfig::new();
        cfg.set(
            "wide-index",
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Error),
            },
        );
        let diags = lint_with_config(&db, &model, &cfg);
        let got = rule_diags(&diags, "wide-index");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].severity, LintSeverity::Error);
    }
}
