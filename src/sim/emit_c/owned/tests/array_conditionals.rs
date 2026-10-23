//! The array fallback is only emitted in the ambiguous branch and owns its data.
use super::*;

#[test]
fn array_conditional_emits_branch_local_captures_and_default_owner() {
    let model = IrModel::new("array_emission".to_owned(), 1).unwrap();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
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
    let merge = body.find("sv4_array_conditional_merge_to(").unwrap();
    assert!(branch < merge);
    assert_eq!(body.matches("sv4_array_conditional_merge_to(").count(), 1);
    assert!(!body.contains("sv4_mux_to("));
    assert!(body[..branch].contains("sv4_move("));
    assert!(!body[branch..merge].contains("sv4_resize_to("));
    assert!(body[merge..].matches("sv4_destroy(").count() >= 4);
    assert!(frame.slots.iter().all(|live| !live));
    assert!(!body.contains("({"));
}

#[test]
fn structure_conditional_emits_one_leaf_merge_and_default_owner() {
    let model = IrModel::new("structure_emission".to_owned(), 1).unwrap();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let expr = IrExpr::new(
        IrExprKind::StructMux {
            sel: Box::new(number(0, 1)),
            a: Box::new(number(0x12a5, 13)),
            b: Box::new(number(0x14a6, 13)),
            leaves: Box::new(IrLeafMerge {
                // Leaves at bits 0 (4 bits, X), 4 (1 bit, 0) and 5 (8 bits, X).
                default: IrConst::packed(vec![], vec![0x1fef], vec![], 13, false, None).unwrap(),
                starts: IrConst::packed(vec![0x31], vec![], vec![], 13, false, None).unwrap(),
            }),
        },
        13,
        false,
        None,
    );
    model.validate_expr(&expr, None).unwrap();
    let value = frame.expression(&expr).unwrap();
    frame.discard(value);
    let body = frame.body();
    assert_eq!(body.matches("sv4_record_conditional_merge").count(), 1);
    assert!(!body.contains("sv4_array_conditional_merge"));
    assert!(!body.contains("sv4_part_select"));
    assert!(!body.contains("sv4_mux_to("));
    assert!(frame.slots.iter().all(|live| !live));
    assert!(!body.contains("({"));
}
