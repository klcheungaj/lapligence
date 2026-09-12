use super::*;
use crate::sim::ir::{IrEventRef, IrExpr, IrExprKind, IrModel, IrPreFn, IrStmt};

fn render(model: &IrModel) -> Result<String, EmitError> {
    let execution = crate::sim::execution::ExecutionModel::lower(model.clone())
        .map_err(EmitError::InvalidIr)?;
    super::render(&execution)
}

#[test]
fn executable_sensitivity_blocks_drive_process_emission() {
    use crate::sim::ir::{IrModelParts, IrProcess, IrShape, IrSignal, IrType};

    let process = IrProcess::new(
        "p_comb".into(),
        "top.comb".into(),
        IrShape::SensLoop {
            reads: vec!["signal_a".into()],
        },
        vec![],
        vec![IrStmt::Finish],
    );
    let ir = IrModel::from_parts(
        "top".into(),
        1,
        IrModelParts {
            signals: vec![IrSignal::new(
                "signal_a".into(),
                Some("top.signal_a".into()),
                IrType::packed(1, false).unwrap(),
                None,
            )
            .unwrap()],
            processes: vec![process],
            spawns: vec!["p_comb".into()],
            ..IrModelParts::default()
        },
    )
    .unwrap();
    let execution = crate::sim::execution::ExecutionModel::lower(ir).unwrap();

    let rendered = super::render(&execution).unwrap();

    assert_eq!(rendered.matches("llg_rt_finish();").count(), 1);
    let loop_start = rendered.find("for (;;) {").unwrap();
    let wait = rendered.find("llg_wait_any").unwrap();
    let finish = rendered.find("llg_rt_finish();").unwrap();
    assert!(loop_start < finish && finish < wait);
}

#[test]
fn executable_loop_blocks_have_cooperative_budget_points() {
    use crate::sim::ir::{IrModelParts, IrProcess, IrShape};

    let process = IrProcess::new(
        "p_loop".into(),
        "top.loop".into(),
        IrShape::Loop,
        vec![],
        vec![IrStmt::Nop],
    );
    let model = IrModel::from_parts(
        "top".into(),
        1,
        IrModelParts {
            processes: vec![process],
            spawns: vec!["p_loop".into()],
            ..IrModelParts::default()
        },
    )
    .unwrap();

    let rendered = render(&model).unwrap();

    assert!(rendered
        .contains("llg_budget_point(\"<synthetic: manually constructed process top.loop>\");"));
}

#[test]
fn nonblocking_event_trigger_renders_nba_operation() {
    use crate::sim::ir::{IrEvent, IrModelParts, IrProcess, IrShape};

    let process = IrProcess::new(
        "p_event".into(),
        "top.initial".into(),
        IrShape::RunOnce,
        vec![],
        vec![IrStmt::NonblockingEventTrigger {
            ev: IrEventRef::Static(0),
            ticks: Some(crate::sim::ir::IrDelay::Constant(2)),
        }],
    );
    let model = IrModel::from_parts(
        "top".into(),
        1,
        IrModelParts {
            events: vec![IrEvent::new("top__event".into())],
            processes: vec![process],
            spawns: vec!["p_event".into()],
            ..IrModelParts::default()
        },
    )
    .unwrap();

    let rendered = render(&model).unwrap();
    assert!(rendered.contains("llg_nba_event_after(&top__event, 2ULL);"));
}

