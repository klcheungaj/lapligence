use super::*;

mod array_conditionals;
mod fixed_array_cells;
mod fixed_array_reductions;
mod fixed_streams;
mod native_values;
mod real_values;
mod sequential_predicates;

fn valid_model() -> IrModel {
    let mut model = IrModel::new("top".to_string(), 1).unwrap();
    model.signals = vec![IrSignal {
        fixed_default: None,
        c_name: "sig".to_string(),
        hdl_name: Some("sig".to_string()),
        ty: IrType::Packed {
            width: 1,
            signed: false,
            two_state: false,
        },
        net_driver: None,
        net_alias: Vec::new(),
        alias: None,
        omit: false,
    }];
    model
}

fn packed_const(value: u64, width: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(
            IrConst::packed(vec![value], vec![], vec![], width, false, None).unwrap(),
        ),
        width,
        false,
        None,
    )
}

#[test]
fn fixed_storage_defaults_and_shapes_must_match_payload_widths() {
    let mut model = valid_model();
    model.signals[0].fixed_default =
        Some(IrConst::packed(vec![0], vec![], vec![], 8, false, None).unwrap());
    assert!(model.validate().is_err());
    model.signals[0].fixed_default = None;
    let mut formal = IrFormal::new(false, 8, false).unwrap();
    formal.fixed_shape = Some(IrContainerElement::FixedArray {
        dimensions: vec![(1, 0)],
        element: Box::new(IrContainerElement::Packed {
            width: 8,
            signed: false,
            two_state: false,
        }),
    });
    model.funcs.push(IrFunc::new(
        "callee".into(),
        None,
        vec![formal],
        vec![],
        vec![],
        vec![],
    ));
    assert!(model.validate().is_err());
}

#[test]
fn resolved_array_elements_require_matching_net_storage() {
    let mut model = valid_model();
    let mut array = IrArray::new("a".into(), "a".into(), 1, false, vec![(0, 1)]).unwrap();
    array.net_elements.push((0, 0));
    model.arrays.push(array);
    assert!(model.validate().is_err());
}

#[test]
fn undriven_net_cell_runs_cover_each_cell_exactly_once() {
    use crate::sim::ir::{IrNetArray, IrNetCellRun, IrNetKind};
    let run = |first, count| IrNetCellRun {
        first,
        count,
        kind: IrNetKind::Tri1,
    };
    let check = |runs: Vec<IrNetCellRun>| {
        let mut model = valid_model();
        let mut array = IrArray::new("a".into(), "a".into(), 4, false, vec![(0, 5)]).unwrap();
        array.net = Some(IrNetArray {
            constant_cells: runs,
        });
        model.arrays.push(array);
        model.validate()
    };
    assert!(check(vec![run(0, 2), run(2, 4)]).is_ok());
    // Uncovered, overlapping, empty and out-of-range runs are all rejected.
    assert!(check(vec![run(0, 5)]).is_err());
    assert!(check(vec![run(0, 3), run(2, 4)]).is_err());
    assert!(check(vec![run(0, 6), run(6, 0)]).is_err());
    assert!(check(vec![run(1, 6)]).is_err());
}

#[test]
fn selected_const_references_cannot_hide_a_writable_binding() {
    let mut model = valid_model();
    let mut formal = IrFormal::new(false, 1, false).unwrap();
    formal.mode = IrFormalMode::Ref;
    model.funcs.push(IrFunc::new(
        "callee".into(),
        None,
        vec![formal],
        vec![],
        vec![],
        vec![],
    ));
    let lhs = IrLhs::PackedSelect {
        target: Box::new(IrLhs::Ref {
            addr: "r0".into(),
            width: 8,
            signed: false,
            two_state: false,
            const_ref: true,
            bit: None,
        }),
        steps: vec![IrPackedSelect {
            base: packed_const(0, 32),
            width: 1,
        }],
        signed: false,
        two_state: false,
    };
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::RefAddr {
                addr: "view".into(),
                width: 1,
                signed: false,
                two_state: false,
                const_ref: false,
                lhs: Box::new(lhs),
                read: Box::new(packed_const(0, 1)),
            }],
            IrDepth::PROC,
            true,
        ))),
        1,
        false,
        None,
    );
    assert!(model.validate_expr(&call, None).is_err());
}

#[test]
fn rejects_out_of_bounds_signal_reference() {
    let mut model = valid_model();
    model.processes.push(IrProcess {
        c_name: "proc".to_string(),
        label: "top.initial".to_string(),
        kind: IrProcessKind::Synthetic,
        shape: IrShape::RunOnce,
        writes: Vec::new(),
        pre_fns: Vec::new(),
        body: vec![IrStmt::Release {
            lhs: IrLhs::Whole(1),
        }],
        program: None,
        origin: crate::sim::semantic::Origin::Synthetic {
            reason: "validation fixture".to_owned(),
        },
    });
    model.spawns.push("proc".to_string());

    let error = model
        .validate()
        .expect_err("invalid signal index must fail");
    assert_eq!(error.path(), "processes[0].body[0].lhs");
    assert!(error.detail().contains("signal index 1"));
}

#[test]
fn reference_bit_targets_validate_shape_and_visit_the_index() {
    let model = valid_model();
    let mut lhs = IrLhs::Ref {
        addr: "r0".to_owned(),
        width: 1,
        signed: false,
        two_state: false,
        const_ref: false,
        bit: Some(Box::new(packed_const(0, 129))),
    };
    let statement = |lhs| IrStmt::Assign {
        lhs,
        rhs: packed_const(1, 1),
        nba: false,
    };
    assert_eq!(
        model
            .statement_capacity(&statement(lhs.clone()), None)
            .unwrap(),
        129
    );
    let mut indices = 0;
    lhs.expressions(&mut |_| indices += 1);
    assert_eq!(indices, 1);
    lhs.expressions_mut(&mut |index| {
        *index = IrExpr::new(IrExprKind::SigRead(7), 1, false, None);
    });
    assert!(model
        .validate_stmt(&statement(lhs.clone()), None)
        .unwrap_err()
        .detail()
        .contains("signal index 7"));
    if let IrLhs::Ref { width, .. } = &mut lhs {
        *width = 2;
    }
    assert!(model
        .validate_stmt(&statement(lhs), None)
        .unwrap_err()
        .detail()
        .contains("one unsigned bit"));
}

