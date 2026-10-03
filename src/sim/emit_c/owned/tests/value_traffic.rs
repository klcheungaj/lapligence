use super::*;

fn read() -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(0), 65, false, None)
}

#[test]
fn pure_operands_borrow_and_reuse_the_arithmetic_result() {
    let model = numeric_model();
    let constants = super::super::super::constants::PackedConstants::default();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: Some(&constants),
    };
    let mut frame = Frame::new(&ctx);
    let value = frame
        .expression(&add(add(read(), number(3, 65), 65), read(), 65))
        .unwrap();
    frame.discard(value);
    assert!(!frame.body().contains("sv4_clone"));
    assert!(!frame.body().contains("sv4_cast"));
    assert_eq!(frame.body().matches("sv4_add_into").count(), 2);
    assert_eq!(frame.slots.len(), 1);
    assert_eq!(constants.lifecycle().matches("sv4_from_limbs").count(), 1);
}

#[test]
fn conversions_elide_only_matching_packed_shapes() {
    let model = numeric_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let value = frame.expression(&read()).unwrap();
    let value = frame.convert(value, 65, false, false, false);
    assert!(!frame.body().contains("sv4_cast"));
    let value = frame.convert(value, 65, true, false, false);
    let value = frame.convert(value, 64, true, false, false);
    let value = frame.convert(value, 64, true, true, false);
    frame.discard(value);
    assert_eq!(frame.body().matches("sv4_cast").count(), 2);
    assert_eq!(frame.body().matches("sv4_to_two_state").count(), 1);
}

#[test]
fn same_shape_resize_operand_borrows_but_retained_expression_owns() {
    let model = numeric_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let resize = IrExpr::new(
        IrExprKind::Resize {
            a: Box::new(read()),
        },
        65,
        false,
        None,
    );
    let operand = frame.operand(&resize).unwrap();
    assert!(operand.slot.is_none());
    frame.discard(operand);
    assert!(frame.body().is_empty());
    let owner = frame.expression(&resize).unwrap();
    assert!(owner.slot.is_some());
    frame.discard(owner);
    assert!(frame.body().contains("sv4_clone"));
    assert!(!frame.body().contains("sv4_resize"));
}

#[test]
fn calls_snapshot_earlier_operands_including_output_calls() {
    let mut model = numeric_model();
    model.funcs[0].formals[0].is_out = true;
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::OutAddr("&G_value".to_owned())],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    );
    assert!(!super::super::operands::stable_expression(&call));
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let value = frame.expression(&add(read(), call, 65)).unwrap();
    frame.discard(value);
    assert!(
        frame.body().find("sv4_clone(&G_value)").unwrap()
            < frame.body().find("f_increment(").unwrap()
    );
}

#[test]
fn selected_assignment_snapshots_rhs_before_calling_selector() {
    let model = numeric_model();
    let selector = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(number(0, 65))],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    );
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    frame
        .statement(&IrStmt::Assign {
            lhs: IrLhs::Bit(0, selector, false),
            rhs: read(),
            nba: false,
        })
        .unwrap();
    assert!(
        frame.body().find("sv4_clone(&G_value)").unwrap()
            < frame.body().find("f_increment(").unwrap()
    );
}

#[test]
fn model_constants_are_distinct_by_shape_and_live_until_close() {
    let pool = super::super::super::constants::PackedConstants::default();
    assert_eq!(
        pool.intern(
            emit_const(match &number(3, 65).kind {
                IrExprKind::Const(c) => c,
                _ => unreachable!(),
            }),
            65,
            false
        ),
        pool.intern(
            emit_const(match &number(3, 65).kind {
                IrExprKind::Const(c) => c,
                _ => unreachable!(),
            }),
            65,
            false
        )
    );
    let execution = ExecutionModel::lower(numeric_model()).unwrap();
    let source = super::super::super::model::render(&execution).unwrap();
    assert!(source.contains("static sv4_t llg_constant_"));
    assert!(source.contains("llg_model_constants_init();"));
    assert!(source.contains("llg_model_constants_destroy();"));
    let body = source
        .split("static void llg_model_constants_init(void) {")
        .nth(1)
        .unwrap();
    assert!(body.contains("sv4_from_limbs"));
    assert!(!source
        .split("static void llg_model_constants_init(void) {")
        .next()
        .unwrap()
        .contains("sv4_from_limbs"));
}

