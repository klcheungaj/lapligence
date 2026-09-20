//! Array merge plans are checked at the public IR boundary.
use super::*;

fn default(width: u32) -> IrConst {
    IrConst::packed(vec![], vec![], vec![], width, false, None).unwrap()
}

fn conditional() -> IrExpr {
    IrExpr::new(
        IrExprKind::ArrayMux {
            sel: Box::new(packed_const(0, 1)),
            a: Box::new(packed_const(0xa55a, 16)),
            b: Box::new(packed_const(0xa65a, 16)),
            element_default: Box::new(default(8)),
        },
        16,
        false,
        None,
    )
}

#[test]
fn array_conditional_valid_plan_counts_the_payload_capacity() {
    let model = valid_model();
    assert_eq!(model.expression_capacity(&conditional(), None).unwrap(), 16);
}

#[test]
fn array_conditional_rejects_nonconcrete_or_misaligned_defaults() {
    let model = valid_model();
    let mut fill = default(8);
    fill.fill = Some(2);
    let mut empty = default(8);
    empty.width = 0;
    for value in [default(3), default(32), IrConst::real(0.0), fill, empty] {
        let mut expr = conditional();
        let IrExprKind::ArrayMux { element_default, .. } = &mut expr.kind else {
            unreachable!();
        };
        **element_default = value;
        assert!(model.validate_expr(&expr, None).is_err());
    }
}

#[test]
fn array_conditional_rejects_payload_shape_and_state_marker_mismatches() {
    let model = valid_model();
    let mut signed = conditional();
    signed.signed = true;
    let mut fill = conditional();
    fill.fill = Some(2);
    let mut zero = conditional();
    zero.width = 0;
    for expr in [signed, fill, zero] {
        assert!(model.validate_expr(&expr, None).is_err());
    }
    for value in [
        packed_const(1, 8),
        IrExpr::new(IrExprKind::Fill(2), 16, false, Some(2)),
    ] {
        let mut expr = conditional();
        let IrExprKind::ArrayMux { a, .. } = &mut expr.kind else {
            unreachable!();
        };
        **a = value;
        assert!(model.validate_expr(&expr, None).is_err());
    }
}

#[test]
fn array_conditional_checks_all_three_child_references() {
    let model = valid_model();
    for index in 0..3 {
        let mut expr = conditional();
        let IrExprKind::ArrayMux { sel, a, b, .. } = &mut expr.kind else {
            unreachable!();
        };
        let target = match index {
            0 => sel,
            1 => a,
            _ => b,
        };
        target.kind = IrExprKind::SigRead(999);
        assert!(model.validate_expr(&expr, None).is_err());
    }
}
