//! The array fallback is only emitted in the ambiguous branch and owns its data.
use super::*;

#[test]
fn array_conditional_emits_branch_local_captures_and_default_owner() {
    let model = IrModel::new("array_emission".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let expr = IrExpr::new(
        IrExprKind::ArrayMux {
            sel: Box::new(number(0, 1)),
            a: Box::new(number(0xa55a, 16)),
            b: Box::new(number(0xa65a, 16)),
            element_default: Box::new(
                IrConst::packed(vec![], vec![0xff], vec![], 8, false, None).unwrap(),
            ),
        },
        16,
        false,
        None,
    );
    model.validate_expr(&expr, None).unwrap();
    let value = frame.expression(&expr).unwrap();
    frame.discard(value);
    let body = frame.body();
    assert!(body.contains("} else if (!"));
    let branch = body.rfind("} else {").unwrap();
    let merge = body.find("sv4_array_conditional_merge(").unwrap();
    assert!(branch < merge);
    assert_eq!(body.matches("sv4_array_conditional_merge(").count(), 1);
    assert!(!body.contains("sv4_mux("));
    assert!(body[..branch].contains("sv4_move("));
    assert!(body[branch..merge].matches("sv4_resize(").count() >= 2);
    assert!(body[merge..].matches("sv4_destroy(").count() >= 4);
    assert!(frame.slots.iter().all(|live| !live));
    assert!(!body.contains("({"));
}

#[test]
fn structure_conditional_emits_each_member_boundary_and_default_owner() {
    let model = IrModel::new("structure_emission".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let expr = IrExpr::new(
        IrExprKind::StructMux {
            sel: Box::new(number(0, 1)),
            a: Box::new(number(0x12a5, 13)),
            b: Box::new(number(0x14a6, 13)),
            members: vec![
                IrConditionalMember {
                    offset: 0,
                    width: 4,
                    default: IrConst::packed(vec![], vec![0xf], vec![], 4, false, None).unwrap(),
                },
                IrConditionalMember {
                    offset: 4,
                    width: 1,
                    default: IrConst::packed(vec![], vec![], vec![], 1, false, None).unwrap(),
                },
                IrConditionalMember {
                    offset: 5,
                    width: 8,
                    default: IrConst::packed(vec![], vec![0xff], vec![], 8, false, None).unwrap(),
                },
            ],
        },
        13,
        false,
        None,
    );
    model.validate_expr(&expr, None).unwrap();
    let value = frame.expression(&expr).unwrap();
    frame.discard(value);
    let body = frame.body();
    assert_eq!(body.matches("sv4_array_conditional_merge(").count(), 3);
    assert!(body.matches("sv4_part_select_set(").count() >= 3);
    assert!(body.contains("sv4_zero(13, 0)"));
    assert!(!body.contains("sv4_mux("));
    assert!(frame.slots.iter().all(|live| !live));
    assert!(!body.contains("({"));
}