#[test]
fn retained_reads_stay_in_frame_storage_across_suspension() {
    let mut model = numeric_model();
    model.processes[0].body = vec![
        IrStmt::DeclLocal {
            name: "saved".to_owned(),
            width: 65,
            signed: false,
            two_state: false,
            init: Some(Box::new(read())),
        },
        IrStmt::Delay {
            ticks: IrDelay::Constant(1),
        },
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: IrExpr::new(IrExprKind::LocalRead("saved".to_owned()), 65, false, None),
            nba: false,
        },
    ];
    let execution = ExecutionModel::lower(model).unwrap();
    let source = super::super::super::model::render(&execution).unwrap();
    assert!(source.contains("sv4_clone(&G_value)"));
    assert!(source.contains("LLG_CO_AWAIT"));
    assert!(source.contains("sv4_move("));
}

#[test]
fn mutating_later_operand_snapshots_the_earlier_read() {
    let model = numeric_model();
    let mutation = IrExpr::new(
        IrExprKind::Mutation(Box::new(IrMutationExpr {
            lhs: IrLhs::Whole(0),
            value: Box::new(number(2, 65)),
            current_width: 65,
            current_signed: false,
            reads_current: false,
            post: false,
        })),
        65,
        false,
        None,
    );
    assert!(!super::super::operands::stable_expression(&mutation));
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let value = frame.expression(&add(read(), mutation, 65)).unwrap();
    frame.discard(value);
    assert!(
        frame.body().find("sv4_clone(&G_value)").unwrap()
            < frame.body().find("llg_ba(&G_value").unwrap()
    );
}

#[test]
fn fills_pool_by_payload_and_preserve_context_conversion() {
    let model = numeric_model();
    let constants = super::super::super::constants::PackedConstants::default();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: Some(&constants),
    };
    let mut frame = Frame::new(&ctx);
    let filled = frame.packed_fill(0, 65, false, true);
    let literal = frame.constant(
        match &number(0, 65).kind {
            IrExprKind::Const(c) => c,
            _ => unreachable!(),
        },
        true,
    );
    assert_eq!(filled.code, literal.code);
    frame.discard(filled);
    frame.discard(literal);
    let fill = IrExpr::new(IrExprKind::Fill(3), 1, false, Some(3));
    let value = frame.operand(&fill).unwrap();
    let value = frame.convert(value, 65, true, false, false);
    assert_eq!((value.width, value.signed, value.fill), (65, true, None));
    assert!(value.slot.is_some());
    frame.discard(value);
    assert_eq!(constants.lifecycle().matches("sv4_from_limbs").count(), 2);
}

#[test]
fn suspending_call_keeps_its_input_snapshot_in_the_frame() {
    let mut model = numeric_model();
    model.funcs[0].ret = None;
    model.funcs[0].is_task = true;
    model.funcs[0].body = vec![IrStmt::Delay {
        ticks: IrDelay::Constant(1),
    }];
    model.processes[0].body = vec![IrStmt::Call(IrCall::new(
        0,
        vec![IrCallArg::Val(read())],
        IrDepth::PROC,
        Vec::new(),
        Vec::new(),
    ))];
    let source =
        super::super::super::model::render(&ExecutionModel::lower(model).unwrap()).unwrap();
    let read = source.find("sv4_clone(&G_value)").unwrap();
    let suspend = source[read..].find("LLG_CO_CALL(").unwrap() + read;
    assert!(read < suspend);
    assert!(source.contains("sv4_t* _llg_t;"));
}

#[test]
fn unary_borrows_and_mismatched_resize_still_converts() {
    let model = numeric_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let unary = IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::BitNeg,
            a: Box::new(read()),
        },
        65,
        false,
        None,
    );
    let value = frame.expression(&unary).unwrap();
    frame.discard(value);
    assert!(frame.body().contains("sv4_bitneg(G_value)"));
    assert!(!frame.body().contains("sv4_clone"));
    for (width, signed) in [(64, false), (65, true)] {
        let resize = IrExpr::new(
            IrExprKind::Resize {
                a: Box::new(read()),
            },
            width,
            signed,
            None,
        );
        let value = frame.operand(&resize).unwrap();
        assert!(value.slot.is_some());
        frame.discard(value);
    }
    assert_eq!(frame.body().matches("sv4_resize").count(), 2);
}

#[test]
fn borrowed_local_materializes_using_its_registered_address() {
    let model = numeric_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.local("value", 65, false, false, None).unwrap();
    let address = frame.lookup("value").unwrap().address;
    let read = IrExpr::new(IrExprKind::LocalRead("value".into()), 65, false, None);
    let value = frame.operand(&read).unwrap();
    assert!(value.slot.is_none());
    let value = frame.own(value);
    assert!(frame.body().contains(&format!("sv4_clone({address})")));
    assert!(!frame.body().contains("sv4_clone(&(*"));
    frame.discard(value);
}

