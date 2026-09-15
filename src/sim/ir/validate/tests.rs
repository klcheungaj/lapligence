use super::*;

fn valid_model() -> IrModel {
    let mut model = IrModel::new("top".to_string(), 1).unwrap();
    model.signals = vec![IrSignal {
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
            IrExprKind::SysFunc(IrSysFunc::Math {
                kind: IrMathFunc::Pow,
                args,
            }),
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
        IrExprKind::SysFunc(IrSysFunc::Realtime {
            precision_fs: 1,
            unit_fs: 0,
        }),
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
fn indexed_lhs_selected_width_contributes_to_capacity() {
    let statement = IrStmt::Assign {
        lhs: IrLhs::IdxPart(
            0,
            packed_const(0, 32),
            packed_const(96, 32),
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
        lhs: IrLhs::IdxPart(0, base, packed_const(8, 32), 8, false, false),
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
            &IrStmt::Object(IrObjectStmt::StringRealtoa(0, real.clone())),
            None,
        )
        .unwrap();
    assert!(model
        .validate_stmt(
            &IrStmt::Object(IrObjectStmt::StringRealtoa(0, packed_const(1, 32))),
            None
        )
        .is_err());
    assert!(model
        .validate_stmt(
            &IrStmt::Object(IrObjectStmt::StringItoa(0, real.clone(), 10)),
            None
        )
        .is_err());
    model.objects[0].ty = IrObjectType::Chandle;
    assert!(model
        .validate_stmt(&IrStmt::Object(IrObjectStmt::StringRealtoa(0, real)), None)
        .is_err());
}

#[test]
fn string_return_storage_requires_its_function_context() {
    let model = valid_model();
    let value = IrStringExpr::LocalRead("_ret".to_owned());
    let statement = IrStmt::Object(IrObjectStmt::StringAssignLocal("_ret".to_owned(), value));
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
        IrStmt::Object(IrObjectStmt::StringPrint(IrStringExpr::Call {
            receiver: None,
            virtual_dispatch: false,
            function: 0,
            args: vec![packed_const(1, width)],
            depth,
        }))
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
