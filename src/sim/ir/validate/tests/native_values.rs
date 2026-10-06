//! Descriptor-backed native values (SIM-003): type, identity and use checks.
use super::*;

fn packed(width: u32) -> IrContainerElement {
    IrContainerElement::Packed {
        width,
        signed: false,
        two_state: false,
    }
}

fn member(name: &str, element: IrContainerElement) -> IrContainerMember {
    IrContainerMember {
        name: name.into(),
        element: Box::new(element),
    }
}

/// struct { string s; real r; chandle h; struct { logic [7:0] p; } i; string a[0:2]; }
fn record() -> IrContainerElement {
    IrContainerElement::Aggregate {
        type_id: 3,
        members: vec![
            member("s", IrContainerElement::String),
            member("r", IrContainerElement::Real { shortreal: false }),
            member("h", IrContainerElement::Chandle),
            member(
                "i",
                IrContainerElement::Aggregate {
                    type_id: 4,
                    members: vec![member("p", packed(8))],
                },
            ),
            member(
                "a",
                IrContainerElement::FixedArray {
                    dimensions: vec![(0, 2)],
                    element: Box::new(IrContainerElement::String),
                },
            ),
        ],
    }
}

fn native_model() -> IrModel {
    let mut model = valid_model();
    model.native_types.push(record());
    model.native_values.push(IrNativeValue {
        c_name: "S_llg_native_0".into(),
        ty: 0,
        activation: false,
        companions: Vec::new(),
    });
    model.native_values.push(IrNativeValue {
        c_name: "S_llg_native_1".into(),
        ty: 0,
        activation: true,
        companions: Vec::new(),
    });
    model
}

fn leaf_access(index: usize, value: usize, path: Vec<u32>, ty: IrClassFieldType) -> IrNativeAccess {
    IrNativeAccess {
        name: format!("_llg_access_{index}"),
        receiver: IrChandleExpr::Null,
        kind: IrNativeAccessKind::ValueItem { value, ty },
        site: None,
        item_path: path,
        function: None,
    }
}

fn function(body: Vec<IrStmt>, formals: Vec<IrFormal>) -> IrFunc {
    IrFunc::new("f".into(), None, formals, vec![], vec![], body)
}

fn native_formal(value: usize, is_out: bool) -> IrFormal {
    let mut formal = IrFormal::new(is_out, 1, false).unwrap();
    formal.width = 0;
    formal.native_value = Some(value);
    formal
}

#[test]
fn native_types_are_bounded_records_with_scalar_leaves() {
    let mut model = native_model();
    model.validate().unwrap();

    let invalid = [
        // A root must be a record or a fixed array, never a scalar leaf.
        IrContainerElement::String,
        IrContainerElement::Aggregate {
            type_id: 1,
            members: vec![],
        },
        IrContainerElement::Aggregate {
            type_id: u64::MAX,
            members: vec![member("s", IrContainerElement::String)],
        },
        IrContainerElement::Aggregate {
            type_id: 1,
            members: vec![member("p", packed(0))],
        },
        IrContainerElement::Aggregate {
            type_id: 1,
            members: vec![member("p", packed(crate::sim::emit_c::LLG_MAX_WIDTH + 1))],
        },
        // Unions, containers, events and class handles have no native leaf form.
        IrContainerElement::Aggregate {
            type_id: 1,
            members: vec![member(
                "u",
                IrContainerElement::Union {
                    type_id: 2,
                    members: vec![member("p", packed(8))],
                },
            )],
        },
        IrContainerElement::Aggregate {
            type_id: 1,
            members: vec![member("e", IrContainerElement::Event)],
        },
        IrContainerElement::FixedArray {
            dimensions: vec![],
            element: Box::new(IrContainerElement::String),
        },
        // 2^24 + 1 items exceed the per-level item limit.
        IrContainerElement::FixedArray {
            dimensions: vec![(0, 1 << 24)],
            element: Box::new(IrContainerElement::String),
        },
        // The item count product overflows 64 bits.
        IrContainerElement::FixedArray {
            dimensions: vec![(i32::MIN, i32::MAX); 3],
            element: Box::new(IrContainerElement::String),
        },
    ];
    for (index, ty) in invalid.into_iter().enumerate() {
        model.native_types[0] = ty;
        assert!(model.validate().is_err(), "invalid native type {index}");
    }

    // Nesting is bounded by the runtime descriptor depth.
    let mut deep = IrContainerElement::String;
    for _ in 0..crate::sim::ir::native_values::LLG_MAX_NATIVE_TYPE_DEPTH {
        deep = IrContainerElement::FixedArray {
            dimensions: vec![(0, 0)],
            element: Box::new(deep),
        };
    }
    model.native_types[0] = deep;
    assert!(model.validate().is_err(), "native type nests too deeply");
}

