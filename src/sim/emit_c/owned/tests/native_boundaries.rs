//! Batch 5 ownership contracts. These tests invoke the emitter, not C patterns.
use super::*;

fn class_model() -> IrModel {
    let mut model = IrModel::new("class_owners".to_owned(), 1).unwrap();
    model.classes.push(IrClass { c_name: "base".to_owned(), base: None, fields: vec![
        IrClassField { c_name: "data".to_owned(), ty: IrClassFieldType::Packed { width: 129, signed: false, two_state: false } },
        IrClassField { c_name: "text".to_owned(), ty: IrClassFieldType::String },
    ] });
    model.objects.push(IrObject { c_name: "G_handle".to_owned(), ty: IrObjectType::Chandle, initial: None });
    model.native_accesses.push(IrNativeAccess { name: "_llg_access_0".to_owned(), receiver: IrChandleExpr::Read(0),
        kind: IrNativeAccessKind::ClassField { class: 0, field: 0 }, site: None, function: None });
    model.class_allocations.push(IrClassAllocation { class: 0, local: "new_object".to_owned(), body: vec![], function: None });
    model
}

#[test]
fn class_objects_have_exact_field_storage_and_explicit_teardown() {
    let mut model = class_model();
    model.processes.push(IrProcess::new("p".to_owned(), "class".to_owned(), IrShape::RunOnce, vec![], vec![
        IrStmt::Object(IrObjectStmt::ChandleAssign(0, IrChandleExpr::Construct(0))),
        IrStmt::Assign { lhs: IrLhs::WholeRef { addr: "&_llg_access_0".to_owned(), width: 129,
            signed: false, two_state: false, shortreal: false }, rhs: number(7, 129), nba: false },
    ]));
    model.spawns.push("p".to_owned());
    let execution = ExecutionModel::lower(model).unwrap();
    let source = super::super::super::model::render(&execution).unwrap();
    assert!(source.contains("llg_class_field_t* fields"));
    assert!(source.contains("calloc(object->count, sizeof(*object->fields))"));
    assert!(source.contains("llg_class_storage_destroy();"));
    assert!(source.contains("llg_class_field("));
    assert!(!source.contains("llg_class_0_t*"));
    assert!(!source.contains("LLG_LIMBS"));
}

#[test]
fn native_access_receiver_is_evaluated_once_at_the_use_site() {
    let mut model = class_model();
    let mut factory = IrFunc::new("f_factory".to_owned(), None, vec![], vec![], vec![], vec![]);
    factory.ret_chandle = true;
    model.funcs.push(factory);
    model.native_accesses[0].receiver = IrChandleExpr::Call { function: 0, args: vec![],
        depth: IrDepth::PROC, receiver: None, virtual_dispatch: false };
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let result = frame.expression(&IrExpr::new(IrExprKind::LocalRead("_llg_access_0".to_owned()), 129, false, None)).unwrap();
    frame.discard(result);
    assert_eq!(frame.body().matches("f_factory(").count(), 1);
    assert!(frame.body().find("f_factory(").unwrap() < frame.body().find("llg_class_field(").unwrap());
}

#[test]
fn invalid_native_access_tables_fail_before_emission() {
    let mut model = class_model();
    model.native_accesses[0].kind = IrNativeAccessKind::ClassField { class: 0, field: 2 };
    assert!(ExecutionModel::lower(model).is_err());
}

#[test]
fn semaphore_creation_and_queries_never_request_legacy_fragments() {
    let model = IrModel::new("sem".to_owned(), 1).unwrap();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.statement(&IrStmt::Object(IrObjectStmt::ChandleDeclareLocal("sem".to_owned(),
        Some(IrChandleExpr::SemaphoreNew(Box::new(number(3, 32))))))).unwrap();
    let query = IrObjectQuery::SemaphoreTryGet(IrChandleExpr::LocalRead("sem".to_owned()), number(2, 32));
    let result = frame.expression(&IrExpr::new(IrExprKind::ObjectQuery(Box::new(query)), 32, true, None)).unwrap();
    frame.discard(result);
    assert!(frame.body().contains("llg_semaphore_new(_llg_t["));
    assert!(frame.body().contains("llg_semaphore_try_get("));
    assert!(!frame.body().contains("({"));
}

