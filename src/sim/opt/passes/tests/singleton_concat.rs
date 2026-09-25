//! A singleton concat remains an unsigned, self-determined expression.
use super::*;

fn concat(part: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::Concat {
            parts: vec![part.clone()],
        },
        part.width,
        false,
        None,
    )
}

#[test]
fn singleton_concat_keeps_signed_child_boundary() {
    let child = IrExpr::new(IrExprKind::FormalRead(0), 4, true, None);
    let mut expression = IrExpr::convert_to(concat(child), 32, true);
    let original = expression.clone();
    ident_expr(&mut expression);
    assert_eq!(expression, original);
    // An assignment/explicit cast must see four unsigned bits, not signed -2.
    let IrExprKind::Convert { a } = &expression.kind else {
        panic!("the wider conversion must remain");
    };
    assert!(!a.signed);
    assert!(matches!(a.kind, IrExprKind::Concat { .. }));
}

#[test]
fn singleton_concat_removes_only_equivalent_unsigned_child() {
    let child = IrExpr::new(IrExprKind::FormalRead(0), 65, false, None);
    let mut expression = concat(child.clone());
    ident_expr(&mut expression);
    assert_eq!(expression, child);
}

#[test]
fn singleton_concat_does_not_expose_fill_markers() {
    for fill in 0..=3 {
        let mut literal = konst(u64::from(fill == 1), 1);
        let IrExprKind::Const(value) = &mut literal.kind else {
            unreachable!();
        };
        value.fill = Some(fill);
        for child in [
            literal,
            IrExpr::new(IrExprKind::Fill(fill), 1, false, None),
            IrExpr::new(IrExprKind::FormalRead(0), 1, false, Some(fill)),
        ] {
            let mut expression = concat(child);
            let original = expression.clone();
            ident_expr(&mut expression);
            assert_eq!(expression, original, "fill={fill}");
        }
    }
}