#[test]
fn native_value_identities_and_leaf_paths_must_resolve() {
    let mut model = native_model();
    model.native_values[0].ty = 1;
    assert!(model.validate().is_err(), "type reference out of bounds");
    model.native_values[0].ty = 0;
    model.native_values[1].activation = false;
    model.native_values[1].c_name = "S_llg_native_0".into();
    assert!(model.validate().is_err(), "duplicate persistent storage");
    model.native_values[1] = IrNativeValue {
        c_name: String::new(),
        ty: 0,
        activation: true,
        companions: Vec::new(),
    };
    model.validate().unwrap();

    let string = IrClassFieldType::String;
    let byte = IrClassFieldType::Packed {
        width: 8,
        signed: false,
        two_state: false,
    };
    model.native_accesses = vec![leaf_access(0, 0, vec![0], string)];
    model.validate().unwrap();
    model.native_accesses = vec![leaf_access(0, 0, vec![3, 0], byte)];
    model.validate().unwrap();
    model.native_accesses = vec![leaf_access(0, 0, vec![4, 2], string)];
    model.validate().unwrap();
    for (path, ty) in [
        (vec![], string),
        (vec![3], byte),
        (vec![3, 1], byte),
        (vec![4, 3], string),
        (vec![9], string),
        (vec![0], byte),
        (vec![3, 0], string),
    ] {
        model.native_accesses = vec![leaf_access(0, 0, path.clone(), ty)];
        assert!(model.validate().is_err(), "leaf path {path:?} {ty:?}");
    }
    model.native_accesses = vec![leaf_access(0, 2, vec![0], string)];
    assert!(model.validate().is_err(), "native value out of bounds");
    let mut access = leaf_access(0, 0, vec![0], string);
    access.receiver = IrChandleExpr::Read(0);
    model.native_accesses = vec![access];
    assert!(model.validate().is_err(), "value items have no receiver");
}

#[test]
fn native_storage_lifetimes_and_copies_are_checked() {
    let mut model = native_model();
    let copy = IrStmt::NativeValueCopy { dst: 0, src: 1 };
    model.funcs.push(function(vec![copy.clone()], vec![]));
    assert!(
        model.validate().is_err(),
        "activation used before declaration"
    );
    model.funcs[0].body = vec![IrStmt::NativeValueDeclare(1), copy.clone()];
    model.validate().unwrap();
    model.funcs[0].body = vec![
        IrStmt::Block(vec![IrStmt::NativeValueDeclare(1)]),
        copy.clone(),
    ];
    assert!(model.validate().is_err(), "activation used after its scope");
    model.funcs[0].body = vec![IrStmt::NativeValueDeclare(1), IrStmt::NativeValueDeclare(1)];
    assert!(model.validate().is_err(), "duplicate declaration");
    model.funcs[0].body = vec![IrStmt::NativeValueDeclare(0)];
    assert!(
        model.validate().is_err(),
        "persistent values are not lexical"
    );
    model.funcs[0].body = vec![IrStmt::NativeValueDeclare(5)];
    assert!(model.validate().is_err(), "declaration out of bounds");

    // A formal binds activation storage without a declaration.
    model.funcs[0].body = vec![copy.clone()];
    model.funcs[0].formals = vec![native_formal(1, false)];
    model.validate().unwrap();

    // Copies require one native type.
    model.native_types.push(IrContainerElement::Aggregate {
        type_id: 9,
        members: vec![member("s", IrContainerElement::String)],
    });
    model.native_values[0].ty = 1;
    assert!(model.validate().is_err(), "copy between native types");
}

