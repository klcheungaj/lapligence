use super::*;

/// A model whose only signal is a 32-bit four-state vector, the packed image
/// of a `logic [7:0] [0:3]`-shaped array.
fn image_model() -> IrModel {
    let mut model = valid_model();
    model.signals[0].ty = IrType::Packed {
        width: 32,
        signed: false,
        two_state: false,
    };
    model
}

fn index_selector() -> IrStreamSelector {
    IrStreamSelector::Indexed {
        base: packed_const(1, 32),
        width: packed_const(2, 32),
        negative: false,
    }
}

fn image_stream(image_width: u32, fallback_width: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::FixedImageStream {
            image: Box::new(packed_const(0, image_width)),
            bounds: (0, 3),
            element_width: 8,
            fallback: Box::new(IrExpr::new(
                IrExprKind::Const(IrConst::integral_default(fallback_width, false)),
                fallback_width,
                false,
                None,
            )),
            selector: Box::new(index_selector()),
        },
        LLG_MAX_WIDTH,
        false,
        None,
    )
}

fn stream(targets: Vec<IrStreamTarget>, nba: bool) -> IrStmt {
    IrStmt::StreamAssign {
        source: packed_const(0, 32),
        slice: 1,
        direction: IrStreamDirection::LeftToRight,
        targets,
        nba,
    }
}

fn image_target(target: IrLhs) -> IrStreamTarget {
    IrStreamTarget::FixedImageSelector {
        target: Box::new(target),
        bounds: (0, 3),
        element_width: 8,
        two_state: false,
        selector: index_selector(),
        two_state_runs: Vec::new(),
    }
}

#[test]
fn image_stream_requires_a_whole_image_and_element_fallback() {
    let model = image_model();
    model
        .validate_expr(&image_stream(32, 8), None)
        .expect("a 4 x 8-bit image streams a selection");
    assert!(
        model.validate_expr(&image_stream(24, 8), None).is_err(),
        "an image narrower than its bounds is rejected"
    );
    assert!(
        model.validate_expr(&image_stream(32, 4), None).is_err(),
        "a fallback must be one element wide"
    );
}

#[test]
fn image_selector_target_requires_its_packed_shape_and_persistent_queued_storage() {
    let model = image_model();
    model
        .validate_stmt(&stream(vec![image_target(IrLhs::Whole(0))], false), None)
        .expect("a whole 32-bit lvalue holds four 8-bit elements");
    model
        .validate_stmt(&stream(vec![image_target(IrLhs::Whole(0))], true), None)
        .expect("a persistent image target may be queued");
    let narrow = IrLhs::Part(0, 23, 0, false);
    assert!(
        model
            .validate_stmt(&stream(vec![image_target(narrow)], false), None)
            .is_err(),
        "a 24-bit lvalue cannot hold four 8-bit elements"
    );
    let local = IrLhs::WholeRef {
        addr: "&_l0".into(),
        width: 32,
        signed: false,
        two_state: false,
        shortreal: false,
    };
    assert!(
        model
            .validate_stmt(&stream(vec![image_target(local)], true), None)
            .is_err(),
        "a queued unpack cannot target activation storage"
    );
}

#[test]
fn queued_fixed_selector_rejects_activation_arrays() {
    let mut model = image_model();
    let mut array = IrArray::new("a".into(), "a".into(), 8, false, vec![(0, 3)]).unwrap();
    model.arrays.push(array.clone());
    let target = || IrStreamTarget::FixedSelector {
        array: 0,
        selector: index_selector(),
    };
    model
        .validate_stmt(&stream(vec![target()], true), None)
        .expect("a persistent array may be queued");
    array.activation = true;
    model.arrays[0] = array;
    assert!(model
        .validate_stmt(&stream(vec![target()], true), None)
        .is_err());
}