#[test]
fn batched_pca_helpers_share_the_models_wide_index_constant() {
    let constants = super::super::super::constants::PackedConstants::default();
    let shape = super::super::super::statements::pca_batches::Shape {
        expression: IrExpr::new(
            IrExprKind::BitSel {
                base: Box::new(read()),
                idx: Box::new(number(1, 65)),
            },
            1,
            false,
            None,
        ),
        source_type: IrType::packed(65, false).unwrap(),
        target_type: IrType::packed(1, false).unwrap(),
    };
    for _ in 0..2 {
        let (body, _) = super::super::pca_batches::helper_body(&shape, Some(&constants)).unwrap();
        assert!(!body.contains("sv4_from_limbs"));
        assert!(body.contains("llg_constant_"));
    }
    assert_eq!(constants.lifecycle().matches("sv4_from_limbs").count(), 1);
}

#[test]
fn pure_short_circuit_and_mux_selectors_borrow_without_changing_branches() {
    let model = numeric_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let logical = IrExpr::new(
        IrExprKind::Bin {
            op: IrBinOp::LogAnd,
            a: Box::new(read()),
            b: Box::new(read()),
        },
        1,
        false,
        None,
    );
    let value = frame.expression(&logical).unwrap();
    frame.discard(value);
    assert!(!frame.body().contains("sv4_clone"));
    assert!(frame.body().contains("} else {"));
    let mux = IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(read()),
            a: Box::new(number(1, 65)),
            b: Box::new(number(2, 65)),
        },
        65,
        false,
        None,
    );
    let value = frame.expression(&mux).unwrap();
    frame.discard(value);
    assert!(!frame.body().contains("sv4_clone(&G_value)"));
    assert!(frame.body().contains("sv4_mux(G_value,"));
    assert!(!frame.body().contains("sv4_resize("));
}

#[test]
fn mux_arms_keep_width_and_sign_conversions_and_pool_expanded_fills() {
    let model = numeric_model();
    let constants = super::super::super::constants::PackedConstants::default();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: Some(&constants),
    };
    let mut frame = Frame::new(&ctx);
    let mut signed = number(2, 65);
    signed.signed = true;
    if let IrExprKind::Const(constant) = &mut signed.kind {
        constant.signed = true;
    }
    let mux = IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(read()),
            a: Box::new(number(1, 64)),
            b: Box::new(signed),
        },
        65,
        false,
        None,
    );
    let value = frame.expression(&mux).unwrap();
    frame.discard(value);
    assert_eq!(frame.body().matches("sv4_resize(").count(), 4);
    let fill = IrExpr::new(IrExprKind::Fill(2), 1, false, Some(2));
    let mux = IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(read()),
            a: Box::new(fill),
            b: Box::new(number(1, 65)),
        },
        65,
        false,
        None,
    );
    let value = frame.expression(&mux).unwrap();
    frame.discard(value);
    assert!(!frame.body().contains("sv4_fill("));
    assert!(constants.lifecycle().contains("sv4_from_limbs"));
}

#[test]
fn owned_one_limb_literals_construct_directly_and_wide_literals_share_storage() {
    let model = numeric_model();
    let constants = super::super::super::constants::PackedConstants::default();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: Some(&constants),
    };
    let mut frame = Frame::new(&ctx);
    let value = frame.expression(&number(3, 64)).unwrap();
    frame.discard(value);
    assert!(frame.body().contains("SV4_INIT(3ULL, 0ULL, 0ULL, 64, 0)"));
    assert!(constants.operands().is_empty());
    let value = frame.operand(&number(3, 64)).unwrap();
    assert!(value.slot.is_some());
    frame.discard(value);
    assert!(constants.operands().is_empty());
    for _ in 0..2 {
        let value = frame.expression(&number(3, 65)).unwrap();
        frame.discard(value);
    }
    assert_eq!(constants.lifecycle().matches("sv4_from_limbs").count(), 1);
    assert_eq!(frame.body().matches("sv4_clone(").count(), 2);
}

#[test]
fn vpi_results_normalize_unproven_runtime_shapes_before_elision() {
    let model = numeric_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let value = frame
        .vpi_call(0, "$sized", &[], Some((5, false)))
        .unwrap()
        .unwrap();
    let value = frame.convert(value, 5, false, false, false);
    frame.discard(value);
    assert_eq!(
        frame.body().matches("llg_vpi_call_function_site(").count(),
        1
    );
    assert_eq!(frame.body().matches("sv4_cast(").count(), 1);
    assert!(frame.body().contains("sv4_cast(_llg_t[0], 5, 0)"));
    assert_eq!(frame.slots.len(), 1);
}