#[test]
fn native_call_operands_match_their_formals() {
    let mut model = native_model();
    model
        .funcs
        .push(function(vec![], vec![native_formal(1, false)]));
    let call = |args| {
        IrStmt::Call(Box::new(IrCall::new(
            0,
            args,
            IrDepth::PROC,
            Vec::new(),
            Vec::new(),
        )))
    };
    let leaf = |items: Vec<u32>, value| IrNativeLeafValue { items, value };
    let string =
        |text: &str| IrNativeLeafExpr::String(IrStringExpr::Literal(text.as_bytes().to_vec()));
    model.processes.push(IrProcess::new(
        "p0".into(),
        "top.p".into(),
        IrShape::RunOnce,
        vec![],
        vec![call(vec![IrCallArg::NativeValue(0)])],
    ));
    model.spawns.push("p0".into());
    model.validate().unwrap();

    let cases: Vec<(Vec<IrCallArg>, bool)> = vec![
        (
            vec![IrCallArg::NativeLeaves {
                ty: 0,
                leaves: vec![
                    leaf(vec![0], string("a")),
                    leaf(vec![3, 0], IrNativeLeafExpr::Packed(packed_const(5, 8))),
                    leaf(vec![1], IrNativeLeafExpr::Packed(packed_const(2, 8))),
                ],
                containers: Vec::new(),
            }],
            true,
        ),
        // Packed values and scalar leaves must agree in kind.
        (
            vec![IrCallArg::NativeLeaves {
                ty: 0,
                leaves: vec![leaf(vec![0], IrNativeLeafExpr::Packed(packed_const(1, 8)))],
                containers: Vec::new(),
            }],
            false,
        ),
        (
            vec![IrCallArg::NativeLeaves {
                ty: 0,
                leaves: vec![leaf(vec![0], string("a")), leaf(vec![0], string("b"))],
                containers: Vec::new(),
            }],
            false,
        ),
        (
            vec![IrCallArg::NativeLeaves {
                ty: 0,
                leaves: vec![leaf(vec![7], string("a"))],
                containers: Vec::new(),
            }],
            false,
        ),
        (
            vec![IrCallArg::NativeLeaves {
                ty: 3,
                leaves: vec![],
                containers: Vec::new(),
            }],
            false,
        ),
        (vec![IrCallArg::NativeValue(4)], false),
        (vec![IrCallArg::Val(packed_const(1, 1))], false),
    ];
    for (index, (args, valid)) in cases.into_iter().enumerate() {
        model.processes[0].body = vec![call(args)];
        assert_eq!(model.validate().is_ok(), valid, "call case {index}");
    }

    // A native formal has no other value ABI.
    model.processes[0].body = vec![call(vec![IrCallArg::NativeValue(0)])];
    model.funcs[0].formals[0].string = true;
    assert!(model.validate().is_err(), "conflicting formal ABI");
    model.funcs[0].formals[0].string = false;
    model.funcs[0].formals[0].native_value = Some(9);
    assert!(model.validate().is_err(), "formal storage out of bounds");
}

