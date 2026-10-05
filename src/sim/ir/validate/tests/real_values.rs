//! SIM-005 real references, real fixed-array operands and real ordering.
use super::*;
use crate::sim::ir::{IrFixedArrayCells, IrFixedArrayOrderMethod, IrRealArrayOrder};

fn real_signal(name: &str) -> IrSignal {
    IrSignal {
        fixed_default: None,
        c_name: name.into(),
        hdl_name: Some(name.into()),
        ty: IrType::Real { shortreal: false },
        net_driver: None,
        net_alias: Vec::new(),
        alias: None,
        omit: false,
    }
}

fn real_array(name: &str, total: i32, activation: bool) -> IrArray {
    let mut array = IrArray::new(name.into(), name.into(), 1, false, vec![(0, total - 1)]).unwrap();
    array.elem_width = 0;
    array.real = true;
    array.activation = activation;
    array
}

fn real_read(signal: usize) -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(signal), 0, false, None)
}

fn real_const(value: f64) -> IrExpr {
    IrExpr::new(IrExprKind::Const(IrConst::real(value)), 0, false, None)
}

fn call(args: Vec<IrCallArg>) -> IrStmt {
    IrStmt::Call(Box::new(IrCall::new(
        0,
        args,
        IrDepth::PROC,
        Vec::new(),
        Vec::new(),
    )))
}

fn model_with(formal: IrFormal, args: Vec<IrCallArg>) -> IrModel {
    let mut model = valid_model();
    model.signals.push(real_signal("r"));
    model.arrays.push(real_array("G_a", 3, false));
    model.arrays.push(real_array("_llg_real_1", 3, true));
    model.funcs.push(IrFunc::new(
        "f".into(),
        None,
        vec![formal],
        vec![],
        vec![],
        vec![],
    ));
    model.processes.push(IrProcess::new(
        "p0".into(),
        "top.p".into(),
        IrShape::RunOnce,
        vec![],
        vec![call(args)],
    ));
    model.spawns.push("p0".into());
    model
}

fn real_ref_formal(const_ref: bool) -> IrFormal {
    let mut formal = IrFormal::new(false, 1, false).unwrap();
    formal.mode = IrFormalMode::Ref;
    formal.width = 0;
    formal.real = true;
    formal.const_ref = const_ref;
    formal
}

fn real_ref_arg(lhs: IrLhs, read: IrExpr) -> IrCallArg {
    IrCallArg::RefAddr {
        addr: "typed_real_reference".into(),
        width: 0,
        signed: false,
        two_state: false,
        const_ref: false,
        lhs: Box::new(lhs),
        read: Box::new(read),
    }
}

#[test]
fn real_references_bind_only_real_storage() {
    let model = model_with(
        real_ref_formal(false),
        vec![real_ref_arg(IrLhs::Whole(1), real_read(1))],
    );
    model.validate().unwrap();

    let element = IrLhs::ArrayElem {
        arr: 0,
        indices: vec![packed_const(1, 32)],
        elem_sel: IrElemSel::Whole,
    };
    let element_read = IrExpr::new(
        IrExprKind::ArrayRead {
            arr: 0,
            indices: vec![packed_const(1, 32)],
            elem_sel: IrElemSel::Whole,
        },
        0,
        false,
        None,
    );
    model_with(
        real_ref_formal(false),
        vec![real_ref_arg(element, element_read.clone())],
    )
    .validate()
    .unwrap();

    // A packed signal, a packed read and a bit-selected reference are not real cells.
    let invalid = [
        real_ref_arg(IrLhs::Whole(0), real_read(1)),
        real_ref_arg(IrLhs::Whole(1), packed_const(1, 1)),
        real_ref_arg(
            IrLhs::Ref {
                addr: "r0".into(),
                width: 0,
                signed: false,
                two_state: false,
                const_ref: false,
                bit: None,
            },
            real_read(1),
        ),
    ];
    for (index, argument) in invalid.into_iter().enumerate() {
        let model = model_with(real_ref_formal(false), vec![argument]);
        assert!(model.validate().is_err(), "invalid real reference {index}");
    }
    // A const actual cannot bind a writable real reference.
    let mut argument = real_ref_arg(IrLhs::Whole(1), real_read(1));
    if let IrCallArg::RefAddr { const_ref, .. } = &mut argument {
        *const_ref = true;
    }
    assert!(model_with(real_ref_formal(false), vec![argument.clone()])
        .validate()
        .is_err());
    model_with(real_ref_formal(true), vec![argument])
        .validate()
        .unwrap();
}

