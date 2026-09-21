//! Folding must preserve immediate array boundaries and ordinary packed muxes.
use super::*;

fn unknown(width: u32) -> IrExpr {
    masked_konst(0, (1u64 << width) - 1, 0, width)
}

fn array(sel: IrExpr, a: IrExpr, b: IrExpr, default: IrExpr) -> IrExpr {
    let width = a.width;
    let IrExprKind::Const(default) = default.kind else {
        unreachable!();
    };
    IrExpr::new(
        IrExprKind::ArrayMux {
            sel: Box::new(sel),
            a: Box::new(a),
            b: Box::new(b),
            element_default: Box::new(default),
        },
        width,
        false,
        None,
    )
}

fn structure(sel: IrExpr, a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::StructMux {
            sel: Box::new(sel),
            a: Box::new(a),
            b: Box::new(b),
            members: vec![
                IrConditionalMember {
                    offset: 0,
                    width: 4,
                    default: IrConst::packed(
                        vec![],
                        vec![0xf],
                        vec![],
                        4,
                        false,
                        None,
                    )
                    .unwrap(),
                },
                IrConditionalMember {
                    offset: 4,
                    width: 1,
                    default: IrConst::packed(vec![0], vec![], vec![], 1, false, None).unwrap(),
                },
                IrConditionalMember {
                    offset: 5,
                    width: 8,
                    default: IrConst::packed(
                        vec![],
                        vec![0xff],
                        vec![],
                        8,
                        false,
                        None,
                    )
                    .unwrap(),
                },
            ],
        },
        13,
        false,
        None,
    )
}

fn check(mut expr: IrExpr, expected: IrExpr) {
    fold_expr(&mut expr);
    let actual = as_packed_const(&expr).expect("array conditional must fold");
    assert_eq!(actual.bits, as_packed_const(&expected).unwrap().bits);
    assert!(!actual.signed);
}

#[test]
fn array_conditional_fold_does_not_reuse_packed_bit_merging() {
    check(
        array(unknown(1), konst(0xa55a, 16), konst(0xa65a, 16), unknown(8)),
        masked_konst(0x5a, 0xff00, 0, 16),
    );
    let mut packed = IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(unknown(1)),
            a: Box::new(konst(0xa5, 8)),
            b: Box::new(konst(0xa6, 8)),
        },
        8,
        false,
        None,
    );
    fold_expr(&mut packed);
    assert_eq!(
        as_packed_const(&packed).unwrap().bits,
        as_packed_const(&masked_konst(0xa4, 3, 0, 8)).unwrap().bits,
    );
}

#[test]
fn structure_conditional_fold_preserves_equal_members_only() {
    check(
        structure(unknown(1), konst(0x12a5, 13), konst(0x14a6, 13)),
        masked_konst(0, 0x1fef, 0, 13),
    );
}

#[test]
fn array_conditional_fold_defaults_entire_rows_and_mixed_state_records() {
    // A differing byte in the first row defaults both bytes, not just the byte.
    check(
        array(
            masked_konst(0, 0, 1, 1),
            konst(0x01020405, 32),
            konst(0x01030405, 32),
            unknown(16),
        ),
        masked_konst(0x0405, 0xffff0000, 0, 32),
    );
    // A record's two-state prefix defaults to zero; its logic byte becomes X.
    check(
        array(
            unknown(1),
            konst(0x07a5, 16),
            konst(0x07a6, 16),
            masked_konst(0, 0xff, 0, 16),
        ),
        masked_konst(0, 0xff, 0, 16),
    );
    check(
        array(unknown(1), konst(0xa5, 8), konst(0xa6, 8), konst(0, 8)),
        konst(0, 8),
    );
}

#[test]
fn array_conditional_fold_uses_known_logical_equality_not_case_equality() {
    for value in [masked_konst(0xa0, 0xf, 0, 8), masked_konst(0xa0, 0, 0xf, 8)] {
        check(array(unknown(1), value.clone(), value, unknown(8)), unknown(8));
    }
    check(
        array(unknown(1), konst(0xa5, 8), konst(0xa5, 8), unknown(8)),
        konst(0xa5, 8),
    );
}

#[test]
fn array_conditional_fold_skips_dead_arms_and_recognizes_dominant_one() {
    let not_constant = IrExpr::new(IrExprKind::SigRead(0), 8, false, None);
    for selector in [konst(1, 1), masked_konst(1, 2, 0, 2)] {
        check(
            array(selector, konst(0xa5, 8), not_constant.clone(), unknown(8)),
            konst(0xa5, 8),
        );
    }
    check(
        array(konst(0, 1), not_constant, konst(0xa6, 8), unknown(8)),
        konst(0xa6, 8),
    );
}

#[test]
fn array_conditional_identity_keeps_unknown_equal_arms_and_effectful_selector() {
    let read = IrExpr::new(IrExprKind::SigRead(0), 8, false, None);
    let mut expr = array(unknown(1), read.clone(), read, unknown(8));
    ident_expr(&mut expr);
    assert!(matches!(expr.kind, IrExprKind::ArrayMux { .. }));
    let selector = IrExpr::new(
        IrExprKind::CallFn(Box::new(crate::sim::ir::IrCallExpr::new(
            0,
            vec![],
            IrDepth::PROC,
            false,
        ))),
        1,
        false,
        None,
    );
    let mut expr = array(selector, konst(7, 8), konst(7, 8), unknown(8));
    fold_expr(&mut expr);
    ident_expr(&mut expr);
    assert!(matches!(expr.kind, IrExprKind::ArrayMux { .. }));
}

#[test]
fn array_conditional_fold_compares_bits_above_sixty_four() {
    let constant = |bits: Vec<u64>, x: Vec<u64>, width| {
        IrExpr::new(
            IrExprKind::Const(IrConst::packed(bits, x, vec![], width, false, None).unwrap()),
            width,
            false,
            None,
        )
    };
    check(
        array(
            unknown(1),
            constant(vec![7, 1], vec![], 65),
            constant(vec![7, 0], vec![], 65),
            constant(vec![], vec![u64::MAX, 1], 65),
        ),
        constant(vec![], vec![u64::MAX, 1], 65),
    );
}

#[test]
fn array_conditional_walkers_keep_reads_in_all_three_operands() {
    let mut model = IrModel::new("array_walk".into(), 1).unwrap();
    model.signals = sigs(4);
    let read = |index| IrExpr::new(IrExprKind::SigRead(index), 8, false, None);
    model.processes.push(IrProcess::new(
        "p".into(),
        "p".into(),
        IrShape::RunOnce,
        vec![],
        vec![assign(
            IrLhs::Whole(3),
            array(read(0), read(1), read(2), unknown(8)),
        )],
    ));
    model.validate().unwrap();
    mark_unused_storage(&mut model, None);
    assert!(model.signals.iter().all(|signal| !signal.omit));
    let mut visits = 0;
    walk_model_exprs_mut(&mut model, &mut |expr| {
        if matches!(expr.kind, IrExprKind::SigRead(_)) {
            visits += 1;
        }
    });
    assert_eq!(visits, 3);
}