#[test]
fn reference_calls_register_descriptors_and_do_not_copy_back() {
    let mut model = numeric_model();
    let mut formal = IrFormal::new(true, 65, false).unwrap();
    formal.mode = IrFormalMode::Ref;
    model.funcs.clear();
    model.funcs.push(IrFunc::new("f_reference".to_owned(), None, vec![formal], vec![], vec![], vec![]));
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let arg = IrCallArg::RefAddr { addr: "not_executable_C".to_owned(), width: 65, signed: false,
        two_state: false, const_ref: false, lhs: Box::new(IrLhs::Whole(0)),
        read: Box::new(IrExpr::new(IrExprKind::SigRead(0), 65, false, None)) };
    assert!(frame.call_values(0, &[arg], IrDepth::PROC).unwrap().is_none());
    let source = frame.body();
    assert!(source.contains("llg_ref_scope_begin_owned();"));
    assert!(source.contains("sizeof(llg_ref_t)"));
    assert!(source.contains(".kind = LLG_REF_WHOLE"));
    assert!(!source.contains("not_executable_C"));
    assert!(!source.contains("llg_ba("));
    assert!(source.find("f_reference(").unwrap() < source.find("llg_value_scopes_end_since").unwrap());
}

#[test]
fn const_ref_callback_reads_are_live_and_not_writable_calls() {
    let mut model = numeric_model();
    model.funcs.clear();
    let mut formal = IrFormal::new(true, 65, false).unwrap();
    formal.mode = IrFormalMode::Ref; formal.const_ref = true;
    model.funcs.push(IrFunc::new("f_read".to_owned(), Some(IrType::packed(65, false).unwrap()),
        vec![formal], vec![], vec![], vec![IrStmt::Return { value: Some(Box::new(
            IrExpr::new(IrExprKind::FormalRead(0), 65, false, None))) }]));
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx); frame.read_only_callback = true;
    let arg = IrCallArg::RefAddr { addr: "typed_reference".to_owned(), width: 65, signed: false,
        two_state: false, const_ref: true, lhs: Box::new(IrLhs::Whole(0)),
        read: Box::new(IrExpr::new(IrExprKind::SigRead(0), 65, false, None)) };
    let call = IrCallExpr::new(0, vec![arg], IrDepth::PROC, false);
    let result = frame.pure_callback_call(&call).unwrap(); frame.discard(result);
    assert!(frame.body().contains("sv4_clone(&G_value)"));
    assert!(!frame.body().contains("f_read("));
    assert!(!frame.body().contains("llg_ba("));
}

#[test]
fn container_inside_uses_owned_items_and_ordinal_associative_reads() {
    let mut model = IrModel::new("inside".to_owned(), 1).unwrap();
    model.containers.push(IrContainer { c_name: "G_set".to_owned(), kind: IrContainerKind::Associative {
        key: IrAssocKey::Integral { width: 32, signed: true, two_state: true } },
        element: IrContainerElement::Packed { width: 129, signed: false, two_state: false }, initial_size: None });
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let result = frame.inside(&number(7, 129), &[IrInsideItem::Container { container: 0 }]).unwrap();
    frame.discard(result);
    assert!(frame.body().contains("llg_assoc_value_at(&G_set,"));
    assert!(frame.body().contains("sv4_wild_eq("));
    assert!(frame.slots.iter().all(|slot| !slot));
}

#[test]
fn mixed_stream_captures_all_selectors_before_publication() {
    let mut model = numeric_model();
    model.containers.push(IrContainer { c_name: "G_queue".to_owned(), kind: IrContainerKind::Queue { maximum_elements: None },
        element: IrContainerElement::Packed { width: 65, signed: false, two_state: false }, initial_size: None });
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.stream_assignment(&number(7, 130), 1, IrStreamDirection::LeftToRight, &[
        IrStreamTarget::Container { container: 0, selector: None },
        IrStreamTarget::Packed { lhs: IrLhs::Whole(0), width: 65 },
    ]).unwrap();
    let source = frame.body();
    assert!(source.rfind("sv4_part_select(").unwrap() < source.find("llg_queue_unstream_assign(").unwrap());
    assert!(source.contains("llg_ba("));
    assert!(frame.slots.iter().all(|slot| !slot));
}

#[test]
fn native_destination_receiver_retains_its_execution_effects() {
    let mut model = class_model();
    model.native_accesses[0].receiver = IrChandleExpr::Construct(0);
    model.processes.push(IrProcess::new("p".to_owned(), "member".to_owned(), IrShape::RunOnce,
        vec![], vec![IrStmt::Assign { lhs: IrLhs::WholeRef { addr: "&_llg_access_0".to_owned(),
            width: 129, signed: false, two_state: false, shortreal: false }, rhs: number(7, 129), nba: false }]));
    model.spawns.push("p".to_owned());
    let execution = ExecutionModel::lower(model).unwrap();
    assert!(execution.processes().iter().any(|process|
        process.effects.contains(&crate::sim::execution::ExecutionEffect::RuntimeService)));
}
