//! Packed policy guards both folding and identity replacement.
use super::*;

fn digit(state: usize) -> IrExpr {
    masked_konst(u64::from(state == 1), u64::from(state == 2), u64::from(state == 3), 1)
}

fn mux(selector: IrExpr, a: IrExpr, b: IrExpr, width: u32) -> IrExpr {
    IrExpr::new(IrExprKind::Mux {
        sel: Box::new(selector), a: Box::new(a), b: Box::new(b),
    }, width, false, None)
}

#[test]
fn packed_conditional_folding_matches_the_independent_published_table() {
    let ambiguous = [
        [0, 2, 2, 2], [2, 1, 2, 2], [2, 2, 2, 2], [2, 2, 2, 2],
    ];
    for selector in 0..4 {
        for (left, row) in ambiguous.iter().enumerate() {
            for (right, merged) in row.iter().enumerate() {
                let expected = match selector { 0 => right, 1 => left, _ => *merged };
                let mut expression = mux(digit(selector), digit(left), digit(right), 1);
                fold_expr(&mut expression);
                assert_eq!(as_packed_const(&expression), as_packed_const(&digit(expected)),
                    "selector={selector} left={left} right={right}");
            }
        }
    }
    let mut dominant_one = mux(masked_konst(8, 4, 2, 4), zkonst(8), konst(0, 8), 8);
    fold_expr(&mut dominant_one);
    assert_eq!(as_packed_const(&dominant_one), as_packed_const(&zkonst(8)));
}

#[test]
fn ambiguous_identical_nonconstant_arms_cannot_be_an_identity() {
    let value = IrExpr::new(IrExprKind::FormalRead(0), 129, false, None);
    for selector in [digit(2), digit(3)] {
        let mut expression = mux(selector, value.clone(), value.clone(), 129);
        ident_expr(&mut expression);
        fold_expr(&mut expression);
        assert!(matches!(expression.kind, IrExprKind::Mux { .. }));
    }
    let mut known = mux(digit(1), value.clone(), value.clone(), 129);
    ident_expr(&mut known);
    assert_eq!(known, value);
}
