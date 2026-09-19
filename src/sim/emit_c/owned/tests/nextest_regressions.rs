//! Regression models exercise the production emitter, not handwritten C shapes.
use super::*;

fn display_value() -> IrStmt {
    IrStmt::Display {
        fmt: "\"%0d\"".to_owned(),
        args: vec![(IrExpr::new(IrExprKind::SigRead(0), 65, false, None), false)],
        newline: true,
        default_radix: IrDisplayRadix::Decimal,
    }
}

fn run_model(model: IrModel, name: &str, expected: &str) -> String {
    let execution = ExecutionModel::lower(model).expect("validate regression execution model");
    let source = super::super::super::model::render(&execution).expect("emit regression model");
    if !crate::sim::build::cmake_available() {
        eprintln!("SKIP: CMake is unavailable; emission was checked but native execution was not");
        return source;
    }
    let directory = toolchain::Directory::new(name);
    let binary = crate::sim::build::build_model_cmake(directory.path(), &[("model.c", &source)])
        .expect("compile emitted regression model");
    let result = toolchain::execute(&binary);
    assert!(
        result.status.success(),
        "{}: {}",
        name,
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout), expected, "{name}");
    source
}

#[test]
fn disabled_timed_call_does_not_copy_out_its_staged_value() {
    let mut model = numeric_model();
    model.funcs.clear();
    let target = IrActivationTarget::new(100, 1);
    let output = IrLhs::WholeRef {
        addr: "o0".to_owned(),
        width: 65,
        signed: false,
        two_state: false,
        shortreal: false,
    };
    model.funcs.push(IrFunc::new(
        "f_cancelled".to_owned(),
        None,
        vec![IrFormal::new(true, 65, false).unwrap()],
        vec![],
        vec![],
        vec![IrStmt::ActivationScope {
            target,
            exit: "task_exit".to_owned(),
            body: vec![
                IrStmt::Assign {
                    lhs: output.clone(),
                    rhs: number(42, 65),
                    nba: false,
                },
                IrStmt::Delay {
                    ticks: IrDelay::Constant(2),
                },
                IrStmt::Assign {
                    lhs: output,
                    rhs: number(99, 65),
                    nba: false,
                },
            ],
        }],
    ));
    model.processes[0].body = vec![
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(7, 65),
            nba: false,
        },
        IrStmt::ActivationScope {
            target,
            exit: "call_exit".to_owned(),
            body: vec![IrStmt::Call(IrCall::new(
                0,
                vec![IrCallArg::OutAddr("&result".to_owned())],
                IrDepth::PROC,
                vec![("result".to_owned(), 0, None)],
                vec![(IrLhs::Whole(0), "result".to_owned(), 65, false)],
            ))],
        },
        display_value(),
    ];
    model.processes.push(IrProcess::new(
        "p_disable".to_owned(),
        "disable".to_owned(),
        IrShape::RunOnce,
        vec![],
        vec![
            IrStmt::Delay {
                ticks: IrDelay::Constant(1),
            },
            IrStmt::DisableTarget { target },
        ],
    ));
    model.spawns.push("p_disable".to_owned());
    let source = run_model(model, "cancelled-copyout", "7\n");
    assert!(source.contains("llg_activation_enter(100u, 1u)"));
    assert!(source.contains("llg_activation_cancelled()"));
}

#[test]
fn forward_jump_exits_inner_activation_but_preserves_outer_locals() {
    let mut model = numeric_model();
    let outer = IrActivationTarget::new(101, 1);
    let inner = IrActivationTarget::new(102, 1);
    model.processes[0].body = vec![IrStmt::ActivationScope {
        target: outer,
        exit: "outer_exit".to_owned(),
        body: vec![
            IrStmt::DeclLocal {
                name: "kept".to_owned(),
                width: 65,
                signed: false,
                two_state: false,
                init: Some(Box::new(number(23, 65))),
            },
            IrStmt::ActivationScope {
                target: inner,
                exit: "inner_exit".to_owned(),
                body: vec![
                    IrStmt::DeclLocal {
                        name: "discarded".to_owned(),
                        width: 65537,
                        signed: false,
                        two_state: false,
                        init: Some(Box::new(number(1, 65537))),
                    },
                    IrStmt::Goto("escaped".to_owned()),
                ],
            },
            IrStmt::Label("escaped".to_owned()),
            IrStmt::DisableTarget { target: inner },
            IrStmt::Assign {
                lhs: IrLhs::Whole(0),
                nba: false,
                rhs: IrExpr::new(IrExprKind::LocalRead("kept".to_owned()), 65, false, None),
            },
            display_value(),
        ],
    }];
    run_model(model, "activation-jump", "23\n");
}

#[test]
fn inertial_statement_compiles_and_commits_after_its_delay() {
    let mut model = numeric_model();
    model.processes[0].body = vec![
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(0, 65),
            nba: false,
        },
        IrStmt::InertialAssign {
            lhs: IrLhs::Whole(0),
            rhs: number(42, 65),
            delay: IrTransitionDelay {
                rise: 2,
                fall: 2,
                turn_off: 2,
            },
        },
        IrStmt::Delay {
            ticks: IrDelay::Constant(1),
        },
        display_value(),
        IrStmt::Delay {
            ticks: IrDelay::Constant(2),
        },
        display_value(),
    ];
    run_model(model, "inertial-owner", "0\n42\n");
}