#[test]
fn runtime_width_limit_is_a_backend_policy_not_an_ir_invariant() {
    let model = IrModel::new("wide".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        activation_label: None,
    };
    for width in [LLG_WIDTH_LIMIT, LLG_WIDTH_LIMIT + 1, u32::MAX] {
        let ty = crate::sim::ir::IrType::packed(width, false).unwrap();
        let wide_model = IrModel::from_parts(
            "wide_storage".to_owned(),
            1,
            crate::sim::ir::IrModelParts {
                signals: vec![crate::sim::ir::IrSignal::new(
                    "wide_signal".to_owned(),
                    None,
                    ty,
                    None,
                )
                .unwrap()],
                ..Default::default()
            },
        )
        .unwrap();
        wide_model.validate().unwrap();
        assert!(render(&wide_model)
            .unwrap_err()
            .to_string()
            .contains("limit"));
        let expression = IrExpr::try_new(IrExprKind::Fill(0), width, false, None).unwrap();
        model.validate_expr(&expression, None).unwrap();
        assert!(render_expr(&ctx, &expression)
            .err()
            .unwrap()
            .to_string()
            .contains("limit"));
        let statement = IrStmt::If {
            cond: expression.clone(),
            then_: Vec::new(),
            els: None,
            check: crate::sim::ir::IrUniquePriorityCheck::None,
        };
        model.validate_stmt(&statement, None).unwrap();
        assert!(render_stmt(&ctx, &statement)
            .err()
            .unwrap()
            .to_string()
            .contains("limit"));
        let helper = IrPreFn::MonEval {
            c_name: "probe".into(),
            args: vec![expression],
            context: None,
            item: false,
        };
        model.validate_pre_fn(&helper, None).unwrap();
        assert!(render_pre_fn(&ctx, &helper)
            .err()
            .unwrap()
            .to_string()
            .contains("limit"));
    }
}

#[test]
fn malformed_concat_cannot_hide_its_derived_width() {
    let model = IrModel::new("wide".to_owned(), 1).unwrap();
    let expression = IrExpr::try_new(
        IrExprKind::Replicate {
            count: u64::from(LLG_WIDTH_LIMIT),
            parts: vec![IrExpr::try_new(IrExprKind::Fill(0), 1, false, None).unwrap()],
        },
        1,
        false,
        None,
    )
    .unwrap();
    assert!(model
        .validate_expr(&expression, None)
        .unwrap_err()
        .to_string()
        .contains("derived width"));
}

#[test]
fn indexed_read_uses_its_elaborated_extent() {
    let model = IrModel::new("indexed_read".to_owned(), 1).unwrap();
    let packed = |value, width| {
        IrExpr::try_new(
            IrExprKind::Const(
                crate::sim::ir::IrConst::packed(
                    vec![value],
                    Vec::new(),
                    Vec::new(),
                    width,
                    false,
                    None,
                )
                .unwrap(),
            ),
            width,
            false,
            None,
        )
        .unwrap()
    };
    let expression = IrExpr::try_new(
        IrExprKind::IdxPartSel {
            base: Box::new(packed(1, 8)),
            base_idx: Box::new(packed(0, 32)),
            width_expr: Box::new(packed(96, 32)),
            neg: false,
        },
        96,
        false,
        None,
    )
    .unwrap();
    assert_eq!(model.expression_capacity(&expression, None).unwrap(), 96);
    let rendered = render_expr(
        &RCtx {
            model: &model,
            func: None,
            activation_label: None,
        },
        &expression,
    )
    .unwrap();
    assert!(rendered.code.ends_with(", 96, 0)"));
    assert!(!rendered.code.contains("sv4_checked_width"));
}

