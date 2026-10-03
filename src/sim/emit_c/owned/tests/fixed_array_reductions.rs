//! Ordered receivers, lexical maps, and bounded live ownership in C11 folds.
use super::*;

fn reduction(source: IrFixedArrayReductionSource, value: IrExpr) -> IrExpr {
    let (width, signed) = (value.width, value.signed);
    IrExpr::new(
        IrExprKind::FixedArrayReduce(Box::new(IrFixedArrayReduction {
            source,
            operation: IrContainerReduction::Sum,
            left: -1,
            right: 0,
            element_width: 8,
            element_signed: false,
            element_two_state: false,
            item_name: "item".into(),
            index_name: "index".into(),
            value,
        })),
        width,
        signed,
        None,
    )
}

fn item() -> IrExpr {
    IrExpr::new(IrExprKind::LocalRead("item".into()), 8, false, None)
}

#[test]
fn fixed_array_reduction_emits_each_binary_method_without_an_identity_seed() {
    let model = IrModel::new("fold".into(), 1).unwrap();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    for (operation, helper) in [
        (IrContainerReduction::Sum, "sv4_add("),
        (IrContainerReduction::Product, "sv4_mul("),
        (IrContainerReduction::BitAnd, "sv4_and("),
        (IrContainerReduction::BitOr, "sv4_or("),
        (IrContainerReduction::BitXor, "sv4_xor("),
    ] {
        let mut frame = Frame::new(&ctx);
        let mut expr = reduction(
            IrFixedArrayReductionSource::Value(Box::new(number(0xff01, 16))),
            item(),
        );
        let IrExprKind::FixedArrayReduce(plan) = &mut expr.kind else {
            unreachable!();
        };
        plan.operation = operation;
        model.validate_expr(&expr, None).unwrap();
        let value = frame.expression(&expr).unwrap();
        frame.discard(value);
        let body = frame.body();
        assert_eq!(body.matches(helper).count(), 1);
        assert!(body.contains(" == 0) {\n"));
        assert!(body.contains("sv4_assign("));
        assert!(body.contains("-1LL + (int64_t)"));
        assert!(!body.contains("llg_container"));
        assert!(!body.contains("({"));
        assert!(frame.slots.iter().all(|live| !live));
    }
}

#[test]
fn fixed_array_reduction_receiver_call_is_captured_before_the_loop_once() {
    let mut model = IrModel::new("fold_call".into(), 1).unwrap();
    model.funcs.push(IrFunc::new(
        "make_payload".into(),
        Some(IrType::packed(16, false).unwrap()),
        vec![],
        vec![],
        vec![],
        vec![IrStmt::Return {
            value: Some(Box::new(number(0x0203, 16))),
        }],
    ));
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(0, vec![], IrDepth::PROC, false))),
        16,
        false,
        None,
    );
    let expr = reduction(IrFixedArrayReductionSource::Value(Box::new(call)), item());
    model.validate_expr(&expr, None).unwrap();
    let value = frame.expression(&expr).unwrap();
    frame.discard(value);
    let body = frame.body();
    assert_eq!(body.matches("make_payload(").count(), 1);
    assert!(body.find("make_payload(").unwrap() < body.find("for (uint64_t").unwrap());
    assert!(frame.slots.iter().all(|live| !live));
}

#[test]
fn fixed_array_reduction_nested_maps_shadow_only_after_capturing_the_outer_item() {
    let model = IrModel::new("fold_nested".into(), 1).unwrap();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let outer_item = IrExpr::new(IrExprKind::LocalRead("item".into()), 16, false, None);
    let inner = reduction(
        IrFixedArrayReductionSource::Value(Box::new(outer_item)),
        item(),
    );
    let mut expr = reduction(
        IrFixedArrayReductionSource::Value(Box::new(number(0x01020304, 32))),
        inner,
    );
    let IrExprKind::FixedArrayReduce(plan) = &mut expr.kind else {
        unreachable!();
    };
    plan.element_width = 16;
    model.validate_expr(&expr, None).unwrap();
    let mut frame = Frame::new(&ctx);
    let depth = frame.bindings.len();
    let value = frame.expression(&expr).unwrap();
    frame.discard(value);
    assert_eq!(frame.body().matches("for (uint64_t").count(), 2);
    assert_eq!(frame.bindings.len(), depth);
    assert!(frame.slots.iter().all(|live| !live));
}

#[test]
fn fixed_array_reduction_direct_storage_has_constant_emission_and_slot_cost() {
    let mut model = IrModel::new("large_memory".into(), 1).unwrap();
    let last = (crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS - 1) as i32;
    model.arrays.push(
        IrArray::new(
            "G_memory".into(),
            "memory".into(),
            8,
            false,
            vec![(0, last)],
        )
        .unwrap(),
    );
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut expr = reduction(IrFixedArrayReductionSource::Array(0), item());
    let IrExprKind::FixedArrayReduce(plan) = &mut expr.kind else {
        unreachable!();
    };
    plan.left = last;
    plan.right = 0;
    model.validate_expr(&expr, None).unwrap();
    let mut frame = Frame::new(&ctx);
    let value = frame.expression(&expr).unwrap();
    frame.discard(value);
    assert!(frame
        .body()
        .contains(&format!("{}ULL", crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS)));
    assert!(frame.body().contains(&format!("{last}LL - (int64_t)")));
    assert!(!frame.body().contains("sv4_concat"));
    assert!(!frame.body().contains("sv4_part_select"));
    assert!(frame.body().len() < 5000);
    assert!(frame.slots.len() < 10);
    assert!(frame.slots.iter().all(|live| !live));
}

#[test]
fn fixed_array_reduction_map_can_read_an_enclosing_formal() {
    let mut model = IrModel::new("captured_map".into(), 1).unwrap();
    model.funcs.push(IrFunc::new(
        "f".into(),
        Some(IrType::packed(32, false).unwrap()),
        vec![IrFormal::new(false, 32, false).unwrap()],
        vec![],
        vec![],
        vec![],
    ));
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: Some(&model.funcs[0]),
        sampled: false,
        activation_label: None,
    };
    let mapped = add(
        IrExpr::convert_to(item(), 32, false),
        IrExpr::new(IrExprKind::FormalRead(0), 32, false, None),
        32,
    );
    let expr = reduction(
        IrFixedArrayReductionSource::Value(Box::new(number(0x0203, 16))),
        mapped,
    );
    model.validate_expr(&expr, ctx.func).unwrap();
    let mut frame = Frame::new(&ctx);
    // A raw expression frame does not run the callee's input-owner prologue.
    frame
        .local("a0", 32, false, false, Some(&number(11, 32)))
        .unwrap();
    let address = frame.lookup("a0").unwrap().address;
    let value = frame.expression(&expr).unwrap();
    assert!(frame.body().contains(&format!("sv4_clone({address})")));
    frame.discard(value);
    assert!(frame.slots.iter().all(|live| !live));
    assert_eq!(frame.body().matches("for (uint64_t").count(), 1);
}