#[test]
fn rejects_array_total_that_disagrees_with_dimensions() {
    let mut model = valid_model();
    model.arrays.push(IrArray {
        activation: false,
        descriptor: false,
        net: None,
        net_elements: Vec::new(),
        element_default: None,
        element_uninitialized: None,
        c_name: "memory".to_string(),
        hdl_name: "memory".to_string(),
        elem_width: 8,
        signed: false,
        two_state: false,
        real: false,
        shortreal: false,
        dims: vec![(3, 0), (1, 0)],
        total: 7,
    });

    let error = model.validate().expect_err("invalid array total must fail");
    assert_eq!(error.path(), "arrays[0].total");
    assert!(error.detail().contains("dimension product 8"));
}

#[test]
fn rejects_array_storage_above_selected_cell_limit() {
    let mut model = valid_model();
    model.arrays.push(IrArray {
        activation: false,
        descriptor: false,
        net: None,
        net_elements: Vec::new(),
        element_default: None,
        element_uninitialized: None,
        c_name: "memory".to_string(),
        hdl_name: "memory".to_string(),
        elem_width: 1,
        signed: false,
        two_state: false,
        real: false,
        shortreal: false,
        dims: vec![(0, crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS as i32)],
        total: crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS + 1,
    });

    let error = model
        .validate()
        .expect_err("over-limit fixed-array storage must fail validation");
    assert_eq!(error.path(), "arrays[0].total");
    assert!(error.detail().contains("selected cell-wise storage limit"));
}

#[test]
fn accepts_a_minimal_well_formed_model() {
    valid_model().validate().expect("minimal model is valid");
}

#[test]
fn runtime_delays_validate_scaling_and_include_expression_capacity() {
    let model = valid_model();
    let delay = |unit_ticks, precision_ticks| IrStmt::Delay {
        ticks: IrDelay::Runtime {
            value: Box::new(packed_const(1, 129)),
            unit_ticks,
            precision_ticks,
        },
    };
    assert_eq!(
        model.statement_capacity(&delay(1000, 100), None).unwrap(),
        129
    );
    for (unit, precision) in [(0, 1), (1, 0), (3, 2), (1, 10)] {
        assert!(model.validate_stmt(&delay(unit, precision), None).is_err());
    }
}

#[test]
fn real_local_captures_require_real_initializers() {
    let model = valid_model();
    let local = |init| IrStmt::DeclLocal {
        name: "capture".into(),
        width: 0,
        signed: true,
        two_state: false,
        init,
    };
    assert!(model.validate_stmt(&local(None), None).is_err());
    assert!(model
        .validate_stmt(&local(Some(Box::new(packed_const(1, 32)))), None)
        .is_err());
    let real = IrExpr::new(IrExprKind::Const(IrConst::real(1.25)), 0, false, None);
    model
        .validate_stmt(&local(Some(Box::new(real))), None)
        .unwrap();
}

#[test]
fn delayed_nba_rejects_unproven_pointer_lifetimes() {
    let statement = IrStmt::DelayedAssign {
        lhs: IrLhs::WholeRef {
            addr: "&local".into(),
            width: 1,
            signed: false,
            two_state: false,
            shortreal: false,
        },
        rhs: packed_const(1, 1),
        ticks: IrDelay::Constant(2),
    };
    let error = valid_model().validate_stmt(&statement, None).unwrap_err();
    assert!(error.detail().contains("persistent"));
}

#[test]
fn nonblocking_assignments_accept_only_static_function_return_slots() {
    let statement = IrStmt::Assign {
        lhs: IrLhs::WholeRef {
            addr: "&_ret".into(),
            width: 8,
            signed: false,
            two_state: false,
            shortreal: false,
        },
        rhs: packed_const(0x99, 8),
        nba: true,
    };
    let mut static_function = IrFunc::new(
        "static_result".into(),
        Some(IrType::Packed {
            width: 8,
            signed: false,
            two_state: false,
        }),
        vec![],
        vec![],
        vec![],
        vec![],
    );
    static_function.automatic = false;
    valid_model()
        .validate_stmt(&statement, Some(&static_function))
        .unwrap();

    let automatic_function = IrFunc::new(
        "automatic_result".into(),
        Some(IrType::Packed {
            width: 8,
            signed: false,
            two_state: false,
        }),
        vec![],
        vec![],
        vec![],
        vec![],
    );
    let error = valid_model()
        .validate_stmt(&statement, Some(&automatic_function))
        .unwrap_err();
    assert!(error.detail().contains("persistent target storage"));
}

#[test]
fn queued_writes_accept_only_registered_static_local_storage() {
    let mut model = valid_model();
    let root = IrLhs::WholeRef {
        addr: "&stored".into(),
        width: 8,
        signed: false,
        two_state: false,
        shortreal: false,
    };
    let selected = IrLhs::PackedSelect {
        target: Box::new(root.clone()),
        steps: vec![IrPackedSelect {
            base: packed_const(0, 32),
            width: 8,
        }],
        signed: false,
        two_state: false,
    };
    let statements = vec![
        IrStmt::Assign {
            lhs: root,
            rhs: packed_const(1, 8),
            nba: true,
        },
        IrStmt::DelayedAssign {
            lhs: selected,
            rhs: packed_const(2, 8),
            ticks: IrDelay::Constant(1),
        },
    ];
    let function = IrFunc::new(
        "task".into(),
        None,
        vec![],
        vec![IrLocal::new("stored".into(), 8, false).unwrap()],
        vec![],
        statements.clone(),
    );
    for statement in &statements {
        model.validate_stmt(statement, Some(&function)).unwrap();
        assert!(model.validate_stmt(statement, None).is_err());
        let mut automatic = function.clone();
        automatic.locals.clear();
        assert!(model.validate_stmt(statement, Some(&automatic)).is_err());
        let mut mismatched = function.clone();
        mismatched.locals[0].width = 16;
        assert!(model.validate_stmt(statement, Some(&mismatched)).is_err());
    }
    model.funcs.push(function);
    model.validate().unwrap();
}

