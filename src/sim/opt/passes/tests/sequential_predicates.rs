//! `&&&` has ordered, ambiguity-stopping semantics, not logical AND semantics.
use super::*;

fn predicate(clauses: Vec<IrExpr>) -> IrExpr {
    IrExpr::new(IrExprKind::Predicate { clauses }, 1, false, None)
}

fn state(value: u32) -> IrExpr {
    match value {
        0 | 1 => konst(u64::from(value), 1),
        2 => masked_konst(0, 1, 0, 1),
        _ => masked_konst(0, 0, 1, 1),
    }
}

#[test]
fn sequential_predicate_fold_all_sixty_four_three_clause_combinations() {
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                let expected = [a, b, c].into_iter().find(|value| *value != 1).unwrap_or(1);
                let mut expr = predicate(vec![state(a), state(b), state(c)]);
                fold_expr(&mut expr);
                let result = as_packed_const(&expr).unwrap();
                let expected = if expected == 3 { 2 } else { expected };
                assert_eq!(result.bits, as_packed_const(&state(expected)).unwrap().bits);
                assert_eq!(expr.width, 1);
                assert!(!expr.signed);
                assert_eq!(expr.fill, None);
            }
        }
    }
}

#[test]
fn sequential_predicate_fold_normalizes_wide_real_and_fill_clauses() {
    let wide = IrExpr::new(
        IrExprKind::Const(IrConst::packed(vec![0, 0, 1], vec![1], vec![], 129, true, None).unwrap()),
        129, true, None,
    );
    for clauses in [
        vec![wide.clone(), konst(1, 1)],
        vec![rkonst(-0.25), wide],
        vec![IrExpr::new(IrExprKind::Fill(1), 65, false, Some(1)), konst(1, 1)],
    ] {
        let mut expr = predicate(clauses);
        fold_expr(&mut expr);
        assert_eq!(as_packed_const(&expr).unwrap().to_u64(), Some(1));
    }
    let mut expr = predicate(vec![rkonst(-0.0), state(2)]);
    fold_expr(&mut expr);
    assert_eq!(as_packed_const(&expr).unwrap().to_u64(), Some(0));
}

fn call() -> IrExpr {
    IrExpr::new(IrExprKind::CallFn(Box::new(crate::sim::ir::IrCallExpr::new(
        0, vec![], IrDepth::PROC, false,
    ))), 1, false, None)
}

#[test]
fn sequential_predicate_fold_can_drop_only_an_unreached_suffix() {
    for decisive in [0, 2, 3] {
        let mut expr = predicate(vec![konst(1, 1), state(decisive), call()]);
        fold_expr(&mut expr);
        let expected = if decisive == 0 { state(0) } else { state(2) };
        assert_eq!(as_packed_const(&expr).unwrap().bits, as_packed_const(&expected).unwrap().bits);
    }
    let mut expr = predicate(vec![call(), konst(0, 1)]);
    fold_expr(&mut expr);
    ident_expr(&mut expr);
    assert!(matches!(expr.kind, IrExprKind::Predicate { .. }),
        "a later zero must not discard a reached effectful clause");
}

#[test]
fn sequential_predicate_identity_never_turns_a_vector_clause_into_a_vector_result() {
    let mut expr = predicate(vec![IrExpr::new(IrExprKind::SigRead(0), 8, false, None)]);
    ident_expr(&mut expr);
    assert!(matches!(expr.kind, IrExprKind::Predicate { .. }));
    assert_eq!(expr.width, 1);
}

#[test]
fn sequential_predicate_walkers_keep_late_reads_and_visit_clauses() {
    let mut model = IrModel::new("predicate_walk".into(), 1).unwrap();
    model.signals = sigs(4);
    model.processes.push(IrProcess::new(
        "p".into(), "p".into(), IrShape::RunOnce, vec![], vec![assign(
            IrLhs::Whole(3), predicate((0..3).map(|index|
                IrExpr::new(IrExprKind::SigRead(index), 8, false, None)).collect()),
        )],
    ));
    model.validate().unwrap();
    mark_unused_storage(&mut model, None);
    assert!(model.signals.iter().all(|signal| !signal.omit));
    let mut visits = 0;
    walk_model_exprs_mut(&mut model, &mut |expr| {
        if matches!(expr.kind, IrExprKind::SigRead(_)) { visits += 1; }
    });
    assert_eq!(visits, 3);
}
