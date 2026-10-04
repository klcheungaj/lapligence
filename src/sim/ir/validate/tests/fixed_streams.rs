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