#[test]
fn inertial_updates_require_persistent_packed_drivers() {
    let model = valid_model();
    let statement = |lhs, rhs| IrStmt::InertialAssign {
        lhs,
        rhs,
        delay: IrTransitionDelay::uniform(2),
    };
    assert_eq!(
        model
            .statement_capacity(&statement(IrLhs::Whole(0), packed_const(1, 129)), None,)
            .unwrap(),
        129
    );
    for lhs in [
        IrLhs::Whole(1),
        IrLhs::WholeRef {
            addr: "&local".into(),
            width: 1,
            signed: false,
            two_state: false,
            shortreal: false,
        },
    ] {
        assert!(model
            .validate_stmt(&statement(lhs, packed_const(1, 1)), None)
            .is_err());
    }
    let real = IrExpr::new(IrExprKind::Const(IrConst::real(1.0)), 0, false, None);
    assert!(model
        .validate_stmt(&statement(IrLhs::Whole(0), real), None)
        .is_err());
}

#[test]
fn math_and_realtime_require_valid_shapes_and_units() {
    let model = valid_model();
    let math = |args, width| {
        IrExpr::new(
            IrExprKind::SysFunc(Box::new(IrSysFunc::Math {
                kind: IrMathFunc::Pow,
                args,
            })),
            width,
            true,
            None,
        )
    };
    model
        .validate_expr(&math(vec![packed_const(2, 2), packed_const(3, 2)], 0), None)
        .unwrap();
    assert!(model
        .validate_expr(&math(vec![packed_const(2, 2)], 0), None)
        .is_err());
    assert!(model
        .validate_expr(
            &math(vec![packed_const(2, 2), packed_const(3, 2)], 32),
            None
        )
        .is_err());
    let time = IrExpr::new(
        IrExprKind::SysFunc(Box::new(IrSysFunc::Realtime {
            precision_fs: 1,
            unit_fs: 0,
        })),
        0,
        true,
        None,
    );
    assert!(model.validate_expr(&time, None).is_err());
}

#[test]
fn variable_aliases_require_matching_canonical_storage() {
    let mut model = valid_model();
    let mut alias = model.signals[0].clone();
    alias.hdl_name = Some("child.sig".into());
    alias.alias = Some(0);
    model.signals.push(alias);
    model.validate().expect("alias shares canonical storage");

    for target in [1, 2] {
        model.signals[1].alias = Some(target);
        assert!(model.validate().unwrap_err().path().ends_with(".alias"));
    }
    model.signals[1].alias = Some(0);
    model.signals[0].omit = true;
    assert!(model.validate().is_err(), "live alias retains its target");
    model.signals[0].omit = false;
    model.signals[1].c_name = "other".into();
    assert!(model.validate().is_err(), "alias uses the same C storage");
}

#[test]
fn evaluated_waits_require_valid_helpers_and_dependencies() {
    let mut model = valid_model();
    let process = IrProcess::new(
        "proc".into(),
        "top.initial".into(),
        IrShape::RunOnce,
        vec![IrPreFn::MonEval {
            c_name: "eval".into(),
            args: vec![packed_const(1, 1)],
            context: None,
            item: false,
            real_item: false,
        }],
        vec![IrStmt::WaitEvents {
            specs: vec![(
                IrWaitSrc::Evaluated {
                    eval: "eval".into(),
                    condition: None,
                    reads: vec!["sig".into()],
                },
                IrEdge::Any,
            )],
        }],
    );
    model.processes.push(process);
    model.spawns.push("proc".into());
    model.validate().expect("valid expression wait");
    model.processes[0].pre_fns.clear();
    assert!(model.validate().unwrap_err().detail().contains("evaluator"));
    model.processes[0].pre_fns.push(IrPreFn::MonEval {
        c_name: "eval".into(),
        args: vec![packed_const(1, 1)],
        context: None,
        item: false,
        real_item: false,
    });
    model.signals[0].omit = true;
    assert!(model
        .validate()
        .unwrap_err()
        .detail()
        .contains("dependency"));
}

#[test]
fn z_array_initialization_requires_valid_four_state_storage() {
    let mut model = valid_model();
    model.init_steps.push(IrInitStep::FillArrayZ(0));
    assert!(model
        .validate()
        .unwrap_err()
        .detail()
        .contains("array index"));
    model
        .arrays
        .push(IrArray::new("array".into(), "array".into(), 65, false, vec![(0, 1)]).unwrap());
    model.validate().expect("four-state Z initialization");
    model.arrays[0].two_state = true;
    assert!(model
        .validate()
        .unwrap_err()
        .detail()
        .contains("four-state"));
}

#[test]
fn statement_initialization_validates_its_body() {
    let execute = |sig| IrInitStep::Execute {
        declaration: 0,
        body: Box::new(IrStmt::Assign {
            lhs: IrLhs::Whole(sig),
            rhs: packed_const(1, 1),
            nba: false,
        }),
    };
    let mut model = valid_model();
    model.init_steps.push(execute(0));
    model
        .validate()
        .expect("a statement initializer may write static storage");
    model.init_steps[0] = execute(1);
    let error = model
        .validate()
        .expect_err("a statement initializer body is validated");
    assert!(
        error.path().starts_with("init_steps[0].body"),
        "{}",
        error.path()
    );
}

#[test]
fn indexed_lhs_selected_width_contributes_to_capacity() {
    let statement = IrStmt::Assign {
        lhs: IrLhs::IdxPart(
            0,
            Box::new(packed_const(0, 32)),
            Box::new(packed_const(96, 32)),
            96,
            false,
            false,
        ),
        rhs: packed_const(1, 1),
        nba: false,
    };

    assert_eq!(
        valid_model().statement_capacity(&statement, None).unwrap(),
        96
    );
}

#[test]
fn streaming_lhs_explicit_width_contributes_to_capacity() {
    let statement = IrStmt::Assign {
        lhs: IrLhs::Stream {
            parts: vec![
                (IrLhs::Part(0, 63, 0, false), 64),
                (IrLhs::Part(0, 31, 0, false), 32),
            ],
            width: 96,
            slice: 8,
            direction: IrStreamDirection::RightToLeft,
        },
        rhs: packed_const(1, 1),
        nba: false,
    };

    assert_eq!(
        valid_model().statement_capacity(&statement, None).unwrap(),
        96
    );
}

