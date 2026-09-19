//! Chain shape, capacity and recursive dependency validation.
use super::*;

fn constant(value: u64, width: u32) -> IrExpr {
    IrExpr::new(IrExprKind::Const(
        IrConst::packed(vec![value], vec![], vec![], width, false, None).unwrap(),
    ), width, false, None)
}
fn fixture() -> (IrModel, IrLhs) {
    let mut model = IrModel::new("selection_validation".to_owned(), 1).unwrap();
    model.arrays.push(IrArray::new("memory".to_owned(), "memory".to_owned(), 16, false, vec![(0, 0)]).unwrap());
    let lhs = IrLhs::ArrayElem {
        arr: 0, indices: vec![constant(0, 32)],
        elem_sel: IrElemSel::PackedChain(vec![
            IrPackedSelect { base: constant(0, 129), width: 8 },
            IrPackedSelect { base: constant(6, 32), width: 4 },
        ]),
    };
    (model, lhs)
}
fn statement(lhs: IrLhs) -> IrStmt {
    IrStmt::Assign { lhs, rhs: constant(15, 4), nba: false }
}
#[test]
fn packed_selection_chain_visits_all_address_operands() {
    let (model, mut lhs) = fixture();
    assert_eq!(model.statement_capacity(&statement(lhs.clone()), None).unwrap(), 129);
    let mut count = 0;
    lhs.expressions(&mut |_| count += 1);
    assert_eq!(count, 3);
    lhs.expressions_mut(&mut |expression| {
        if expression.width == 129 { *expression = IrExpr::new(IrExprKind::SigRead(7), 129, false, None); }
    });
    let error = model.validate_stmt(&statement(lhs), None).unwrap_err();
    assert!(error.path().contains("steps[0].base"));
    assert!(error.detail().contains("signal index 7"));
}
#[test]
fn packed_selection_chain_rejects_empty_zero_and_nonintegral_steps() {
    for mode in 0..3 {
        let (model, mut lhs) = fixture();
        let IrLhs::ArrayElem { elem_sel: IrElemSel::PackedChain(steps), .. } = &mut lhs else { unreachable!() };
        if mode == 0 { steps.clear(); }
        else if mode == 1 { steps[0].width = 0; }
        else { steps[0].base = IrExpr::new(IrExprKind::Const(IrConst {
            bits: vec![], x: vec![], z: vec![], width: 0, signed: true, real: Some(1.0), fill: None,
        }), 0, true, None); }
        assert!(model.validate_stmt(&statement(lhs), None).is_err());
    }
}
#[test]
fn packed_selection_read_checks_its_final_width() {
    let (model, lhs) = fixture();
    let IrLhs::ArrayElem { arr, indices, elem_sel } = lhs else { unreachable!() };
    let read = IrExpr::new(IrExprKind::ArrayRead { arr, indices, elem_sel }, 9, false, None);
    let error = model.validate_stmt(&IrStmt::Assign {
        lhs: IrLhs::ArrayElem { arr: 0, indices: vec![constant(0, 32)], elem_sel: IrElemSel::Whole },
        rhs: read, nba: false,
    }, None).unwrap_err();
    assert!(error.detail().contains("result shape disagrees"));
}
