//! Source-level regressions for the third ownership repair batch. These invoke
//! the production emitter; they are not replacements for HDL integration tests.
use super::*;

#[test]
fn pure_callback_inlines_owned_formals_without_native_writes() {
    let mut model = numeric_model();
    model.funcs[0].automatic = true;
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
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(number(3, 65))],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    );
    let value = frame.expression(&call).unwrap();
    frame.discard(value);
    assert!(frame.slots.iter().all(|used| !used));
    assert!(frame.formal_overrides.is_empty());
    assert!(frame.body().contains("sv4_add_to("));
    assert!(!frame.body().contains("f_increment("));
    assert!(!frame.body().contains("llg_ba("));
    assert!(!frame.body().contains("llg_ba_from("));
}

#[test]
fn persistent_local_functions_remain_rejected_in_read_only_callbacks() {
    let mut model = numeric_model();
    model.funcs[0].automatic = false;
    model.funcs[0]
        .locals
        .push(IrLocal::new("_persistent".to_owned(), 65, false).unwrap());
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
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(number(3, 65))],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    );
    assert!(frame
        .expression(&call)
        .err()
        .unwrap()
        .contains("side-effect-capable"));
}

#[test]
fn static_formal_copies_are_private_in_read_only_callbacks() {
    let mut model = numeric_model();
    model.funcs[0].automatic = false;
    model.funcs[0].callback_return_independent = true;
    model.funcs[0].body.insert(
        0,
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: IrExpr::new(IrExprKind::FormalRead(0), 65, false, None),
            nba: false,
        },
    );
    model.funcs[0].callback_private_formal_copies.push((0, 0));
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
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(number(3, 65))],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    );
    let value = frame.expression(&call).unwrap();
    frame.discard(value);
    assert!(!frame.body().contains("llg_ba(&G_value"));
    assert!(!frame.body().contains("llg_ba_from(&G_value"));
    assert!(frame.body().contains("sv4_add_to("));
}

#[test]
fn static_callback_returns_require_a_lowering_proof() {
    let mut model = numeric_model();
    model.funcs[0].automatic = false;
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
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(number(3, 65))],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    );
    let error = frame
        .expression(&call)
        .err()
        .expect("unproven static callback result is rejected");
    assert!(error.contains("persistent return state"), "{error}");
}

#[test]
fn streaming_prepares_all_values_before_any_publication() {
    let model = numeric_model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let rhs = frame.expression(&number(7, 130)).unwrap();
    // Deliberately alias both destinations: each write must read the already
    // captured RHS, never the value installed by the preceding destination.
    let lhs = IrLhs::Stream {
        parts: vec![(IrLhs::Whole(0), 65), (IrLhs::Whole(0), 65)],
        width: 130,
        slice: 64,
        direction: IrStreamDirection::RightToLeft,
    };
    let writes = frame.prepare_assignment(&lhs, rhs).unwrap();
    assert_eq!(writes.len(), 2);
    assert_eq!(frame.body().matches("sv4_unstream_to(").count(), 1);
    assert!(!frame.body().contains("llg_ba("));
    assert!(!frame.body().contains("llg_ba_from("));
    for (target, value) in writes {
        frame.store(&target, value, false, "0").unwrap();
        frame.release_target(target);
    }
    assert!(frame.slots.iter().all(|used| !used));
    assert_eq!(frame.body().matches("llg_ba_from(&G_value,").count(), 2);
}

#[test]
fn inline_event_capture_keeps_identity_and_respects_lexical_scope() {
    let model = numeric_model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.begin_block(&[]);
    frame
        .statement(&IrStmt::EventCapture {
            name: "event_arg".to_owned(),
            source: IrEventRef::Null,
        })
        .unwrap();
    let captured = IrEventRef::Captured("event_arg".to_owned());
    assert!(frame
        .event_address(&captured)
        .unwrap()
        .starts_with("&_llg_event_capture_"));
    frame.end_block();
    assert!(frame.event_address(&captured).is_err());
    assert!(!frame.body().contains("NULL->"));
    assert!(frame.body().contains("llg_event_t* _llg_scalar_"));
}

#[test]
fn inline_expanded_templates_are_not_callable() {
    let mut model = numeric_model();
    model.funcs[0].inline_expanded = true;
    assert!(model::inline_template(&model.funcs[0]));
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let call = IrCallExpr::new(0, vec![IrCallArg::Val(number(3, 65))], IrDepth::PROC, false);
    assert!(frame
        .call_expression(&call)
        .err()
        .unwrap()
        .contains("must be expanded"));
}