#[test]
fn memory_views_bound_their_last_element() {
    let mut model = valid_model();
    model
        .arrays
        .push(IrArray::new("m".into(), "m".into(), 8, false, vec![(0, 9)]).unwrap());
    let view = |origin: u64, dims: (i32, i32)| IrMemoryView {
        array: 0,
        origin,
        selectors: Vec::new(),
        sliced: true,
        dims: vec![dims],
        strides: vec![1],
        total: u64::from(dims.0.abs_diff(dims.1)) + 1,
    };
    let fill = |view: IrMemoryView| IrStmt::Memory {
        write: false,
        path: IrStringExpr::Literal(b"m.hex".to_vec()),
        view: Box::new(view),
        radix: IrMemoryRadix::Hex,
        addressing: IrMemoryAddressingPolicy::SystemVerilog2009,
        enum_values: None,
        start: None,
        finish: None,
    };
    model
        .validate_stmt(&fill(view(9, (9, 9))), None)
        .expect("a one-element view of the last cell is in bounds");
    model
        .validate_stmt(&fill(view(2, (2, 9))), None)
        .expect("a view ending at the last cell is in bounds");
    assert!(
        model.validate_stmt(&fill(view(3, (2, 9))), None).is_err(),
        "a view one cell past the array is rejected"
    );
}

#[test]
fn image_selector_two_state_runs_stay_disjoint_inside_one_element() {
    let model = image_model();
    let runs = |two_state_runs: Vec<(u32, u32)>| {
        let IrStreamTarget::FixedImageSelector {
            target,
            bounds,
            element_width,
            two_state,
            selector,
            ..
        } = image_target(IrLhs::Whole(0))
        else {
            unreachable!("image target")
        };
        stream(
            vec![IrStreamTarget::FixedImageSelector {
                target,
                bounds,
                element_width,
                two_state,
                selector,
                two_state_runs,
            }],
            false,
        )
    };
    model
        .validate_stmt(&runs(vec![(0, 2), (4, 3)]), None)
        .expect("ascending disjoint runs inside an 8-bit element");
    for (invalid, reason) in [
        (vec![(0, 0)], "an empty run"),
        (vec![(0, 4), (2, 2)], "overlapping runs"),
        (vec![(6, 3)], "a run past the element"),
    ] {
        assert!(
            model.validate_stmt(&runs(invalid), None).is_err(),
            "{reason}"
        );
    }
}

/// A 5,000-cell descriptor destination, a 5,000-cell descriptor source and a
/// four-cell dense array, all of 8-bit cells.
fn descriptor_stream_model() -> IrModel {
    let mut model = valid_model();
    for (name, right) in [("dst", 4999), ("src", 4999), ("dense", 3)] {
        model
            .arrays
            .push(IrArray::new(name.into(), name.into(), 8, false, vec![(0, right)]).unwrap());
    }
    model
}

fn whole_view(model: &IrModel, array: usize) -> IrMemoryView {
    let total = model.arrays[array].total;
    IrMemoryView {
        array,
        origin: 0,
        selectors: Vec::new(),
        sliced: false,
        dims: model.arrays[array].dims.clone(),
        strides: vec![1],
        total,
    }
}

#[test]
fn descriptor_stream_operands_check_cells_and_storage() {
    let model = descriptor_stream_model();
    let assign = |src: IrFixedValue| IrStmt::FixedValueAssign {
        dst: whole_view(&model, 0),
        src: Box::new(src),
        nba: false,
    };
    let packed = |width: u32, cell_width: u32, runtime_sized: bool| IrFixedValue::Packed {
        value: Box::new(packed_const(0, width)),
        cell_width,
        runtime_sized,
    };
    let selected = |array: usize| IrFixedValue::Selected {
        array,
        selector: index_selector(),
    };
    let stream = |parts: Vec<IrFixedValue>| IrFixedValue::Stream { parts, slice: 0 };
    model
        .validate_stmt(
            &assign(stream(vec![
                packed(16, 8, false),
                selected(1),
                IrFixedValue::Dense(whole_view(&model, 2)),
                packed(32, 8, true),
            ])),
            None,
        )
        .expect("packed, selected, dense and runtime-sized operands");
    model
        .validate_stmt(&assign(packed(40_000, 8, false)), None)
        .expect("a packed operand of the destination's width");
    for (invalid, reason) in [
        (stream(vec![packed(12, 8, false)]), "partial packed cells"),
        (stream(vec![selected(2)]), "a selection of dense storage"),
        (
            stream(vec![IrFixedValue::Dense(whole_view(&model, 1))]),
            "a dense view of descriptor storage",
        ),
        (selected(1), "a runtime-sized whole operand"),
        (packed(16, 8, false), "a narrower whole operand"),
    ] {
        assert!(
            model.validate_stmt(&assign(invalid), None).is_err(),
            "{reason}"
        );
    }
}
