//! Structured emission must guard effects and retain one-bit truth owners.
use super::*;

fn predicate(clauses: Vec<IrExpr>) -> IrExpr {
    IrExpr::new(IrExprKind::Predicate { clauses }, 1, false, None)
}

#[test]
fn sequential_predicate_emission_guards_every_clause_and_releases_owners() {
    let model = numeric_model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(number(7, 65))],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    );
    let expr = predicate(vec![number(0, 1), call, number(1, 129)]);
    model.validate_expr(&expr, None).unwrap();
    let result = frame.expression(&expr).unwrap();
    assert_eq!(result.width, 1);
    assert!(!result.signed);
    frame.discard(result);
    let body = frame.body();
    assert_eq!(body.matches("if (sv4_to_bool(").count(), 3);
    assert_eq!(body.matches("sv4_reduce_or(").count(), 3);
    assert_eq!(body.matches("f_increment(").count(), 1);
    let guards: Vec<_> = body
        .match_indices("if (sv4_to_bool(")
        .map(|(index, _)| index)
        .collect();
    let call = body.find("f_increment(").unwrap();
    assert!(
        guards[1] < call && call < guards[2],
        "call must stay inside its clause guard"
    );
    assert!(body.matches("sv4_move(").count() >= 3);
    assert!(!body.contains("sv4_logand("));
    assert!(!body.contains("({"));
    assert!(frame.slots.iter().all(|live| !live));
}

#[test]
fn sequential_predicate_emission_normalizes_real_truth_without_rounding() {
    let model = IrModel::new("predicate_real".into(), 1).unwrap();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let expr = predicate(vec![
        IrExpr::new(IrExprKind::Const(IrConst::real(0.25)), 0, false, None),
        number(1, 1),
    ]);
    model.validate_expr(&expr, None).unwrap();
    let result = frame.expression(&expr).unwrap();
    frame.discard(result);
    let body = frame.body();
    assert!(body.contains("llg_real_to_bool("));
    assert!(!body.contains("sv4_from_real("));
    assert_eq!(body.matches("sv4_reduce_or(").count(), 1);
    assert!(frame.slots.iter().all(|live| !live));
}

#[test]
fn sequential_predicate_ambiguous_real_mux_evaluates_both_arms_and_yields_zero() {
    let model = IrModel::new("predicate_real_result".into(), 1).unwrap();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let real = |value| IrExpr::new(IrExprKind::Const(IrConst::real(value)), 0, false, None);
    let expr = IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(predicate(vec![number(1, 1), number(0, 1)])),
            a: Box::new(real(2.5)),
            b: Box::new(real(3.5)),
        },
        0,
        false,
        None,
    );
    model.validate_expr(&expr, None).unwrap();
    let result = frame.expression(&expr).unwrap();
    frame.discard(result);
    let body = frame.body();
    assert!(body.contains("} else if (!"));
    let ambiguous = &body[body.rfind("} else {").unwrap()..];
    assert!(ambiguous.contains("2.5") && ambiguous.contains("3.5"));
    assert!(ambiguous.contains("= 0.0;"));
    assert!(frame.slots.iter().all(|live| !live));
}