#[test]
fn indexed_lhs_preserves_wide_base_expression_capacity() {
    let part = || packed_const(0, 32);
    let base = IrExpr::new(
        IrExprKind::Concat {
            parts: vec![part(), part(), part()],
        },
        96,
        false,
        None,
    );
    let statement = IrStmt::Assign {
        lhs: IrLhs::IdxPart(
            0,
            Box::new(base),
            Box::new(packed_const(8, 32)),
            8,
            false,
            false,
        ),
        rhs: packed_const(1, 1),
        nba: false,
    };

    assert_eq!(
        valid_model().statement_capacity(&statement, None).unwrap(),
        96
    );
}

#[test]
fn streaming_and_inside_children_contribute_to_capacity() {
    let stream = IrExpr::new(
        IrExprKind::Stream {
            value: Box::new(packed_const(0xa5, 128)),
            slice: 8,
            direction: IrStreamDirection::RightToLeft,
        },
        128,
        false,
        None,
    );
    assert_eq!(
        valid_model().expression_capacity(&stream, None).unwrap(),
        128
    );

    let inside = IrExpr::new(
        IrExprKind::Inside {
            value: Box::new(packed_const(1, 512)),
            items: vec![
                IrInsideItem::Value(packed_const(1, 8)),
                IrInsideItem::Range {
                    low: packed_const(0, 32),
                    high: packed_const(3, 32),
                },
            ],
        },
        1,
        false,
        None,
    );
    assert_eq!(
        valid_model().expression_capacity(&inside, None).unwrap(),
        512
    );
}

#[test]
fn constructors_reject_invalid_local_invariants() {
    assert!(IrModel::new("top".to_string(), 0).is_err());
    assert!(IrType::packed(0, false).is_err());
    assert!(IrExpr::try_new(IrExprKind::Fill(4), 1, false, Some(4)).is_err());
    assert!(IrArray::new("a".into(), "a".into(), 8, false, Vec::new()).is_err());
    assert!(IrNetGroup::new("n".into(), 1, false, IrNetKind::Wire, 0).is_err());
    assert!(IrNetGroup::new(
        "n".into(),
        1,
        false,
        IrNetKind::Wire,
        LLG_MAX_NET_DRIVERS + 1,
    )
    .is_err());
}

#[test]
fn packed_constructor_checks_the_declared_high_limb() {
    let value = IrConst::packed(vec![u64::MAX, 1], vec![], vec![], 65, false, None)
        .expect("bit 64 is inside a 65-bit value");
    assert_eq!(value.bits(), &[u64::MAX, 1]);

    let error = IrConst::packed(vec![0, 2], vec![], vec![], 65, false, None)
        .expect_err("bit 65 lies outside a 65-bit value");
    assert_eq!(error.path(), "const.bits");
}

#[test]
fn function_local_initializer_uses_formals_and_contributes_to_capacity() {
    let formal = IrFormal::new(false, 512, false).expect("valid formal");
    let formal_read = IrExpr::new(IrExprKind::FormalRead(0), 512, false, None);
    let mut local = IrLocal::new("local".to_string(), 8, false).expect("valid local");
    local.initial = Some(IrExpr::resize_to(formal_read, 8, false));
    let function = IrFunc::new(
        "f".to_string(),
        None,
        vec![formal],
        vec![local],
        Vec::new(),
        Vec::new(),
    );
    let model = IrModel::from_parts(
        "top".to_string(),
        1,
        IrModelParts {
            funcs: vec![function],
            ..IrModelParts::default()
        },
    )
    .expect("a typed local initializer may read its function formal");

    assert_eq!(model.packed_capacity().unwrap(), 512);
}

#[test]
fn public_parts_reject_local_initializer_with_wrong_type() {
    let formal = IrFormal::new(false, 8, false).expect("valid formal");
    let mut local = IrLocal::new("local".to_string(), 8, true).expect("valid local");
    local.initial = Some(IrExpr::new(IrExprKind::FormalRead(0), 8, false, None));
    let function = IrFunc::new(
        "f".to_string(),
        None,
        vec![formal],
        vec![local],
        Vec::new(),
        Vec::new(),
    );

    let error = IrModel::from_parts(
        "top".to_string(),
        1,
        IrModelParts {
            funcs: vec![function],
            ..IrModelParts::default()
        },
    )
    .expect_err("a local initializer must have the declaration's exact packed type");
    assert_eq!(error.path(), "funcs[0].locals[0].initial");
    assert!(error.detail().contains("initializer type"));
}

#[test]
fn public_parts_build_a_nonempty_valid_model() {
    let signal = IrSignal::new(
        "sig".to_string(),
        Some("top.sig".to_string()),
        IrType::packed(1, false).unwrap(),
        None,
    )
    .unwrap();
    let model = IrModel::from_parts(
        "top".to_string(),
        1,
        IrModelParts {
            signals: vec![signal],
            ..IrModelParts::default()
        },
    )
    .expect("a nonempty valid model must be constructible through the public API");
    assert_eq!(model.signals().len(), 1);
    assert_eq!(model.signal(0).hdl_name(), Some("top.sig"));
}

#[test]
fn public_parts_reject_invalid_cross_table_references() {
    let process = IrProcess::new(
        "proc".to_string(),
        "top.initial".to_string(),
        IrShape::RunOnce,
        Vec::new(),
        vec![IrStmt::Release {
            lhs: IrLhs::Whole(0),
        }],
    );
    let error = IrModel::from_parts(
        "top".to_string(),
        1,
        IrModelParts {
            processes: vec![process],
            spawns: vec!["proc".to_string()],
            ..IrModelParts::default()
        },
    )
    .expect_err("a model cannot reference a missing signal");
    assert_eq!(error.path(), "processes[0].body[0].lhs");
}