#[test]
fn input_event_formals_pass_the_object_identity_by_value() {
    let mut model = numeric_model();
    model.funcs[0].formals[0].event = true;
    assert!(!model::inline_template(&model.funcs[0]));
    model.funcs[0].formals[0].mode = IrFormalMode::Output;
    model.funcs[0].formals[0].is_out = true;
    let fields = crate::sim::emit_c::model::owned_func_param_fields(&model.funcs[0]);
    assert_eq!(fields[0], ("llg_event_t*".to_owned(), "o0".to_owned()));
    model.funcs[0].formals[0].mode = IrFormalMode::Input;
    model.funcs[0].formals[0].is_out = false;
    let fields = crate::sim::emit_c::model::owned_func_param_fields(&model.funcs[0]);
    assert_eq!(fields[0], ("llg_event_t".to_owned(), "a0".to_owned()));
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let call = IrCallExpr::new(
        0,
        vec![IrCallArg::EventVal(IrEventRef::Null)],
        IrDepth::PROC,
        false,
    );
    let value = frame.call_expression(&call).unwrap();
    frame.discard(value);
    assert!(frame.body().contains("llg_event_t* _llg_scalar_"));
    assert!(frame.body().contains("->object : NULL }"));
    assert!(!frame.body().contains("NULL->"));
}

#[test]
fn sampled_expression_uses_snapshot_reads_then_restores_live_reads() {
    let model = numeric_model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let read = IrExpr::new(IrExprKind::SigRead(0), 65, false, None);
    let sampled = IrExpr::new(
        IrExprKind::SysFunc(Box::new(IrSysFunc::Sampled(IrSampledCall::new(
            IrSampledFunc::Sampled,
            read.clone(),
            None,
            0,
        )))),
        65,
        false,
        None,
    );
    let value = frame.expression(&sampled).unwrap();
    frame.discard(value);
    assert!(!frame.sampled_reads);
    let value = frame.expression(&read).unwrap();
    frame.discard(value);
    assert!(frame.body().contains("llg_sampled_copy(&G_value,"));
    assert!(find_copy(frame.body(), "&G_value").is_some());
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn alias_lifecycle_initializes_visible_owners_and_resets_bindings() {
    let mut model = numeric_model();
    model
        .net_groups
        .push(IrNetGroup::new("net".to_owned(), 65, false, IrNetKind::Wire, 1).unwrap());
    model.signals[0].net_alias.push(IrNetAliasBinding {
        group: 0,
        slot: 0,
        signal_bit: 0,
        group_bit: 0,
    });
    let mut source = String::new();
    model::storage_lifecycle(
        &model,
        &super::super::super::constants::PackedConstants::default(),
        crate::sim::value_backend::ValueBackend::Legacy,
        &mut source,
    )
    .unwrap();
    assert!(source.contains("&llg_net_alias_0,"));
    assert!(source.contains("sv4_copy(&alias->visible, alias->storage)"));
    assert!(source.contains("sv4_destroy(&llg_storage_0[_llg_n]->visible)"));
    assert_eq!(source.matches("net->n_aliases = 0;").count(), 1);
    assert!(source.contains("llg_net_alias_clear(llg_net_storage[_llg_n].net);"));
    assert!(
        source.find("net->n_aliases = 0;").unwrap() < source.find("llg_net_alias_bind(").unwrap()
    );
}

#[test]
fn qualified_case_compares_candidates_before_running_selected_body() {
    let model = numeric_model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let items = vec![
        IrCaseItem::new(
            vec![number(1, 65)],
            vec![IrStmt::Assign {
                lhs: IrLhs::Whole(0),
                rhs: number(8, 65),
                nba: false,
            }],
        ),
        IrCaseItem::new(vec![number(1, 65)], vec![IrStmt::Nop]),
    ];
    let check = IrUniquePriorityCheck::Unique(crate::sim::semantic::Origin::Synthetic {
        reason: "qualified".to_owned(),
    });
    frame
        .qualified_case(&number(1, 65), IrCaseKind::Exact, &items, &check)
        .unwrap();
    assert_eq!(frame.body().matches("sv4_case_eq_to(").count(), 2);
    assert!(
        frame.body().rfind("sv4_case_eq_to(").unwrap()
            < frame.body().find("llg_unique_priority_check(").unwrap()
    );
    assert!(
        frame.body().find("llg_unique_priority_check(").unwrap()
            < frame.body().find("llg_ba_from(").unwrap()
    );
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn clocking_drive_passes_registered_payload_to_the_runtime() {
    let model = numeric_model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    frame
        .clocking_drive(
            &IrLhs::Whole(0),
            &number(4, 65),
            &IrDelay::Constant(1),
            &[(IrWaitSrc::Sig("G_value".to_owned()), IrEdge::Posedge)],
        )
        .unwrap();
    assert!(frame
        .body()
        .contains("llg_clocking_nba_sync_after(&G_value,"));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn vpi_arguments_are_borrowed_from_registered_owners() {
    let model = numeric_model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let value = frame
        .vpi_call(
            0,
            "$probe",
            &[number(2, 65), number(3, 65)],
            Some((65, false)),
        )
        .unwrap()
        .unwrap();
    frame.discard(value);
    assert!(frame.body().contains("llg_vpi_arg_t _llg_vpi_args_"));
    assert_eq!(
        frame
            .body()
            .matches("llg_vpi_call_function_site_to(")
            .count(),
        1
    );
    assert!(
        frame.body().find("llg_vpi_call_function_site_to(").unwrap()
            < frame.body().find("sv4_destroy(").unwrap()
    );
    assert!(frame.slots.iter().all(|used| !used));
}
