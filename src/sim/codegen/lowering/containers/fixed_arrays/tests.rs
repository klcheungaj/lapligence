//! Positional deconstruction captures coordinates before publishing any leaf.
use super::*;
use crate::sim::ir::{IrCallExpr, IrDepth, IrPackedSelect};

fn local_target(name: &str, width: u32) -> IrLhs {
    IrLhs::WholeRef {
        addr: format!("&{name}"),
        width,
        signed: true,
        two_state: false,
        shortreal: false,
    }
}

fn selected_target(name: &str, selector: IrExpr) -> IrLhs {
    IrLhs::PackedSelect {
        target: Box::new(local_target(name, 64)),
        steps: vec![IrPackedSelect {
            base: selector,
            width: 32,
        }],
        signed: true,
        two_state: false,
    }
}

#[test]
fn positional_targets_capture_old_selector_values_as_a_group() {
    let db = Db::from_test_nodes("pattern", Vec::new(), Vec::new(), HashMap::new()).unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    let original = IrExpr::new(IrExprKind::LocalRead("index".to_owned()), 32, true, None);
    let mut statements = Vec::new();
    let targets = cg
        .capture_pattern_lvalue_targets(
            vec![
                local_target("index", 32),
                selected_target("memory", original.clone()),
            ],
            "tuple",
            &mut statements,
        )
        .unwrap();
    assert_eq!(statements.len(), 1);
    let IrStmt::DeclLocal {
        name,
        init: Some(init),
        ..
    } = &statements[0]
    else {
        panic!("all coordinate captures must precede every destination write");
    };
    assert_eq!(init.as_ref(), &original);
    assert_eq!(targets[0], local_target("index", 32));
    let IrLhs::PackedSelect { steps, .. } = &targets[1] else {
        panic!("selected target")
    };
    assert_eq!(steps[0].base.kind, IrExprKind::LocalRead(name.clone()));
    assert_ne!(steps[0].base, original);
}

#[test]
fn positional_target_capture_keeps_repeated_source_occurrences_distinct() {
    let db = Db::from_test_nodes("pattern", Vec::new(), Vec::new(), HashMap::new()).unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    let selector = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            Vec::new(),
            IrDepth::FUNC,
            false,
        ))),
        32,
        true,
        None,
    );
    let nested = IrLhs::Stream {
        parts: vec![
            (selected_target("left", selector.clone()), 32),
            (selected_target("right", selector.clone()), 32),
        ],
        width: 64,
        slice: 1,
        direction: IrStreamDirection::LeftToRight,
    };
    let mut statements = Vec::new();
    let frozen = cg
        .capture_pattern_lvalue_targets(vec![nested], "nested", &mut statements)
        .unwrap();
    assert_eq!(
        statements.len(),
        2,
        "equal selector syntax is not one evaluation"
    );
    let mut names = Vec::new();
    for statement in &statements {
        let IrStmt::DeclLocal {
            name,
            init: Some(init),
            ..
        } = statement
        else {
            panic!("capture")
        };
        assert_eq!(init.as_ref(), &selector);
        names.push(name.clone());
    }
    assert_ne!(names[0], names[1]);
    let IrLhs::Stream { parts, .. } = &frozen[0] else {
        panic!("stream retained")
    };
    for ((part, _), name) in parts.iter().zip(names) {
        let IrLhs::PackedSelect { steps, .. } = part else {
            panic!("selected leaf")
        };
        assert_eq!(steps[0].base.kind, IrExprKind::LocalRead(name));
    }
}

#[test]
fn failed_positional_capture_does_not_publish_a_partial_prefix() {
    let db = Db::from_test_nodes("pattern", Vec::new(), Vec::new(), HashMap::new()).unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    let good = selected_target("a", lhs_integer_expr(0));
    let bad = selected_target("b", IrExpr::new(IrExprKind::FormalRead(0), 0, false, None));
    let mut statements = Vec::new();
    assert!(cg
        .capture_pattern_lvalue_targets(vec![good, bad], "invalid", &mut statements)
        .is_err());
    assert!(
        statements.is_empty(),
        "no incomplete destination prefix may escape"
    );
}

fn cell(name: &str, width: u32) -> IrExpr {
    IrExpr::new(IrExprKind::LocalRead(name.to_owned()), width, false, None)
}

fn part_of(value: &IrExpr) -> (String, i64, i64) {
    let IrExprKind::PartSel { base, left, right } = &value.kind else {
        panic!("expected a part select, got {value:?}");
    };
    let IrExprKind::LocalRead(name) = &base.kind else {
        panic!("expected a local base, got {base:?}");
    };
    (name.clone(), *left, *right)
}

/// IEEE 1800-2009 10.10: a row target joins consecutive source cells into
/// its packed image, while a packed leaf still takes part of one value.
#[test]
fn split_joins_cells_into_row_targets_and_splits_wide_values() {
    let values = Codegen::p30_split_pattern_source_values(
        "tb",
        vec![
            cell("c0", 8),
            cell("c1", 8),
            cell("c2", 8),
            cell("wide", 16),
        ],
        &[(16, false), (8, true), (8, false), (8, false)],
    )
    .expect("aligned row and leaf targets");
    assert_eq!(values.len(), 4);
    let IrExprKind::Concat { parts } = &values[0].kind else {
        panic!("a 16-bit row joins two cells: {:?}", values[0]);
    };
    assert_eq!(values[0].width, 16);
    let parts: Vec<_> = parts.iter().map(part_of).collect();
    assert_eq!(
        parts,
        [("c0".to_owned(), 7, 0), ("c1".to_owned(), 7, 0)],
        "the leftmost cell is the most significant"
    );
    assert_eq!(part_of(&values[1]), ("c2".to_owned(), 7, 0));
    assert!(
        values[1].signed,
        "a whole single cell keeps the target sign"
    );
    assert_eq!(part_of(&values[2]), ("wide".to_owned(), 15, 8));
    assert_eq!(part_of(&values[3]), ("wide".to_owned(), 7, 0));
}

#[test]
fn split_rejects_unconsumed_or_missing_source_bits() {
    let extra = Codegen::p30_split_pattern_source_values(
        "tb",
        vec![cell("a", 8), cell("b", 8)],
        &[(8, false)],
    )
    .expect_err("a source cell is left over");
    assert!(extra.contains("fewer RHS positions"), "{extra}");
    let missing =
        Codegen::p30_split_pattern_source_values("tb", vec![cell("a", 8)], &[(16, false)])
            .expect_err("the row needs two cells");
    assert!(missing.contains("RHS supplies only 0 values"), "{missing}");
}
