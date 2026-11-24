//! The identity rules run at one node; the expression walk brings them to every
//! node, operands first.
use super::*;

fn identity_resize(a: IrExpr) -> IrExpr {
    let (width, signed) = (a.width, a.signed);
    IrExpr::new(IrExprKind::Resize { a: Box::new(a) }, width, signed, None)
}

fn sum(a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::Bin {
            op: IrBinOp::Add,
            a: Box::new(a),
            b: Box::new(b),
        },
        8,
        false,
        None,
    )
}

fn formal(index: usize) -> IrExpr {
    IrExpr::new(IrExprKind::FormalRead(index), 8, false, None)
}

/// A rule that also rewrote its operands would visit each node once per
/// enclosing node.
#[test]
fn an_identity_leaves_the_operands_to_the_walk() {
    let original = sum(identity_resize(formal(0)), formal(1));
    let mut expression = original.clone();
    ident_expr(&mut expression);
    assert_eq!(expression, original);
    walk_expr_mut(&mut expression, &mut ident_expr);
    assert_eq!(expression, sum(formal(0), formal(1)));
}

#[test]
fn one_walk_removes_nested_identities() {
    let mut expression = sum(formal(0), formal(1));
    for level in 0..32 {
        expression = identity_resize(expression);
        if level % 4 == 0 {
            expression = sum(expression, identity_resize(identity_resize(formal(2))));
        }
    }
    walk_expr_mut(&mut expression, &mut ident_expr);
    let expected = (0..8).fold(sum(formal(0), formal(1)), |a, _| sum(a, formal(2)));
    assert_eq!(expression, expected);
}
