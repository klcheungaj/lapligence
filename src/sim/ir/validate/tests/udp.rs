use super::*;

fn table() -> IrUdpTable {
    IrUdpTable {
        name: "udp".into(),
        input_count: 2,
        rows: vec![IrUdpRow {
            inputs: vec![IrUdpInput::Binary, IrUdpInput::Any],
            output: IrUdpOutput::One,
        }],
    }
}

fn eval(inputs: Vec<IrExpr>) -> IrExpr {
    IrExpr::new(IrExprKind::UdpEval { table: 0, inputs }, 1, false, None)
}

#[test]
fn udp_ir_validates_definition_references_rows_and_scalar_operands() {
    let mut model = valid_model();
    let expr = eval(vec![packed_const(0, 1), packed_const(1, 1)]);
    assert!(model.validate_expr(&expr, None).is_err());
    model.udp_tables.push(table());
    model.validate_expr(&expr, None).unwrap();
    assert_eq!(model.expression_capacity(&expr, None).unwrap(), 1);
    for inputs in [
        vec![],
        vec![packed_const(0, 2), packed_const(1, 1)],
        vec![
            packed_const(0, 1),
            IrExpr::new(IrExprKind::SigRead(999), 1, false, None),
        ],
        vec![
            packed_const(0, 1),
            IrExpr::new(IrExprKind::Const(IrConst::real(0.0)), 0, false, None),
        ],
    ] {
        assert!(model.validate_expr(&eval(inputs), None).is_err());
    }
    for (width, signed, fill) in [(2, false, None), (1, true, None), (1, false, Some(0))] {
        let mut bad = expr.clone();
        bad.width = width;
        bad.signed = signed;
        bad.fill = fill;
        assert!(model.validate_expr(&bad, None).is_err());
    }
    model.udp_tables[0].rows[0].inputs.pop();
    assert!(model.validate().is_err());
    assert!(model.validate_expr(&expr, None).is_err());
    model.udp_tables[0] = table();
    model.udp_tables[0].input_count = 0;
    assert!(model.validate().is_err());
    model.udp_tables[0].input_count = usize::MAX;
    assert!(model.validate().is_err());
}
