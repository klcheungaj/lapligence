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

fn structure_conditional() -> IrExpr {
    let default = |width: u32, x: u64| {
        IrConst::packed(
            vec![],
            (x != 0).then_some(vec![x]).unwrap_or_default(),
            vec![],
            width,
            false,
            None,
        )
        .unwrap()
    };
    IrExpr::new(
        IrExprKind::StructMux {
            sel: Box::new(packed_const(0, 1)),
            a: Box::new(packed_const(0x12a5, 13)),
            b: Box::new(packed_const(0x14a6, 13)),
            members: vec![
                IrConditionalMember {
                    offset: 0,
                    width: 4,
                    default: default(4, 0xf),
                },
                IrConditionalMember {
                    offset: 4,
                    width: 1,
                    default: default(1, 0),
                },
                IrConditionalMember {
                    offset: 5,
                    width: 8,
                    default: default(8, 0xff),
                },
            ],
        },
        13,
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
        let IrExprKind::ArrayMux {
            element_default, ..
        } = &mut expr.kind
        else {
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

#[test]
fn structure_conditional_validates_boundaries_and_defaults() {
    let model = valid_model();
    assert!(model.validate_expr(&structure_conditional(), None).is_ok());

    let mut overlap = structure_conditional();
    let IrExprKind::StructMux { members, .. } = &mut overlap.kind else {
        unreachable!();
    };
    members[1].offset = 3;
    assert!(model.validate_expr(&overlap, None).is_err());

    let mut incomplete = structure_conditional();
    let IrExprKind::StructMux { members, .. } = &mut incomplete.kind else {
        unreachable!();
    };
    members.pop();
    assert!(model.validate_expr(&incomplete, None).is_err());

    let mut bad_default = structure_conditional();
    let IrExprKind::StructMux { members, .. } = &mut bad_default.kind else {
        unreachable!();
    };
    members[0].default.width = 3;
    assert!(model.validate_expr(&bad_default, None).is_err());
}

#[test]
fn structure_conditional_checks_all_three_child_references() {
    let model = valid_model();
    for index in 0..3 {
        let mut expr = structure_conditional();
        let IrExprKind::StructMux { sel, a, b, .. } = &mut expr.kind else {
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
