//! Source-level regressions for the third ownership repair batch. These invoke
//! the production emitter; they are not replacements for HDL integration tests.
use super::*;

#[test]
fn pure_callback_inlines_owned_formals_without_native_writes() {
    let mut model = numeric_model();
    model.funcs[0].automatic = true;
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.read_only_callback = true;
    let call = IrExpr::new(IrExprKind::CallFn(Box::new(IrCallExpr::new(
        0, vec![IrCallArg::Val(number(3, 65))], IrDepth::PROC, false))), 65, false, None);
    let value = frame.expression(&call).unwrap();
    frame.discard(value);
    assert!(frame.slots.iter().all(|used| !used));
    assert!(frame.formal_overrides.is_empty());
    assert!(frame.body().contains("sv4_add("));
    assert!(!frame.body().contains("f_increment("));
    assert!(!frame.body().contains("llg_ba("));
}

#[test]
fn persistent_return_functions_remain_rejected_in_read_only_callbacks() {
    let mut model = numeric_model();
    model.funcs[0].automatic = false;
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.read_only_callback = true;
    let call = IrExpr::new(IrExprKind::CallFn(Box::new(IrCallExpr::new(
        0, vec![IrCallArg::Val(number(3, 65))], IrDepth::PROC, false))), 65, false, None);
    assert!(frame.expression(&call).err().unwrap().contains("side-effect-capable"));
}

#[test]
fn streaming_prepares_all_values_before_any_publication() {
    let model = numeric_model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let rhs = frame.expression(&number(7, 130)).unwrap();
    // Deliberately alias both destinations: each write must read the already
    // captured RHS, never the value installed by the preceding destination.
    let lhs = IrLhs::Stream { parts: vec![(IrLhs::Whole(0), 65), (IrLhs::Whole(0), 65)],
        width: 130, slice: 64, direction: IrStreamDirection::RightToLeft };
    let writes = frame.prepare_assignment(&lhs, rhs).unwrap();
    assert_eq!(writes.len(), 2);
    assert_eq!(frame.body().matches("sv4_unstream(").count(), 1);
    assert!(!frame.body().contains("llg_ba("));
    for (target, value) in writes {
        frame.store(&target, value, false, "0").unwrap();
        frame.release_target(target);
    }
    assert!(frame.slots.iter().all(|used| !used));
    assert_eq!(frame.body().matches("llg_ba(&G_value,").count(), 2);
}

#[test]
fn inline_event_capture_keeps_identity_and_respects_lexical_scope() {
    let model = numeric_model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.begin_block(&[]);
    frame.statement(&IrStmt::EventCapture { name: "event_arg".to_owned(), source: IrEventRef::Null }).unwrap();
    let captured = IrEventRef::Captured("event_arg".to_owned());
    assert!(frame.event_address(&captured).unwrap().starts_with("&_llg_event_capture_"));
    frame.end_block();
    assert!(frame.event_address(&captured).is_err());
    assert!(!frame.body().contains("NULL->"));
    assert!(frame.body().contains("llg_event_t* _llg_scalar_"));
}

#[test]
fn event_formal_templates_do_not_use_the_numeric_call_abi() {
    let mut model = numeric_model();
    model.funcs[0].formals[0].event = true;
    assert!(model::inline_event_template(&model.funcs[0]));
    assert!(model::check_model(&model).is_ok());
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let call = IrCallExpr::new(0, vec![IrCallArg::Val(number(3, 65))], IrDepth::PROC, false);
    assert!(frame.call_expression(&call).err().unwrap().contains("must be inlined"));
}

#[test]
fn sampled_expression_uses_snapshot_reads_then_restores_live_reads() {
    let model = numeric_model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let read = IrExpr::new(IrExprKind::SigRead(0), 65, false, None);
    let sampled = IrExpr::new(IrExprKind::SysFunc(IrSysFunc::Sampled(IrSampledCall::new(
        IrSampledFunc::Sampled, read.clone(), None, 0))), 65, false, None);
    let value = frame.expression(&sampled).unwrap(); frame.discard(value);
    assert!(!frame.sampled_reads);
    let value = frame.expression(&read).unwrap(); frame.discard(value);
    assert!(frame.body().contains("llg_sampled_copy(&G_value,"));
    assert!(frame.body().contains("sv4_clone(&G_value)"));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn alias_lifecycle_initializes_visible_owners_and_resets_bindings() {
    let mut model = numeric_model();
    model.net_groups.push(IrNetGroup::new("net".to_owned(), 65, false, IrNetKind::Wire, 1).unwrap());
    model.signals[0].net_alias.push(IrNetAliasBinding { group: 0, slot: 0, signal_bit: 0, group_bit: 0 });
    let mut source = String::new();
    model::storage_lifecycle(&model, &mut source).unwrap();
    assert!(source.contains("sv4_copy(&llg_net_alias_0.visible, &G_value)"));
    assert!(source.contains("sv4_destroy(&llg_net_alias_0.visible)"));
    assert_eq!(source.matches("net.n_aliases = 0;").count(), 2);
    assert!(source.find("net.n_aliases = 0;").unwrap() < source.find("llg_net_alias_bind(").unwrap());
}

#[test]
fn qualified_case_compares_candidates_before_running_selected_body() {
    let model = numeric_model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let items = vec![IrCaseItem::new(vec![number(1, 65)], vec![IrStmt::Assign {
        lhs: IrLhs::Whole(0), rhs: number(8, 65), nba: false }]),
        IrCaseItem::new(vec![number(1, 65)], vec![IrStmt::Nop])];
    let check = IrUniquePriorityCheck::Unique(crate::sim::semantic::Origin::Synthetic { reason: "qualified".to_owned() });
    frame.qualified_case(&number(1, 65), IrCaseKind::Exact, &items, &check).unwrap();
    assert_eq!(frame.body().matches("sv4_case_eq(").count(), 2);
    assert!(frame.body().rfind("sv4_case_eq(").unwrap() < frame.body().find("llg_unique_priority_check(").unwrap());
    assert!(frame.body().find("llg_unique_priority_check(").unwrap() < frame.body().find("llg_ba(").unwrap());
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn clocking_drive_passes_registered_payload_to_the_runtime() {
    let model = numeric_model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.clocking_drive(&IrLhs::Whole(0), &number(4, 65), &IrDelay::Constant(1),
        &[(IrWaitSrc::Sig("G_value".to_owned()), IrEdge::Posedge)]).unwrap();
    assert!(frame.body().contains("llg_clocking_nba_sync_after(&G_value,"));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn vpi_arguments_are_borrowed_from_registered_owners() {
    let model = numeric_model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let value = frame.vpi_call(0, "$probe", &[number(2, 65), number(3, 65)], Some((65, false))).unwrap().unwrap();
    frame.discard(value);
    assert!(frame.body().contains("llg_vpi_arg_t _llg_vpi_args_"));
    assert_eq!(frame.body().matches("llg_vpi_call_function_site(").count(), 1);
    assert!(frame.body().find("llg_vpi_call_function_site(").unwrap() < frame.body().find("sv4_destroy(").unwrap());
    assert!(frame.slots.iter().all(|used| !used));
}
