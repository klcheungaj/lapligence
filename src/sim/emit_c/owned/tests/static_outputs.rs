//! Static output formals retain storage; inout still copies the caller value.
use super::*;

fn model() -> IrModel {
    let mut model = IrModel::new("static_outputs".to_owned(), 1).unwrap();
    let wide = IrType::Packed {
        width: 65,
        signed: false,
        two_state: false,
    };
    for name in ["G_static", "G_actual"] {
        model
            .signals
            .push(IrSignal::new(name.to_owned(), None, wide, None).unwrap());
    }
    let ret = IrType::Packed {
        width: 8,
        signed: false,
        two_state: false,
    };
    model
        .signals
        .push(IrSignal::new("G_result".to_owned(), None, ret, None).unwrap());
    let destination = IrLhs::WholeRef {
        addr: "o1".to_owned(),
        width: 65,
        signed: false,
        two_state: false,
        shortreal: false,
    };
    let mut function = IrFunc::new(
        "f_static".to_owned(),
        Some(ret),
        vec![
            IrFormal::new(false, 1, false).unwrap(),
            IrFormal::new(true, 65, false).unwrap(),
        ],
        Vec::new(),
        Vec::new(),
        vec![
            IrStmt::If {
                cond: IrExpr::new(IrExprKind::FormalRead(0), 1, false, None),
                then_: vec![IrStmt::Assign {
                    lhs: destination.clone(),
                    rhs: number(21, 65),
                    nba: false,
                }],
                els: None,
                check: IrUniquePriorityCheck::None,
            },
            IrStmt::Return {
                value: Some(Box::new(number(7, 8))),
            },
        ],
    );
    function.automatic = false;
    model.funcs.push(function.clone());
    function.c_name = "f_inout".to_owned();
    function.formals[1].mode = IrFormalMode::Inout;
    function.body = vec![
        IrStmt::Assign {
            lhs: destination,
            rhs: add(
                IrExpr::new(IrExprKind::FormalRead(1), 65, false, None),
                number(1, 65),
                65,
            ),
            nba: false,
        },
        IrStmt::Return {
            value: Some(Box::new(number(8, 8))),
        },
    ];
    model.funcs.push(function);
    model
}

fn call(function: usize, write: bool, inout: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            function,
            vec![
                IrCallArg::OutTemp {
                    name: "_output".to_owned(),
                    init: inout
                        .then(|| Box::new(IrExpr::new(IrExprKind::SigRead(1), 65, false, None))),
                    writeback: Box::new(IrLhs::Whole(1)),
                    storage_addr: Some("&G_static".to_owned()),
                    storage_lhs: Some(Box::new(IrLhs::Whole(0))),
                    storage_read: Some(Box::new(IrExpr::new(
                        IrExprKind::SigRead(0),
                        65,
                        false,
                        None,
                    ))),
                    selector_inits: Vec::new(),
                },
                IrCallArg::Val(number(u64::from(write), 1)),
            ],
            IrDepth::PROC,
            false,
        ))),
        8,
        false,
        None,
    )
}

#[test]
fn static_output_call_has_no_copy_in_but_static_inout_does() {
    let model = model();
    for inout in [false, true] {
        let ctx = RCtx {
            model: &model,
            func: None,
            sampled: false,
            activation_label: None,
        };
        let mut frame = Frame::new(&ctx);
        let value = frame
            .expression(&call(usize::from(inout), false, inout))
            .unwrap();
        frame.discard(value);
        let source = frame.body();
        let invocation = if inout { "f_inout(" } else { "f_static(" };
        let before_call = &source[..source.find(invocation).expect("callee invocation")];
        assert_eq!(before_call.contains("sv4_move(&G_static,"), inout);
        assert!(source.contains("llg_value_scopes_end_since("));
        assert!(frame.slots.iter().all(|used| !used));
    }
}

#[test]
fn generated_static_output_and_inout_calls_preserve_distinct_copy_contracts() {
    assert!(crate::sim::build::cmake_available(), "CMake is required");
    for optimized in [false, true] {
        let mut model = model();
        let mut body = Vec::new();
        for (function, write, inout, caller) in [
            (0, true, false, 0),
            (0, false, false, 99),
            (1, false, true, 40),
            (1, false, true, 7),
        ] {
            body.push(IrStmt::Assign {
                lhs: IrLhs::Whole(1),
                rhs: number(caller, 65),
                nba: false,
            });
            body.push(IrStmt::Assign {
                lhs: IrLhs::Whole(2),
                rhs: call(function, write, inout),
                nba: false,
            });
            body.push(IrStmt::Display {
                fmt: "\"%0d\"".to_owned(),
                args: vec![(IrExpr::new(IrExprKind::SigRead(1), 65, false, None), false)],
                newline: true,
                default_radix: IrDisplayRadix::Decimal,
            });
        }
        model.processes.push(IrProcess::new(
            "p_outputs".to_owned(),
            "outputs".to_owned(),
            IrShape::RunOnce,
            Vec::new(),
            body,
        ));
        model.spawns.push("p_outputs".to_owned());
        let mut execution = ExecutionModel::lower(model).unwrap();
        let config = if optimized {
            crate::sim::opt::OptConfig::default()
        } else {
            crate::sim::opt::OptConfig::none()
        };
        crate::sim::opt::run(&mut execution, &config).unwrap();
        let source = super::super::super::model::render(&execution).unwrap();
        let directory = toolchain::Directory::new("static-outputs");
        let binary =
            crate::sim::build::build_model_cmake(directory.path(), &[("model.c", &source)])
                .unwrap();
        let output = toolchain::execute(&binary);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "21\n21\n41\n8\n");
        toolchain::assert_quiet_end(&output, 0);
    }
}