#[test]
fn detached_nested_expression_checks_exact_formal_type() {
    let context = IrFunc::new(
        "context".to_string(),
        None,
        vec![IrFormal::new(false, 8, true).unwrap()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let expression = IrExpr::new(
        IrExprKind::Bin {
            op: IrBinOp::Add,
            a: Box::new(IrExpr::new(IrExprKind::FormalRead(0), 4, false, None)),
            b: Box::new(IrExpr::new(
                IrExprKind::Const(
                    IrConst::packed(vec![1], vec![], vec![], 4, false, None).unwrap(),
                ),
                4,
                false,
                None,
            )),
        },
        4,
        false,
        None,
    );

    let error = valid_model()
        .validate_expr(&expression, Some(&context))
        .expect_err("a nested formal read must carry the formal's exact type");
    assert_eq!(error.path(), "expr.a");
    assert!(error.detail().contains("formal type"));
}

#[test]
fn call_arguments_follow_output_then_input_parameter_order() {
    let callee = IrFunc::new(
        "callee".to_string(),
        Some(IrType::packed(1, false).unwrap()),
        vec![
            IrFormal::new(false, 8, false).unwrap(),
            IrFormal::new(true, 16, true).unwrap(),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let model = IrModel::from_parts(
        "top".to_string(),
        1,
        IrModelParts {
            funcs: vec![callee],
            ..IrModelParts::default()
        },
    )
    .unwrap();
    let input = IrExpr::new(
        IrExprKind::Const(IrConst::packed(vec![7], vec![], vec![], 8, false, None).unwrap()),
        8,
        false,
        None,
    );
    let valid = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![
                IrCallArg::OutAddr("&out".to_string()),
                IrCallArg::Val(input.clone()),
            ],
            IrDepth::PROC,
            false,
        ))),
        1,
        false,
        None,
    );
    model
        .validate_expr(&valid, None)
        .expect("C-order output then input arguments are valid");

    let invalid = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![
                IrCallArg::Val(input),
                IrCallArg::OutAddr("&out".to_string()),
            ],
            IrDepth::PROC,
            false,
        ))),
        1,
        false,
        None,
    );
    let error = model
        .validate_expr(&invalid, None)
        .expect_err("argument variants must agree with formal directions");
    assert_eq!(error.path(), "expr.args[0]");
    assert!(error.detail().contains("address argument"));
}

#[test]
fn string_real_queries_require_real_result_metadata() {
    let model = valid_model();
    let query = IrObjectQuery::StringAtoreal(IrStringExpr::Literal(b"1.5".to_vec()));
    let expression = |width, signed| {
        IrExpr::new(
            IrExprKind::ObjectQuery(Box::new(query.clone())),
            width,
            signed,
            None,
        )
    };
    model.validate_expr(&expression(0, true), None).unwrap();
    assert!(model.validate_expr(&expression(32, true), None).is_err());
    assert!(model.validate_expr(&expression(0, false), None).is_err());
    let packed_query = IrExpr::new(
        IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringLen(IrStringExpr::Literal(
            vec![],
        )))),
        0,
        true,
        None,
    );
    assert!(model.validate_expr(&packed_query, None).is_err());
}

#[test]
fn string_realtoa_requires_real_argument_and_string_storage() {
    let mut model = valid_model();
    model.objects.push(IrObject {
        c_name: "text".to_owned(),
        ty: IrObjectType::String,
        initial: None,
    });
    let real = IrExpr::new(IrExprKind::Const(IrConst::real(1.5)), 0, false, None);
    model
        .validate_stmt(
            &IrStmt::Object(Box::new(IrObjectStmt::StringRealtoa(0, real.clone()))),
            None,
        )
        .unwrap();
    assert!(model
        .validate_stmt(
            &IrStmt::Object(Box::new(IrObjectStmt::StringRealtoa(
                0,
                packed_const(1, 32)
            ))),
            None
        )
        .is_err());
    assert!(model
        .validate_stmt(
            &IrStmt::Object(Box::new(IrObjectStmt::StringItoa(0, real.clone(), 10))),
            None
        )
        .is_err());
    model.objects[0].ty = IrObjectType::Chandle;
    assert!(model
        .validate_stmt(
            &IrStmt::Object(Box::new(IrObjectStmt::StringRealtoa(0, real))),
            None
        )
        .is_err());
}

#[test]
fn string_return_storage_requires_its_function_context() {
    let model = valid_model();
    let value = IrStringExpr::LocalRead("_ret".to_owned());
    let statement = IrStmt::Object(Box::new(IrObjectStmt::StringAssignLocal(
        "_ret".to_owned(),
        value,
    )));
    assert!(model.validate_stmt(&statement, None).is_err());
    let mut function = IrFunc::new("string_fn".to_owned(), None, vec![], vec![], vec![], vec![]);
    assert!(model.validate_stmt(&statement, Some(&function)).is_err());
    function.ret_string = true;
    model.validate_stmt(&statement, Some(&function)).unwrap();
    let helper = IrPreFn::Branch {
        c_name: "helper".to_owned(),
        body: vec![statement],
    };
    assert!(model.validate_pre_fn(&helper, Some(&function)).is_err());
}

#[test]
fn string_calls_validate_packed_arguments_and_depth_context() {
    let mut model = valid_model();
    let mut function = IrFunc::new(
        "string_fn".to_owned(),
        None,
        vec![IrFormal::new(false, 128, false).unwrap()],
        vec![],
        vec![],
        vec![],
    );
    function.ret_string = true;
    model.funcs.push(function.clone());
    let statement = |width, depth| {
        IrStmt::Object(Box::new(IrObjectStmt::StringPrint(IrStringExpr::Call {
            receiver: None,
            virtual_dispatch: false,
            function: 0,
            args: vec![packed_const(1, width)],
            depth,
        })))
    };
    assert_eq!(
        model
            .statement_capacity(&statement(128, IrDepth::PROC), None)
            .unwrap(),
        128
    );
    assert!(model
        .validate_stmt(&statement(64, IrDepth::PROC), None)
        .is_err());
    assert!(model
        .validate_stmt(&statement(128, IrDepth::FUNC), None)
        .is_err());
    model
        .validate_stmt(&statement(128, IrDepth::FUNC), Some(&function))
        .unwrap();
}

