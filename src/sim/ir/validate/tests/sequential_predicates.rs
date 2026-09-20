//! Every clause remains visible to validation and capacity accounting.
use super::*;

fn predicate(clauses: Vec<IrExpr>) -> IrExpr {
    IrExpr::new(IrExprKind::Predicate { clauses }, 1, false, None)
}

#[test]
fn sequential_predicate_ir_rejects_empty_and_noncanonical_results() {
    let model = valid_model();
    let mut wide = predicate(vec![packed_const(0, 1)]);
    wide.width = 2;
    let mut signed = predicate(vec![packed_const(0, 1)]);
    signed.signed = true;
    let mut fill = predicate(vec![packed_const(0, 1)]);
    fill.fill = Some(0);
    for expr in [predicate(vec![]), wide, signed, fill] {
        assert!(model.validate_expr(&expr, None).is_err());
    }
}

#[test]
fn sequential_predicate_ir_validates_all_clauses_and_counts_wide_inputs() {
    let model = valid_model();
    let expr = predicate(vec![
        packed_const(1, 1), packed_const(1, 129),
        IrExpr::new(IrExprKind::Const(IrConst::real(-0.25)), 0, false, None),
    ]);
    assert_eq!(model.expression_capacity(&expr, None).unwrap(), 129);
    for index in 0..3 {
        let mut bad = expr.clone();
        let IrExprKind::Predicate { clauses } = &mut bad.kind else { unreachable!(); };
        clauses[index] = IrExpr::new(IrExprKind::SigRead(999), 1, false, None);
        let error = model.validate_expr(&bad, None).unwrap_err().to_string();
        assert!(error.contains(&format!("clauses[{index}]")), "{error}");
    }
}