fn real_array_formal(array: usize, is_out: bool) -> IrFormal {
    let mut formal = IrFormal::new(is_out, 1, false).unwrap();
    formal.width = 0;
    formal.real_array = Some(array);
    formal
}

#[test]
fn real_array_operands_match_their_formals() {
    model_with(real_array_formal(1, false), vec![IrCallArg::RealArray(0)])
        .validate()
        .unwrap();
    model_with(
        real_array_formal(1, false),
        vec![IrCallArg::RealArrayValues(vec![
            real_const(1.0),
            real_const(2.0),
            real_const(3.0),
        ])],
    )
    .validate()
    .unwrap();

    let cases = [
        // Element count and element domain must agree with the formal.
        (
            real_array_formal(1, false),
            IrCallArg::RealArrayValues(vec![real_const(1.0)]),
        ),
        (
            real_array_formal(1, false),
            IrCallArg::RealArrayValues(vec![real_const(1.0), real_const(2.0), packed_const(3, 8)]),
        ),
        // Values cannot feed an output, and a packed operand cannot feed a real array.
        (
            real_array_formal(1, true),
            IrCallArg::RealArrayValues(vec![real_const(1.0), real_const(2.0), real_const(3.0)]),
        ),
        (real_array_formal(1, false), IrCallArg::Val(real_const(1.0))),
        // The lexical result array is not declared outside a call operand.
        (real_array_formal(1, false), IrCallArg::RealArray(1)),
    ];
    for (index, (formal, argument)) in cases.into_iter().enumerate() {
        let model = model_with(formal, vec![argument]);
        assert!(
            model.validate().is_err(),
            "invalid real-array operand {index}"
        );
    }

    let mut model = model_with(real_array_formal(1, false), vec![IrCallArg::RealArray(0)]);
    model.arrays[0].shortreal = true;
    assert!(model.validate().is_err(), "shortreal and real cells differ");
    let mut model = model_with(real_array_formal(1, false), vec![IrCallArg::RealArray(0)]);
    model.funcs[0].formals[0].real = true;
    assert!(
        model.validate().is_err(),
        "a real-array formal is not a real scalar"
    );
}

#[test]
fn real_array_ordering_needs_real_scalar_elements() {
    let order = |array, method| {
        IrStmt::RealArrayOrder(Box::new(IrRealArrayOrder {
            cells: IrFixedArrayCells {
                array,
                prefix: Vec::new(),
            },
            method,
        }))
    };
    let mut model = valid_model();
    model.arrays.push(real_array("G_a", 3, false));
    model
        .arrays
        .push(IrArray::new("G_p".into(), "p".into(), 8, false, vec![(0, 2)]).unwrap());
    let mut rows = real_array("G_m", 2, false);
    rows.dims = vec![(0, 1), (0, 1)];
    rows.total = 4;
    model.arrays.push(rows);
    model.processes.push(IrProcess::new(
        "p0".into(),
        "top.p".into(),
        IrShape::RunOnce,
        vec![],
        vec![
            order(0, IrFixedArrayOrderMethod::Sort),
            order(2, IrFixedArrayOrderMethod::Reverse),
        ],
    ));
    model.spawns.push("p0".into());
    model.validate().unwrap();
    for (array, method) in [
        (1, IrFixedArrayOrderMethod::Sort),
        (2, IrFixedArrayOrderMethod::Sort),
        (7, IrFixedArrayOrderMethod::Reverse),
    ] {
        model.processes[0].body = vec![order(array, method)];
        assert!(model.validate().is_err(), "invalid real ordering {array}");
    }
}

#[test]
fn sampled_value_registration_requires_a_packed_signal() {
    let mut model = valid_model();
    let packed = model.signals.len() - 1;
    model.signals.push(real_signal("r"));
    let real = model.signals.len() - 1;
    model
        .init_steps
        .push(IrInitStep::RegisterSampledValue(packed));
    model.validate().unwrap();
    for invalid in [real, real + 1] {
        let mut model = model.clone();
        model
            .init_steps
            .push(IrInitStep::RegisterSampledValue(invalid));
        assert!(model.validate().is_err(), "sampled value source {invalid}");
    }
}
