use super::*;

fn udp_model(rows: usize) -> IrModel {
    let mut model = IrModel::new("udp_tables".into(), 1).unwrap();
    model.udp_tables.push(IrUdpTable {
        name: "primitive".into(),
        input_count: 2,
        rows: vec![
            IrUdpRow {
                inputs: vec![IrUdpInput::Binary, IrUdpInput::Unknown],
                output: IrUdpOutput::One,
            };
            rows
        ],
    });
    model
}

fn udp_expr() -> IrExpr {
    IrExpr::new(
        IrExprKind::UdpEval {
            table: 0,
            inputs: vec![number(0, 1), number(1, 1)],
        },
        1,
        false,
        None,
    )
}

#[test]
fn udp_gate_body_is_constant_size_with_many_rows() {
    let mut bodies = Vec::new();
    for rows in [7, 4096] {
        let model = udp_model(rows);
        model.validate_expr(&udp_expr(), None).unwrap();
        let ctx = RCtx {
            value_backend: crate::sim::value_backend::ValueBackend::Legacy,
            model: &model,
            func: None,
            sampled: false,
            activation_label: None,
            constants: None,
        };
        let mut frame = Frame::new(&ctx);
        let result = frame.expression(&udp_expr()).unwrap();
        frame.discard(result);
        assert!(frame.slots.iter().all(|live| !live));
        let body = frame.body();
        assert_eq!(body.matches("sv4_udp_eval_to(").count(), 1);
        assert_eq!(body.matches("llg_udp_table_0").count(), 1);
        assert!(!body.contains("sv4_mux_to("));
        bodies.push(body.len());
    }
    assert_eq!(bodies[1] - bodies[0], 3);
    assert!(bodies[1] < 1000);
}

#[test]
fn udp_instances_share_one_file_scope_table() {
    let mut model = udp_model(4096);
    model.signals.push(
        IrSignal::new(
            "G_out".into(),
            None,
            IrType::Packed {
                width: 1,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    for index in 0..8 {
        let name = format!("p_gate_{index}");
        model.processes.push(IrProcess::new(
            name.clone(),
            name.clone(),
            IrShape::RunOnce,
            Vec::new(),
            vec![IrStmt::Assign {
                lhs: IrLhs::Whole(0),
                rhs: udp_expr(),
                nba: false,
            }],
        ));
        model.spawns.push(name);
    }
    let execution = ExecutionModel::lower(model).unwrap();
    let source =
        super::super::super::model::render_with_sharing_threshold(&execution, usize::MAX).unwrap();
    assert_eq!(
        source
            .matches("static const uint8_t llg_udp_table_")
            .count(),
        1
    );
    assert_eq!(source.matches("sv4_udp_eval_to(").count(), 8);
    assert!(source.len() < 200_000);
}

#[test]
fn udp_inputs_are_evaluated_once_in_order_even_for_wildcard_rows() {
    let mut model = udp_model(16);
    model.udp_tables[0].rows[0].inputs = vec![IrUdpInput::Any; 2];
    let ty = IrType::Packed {
        width: 1,
        signed: false,
        two_state: false,
    };
    for name in ["f_left", "f_right"] {
        model.funcs.push(IrFunc::new(
            name.into(),
            Some(ty),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![IrStmt::Return {
                value: Some(Box::new(number(0, 1))),
            }],
        ));
    }
    let expr = IrExpr::new(
        IrExprKind::UdpEval {
            table: 0,
            inputs: (0..2)
                .map(|function| {
                    IrExpr::new(
                        IrExprKind::CallFn(Box::new(IrCallExpr::new(
                            function,
                            Vec::new(),
                            IrDepth::PROC,
                            false,
                        ))),
                        1,
                        false,
                        None,
                    )
                })
                .collect(),
        },
        1,
        false,
        None,
    );
    model.validate_expr(&expr, None).unwrap();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let result = frame.expression(&expr).unwrap();
    frame.discard(result);
    let body = frame.body();
    assert_eq!(body.matches("f_left(").count(), 1);
    assert_eq!(body.matches("f_right(").count(), 1);
    assert!(body.find("f_left(").unwrap() < body.find("f_right(").unwrap());
    assert!(body.find("f_right(").unwrap() < body.find("sv4_udp_eval_to(").unwrap());
    assert!(frame.slots.iter().all(|live| !live));
}