#[test]
fn selected_net_driver_preserves_member_state_conversion() {
    use crate::sim::ir::{IrLhs, IrModelParts, IrNetGroup, IrNetKind, IrSignal, IrType};
    let model = IrModel::from_parts(
        "member_driver".to_owned(),
        1,
        IrModelParts {
            signals: vec![IrSignal::new(
                "net.resolved".to_owned(),
                None,
                IrType::packed(8, false).unwrap(),
                Some((0, 0)),
            )
            .unwrap()],
            net_groups: vec![
                IrNetGroup::new("net".to_owned(), 8, false, IrNetKind::Wire, 1).unwrap(),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    let statement = IrStmt::Assign {
        lhs: IrLhs::Part(0, 7, 4, true),
        rhs: IrExpr::try_new(IrExprKind::Fill(2), 4, false, Some(2)).unwrap(),
        nba: false,
    };
    let rendered = render_stmt(
        &RCtx {
            model: &model,
            func: None,
            activation_label: None,
        },
        &statement,
    )
    .unwrap();
    assert!(rendered.contains("sv4_to_two_state(sv4_fill(2, 4, 0))"));
    assert!(rendered.contains("llg_net_write(&net, 0, _t)"));
}

#[test]
fn detached_fragments_reject_missing_storage_before_rendering() {
    let model = IrModel::new("empty".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        activation_label: None,
    };
    let expression = IrExpr::try_new(IrExprKind::SigRead(7), 1, false, None).unwrap();
    let error = match render_expr(&ctx, &expression) {
        Ok(_) => panic!("missing signal was rendered"),
        Err(error) => error,
    };
    assert!(matches!(error, EmitError::InvalidIr(_)));
    assert!(std::error::Error::source(&error).is_some());

    let statement = IrStmt::If {
        cond: expression.clone(),
        then_: Vec::new(),
        els: None,
        check: crate::sim::ir::IrUniquePriorityCheck::None,
    };
    assert!(matches!(
        render_stmt(&ctx, &statement),
        Err(EmitError::InvalidIr(_))
    ));
    let helper = IrPreFn::MonEval {
        c_name: "monitor".to_owned(),
        args: vec![expression],
        context: None,
        item: false,
    };
    assert!(matches!(
        render_pre_fn(&ctx, &helper),
        Err(EmitError::InvalidIr(_))
    ));
}

#[test]
fn model_render_rejects_invalid_internal_precision() {
    let mut model = IrModel::new("empty".to_owned(), 1).unwrap();
    model.precision_fs = 0;
    assert!(matches!(render(&model), Err(EmitError::InvalidIr(_))));
}

#[test]
fn output_temporary_uses_its_declared_formal_after_c_argument_reordering() {
    use crate::sim::ir::{
        IrCallArg, IrCallExpr, IrConst, IrDepth, IrFormal, IrFunc, IrLhs, IrModelParts, IrType,
    };
    let function = IrFunc::new(
        "mixed".to_owned(),
        Some(IrType::Packed {
            width: 1,
            signed: false,
            two_state: false,
        }),
        vec![
            IrFormal::new(false, 8, false).unwrap(),
            IrFormal::new(true, 16, true).unwrap(),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let model = IrModel::from_parts(
        "mixed".to_owned(),
        1,
        IrModelParts {
            funcs: vec![function],
            ..Default::default()
        },
    )
    .unwrap();
    let input = IrExpr::try_new(
        IrExprKind::Const(
            IrConst::packed(vec![1], Vec::new(), Vec::new(), 8, false, None).unwrap(),
        ),
        8,
        false,
        None,
    )
    .unwrap();
    let call = IrCallExpr::new(
        0,
        vec![
            IrCallArg::OutTemp {
                name: "_out".to_owned(),
                init: None,
                writeback: Box::new(IrLhs::WholeRef {
                    addr: "&target".to_owned(),
                    width: 16,
                    signed: true,
                    two_state: false,
                    shortreal: false,
                }),
                storage_addr: None,
                storage_lhs: None,
                storage_read: None,
                selector_inits: Vec::new(),
            },
            IrCallArg::Val(input),
        ],
        IrDepth::PROC,
        false,
    );
    let expression = IrExpr::try_new(IrExprKind::CallFn(Box::new(call)), 1, false, None).unwrap();
    let rendered = render_expr(
        &RCtx {
            model: &model,
            func: None,
            activation_label: None,
        },
        &expression,
    )
    .unwrap();
    assert!(
        rendered.code.contains("sv4_t _out = sv4_x(16, 1)"),
        "{}",
        rendered.code
    );
}

#[test]
fn captured_fork_emits_owned_frame_lifecycle() {
    use crate::sim::ir::{
        FrameId, IrCapture, IrCapturedBranch, IrModelParts, IrProcess, IrShape, StorageLifetime,
        StorageOwnership, StorageRef,
    };

    let frame = FrameId::new(2);
    let storage = StorageRef::for_declaration(
        frame,
        0,
        17,
        StorageLifetime::Automatic,
        StorageOwnership::Owned,
    );
    let value = IrExpr::try_new(
        IrExprKind::Const(
            crate::sim::ir::IrConst::packed(vec![0x5a], vec![0], vec![0], 8, false, None).unwrap(),
        ),
        8,
        false,
        None,
    )
    .unwrap();
    let capture = IrCapture::new(storage, value);
    let branch = IrCapturedBranch::new(
        "p_capture_branch".into(),
        "top.capture".into(),
        frame,
        vec![capture.clone()],
    );
    let process = IrProcess::new(
        "p_host".into(),
        "top.host".into(),
        IrShape::RunOnce,
        vec![IrPreFn::CapturedBranch {
            c_name: "p_capture_branch".into(),
            frame,
            captures: vec![capture],
            body: vec![IrStmt::Nop],
        }],
        vec![IrStmt::CapturedFork {
            join_kind: crate::sim::ir::IrJoinKind::None,
            branches: vec![branch],
            target: None,
        }],
    );
    let model = IrModel::from_parts(
        "top".into(),
        1,
        IrModelParts {
            processes: vec![process],
            spawns: vec!["p_host".into()],
            ..IrModelParts::default()
        },
    )
    .unwrap();

    let rendered = render(&model).unwrap();
    assert!(rendered.contains("llg_frame_new(1)"));
    assert!(rendered.contains("llg_frame_capture_value(_frame_2, 0u"));
    assert!(rendered.contains("llg_fork_with_frame(p_capture_branch"));
    assert!(rendered.contains("llg_frame_read_value(llg_proc_frame(self), 0u)"));
    assert!(rendered.contains("llg_frame_release(_frame_2)"));
}

#[test]
fn evaluated_event_emits_owned_context_and_contextual_callback() {
    use crate::sim::ir::{
        FrameId, IrDependency, IrEventCapture, IrEventContext, IrModelParts, IrProcess, IrShape,
        StorageLifetime, StorageOwnership, StorageRef,
    };

    let frame = FrameId::new(4);
    let storage = StorageRef::for_declaration(
        frame,
        0,
        23,
        StorageLifetime::Automatic,
        StorageOwnership::Owned,
    );
    let callback_value =
        IrExpr::try_new(IrExprKind::LocalRead("_local".into()), 1, false, None).unwrap();
    let initial = IrExpr::try_new(IrExprKind::Fill(0), 1, false, None).unwrap();
    let context = IrEventContext::new(
        frame,
        vec![IrEventCapture::new(storage, "_local".into(), initial)],
    );
    let process = IrProcess::new(
        "p_event".into(),
        "top.event".into(),
        IrShape::RunOnce,
        vec![IrPreFn::MonEval {
            c_name: "p_eval".into(),
            args: vec![callback_value],
            context: Some(context),
            item: false,
        }],
        vec![IrStmt::WaitEvents {
            specs: vec![(
                crate::sim::ir::IrWaitSrc::Evaluated {
                    eval: "p_eval".into(),
                    condition: None,
                    reads: vec![IrDependency::Scalar("signal".into())],
                },
                crate::sim::ir::IrEdge::Any,
            )],
        }],
    );
    let model = IrModel::from_parts(
        "top".into(),
        1,
        IrModelParts {
            signals: vec![crate::sim::ir::IrSignal::new(
                "signal".into(),
                None,
                crate::sim::ir::IrType::packed(1, false).unwrap(),
                None,
            )
            .unwrap()],
            processes: vec![process],
            spawns: vec!["p_event".into()],
            ..IrModelParts::default()
        },
    )
    .unwrap();

    let rendered = render(&model).unwrap();
    assert!(rendered.contains("static void p_eval(sv4_t* out, void* context)"));
    assert!(rendered.contains("out[0] = llg_frame_read_value((const llg_frame_t*)context, 0u);"));
    assert!(rendered.contains("llg_frame_t* _event_frame_4 = llg_frame_new(1u);"));
    assert!(rendered.contains(".eval_context = _event_frame_4"));
    assert!(rendered.contains("llg_wait_expressions(_events, 1);"));
}
