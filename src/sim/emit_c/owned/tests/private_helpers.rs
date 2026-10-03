//! Private composite writes stay within the callback's owned temporary frame.
use super::*;

fn local(name: &str) -> IrLhs {
    IrLhs::WholeRef {
        addr: format!("&{name}"),
        width: 65,
        signed: false,
        two_state: false,
        shortreal: false,
    }
}

fn model() -> IrModel {
    let mut model = IrModel::new("private_helpers".into(), 1).unwrap();
    let ty = IrType::Packed {
        width: 65,
        signed: false,
        two_state: false,
    };
    model.signals.push(
        IrSignal::new(
            "G_input".into(),
            None,
            IrType::Packed {
                width: 130,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    let body = vec![
        IrStmt::DeclLocal {
            name: "high_value".into(),
            width: 65,
            signed: false,
            two_state: false,
            init: None,
        },
        IrStmt::DeclLocal {
            name: "low_value".into(),
            width: 65,
            signed: false,
            two_state: false,
            init: None,
        },
        IrStmt::Assign {
            lhs: IrLhs::Stream {
                parts: vec![(local("high_value"), 65), (local("low_value"), 65)],
                width: 130,
                slice: 1,
                direction: IrStreamDirection::LeftToRight,
            },
            rhs: IrExpr::new(IrExprKind::FormalRead(0), 130, false, None),
            nba: false,
        },
        IrStmt::If {
            cond: IrExpr::new(IrExprKind::LocalRead("low_value".into()), 65, false, None),
            then_: vec![IrStmt::Return {
                value: Some(Box::new(IrExpr::new(
                    IrExprKind::LocalRead("low_value".into()),
                    65,
                    false,
                    None,
                ))),
            }],
            els: Some(vec![IrStmt::Return {
                value: Some(Box::new(number(0, 65))),
            }]),
            check: IrUniquePriorityCheck::Unique(crate::sim::semantic::Origin::Synthetic {
                reason: "private helper branch".into(),
            }),
        },
    ];
    let mut function = IrFunc::new(
        "f_private".into(),
        Some(ty),
        vec![IrFormal::new(false, 130, false).unwrap()],
        Vec::new(),
        Vec::new(),
        body,
    );
    function.automatic = true;
    model.funcs.push(function);
    model
}

fn call() -> IrExpr {
    IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(IrExpr::new(
                IrExprKind::SigRead(0),
                130,
                false,
                None,
            ))],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    )
}

#[test]
fn callback_composite_writes_release_owners_without_publication() {
    let model = model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.read_only_callback = true;
    let value = frame.expression(&call()).unwrap();
    frame.discard(value);
    let source = frame.body();
    assert!(source.contains("sv4_part_select("));
    assert!(
        source.contains("llg_unique_priority_check("),
        "do not erase qualified diagnostics"
    );
    assert!(!source.contains("llg_ba("));
    assert!(!source.contains("llg_net_write("));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn callback_composite_cannot_hide_a_visible_or_reference_target() {
    for external in [
        IrLhs::Whole(0),
        IrLhs::Ref {
            addr: "r0".into(),
            width: 65,
            signed: false,
            two_state: false,
            const_ref: false,
            bit: None,
        },
    ] {
        let target = IrLhs::Stream {
            parts: vec![(local("private"), 65), (external, 65)],
            width: 130,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        };
        assert!(!super::super::pure_calls::private_callback_target(&target));
    }
    assert!(!super::super::pure_calls::private_callback_target(
        &IrLhs::Stream {
            parts: Vec::new(),
            width: 0,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        }
    ));
}

#[test]
fn generated_private_composite_callback_preserves_event_values_and_cleanup() {
    assert!(crate::sim::build::cmake_available(), "CMake is required");
    for optimized in [false, true] {
        let mut model = model();
        let wait = IrStmt::WaitEvents {
            specs: vec![(
                IrWaitSrc::Evaluated {
                    eval: "evaluate_private".into(),
                    condition: None,
                    reads: vec![IrDependency::Scalar("G_input".into())],
                },
                IrEdge::Any,
            )],
        };
        let display = IrStmt::Display {
            fmt: "\"%0d\"".into(),
            args: vec![(call(), false)],
            newline: true,
            default_radix: IrDisplayRadix::Decimal,
        };
        model.processes.push(IrProcess::new(
            "p_observe".into(),
            "observe".into(),
            IrShape::RunOnce,
            vec![IrPreFn::MonEval {
                c_name: "evaluate_private".into(),
                args: vec![call()],
                context: None,
                item: false,
            }],
            vec![wait.clone(), display.clone(), wait, display],
        ));
        model.processes.push(IrProcess::new(
            "p_stimulus".into(),
            "stimulus".into(),
            IrShape::RunOnce,
            Vec::new(),
            vec![
                IrStmt::Delay {
                    ticks: IrDelay::Constant(1),
                },
                IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs: number(7, 130),
                    nba: false,
                },
                IrStmt::Delay {
                    ticks: IrDelay::Constant(1),
                },
                IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs: number(9, 130),
                    nba: false,
                },
            ],
        ));
        model.spawns = vec!["p_observe".into(), "p_stimulus".into()];
        let mut execution = ExecutionModel::lower(model).unwrap();
        let options = if optimized {
            crate::sim::opt::OptConfig::default()
        } else {
            crate::sim::opt::OptConfig::none()
        };
        crate::sim::opt::run(&mut execution, &options).unwrap();
        let source = super::super::super::model::render(&execution).unwrap();
        let directory = toolchain::Directory::new("private-composite");
        let binary =
            crate::sim::build::build_model_cmake(directory.path(), &[("model.c", &source)])
                .unwrap();
        let output = toolchain::execute(&binary);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "7\n9\n");
        toolchain::assert_quiet_end(&output, 2);
    }
}
