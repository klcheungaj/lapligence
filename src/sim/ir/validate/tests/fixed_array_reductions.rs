//! Public IR checks for fixed-array folds and their receiver/map children.
use super::*;

fn reduction() -> IrExpr {
    IrExpr::new(
        IrExprKind::FixedArrayReduce(Box::new(IrFixedArrayReduction {
            source: IrFixedArrayReductionSource::Value(Box::new(packed_const(0xff01, 16))),
            operation: IrContainerReduction::Sum,
            left: -2,
            right: -1,
            element_width: 8,
            element_signed: false,
            element_two_state: false,
            item_name: "item".into(),
            index_name: "index".into(),
            value: IrExpr::new(IrExprKind::LocalRead("item".into()), 8, false, None),
        })),
        8,
        false,
        None,
    )
}

fn plan(expr: &mut IrExpr) -> &mut IrFixedArrayReduction {
    let IrExprKind::FixedArrayReduce(plan) = &mut expr.kind else {
        unreachable!();
    };
    plan
}

#[test]
fn fixed_array_reduction_valid_payload_counts_capacity() {
    assert_eq!(valid_model().expression_capacity(&reduction(), None).unwrap(), 32);
}

#[test]
fn fixed_array_reduction_direct_storage_uses_ordinal_not_declared_bounds() {
    let mut model = valid_model();
    model.arrays.push(IrArray::new("a".into(), "a".into(), 8, false, vec![(9, 8)]).unwrap());
    let mut expr = reduction();
    plan(&mut expr).source = IrFixedArrayReductionSource::Array(0);
    model.validate_expr(&expr, None).unwrap();
    // A large memory does not impose a packed-payload width on the fold.
    let last = (crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS - 1) as i32;
    model.arrays[0].dims = vec![(0, last)];
    model.arrays[0].total = crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS;
    plan(&mut expr).left = 0;
    plan(&mut expr).right = last;
    assert_eq!(model.expression_capacity(&expr, None).unwrap(), 32);
}

#[test]
fn fixed_array_reduction_rejects_storage_above_selected_cell_limit() {
    let mut model = valid_model();
    model
        .arrays
        .push(IrArray::new("a".into(), "a".into(), 8, false, vec![(0, 1)]).unwrap());
    let mut expr = reduction();
    plan(&mut expr).source = IrFixedArrayReductionSource::Array(0);
    let over_limit = crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS as i32;
    model.arrays[0].dims = vec![(0, over_limit)];
    model.arrays[0].total = crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS + 1;
    plan(&mut expr).left = 0;
    plan(&mut expr).right = over_limit;
    assert!(
        model
            .validate_expr(&expr, None)
            .expect_err("over-limit direct storage must be rejected")
            .detail()
            .contains("selected cell-wise storage limit")
    );
}

#[test]
fn fixed_array_reduction_rejects_mismatched_storage_shapes() {
    let mut model = valid_model();
    model.arrays.push(IrArray::new("a".into(), "a".into(), 8, false, vec![(0, 1)]).unwrap());
    let mut expr = reduction();
    plan(&mut expr).source = IrFixedArrayReductionSource::Array(1);
    assert!(model.validate_expr(&expr, None).is_err());
    plan(&mut expr).source = IrFixedArrayReductionSource::Array(0);
    model.arrays[0].elem_width = 7;
    assert!(model.validate_expr(&expr, None).is_err());
    model.arrays[0].elem_width = 8;
    model.arrays[0].dims = vec![(0, 0), (0, 1)];
    assert!(model.validate_expr(&expr, None).is_err());
    model.arrays[0].dims = vec![(0, 1)];
    model.arrays[0].real = true;
    assert!(model.validate_expr(&expr, None).is_err());
}

#[test]
fn fixed_array_reduction_rejects_invalid_value_map_and_shape_references() {
    let model = valid_model();
    let mut expr = reduction();
    plan(&mut expr).element_width = 7;
    assert!(model.validate_expr(&expr, None).is_err());
    let mut expr = reduction();
    plan(&mut expr).source = IrFixedArrayReductionSource::Value(Box::new(
        IrExpr::new(IrExprKind::SigRead(99), 16, false, None),
    ));
    assert!(model.validate_expr(&expr, None).is_err());
    let mut expr = reduction();
    plan(&mut expr).value = IrExpr::new(IrExprKind::SigRead(99), 8, false, None);
    assert!(model.validate_expr(&expr, None).is_err());
    let mut expr = reduction();
    plan(&mut expr).value.signed = true;
    assert!(model.validate_expr(&expr, None).is_err());
    let mut expr = reduction();
    plan(&mut expr).index_name = "item".into();
    assert!(model.validate_expr(&expr, None).is_err());
}

#[test]
fn fixed_array_reduction_fill_map_is_self_determined_not_a_fill_result() {
    let model = valid_model();
    let mut expr = reduction();
    expr.width = 1;
    plan(&mut expr).value = IrExpr::new(IrExprKind::Fill(1), 1, false, Some(1));
    model.validate_expr(&expr, None).unwrap();
    expr.fill = Some(1);
    assert!(model.validate_expr(&expr, None).is_err());
}

#[test]
fn fixed_array_reduction_validates_lexical_item_and_index_shapes() {
    let model = valid_model();
    let mut expr = reduction();
    expr.width = 7;
    plan(&mut expr).value.width = 7;
    assert!(model.validate_expr(&expr, None).is_err());
    let mut expr = reduction();
    plan(&mut expr).value = IrExpr::new(IrExprKind::LocalRead("index".into()), 8, false, None);
    assert!(model.validate_expr(&expr, None).is_err());
    expr.width = 32;
    expr.signed = true;
    plan(&mut expr).value = IrExpr::new(IrExprKind::LocalRead("index".into()), 32, true, None);
    model.validate_expr(&expr, None).unwrap();
}

#[test]
fn fixed_array_reduction_restores_bindings_after_a_nested_map_error() {
    let model = valid_model();
    let validator = Validator::new(&model);
    let mut outer = reduction();
    plan(&mut outer).element_width = 16;
    plan(&mut outer).source = IrFixedArrayReductionSource::Value(Box::new(packed_const(0x01020304, 32)));
    let mut inner = reduction();
    plan(&mut inner).source = IrFixedArrayReductionSource::Value(Box::new(
        IrExpr::new(IrExprKind::LocalRead("item".into()), 16, false, None),
    ));
    // The inner receiver reads the outer item before shadowing its binding.
    plan(&mut outer).value = inner.clone();
    validator.validate_expr(&outer, &[], "fold").unwrap();
    assert!(validator.reduction_bindings.borrow().is_empty());
    plan(&mut inner).value = IrExpr::new(IrExprKind::LocalRead("index".into()), 8, false, None);
    plan(&mut outer).value = inner;
    assert!(validator.validate_expr(&outer, &[], "fold").is_err());
    assert!(validator.reduction_bindings.borrow().is_empty());
}
