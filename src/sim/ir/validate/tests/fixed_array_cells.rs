//! Public IR checks for cell-wise fixed-array views and in-place ordering.
use super::*;

fn model_with_rows() -> IrModel {
    let mut model = valid_model();
    model
        .arrays
        .push(IrArray::new("rows".into(), "rows".into(), 8, true, vec![(0, 2), (19, 0)]).unwrap());
    model
}

fn order(method: IrFixedArrayOrderMethod, prefix: Vec<IrExpr>) -> IrStmt {
    IrStmt::FixedArrayOrder(Box::new(IrFixedArrayOrder {
        cells: IrFixedArrayCells { array: 0, prefix },
        method,
        item_name: "item".into(),
        index_name: "index".into(),
        item_signed: true,
        item_two_state: false,
        key: None,
    }))
}

#[test]
fn component_fixed_array_cells_shape_follows_the_selected_dimension() {
    let model = model_with_rows();
    let whole = IrFixedArrayCells {
        array: 0,
        prefix: Vec::new(),
    };
    let shape = whole.shape(&model).unwrap();
    assert_eq!(
        (
            shape.left,
            shape.right,
            shape.count,
            shape.element_cells,
            shape.cells
        ),
        (0, 2, 3, 20, 60)
    );
    let row = IrFixedArrayCells {
        array: 0,
        prefix: vec![packed_const(1, 32)],
    };
    let shape = row.shape(&model).unwrap();
    assert_eq!(
        (shape.left, shape.right, shape.count, shape.element_cells),
        (19, 0, 20, 1)
    );
    let element = IrFixedArrayCells {
        array: 0,
        prefix: vec![packed_const(1, 32), packed_const(1, 32)],
    };
    assert!(element.shape(&model).is_none());
}

#[test]
fn component_fixed_array_order_validates_receiver_and_key() {
    let model = model_with_rows();
    model
        .validate_stmt(&order(IrFixedArrayOrderMethod::Reverse, Vec::new()), None)
        .unwrap();
    model
        .validate_stmt(
            &order(IrFixedArrayOrderMethod::Sort, vec![packed_const(2, 32)]),
            None,
        )
        .unwrap();

    // Selecting every dimension names one element, not reorderable cells.
    let element = order(
        IrFixedArrayOrderMethod::Sort,
        vec![packed_const(0, 32), packed_const(0, 32)],
    );
    assert!(model.validate_stmt(&element, None).is_err());

    let mut missing = order(IrFixedArrayOrderMethod::Reverse, Vec::new());
    let IrStmt::FixedArrayOrder(plan) = &mut missing else {
        unreachable!();
    };
    plan.cells.array = 7;
    assert!(model.validate_stmt(&missing, None).is_err());

    let mut keyed_reverse = order(IrFixedArrayOrderMethod::Reverse, Vec::new());
    let IrStmt::FixedArrayOrder(plan) = &mut keyed_reverse else {
        unreachable!();
    };
    plan.key = Some(packed_const(1, 4));
    assert!(model.validate_stmt(&keyed_reverse, None).is_err());

    // A key reads the bound element and declared index, but never a real.
    let mut keyed = order(IrFixedArrayOrderMethod::RSort, vec![packed_const(0, 32)]);
    let IrStmt::FixedArrayOrder(plan) = &mut keyed else {
        unreachable!();
    };
    plan.key = Some(IrExpr::new(
        IrExprKind::LocalRead("index".into()),
        32,
        true,
        None,
    ));
    model.validate_stmt(&keyed, None).unwrap();
    let IrStmt::FixedArrayOrder(plan) = &mut keyed else {
        unreachable!();
    };
    plan.index_name = "item".into();
    assert!(model.validate_stmt(&keyed, None).is_err());

    let mut real_prefix = order(
        IrFixedArrayOrderMethod::Sort,
        vec![IrExpr::new(
            IrExprKind::Const(IrConst::real(1.0)),
            0,
            false,
            None,
        )],
    );
    assert!(model.validate_stmt(&real_prefix, None).is_err());
    let IrStmt::FixedArrayOrder(plan) = &mut real_prefix else {
        unreachable!();
    };
    plan.cells.prefix = vec![packed_const(0, 32)];
    model.validate_stmt(&real_prefix, None).unwrap();
}

#[test]
fn component_fixed_array_cells_reduction_and_inside_shapes() {
    let model = model_with_rows();
    let reduce = |prefix: Vec<IrExpr>, element_width: u32| {
        IrExpr::new(
            IrExprKind::FixedArrayReduce(Box::new(IrFixedArrayReduction {
                source: IrFixedArrayReductionSource::Cells(IrFixedArrayCells { array: 0, prefix }),
                operation: IrContainerReduction::Sum,
                left: 19,
                right: 0,
                element_width,
                element_signed: true,
                element_two_state: false,
                item_name: "item".into(),
                index_name: "index".into(),
                value: IrExpr::new(
                    IrExprKind::LocalRead("item".into()),
                    element_width,
                    true,
                    None,
                ),
            })),
            element_width,
            true,
            None,
        )
    };
    model
        .validate_expr(&reduce(vec![packed_const(1, 32)], 8), None)
        .unwrap();
    // Rows are not cell elements, and the element width must be the cell's.
    assert!(model.validate_expr(&reduce(Vec::new(), 8), None).is_err());
    assert!(model
        .validate_expr(&reduce(vec![packed_const(1, 32)], 16), None)
        .is_err());

    let inside = |prefix: Vec<IrExpr>| {
        IrExpr::new(
            IrExprKind::Inside {
                value: Box::new(packed_const(3, 8)),
                items: vec![IrInsideItem::Cells(IrFixedArrayCells { array: 0, prefix })],
            },
            1,
            false,
            None,
        )
    };
    model.validate_expr(&inside(Vec::new()), None).unwrap();
    model
        .validate_expr(&inside(vec![packed_const(1, 32)]), None)
        .unwrap();
    assert!(model
        .validate_expr(
            &inside(vec![packed_const(1, 32), packed_const(1, 32)]),
            None
        )
        .is_err());
}