#[test]
fn ir_invalid_cross_reference_rejects_out_of_bounds_tables() {
    let model = valid_model();

    // A container expression that names a container outside the model table.
    let container = IrExpr::new(
        IrExprKind::Container(Box::new(IrContainerExpr::Size(9))),
        32,
        true,
        None,
    );
    let error = model.validate_expr(&container, None).unwrap_err();
    assert!(
        error.detail().contains("index 9 is out of bounds"),
        "{error}"
    );

    // An object statement that names an object outside the model table.
    let error = model
        .validate_stmt(
            &IrStmt::Object(Box::new(IrObjectStmt::StringAssign(
                9,
                IrStringExpr::Literal(b"text".to_vec()),
            ))),
            None,
        )
        .unwrap_err();
    assert!(error.detail().contains("index 9"), "{error}");

    // A statement call that names a function outside the model table.
    let error = model
        .validate_stmt(
            &IrStmt::Call(Box::new(IrCall::new(
                9,
                Vec::new(),
                IrDepth::PROC,
                Vec::new(),
                Vec::new(),
            ))),
            None,
        )
        .unwrap_err();
    assert!(error.detail().contains("function index 9"), "{error}");

    // A sampled `$past` that names a history domain outside the model table.
    let past = IrExpr::new(
        IrExprKind::SysFunc(Box::new(IrSysFunc::Sampled(IrSampledCall::new(
            IrSampledFunc::Past,
            packed_const(1, 8),
            Some(9),
            1,
        )))),
        8,
        false,
        None,
    );
    let error = model.validate_expr(&past, None).unwrap_err();
    assert!(error.detail().contains("history metadata"), "{error}");

    // An array-element lvalue that names an array outside the model table.
    let error = model
        .validate_stmt(
            &IrStmt::Assign {
                lhs: IrLhs::ArrayElem {
                    arr: 9,
                    indices: vec![packed_const(0, 32)],
                    elem_sel: IrElemSel::Whole,
                },
                rhs: packed_const(1, 8),
                nba: false,
            },
            None,
        )
        .unwrap_err();
    assert!(error.detail().contains("array"), "{error}");
}

#[test]
fn activation_packed_selection_validates_each_step_and_visits_its_indices() {
    let model = valid_model();
    let mut lhs = IrLhs::PackedSelect {
        target: Box::new(IrLhs::WholeRef {
            addr: "&a0".to_owned(),
            width: 16,
            signed: false,
            two_state: false,
            shortreal: false,
        }),
        steps: vec![IrPackedSelect {
            base: packed_const(0, 129),
            width: 8,
        }],
        signed: false,
        two_state: false,
    };
    let statement = |lhs| IrStmt::Assign {
        lhs,
        rhs: packed_const(7, 8),
        nba: false,
    };
    assert_eq!(
        model
            .statement_capacity(&statement(lhs.clone()), None)
            .unwrap(),
        129
    );
    let mut visits = 0;
    lhs.expressions(&mut |_| visits += 1);
    assert_eq!(visits, 1);
    lhs.expressions_mut(&mut |expr| *expr = IrExpr::new(IrExprKind::SigRead(7), 1, false, None));
    assert!(model
        .validate_stmt(&statement(lhs), None)
        .unwrap_err()
        .detail()
        .contains("signal index 7"));
}

#[test]
fn activation_packed_selection_rejects_empty_plans_and_queued_local_writes() {
    let model = valid_model();
    let root = IrLhs::WholeRef {
        addr: "&a0".to_owned(),
        width: 16,
        signed: false,
        two_state: false,
        shortreal: false,
    };
    let empty = IrLhs::PackedSelect {
        target: Box::new(root.clone()),
        steps: vec![],
        signed: false,
        two_state: false,
    };
    assert!(model
        .validate_stmt(
            &IrStmt::Assign {
                lhs: empty,
                rhs: packed_const(0, 8),
                nba: false
            },
            None
        )
        .is_err());
    let selected = IrLhs::PackedSelect {
        target: Box::new(root),
        steps: vec![IrPackedSelect {
            base: packed_const(0, 32),
            width: 8,
        }],
        signed: false,
        two_state: false,
    };
    let error = model
        .validate_stmt(
            &IrStmt::Assign {
                lhs: selected,
                rhs: packed_const(1, 8),
                nba: true,
            },
            None,
        )
        .unwrap_err();
    assert!(
        error.detail().contains("persistent target storage"),
        "{error}"
    );
}

mod udp;

#[test]
fn nonflattened_fixed_operations_validate_shapes_and_activation_scopes() {
    let mut model = valid_model();
    model.arrays = vec![
        IrArray::new("source".into(), "source".into(), 8, false, vec![(0, 4096)]).unwrap(),
        IrArray::new("target".into(), "target".into(), 8, false, vec![(-5, 4091)]).unwrap(),
    ];
    let copy = IrStmt::FixedArrayCopy {
        dst: 1,
        src: 0,
        nba: false,
        slice: 0,
    };
    model.funcs.push(IrFunc::new(
        "copy".into(),
        None,
        vec![],
        vec![],
        vec![],
        vec![copy.clone()],
    ));
    model.validate().unwrap();
    model.arrays[1].dims = vec![(0, 16), (0, 240)];
    assert!(
        model.validate().is_err(),
        "equal cell count cannot erase rank"
    );
    model.arrays[1].dims = vec![(-5, 4091)];
    model.arrays[0].activation = true;
    assert!(
        model.validate().is_err(),
        "activation read before declaration"
    );
    model.funcs[0].body = vec![IrStmt::FixedArrayDeclare(0), copy.clone()];
    model.validate().unwrap();
    model.funcs[0].body = vec![
        IrStmt::Block(vec![IrStmt::FixedArrayDeclare(0)]),
        copy.clone(),
    ];
    assert!(model.validate().is_err(), "activation escaped its block");
    model.arrays[0].activation = false;
    model.funcs[0].body = vec![IrStmt::FixedArrayCopy {
        dst: 1,
        src: 0,
        nba: false,
        slice: 3,
    }];
    assert!(model.validate().is_err(), "unaligned descriptor stream");
    model.funcs[0].body = vec![IrStmt::FixedArrayFill {
        array: 1,
        value: packed_const(0, 7),
        nba: false,
    }];
    assert!(
        model.validate().is_err(),
        "fill width must match one element"
    );
}