#[test]
fn typed_strobe_callback_observes_the_nba_value() {
    let mut model = numeric_model();
    model.processes[0].pre_fns.push(IrPreFn::DisplayEval {
        c_name: "print_current".to_owned(),
        time_unit_fs: 1,
        args: vec![IrDisplayArg::Packed(IrExpr::new(
            IrExprKind::SigRead(0),
            65,
            false,
            None,
        ))],
    });
    model.processes[0].body = vec![
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(0, 65),
            nba: false,
        },
        IrStmt::MonitorSet {
            strobe: true,
            fmt: "\"%0d\"".to_owned(),
            eval: "print_current".to_owned(),
            n_args: 1,
            reads: vec![],
            default_radix: IrDisplayRadix::Decimal,
            scope: "regression".to_owned(),
            descriptor: None,
        },
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(42, 65),
            nba: true,
        },
        IrStmt::Delay {
            ticks: IrDelay::Constant(1),
        },
    ];
    run_model(model, "strobe-owner", "42\n");
}

#[test]
fn force_evaluator_owns_its_result_and_release_keeps_variable_value() {
    let mut model = numeric_model();
    model.processes[0].pre_fns.push(IrPreFn::ForceEval {
        c_name: "force_value".to_owned(),
        value: number(42, 65),
        real: false,
    });
    model.processes[0].body = vec![
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(7, 65),
            nba: false,
        },
        IrStmt::Force {
            lhs: IrLhs::Whole(0),
            value: number(42, 65),
            eval: "force_value".to_owned(),
            reads: vec![],
        },
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(99, 65),
            nba: false,
        },
        display_value(),
        IrStmt::Release {
            lhs: IrLhs::Whole(0),
        },
        display_value(),
    ];
    run_model(model, "force-owner", "42\n42\n");
}

#[test]
fn loop_budget_is_not_charged_for_the_final_false_condition() {
    let model = numeric_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame
        .statement(&IrStmt::While {
            cond: number(0, 1),
            body: vec![],
        })
        .unwrap();
    let source = frame.body();
    assert!(source.find("break;").unwrap() < source.find("llg_budget_point(").unwrap());
}

#[test]
fn wide_nba_publication_snapshot_survives_source_release() {
    let mut model = numeric_model();
    model.funcs.clear();
    let wide = 65537u32;
    model.signals.push(
        IrSignal::new(
            "G_captured".to_owned(),
            None,
            IrType::Packed {
                width: wide,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    // The block-local owner is released before the NBA commits; the committed
    // signal must retain an independent clone of the widened source.
    let widened = IrExpr::new(
        IrExprKind::Resize {
            a: Box::new(IrExpr::new(IrExprKind::SigRead(0), 65, false, None)),
        },
        wide,
        false,
        None,
    );
    model.processes[0].body = vec![
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(0x1234, 65),
            nba: false,
        },
        IrStmt::Block(vec![
            IrStmt::DeclLocal {
                name: "wide".to_owned(),
                width: wide,
                signed: false,
                two_state: false,
                init: Some(Box::new(widened)),
            },
            IrStmt::Assign {
                lhs: IrLhs::Whole(1),
                rhs: IrExpr::new(IrExprKind::LocalRead("wide".to_owned()), wide, false, None),
                nba: true,
            },
        ]),
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(0, 65),
            nba: false,
        },
        IrStmt::Delay {
            ticks: IrDelay::Constant(1),
        },
        IrStmt::Display {
            fmt: "\"%0d\"".to_owned(),
            args: vec![(
                IrExpr::new(IrExprKind::SigRead(1), wide, false, None),
                false,
            )],
            newline: true,
            default_radix: IrDisplayRadix::Decimal,
        },
        display_value(),
    ];
    run_model(model, "wide-nba-snapshot", "4660\n0\n");
}

#[test]
fn disabled_wide_local_scope_unwinds_before_exit() {
    let mut model = numeric_model();
    model.funcs.clear();
    let target = IrActivationTarget::new(200, 1);
    model.processes[0].body = vec![
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(7, 65),
            nba: false,
        },
        IrStmt::ActivationScope {
            target,
            exit: "wide_exit".to_owned(),
            body: vec![
                IrStmt::DeclLocal {
                    name: "wide".to_owned(),
                    width: 65537,
                    signed: false,
                    two_state: false,
                    init: Some(Box::new(number(1, 65537))),
                },
                IrStmt::Delay {
                    ticks: IrDelay::Constant(5),
                },
                IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs: number(99, 65),
                    nba: false,
                },
            ],
        },
        display_value(),
    ];
    model.processes.push(IrProcess::new(
        "p_disable".to_owned(),
        "disable".to_owned(),
        IrShape::RunOnce,
        vec![],
        vec![
            IrStmt::Delay {
                ticks: IrDelay::Constant(1),
            },
            IrStmt::DisableTarget { target },
        ],
    ));
    model.spawns.push("p_disable".to_owned());
    let source = run_model(model, "disabled-wide-local", "7\n");
    assert!(
        source.contains("llg_activation_enter(200u, 1u)"),
        "wide temporary must run inside a registered activation scope"
    );
    assert!(
        source.contains("wide_exit"),
        "cancellation must leave through the scope unwind label"
    );
}
