//! Array and record merge plans are checked at the public IR boundary.
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
    IrExpr::new(
        IrExprKind::StructMux {
            sel: Box::new(packed_const(0, 1)),
            a: Box::new(packed_const(0x12a5, 13)),
            b: Box::new(packed_const(0x14a6, 13)),
            // Leaves at bits 0 (4 bits, X), 4 (1 bit, 0) and 5 (8 bits, X).
            leaves: Box::new(IrLeafMerge {
                default: IrConst::packed(vec![], vec![0x1fef], vec![], 13, false, None).unwrap(),
                starts: IrConst::packed(vec![0x31], vec![], vec![], 13, false, None).unwrap(),
            }),
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
fn structure_conditional_validates_leaf_starts_and_defaults() {
    let model = valid_model();
    assert!(model.validate_expr(&structure_conditional(), None).is_ok());

    // The payload is two periods of a 13-bit record default.
    let mut repeated = structure_conditional();
    repeated.width = 26;
    let IrExprKind::StructMux { a, b, .. } = &mut repeated.kind else {
        unreachable!();
    };
    **a = packed_const(0x12a5, 26);
    **b = packed_const(0x14a6, 26);
    assert!(model.validate_expr(&repeated, None).is_ok());

    let invalid: [fn(&mut IrLeafMerge); 6] = [
        // No leaf starts at bit 0.
        |leaves| leaves.starts.bits = vec![0x30],
        // An unknown start bit.
        |leaves| leaves.starts.x = vec![0x2],
        // Starts and default disagree in width.
        |leaves| leaves.starts.width = 12,
        // A period that does not divide the payload.
        |leaves| {
            leaves.default.width = 12;
            leaves.default.x = vec![0xfef];
            leaves.starts.width = 12;
        },
        |leaves| leaves.default = IrConst::real(0.0),
        |leaves| leaves.default.fill = Some(2),
    ];
    for change in invalid {
        let mut expr = structure_conditional();
        let IrExprKind::StructMux { leaves, .. } = &mut expr.kind else {
            unreachable!();
        };
        change(leaves);
        assert!(model.validate_expr(&expr, None).is_err());
    }
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