#[test]
fn descriptor_value_assignment_requires_matching_descriptor_shape() {
    let mut model = valid_model();
    let rows = |c_name: &str, dims: Vec<(i32, i32)>, total: u64| IrArray {
        activation: false,
        descriptor: false,
        net: None,
        net_elements: Vec::new(),
        element_default: None,
        element_uninitialized: None,
        c_name: c_name.to_string(),
        hdl_name: c_name.to_string(),
        elem_width: 17,
        signed: false,
        two_state: false,
        real: false,
        shortreal: false,
        dims,
        total,
    };
    model
        .arrays
        .push(rows("pair", vec![(0, 1), (0, 65536)], 131_074));
    model.arrays.push(rows("row", vec![(0, 65536)], 65_537));
    model.arrays.push(rows("short", vec![(0, 65535)], 65_536));
    let whole = |array: usize, dims: Vec<(i32, i32)>, total: u64| IrMemoryView {
        array,
        origin: 0,
        selectors: Vec::new(),
        sliced: false,
        strides: vec![1; dims.len()],
        dims,
        total,
    };
    let selected_row = IrMemoryView {
        array: 0,
        origin: 0,
        selectors: vec![IrMemorySelector {
            dimension: 0,
            left: 0,
            right: 1,
            stride: 65_537,
            value: packed_const(1, 32),
        }],
        sliced: false,
        dims: vec![(0, 65536)],
        strides: vec![1],
        total: 65_537,
    };
    let assign = |dst: IrMemoryView, src: IrMemoryView| IrStmt::FixedValueAssign {
        dst,
        src: Box::new(IrFixedValue::Array(src)),
        nba: false,
    };
    model
        .validate_stmt(
            &assign(selected_row.clone(), whole(1, vec![(0, 65536)], 65_537)),
            None,
        )
        .expect("a whole row copies into a selected descriptor row");
    let error = model
        .validate_stmt(
            &assign(selected_row, whole(2, vec![(0, 65535)], 65_536)),
            None,
        )
        .expect_err("a shorter source must not copy into a selected row");
    assert!(error
        .detail()
        .contains("incompatible descriptor assignment"));
}

#[test]
fn descriptor_cast_requires_equal_size_lexical_shape() {
    let mut model = valid_model();
    let array = |c_name: &str, elem_width: u32, dims: Vec<(i32, i32)>, activation: bool| {
        let total = dims
            .iter()
            .map(|(left, right)| u64::from(left.abs_diff(*right)) + 1)
            .product();
        IrArray {
            activation,
            descriptor: false,
            net: None,
            net_elements: Vec::new(),
            element_default: None,
            element_uninitialized: None,
            c_name: c_name.to_string(),
            hdl_name: c_name.to_string(),
            elem_width,
            signed: false,
            two_state: false,
            real: false,
            shortreal: false,
            dims,
            total,
        }
    };
    model
        .arrays
        .push(array("source", 17, vec![(0, 65535)], false));
    model
        .arrays
        .push(array("pairs", 34, vec![(0, 32767)], true));
    model
        .arrays
        .push(array("short", 34, vec![(0, 32766)], true));
    model
        .arrays
        .push(array("storage", 34, vec![(0, 32767)], false));
    let source = IrMemoryView {
        array: 0,
        origin: 0,
        selectors: Vec::new(),
        sliced: false,
        dims: vec![(0, 65535)],
        strides: vec![1],
        total: 65_536,
    };
    let target = IrMemoryView {
        array: 3,
        origin: 0,
        selectors: Vec::new(),
        sliced: false,
        dims: vec![(0, 32767)],
        strides: vec![1],
        total: 32_768,
    };
    let assign = |shape: usize| IrStmt::FixedValueAssign {
        dst: target.clone(),
        src: Box::new(IrFixedValue::Convert {
            value: Box::new(IrFixedValue::Array(source.clone())),
            array: shape,
        }),
        nba: false,
    };
    model
        .validate_stmt(&assign(1), None)
        .expect("an equal-size lexical cast shape reshapes descriptor cells");
    for (shape, reason) in [
        (2, "a shorter cast shape"),
        (3, "non-lexical storage"),
        (9, "an unknown array"),
    ] {
        let error = model.validate_stmt(&assign(shape), None).expect_err(reason);
        assert!(error.detail().contains("fixed cast"), "{reason}: {error:?}");
    }
}

#[test]
fn rejects_invalid_index_default_with_wrong_width() {
    let mut model = valid_model();
    model.arrays.push(IrArray {
        activation: false,
        descriptor: false,
        net: None,
        net_elements: Vec::new(),
        element_default: None,
        element_uninitialized: Some(
            IrConst::packed(vec![0], vec![0b111], vec![0], 3, false, None).expect("constant"),
        ),
        c_name: "records".to_string(),
        hdl_name: "records".to_string(),
        elem_width: 6,
        signed: false,
        two_state: false,
        real: false,
        shortreal: false,
        dims: vec![(0, 2)],
        total: 3,
    });
    let error = model
        .validate()
        .expect_err("an invalid-index default must match the element width");
    assert!(error.detail().contains("element default"));
}

#[test]
fn record_columns_use_descriptor_storage_regardless_of_extent() {
    // A one-cell column of a column-layout record (RTL-101) is descriptor
    // storage only through its explicit flag; the same dense shape without
    // it keeps packed cells and cannot be a descriptor operand.
    let cell = |c_name: &str, descriptor: bool| IrArray {
        activation: false,
        descriptor,
        net: None,
        net_elements: Vec::new(),
        element_default: None,
        element_uninitialized: None,
        c_name: c_name.to_string(),
        hdl_name: c_name.to_string(),
        elem_width: 8,
        signed: false,
        two_state: false,
        real: false,
        shortreal: false,
        dims: vec![(0, 0)],
        total: 1,
    };
    let mut model = valid_model();
    model.arrays.push(cell("left", true));
    model.arrays.push(cell("right", true));
    model.arrays.push(cell("dense", false));
    assert!(model.arrays[0].sparse() && !model.arrays[2].sparse());
    let view = |array: usize| IrMemoryView {
        array,
        origin: 0,
        selectors: Vec::new(),
        sliced: false,
        dims: vec![(0, 0)],
        strides: vec![1],
        total: 1,
    };
    let assign = |dst: usize, src: usize| IrStmt::FixedValueAssign {
        dst: view(dst),
        src: Box::new(IrFixedValue::Array(view(src))),
        nba: false,
    };
    model
        .validate_stmt(&assign(0, 1), None)
        .expect("descriptor columns copy as descriptor values");
    for (dst, src) in [(0, 2), (2, 0)] {
        model
            .validate_stmt(&assign(dst, src), None)
            .expect_err("a dense array is not a descriptor operand");
    }
}