#[test]
fn native_companion_containers_follow_their_values() {
    let queue = |activation: bool, name: &str| IrContainer {
        c_name: name.into(),
        element: packed(32),
        kind: IrContainerKind::Queue {
            maximum_elements: None,
        },
        initial_size: None,
        activation,
        class_field: None,
    };
    let mut model = valid_model();
    // struct { string s; int q[$]; }: the queue is a null descriptor slot.
    model.native_types.push(IrContainerElement::Aggregate {
        type_id: 5,
        members: vec![
            member("s", IrContainerElement::String),
            member(
                "q",
                IrContainerElement::Container {
                    type_id: 6,
                    kind: "Queue".into(),
                    element: Box::new(packed(32)),
                },
            ),
        ],
    });
    // 0: a static value's companion, 1: the callee formal's companion.
    model.containers.push(queue(false, "S_llg_container_0"));
    model.containers.push(queue(true, "S_llg_container_1"));
    model.native_values.push(IrNativeValue {
        c_name: "S_llg_native_0".into(),
        ty: 0,
        activation: false,
        companions: vec![0],
    });
    model.native_values.push(IrNativeValue {
        c_name: "S_llg_native_1".into(),
        ty: 0,
        activation: true,
        companions: vec![1],
    });
    let mut formal = native_formal(1, false);
    formal.native_companions = vec![1];
    model.funcs.push(function(vec![], vec![formal]));
    let call = |args| {
        IrStmt::Call(Box::new(IrCall::new(
            0,
            args,
            IrDepth::PROC,
            Vec::new(),
            Vec::new(),
        )))
    };
    model.processes.push(IrProcess::new(
        "p0".into(),
        "top.p".into(),
        IrShape::RunOnce,
        vec![],
        vec![call(vec![IrCallArg::NativeValue(0)])],
    ));
    model.spawns.push("p0".into());
    model.validate().unwrap();

    // Leaf operands copy one source container per companion.
    for (containers, valid) in [(vec![0], true), (vec![], false), (vec![7], false)] {
        model.processes[0].body = vec![call(vec![IrCallArg::NativeLeaves {
            ty: 0,
            leaves: vec![],
            containers,
        }])];
        assert_eq!(model.validate().is_ok(), valid, "leaf operand containers");
    }
    model.processes[0].body = vec![call(vec![IrCallArg::NativeValue(0)])];

    // A formal lists exactly its value's companions.
    model.funcs[0].formals[0].native_companions = Vec::new();
    assert!(model.validate().is_err(), "formal companions");
    model.funcs[0].formals[0].native_companions = vec![1];

    // Companions share their value's lifetime and exist.
    model.native_values[0].companions = vec![1];
    assert!(model.validate().is_err(), "companion lifetime");
    model.native_values[0].companions = vec![9];
    assert!(model.validate().is_err(), "companion bounds");
    model.native_values[0].companions = vec![];
    assert!(model.validate().is_err(), "argument companion count");
}

#[test]
fn statically_bound_record_refs_take_the_bound_operand_only() {
    let mut model = native_model();
    let mut formal = native_formal(1, false);
    formal.mode = IrFormalMode::Ref;
    // Function 0 is the template; function 1 binds its formal statically.
    model.funcs.push(function(vec![], vec![formal.clone()]));
    let mut bound = function(vec![], vec![formal]);
    bound.bound_native_refs = vec![0];
    model.funcs.push(bound);
    let call = |function, args| {
        IrStmt::Call(Box::new(IrCall::new(
            function,
            args,
            IrDepth::PROC,
            Vec::new(),
            Vec::new(),
        )))
    };
    model.processes.push(IrProcess::new(
        "p0".into(),
        "top.p".into(),
        IrShape::RunOnce,
        vec![],
        vec![
            call(0, vec![IrCallArg::NativeValue(0)]),
            call(1, vec![IrCallArg::NativeRefBound]),
        ],
    ));
    model.spawns.push("p0".into());
    model.validate().unwrap();

    // The bound marker and value operands never stand in for each other.
    for (function, argument) in [
        (0, IrCallArg::NativeRefBound),
        (1, IrCallArg::NativeValue(0)),
    ] {
        model.processes[0].body = vec![call(function, vec![argument])];
        assert!(model.validate().is_err(), "operand for function {function}");
    }
    model.processes[0].body = vec![call(1, vec![IrCallArg::NativeRefBound])];

    // The bound body has no value for that formal.
    model.funcs[1].body = vec![IrStmt::NativeValueCopy { dst: 0, src: 1 }];
    assert!(model.validate().is_err(), "bound formal value used");
    model.funcs[1].body = Vec::new();

    // Only native `ref` formals bind statically.
    model.funcs[1].formals[0].mode = IrFormalMode::Input;
    assert!(model.validate().is_err(), "bound input formal");
    model.funcs[1].formals[0].mode = IrFormalMode::Ref;
    model.funcs[1].bound_native_refs = vec![1];
    assert!(model.validate().is_err(), "bound formal out of bounds");
}
