use super::*;

fn policy(model: &IrModel, statements: &[IrStmt]) -> super::super::frame_cells::CellEligibility {
    super::super::frame_cells::CellEligibility::analyze(
        &RCtx {
            value_backend: crate::sim::value_backend::ValueBackend::Legacy,
            model,
            func: None,
            sampled: false,
            activation_label: None,
        },
        statements,
    )
}

fn target(name: &str) -> IrLhs {
    IrLhs::WholeRef {
        addr: format!("&{name}"),
        width: 65,
        signed: false,
        two_state: false,
        shortreal: false,
    }
}

#[test]
fn nba_and_local_waits_exclude_only_their_cells() {
    let model = numeric_model();
    let statements = vec![
        IrStmt::Assign {
            lhs: target("nba"),
            rhs: number(42, 65),
            nba: true,
        },
        IrStmt::DelayedAssign {
            lhs: target("future"),
            rhs: number(42, 65),
            ticks: IrDelay::Constant(1),
        },
        IrStmt::WaitEvents {
            specs: vec![(IrWaitSrc::Sig("event".into()), IrEdge::Any)],
        },
        IrStmt::WaitAny {
            sens: vec![IrDependency::real("real_wait")],
        },
    ];
    let eligibility = policy(&model, &statements);
    for name in ["nba", "future", "event", "real_wait"] {
        assert!(!eligibility.permits(name));
    }
    assert!(eligibility.permits("private"));
}

#[test]
fn ref_and_output_call_actuals_keep_heap_identity() {
    let model = numeric_model();
    let statements = vec![IrStmt::Call(IrCall::new(
        0,
        vec![
            IrCallArg::RefAddr {
                addr: "&reference".into(),
                lhs: Box::new(target("reference")),
                read: Box::new(number(0, 65)),
                width: 65,
                signed: false,
                two_state: false,
                const_ref: false,
            },
            IrCallArg::OutAddr("&output".into()),
        ],
        IrDepth::PROC,
        Vec::new(),
        Vec::new(),
    ))];
    let eligibility = policy(&model, &statements);
    assert!(!eligibility.permits("reference"));
    assert!(!eligibility.permits("output"));
    assert!(eligibility.permits("private"));
}

#[test]
fn callbacks_captures_clocking_and_foreign_operations_fail_closed() {
    let model = numeric_model();
    let statements = [
        IrStmt::CapturedFork {
            join_kind: IrJoinKind::None,
            branches: Vec::new(),
            target: None,
        },
        IrStmt::MonitorSet {
            strobe: false,
            fmt: String::new(),
            eval: "eval".into(),
            n_args: 0,
            reads: Vec::new(),
            default_radix: IrDisplayRadix::Decimal,
            scope: "tb".into(),
            descriptor: None,
        },
        IrStmt::MonitorSet {
            strobe: true,
            fmt: String::new(),
            eval: "eval".into(),
            n_args: 0,
            reads: Vec::new(),
            default_radix: IrDisplayRadix::Decimal,
            scope: "tb".into(),
            descriptor: None,
        },
        IrStmt::ClockingCycleWait {
            count: number(1, 65),
            specs: Vec::new(),
        },
        IrStmt::ClockingDrive {
            lhs: target("clocking"),
            rhs: number(0, 65),
            ticks: IrDelay::Constant(0),
            specs: Vec::new(),
        },
        IrStmt::VpiCall {
            site: 0,
            name: "$foreign".into(),
            args: Vec::new(),
        },
        IrStmt::Force {
            lhs: target("forced"),
            value: number(0, 65),
            eval: "eval".into(),
            reads: Vec::new(),
        },
    ];
    for statement in statements {
        assert!(!policy(&model, &[statement]).permits("private"));
    }
}

#[test]
fn unknown_expression_cannot_silently_admit_a_cell() {
    let model = numeric_model();
    let statements = [IrStmt::Return {
        value: Some(Box::new(IrExpr::new(
            IrExprKind::Verbatim {
                code: "opaque".into(),
                width: 65,
                signed: false,
            },
            65,
            false,
            None,
        ))),
    }];
    assert!(!policy(&model, &statements).permits("private"));
}

#[test]
fn even_pure_dpi_and_spawning_callees_are_unproven() {
    let mut model = numeric_model();
    let call = IrStmt::Call(IrCall::new(
        0,
        vec![IrCallArg::Val(number(1, 65))],
        IrDepth::PROC,
        Vec::new(),
        Vec::new(),
    ));
    assert!(policy(&model, std::slice::from_ref(&call)).permits("private"));
    model.funcs[0].dpi = Some(IrDpiImport {
        c_name: "foreign".into(),
        context: false,
        pure: true,
    });
    assert!(!policy(&model, std::slice::from_ref(&call)).permits("private"));
    model.funcs[0].dpi = None;
    model.funcs[0].body.push(IrStmt::Fork {
        join_kind: IrJoinKind::None,
        branches: Vec::new(),
        target: None,
    });
    assert!(!policy(&model, &[call]).permits("private"));
}

#[test]
fn context_free_wait_evaluators_exclude_their_typed_reads_only() {
    let mut model = numeric_model();
    model.processes[0].pre_fns.push(IrPreFn::MonEval {
        c_name: "wait_eval".into(),
        args: vec![IrExpr::new(
            IrExprKind::LocalRead("observed".into()),
            65,
            false,
            None,
        )],
        context: None,
        item: false,
    });
    let statement = IrStmt::WaitEvents {
        specs: vec![(
            IrWaitSrc::Evaluated {
                eval: "wait_eval".into(),
                condition: None,
                reads: Vec::new(),
            },
            IrEdge::Any,
        )],
    };
    let eligibility = policy(&model, std::slice::from_ref(&statement));
    assert!(!eligibility.permits("observed"));
    assert!(eligibility.permits("private"));
    if let IrPreFn::MonEval { context, .. } = model.processes[0].pre_fns.last_mut().unwrap() {
        *context = Some(IrEventContext::new(FrameId::new(0), Vec::new()));
    }
    assert!(!policy(&model, &[statement]).permits("private"));
}