#[test]
fn force_dependencies_must_name_persistent_fixed_arrays() {
    let array = |activation| IrArray {
        activation,
        descriptor: false,
        net: None,
        net_elements: Vec::new(),
        element_default: None,
        element_uninitialized: None,
        c_name: "cells".to_string(),
        hdl_name: "cells".to_string(),
        elem_width: 1,
        signed: false,
        two_state: false,
        real: false,
        shortreal: false,
        dims: vec![(0, 3)],
        total: 4,
    };
    let model_with = |activation: bool, dependency: IrDependency| {
        let mut model = valid_model();
        model.arrays.push(array(activation));
        model.processes.push(IrProcess {
            c_name: "proc".to_string(),
            label: "top.initial".to_string(),
            kind: IrProcessKind::Synthetic,
            shape: IrShape::RunOnce,
            writes: Vec::new(),
            pre_fns: Vec::new(),
            body: vec![IrStmt::Force {
                lhs: IrLhs::Whole(0),
                value: Box::new(packed_const(1, 1)),
                eval: "eval".to_string(),
                reads: Vec::new(),
                dependencies: vec![dependency],
            }],
            program: None,
            origin: crate::sim::semantic::Origin::Synthetic {
                reason: "validation fixture".to_owned(),
            },
        });
        model.spawns.push("proc".to_string());
        model
    };
    model_with(false, IrDependency::ArrayContents(0))
        .validate()
        .expect("a persistent array marker drives the force");
    model_with(false, IrDependency::ArrayElement { array: 0, index: 3 })
        .validate()
        .expect("an element marker drives the force");
    for (activation, dependency) in [
        (true, IrDependency::ArrayContents(0)),
        (false, IrDependency::ArrayElement { array: 0, index: 4 }),
        (false, IrDependency::ArrayContents(1)),
        (false, IrDependency::Scalar("sig".to_string())),
    ] {
        let error = model_with(activation, dependency)
            .validate()
            .expect_err("force dependency must name a persistent array marker");
        assert_eq!(error.path(), "processes[0].body[0].dependencies[0]");
    }
}

#[test]
fn runtime_queries_name_existing_storage_with_their_fixed_shape() {
    let model_with = |query: IrRuntimeQuery, width: u32| {
        let mut model = valid_model();
        model
            .events
            .push(crate::sim::ir::IrEvent::new("ev".to_string()));
        model.processes.push(IrProcess {
            c_name: "proc".to_string(),
            label: "top.initial".to_string(),
            kind: IrProcessKind::Synthetic,
            shape: IrShape::RunOnce,
            writes: Vec::new(),
            pre_fns: Vec::new(),
            body: vec![IrStmt::DeclLocal {
                name: "probe".to_string(),
                width,
                signed: false,
                init: Some(Box::new(IrExpr::new(
                    IrExprKind::RuntimeQuery(query),
                    width,
                    false,
                    None,
                ))),
                two_state: true,
            }],
            program: None,
            origin: crate::sim::semantic::Origin::Synthetic {
                reason: "validation fixture".to_owned(),
            },
        });
        model.spawns.push("proc".to_string());
        model
    };
    model_with(IrRuntimeQuery::EventTriggerCount(0), 64)
        .validate()
        .expect("a static event's trigger count is 64 bits");
    model_with(IrRuntimeQuery::ForceSourceActive(0), 1)
        .validate()
        .expect("a force-source query is one bit");
    for (query, width) in [
        (IrRuntimeQuery::EventTriggerCount(0), 32),
        (IrRuntimeQuery::EventTriggerCount(1), 64),
        (IrRuntimeQuery::ForceSourceActive(0), 2),
        (IrRuntimeQuery::ForceSourceActive(1), 1),
    ] {
        model_with(query.clone(), width)
            .validate()
            .expect_err("a runtime query must name storage and keep its shape");
    }
}

#[test]
fn statement_sequences_admit_only_non_suspending_setup() {
    // A sequence expression (RTL-101b) declares lexical storage, copies and
    // calls, then yields its value; a statement that can suspend or that
    // queues an update is never part of an expression.
    let mut model = valid_model();
    model.arrays.push(IrArray {
        activation: true,
        descriptor: false,
        net: None,
        net_elements: Vec::new(),
        element_default: None,
        element_uninitialized: None,
        c_name: "column".to_string(),
        hdl_name: String::new(),
        elem_width: 1,
        signed: false,
        two_state: false,
        real: false,
        shortreal: false,
        dims: vec![(0, 0)],
        total: 1,
    });
    let read = IrExpr::new(
        IrExprKind::ArrayRead {
            arr: 0,
            indices: vec![packed_const(0, 32)],
            elem_sel: IrElemSel::Whole,
        },
        1,
        false,
        None,
    );
    let sequence = |statements: Vec<IrStmt>, value: IrExpr| {
        let (width, signed) = (value.width, value.signed);
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: IrExpr::new(
                IrExprKind::Sequence(Box::new(IrSequenceExpr { statements, value })),
                width,
                signed,
                None,
            ),
            nba: false,
        }
    };
    model
        .validate_stmt(
            &sequence(vec![IrStmt::FixedArrayDeclare(0)], read.clone()),
            None,
        )
        .expect("a declaration followed by a read is a sequence");
    model
        .validate_stmt(
            &sequence(
                vec![
                    IrStmt::FixedArrayDeclare(0),
                    IrStmt::Delay {
                        ticks: IrDelay::Constant(1),
                    },
                ],
                read.clone(),
            ),
            None,
        )
        .expect_err("a delay suspends");
    model
        .validate_stmt(
            &sequence(
                vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs: packed_const(1, 1),
                    nba: true,
                }],
                packed_const(1, 1),
            ),
            None,
        )
        .expect_err("a nonblocking assignment queues an update");
}
