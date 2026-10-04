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
        assert_eq!(body.matches("llg_udp_index_0[").count(), 1);
        assert_eq!(body.matches("sv4_fill_to(").count(), 1);
        assert!(!body.contains("sv4_udp_eval"));
        assert!(!body.contains("sv4_mux_to("));
        bodies.push(body.to_owned());
    }
    // The dense index makes the evaluation independent of the row count.
    assert_eq!(bodies[0], bodies[1]);
    assert!(bodies[1].len() < 1000);
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
            .matches("static const uint8_t llg_udp_index_")
            .count(),
        1
    );
    assert!(!source.contains("llg_udp_table_"));
    assert_eq!(source.matches("llg_udp_index_0[(").count(), 8);
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
    assert!(body.find("f_right(").unwrap() < body.find("llg_udp_index_0[").unwrap());
    assert!(frame.slots.iter().all(|live| !live));
}

/// Independent oracle: scan rows in source order, Z already folded into X.
fn first_match(table: &IrUdpTable, states: &[u8]) -> u8 {
    table
        .rows
        .iter()
        .find(|row| {
            row.inputs
                .iter()
                .zip(states)
                .all(|(mask, state)| *mask as u8 & (1 << state) != 0)
        })
        .map_or(2, |row| row.output as u8)
}

#[test]
fn dense_index_matches_first_row_scan_for_every_combination() {
    use IrUdpInput::{Any, Binary, One, Unknown, Zero};
    let masks = [Zero, One, Unknown, Binary, Any];
    let outputs = [IrUdpOutput::Zero, IrUdpOutput::One, IrUdpOutput::Unknown];
    // Deterministic pseudo-random tables with overlapping compatible and
    // order-dependent rows over 1..=4 inputs, plus the empty table.
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move |bound: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % bound as u64) as usize
    };
    for input_count in 1..=4 {
        for rows in [0, 1, 3, 9, 40] {
            let table = IrUdpTable {
                name: "oracle".into(),
                input_count,
                rows: (0..rows)
                    .map(|_| IrUdpRow {
                        inputs: (0..input_count).map(|_| masks[next(masks.len())]).collect(),
                        output: outputs[next(outputs.len())],
                    })
                    .collect(),
            };
            let dense = super::super::udp::dense_index(&table).expect("small tables are dense");
            assert_eq!(dense.len(), 3usize.pow(input_count as u32));
            for (position, state) in dense.iter().enumerate() {
                let mut digits = vec![0u8; input_count];
                let mut rest = position;
                for digit in digits.iter_mut().rev() {
                    *digit = (rest % 3) as u8;
                    rest /= 3;
                }
                assert_eq!(
                    *state,
                    first_match(&table, &digits),
                    "{table:?} at {digits:?}"
                );
            }
        }
    }
}

#[test]
fn dense_index_limit_is_ten_inputs() {
    let table = |input_count| IrUdpTable {
        name: "wide".into(),
        input_count,
        rows: vec![IrUdpRow {
            inputs: vec![IrUdpInput::Any; input_count],
            output: IrUdpOutput::One,
        }],
    };
    let dense = super::super::udp::dense_index(&table(10)).expect("ten inputs are dense");
    assert_eq!(dense.len(), 59_049);
    assert!(dense.iter().all(|state| *state == 1));
    assert!(super::super::udp::dense_index(&table(11)).is_none());
}

#[test]
fn definitions_above_the_dense_limit_keep_the_row_scan() {
    let mut model = IrModel::new("udp_wide".into(), 1).unwrap();
    model.udp_tables.push(IrUdpTable {
        name: "wide".into(),
        input_count: 11,
        rows: vec![IrUdpRow {
            inputs: vec![IrUdpInput::Binary; 11],
            output: IrUdpOutput::One,
        }],
    });
    let expr = IrExpr::new(
        IrExprKind::UdpEval {
            table: 0,
            inputs: (0..11).map(|bit| number(bit % 2, 1)).collect(),
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
    assert_eq!(body.matches("sv4_udp_eval_to(").count(), 1);
    assert!(!body.contains("llg_udp_index_"));
    let mut tables = String::new();
    super::super::udp::tables(&model, &mut tables);
    assert!(tables.contains("static const uint8_t llg_udp_table_0[]"));
    assert!(!tables.contains("llg_udp_digit"));
}

#[test]
fn scalar_inputs_and_constant_bits_are_read_in_place() {
    let mut model = udp_model(3);
    model.signals.push(
        IrSignal::new(
            "G_bus".into(),
            None,
            IrType::Packed {
                width: 8,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    let bus = IrExpr::new(IrExprKind::SigRead(0), 8, false, None);
    let bit = IrExpr::new(
        IrExprKind::BitSel {
            base: Box::new(bus.clone()),
            idx: Box::new(number(5, 32)),
        },
        1,
        false,
        None,
    );
    let part = IrExpr::new(
        IrExprKind::PartSel {
            base: Box::new(bus),
            left: 2,
            right: 2,
        },
        1,
        false,
        None,
    );
    let expr = IrExpr::new(
        IrExprKind::UdpEval {
            table: 0,
            inputs: vec![bit, part],
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
    assert!(frame.slots.iter().all(|live| !live));
    let body = frame.body();
    assert!(body.contains("llg_sv4_state(G_bus, 5ULL)"), "{body}");
    assert!(body.contains("llg_sv4_state(G_bus, 2ULL)"), "{body}");
    assert!(!body.contains("sv4_copy("), "{body}");
    assert!(!body.contains("sv4_bit_select"), "{body}");
    assert!(!body.contains("sv4_part_select"), "{body}");
    assert_eq!(body.matches("sv4_fill_to(").count(), 1, "{body}");
}
