//! Native-value emission contracts. Runtime execution is covered separately.
use super::super::native::NativeKind;
use super::*;

fn container_model(element: IrContainerElement) -> IrModel {
    let mut model = IrModel::new("native_values".to_owned(), 1).unwrap();
    model.containers.push(IrContainer {
        c_name: "G_values".to_owned(),
        element,
        kind: IrContainerKind::Dynamic,
        initial_size: None,
    });
    model
}

#[test]
fn native_return_slot_precedes_argument_cleanup_mark() {
    let mut model = IrModel::new("native_call".to_owned(), 1).unwrap();
    let mut function = IrFunc::new("f_text".to_owned(), None, vec![], vec![], vec![], vec![]);
    function.ret_string = true;
    model.funcs.push(function);
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let result = frame
        .native_call(0, &[], IrDepth::PROC, NativeKind::String)
        .unwrap();
    let address = result.address.clone();
    frame.native_discard(result);
    let source = frame.body();
    assert!(
        source.find("llg_value_scope_begin_object").unwrap()
            < source.find("llg_value_scope_mark()").unwrap()
    );
    assert!(source.contains(&format!("*({address}) = f_text(")));
    assert!(source.find("f_text(").unwrap() < source.find("llg_value_scopes_end_since(").unwrap());
}

#[test]
fn string_formals_are_copied_into_registered_callee_owners() {
    let mut model = IrModel::new("native_formal".to_owned(), 1).unwrap();
    let mut formal = IrFormal::new(false, 1, false).unwrap();
    formal.string = true;
    let mut function = IrFunc::new(
        "f_text".to_owned(),
        None,
        vec![formal],
        vec![],
        vec![],
        vec![
            IrStmt::Object(IrObjectStmt::StringAssignLocal(
                "_ret".to_owned(),
                IrStringExpr::FormalRead(0),
            )),
            IrStmt::Return { value: None },
        ],
    );
    function.ret_string = true;
    model.funcs.push(function);
    let function = &model.funcs[0];
    let ctx = RCtx {
        model: &model,
        func: Some(function),
        sampled: false,
        activation_label: None,
    };
    let source = super::super::model::function(&ctx, function).unwrap();
    assert!(source.contains("llg_string_clone(&a0)"));
    assert!(source.contains("llg_value_scope_begin_object(sizeof(llg_string_t)"));
    let returned = source
        .find("llg_string_t _llg_returned = llg_string_clone(")
        .unwrap();
    assert!(
        returned
            < source
                .rfind("llg_value_scopes_end_since(_llg_frame_base)")
                .unwrap()
    );
    assert!(!source.contains("llg_string_destroy(&a0)"));
}

#[test]
fn native_model_lifecycle_uses_typed_notification_and_release() {
    let mut model = IrModel::new("native_lifecycle".to_owned(), 1).unwrap();
    model.objects.push(IrObject {
        c_name: "G_text".to_owned(),
        ty: IrObjectType::String,
        initial: Some(IrStringExpr::Literal(b"abc".to_vec())),
    });
    model.objects.push(IrObject {
        c_name: "G_process".to_owned(),
        ty: IrObjectType::Process,
        initial: None,
    });
    let mut source = String::new();
    super::super::model::storage_lifecycle(&model, &mut source).unwrap();
    assert!(source.contains("G_text.notify = llg_dependency_changed"));
    assert!(source.contains("G_text.dependency = &G_text_llg_dep"));
    assert!(source.contains("llg_string_destroy(&G_text)"));
    assert!(source.contains("llg_process_assign(&G_process, NULL)"));
}

#[test]
fn container_read_keeps_index_owner_until_after_runtime_call() {
    let model = container_model(IrContainerElement::Packed {
        width: 129,
        signed: false,
        two_state: false,
    });
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let expression = IrExpr::new(
        IrExprKind::Container(Box::new(IrContainerExpr::Get {
            container: 0,
            index: Box::new(number(2, 32)),
        })),
        129,
        false,
        None,
    );
    let result = frame.expression(&expression).unwrap();
    frame.discard(result);
    let source = frame.body();
    assert!(source.contains("llg_dyn_get(&G_values, _llg_t["));
    assert!(source.find("llg_dyn_get(").unwrap() < source.find("sv4_destroy(").unwrap());
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn string_keys_stay_borrowed_across_mutation_callbacks() {
    let source = super::super::containers::key_adapters();
    assert!(source.contains("llg_owned_assoc_set_string"));
    assert!(source.contains("llg_assoc_set_string(array, key.data, key.len, value)"));
    assert!(!source.contains("llg_string_destroy(&key)"));
    assert!(!source.contains("llg_string_take(&key)"));
}

#[test]
fn file_scan_uses_registered_source_and_typed_borrowed_target() {
    let mut model = numeric_model();
    model.funcs.clear();
    model.processes.clear();
    model.spawns.clear();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let input = IrFileInput::ScanString {
        source: IrStringExpr::Literal(b"7".to_vec()),
        format: IrPlusArgText::Literal("%d".to_owned()),
        targets: vec![IrFileInputTarget::Packed {
            lhs: Box::new(IrLhs::Whole(0)),
            width: 65,
            signed: false,
            two_state: false,
        }],
    };
    let result = frame.file_input(&input).unwrap();
    frame.discard(result);
    let source = frame.body();
    assert!(source.contains("llg_ref_t _llg_input_reference_"));
    assert!(source.contains(".kind = LLG_FILE_INPUT_PACKED"));
    assert!(
        source.find("llg_value_scope_begin_object").unwrap()
            < source.find("llg_string_scanf(").unwrap()
    );
    assert!(
        source.find("llg_string_scanf(").unwrap() < source.find("llg_value_scope_end(").unwrap()
    );
}

#[test]
fn enum_navigation_releases_receiver_members_step_and_result() {
    let model = IrModel::new("enum_owners".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let expression = IrExpr::new(
        IrExprKind::EnumMethod(Box::new(IrEnumQuery {
            method: IrEnumMethod::Next,
            receiver: Some(Box::new(number(3, 65))),
            step: Some(Box::new(number(1, 32))),
            members: vec![
                IrEnumMember {
                    value: number(3, 65),
                    name: b"three".to_vec(),
                },
                IrEnumMember {
                    value: number(7, 65),
                    name: b"seven".to_vec(),
                },
            ],
            default: number(0, 65),
        })),
        65,
        false,
        None,
    );
    let result = frame.expression(&expression).unwrap();
    frame.discard(result);
    assert!(frame.body().contains("sv4_enum_navigate(_llg_t["));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn native_read_only_callbacks_reject_random_state_mutation() {
    let model = IrModel::new("native_readonly".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.read_only_callback = true;
    let error = frame
        .system_expression(&IrSysFunc::Urandom { seed: None }, &number(0, 32))
        .err()
        .unwrap();
    assert!(error.contains("read-only callbacks"));
}

#[test]
fn generic_queue_queries_use_the_generic_runtime_layout() {
    let mut model = container_model(IrContainerElement::String);
    model.containers[0].kind = IrContainerKind::Queue {
        maximum_elements: None,
    };
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let query = IrArrayQuery {
        kind: IrArrayQueryKind::Size,
        dimension: None,
        target: IrArrayQueryTarget::Container {
            container: 0,
            dimensions: vec![IrArrayDimension {
                left: None,
                right: None,
            }],
        },
    };
    let result = frame.array_query(&query, &number(0, 32)).unwrap();
    frame.discard(result);
    assert!(frame.body().contains("llg_queue_value_size(&G_values)"));
    assert!(!frame.body().contains("llg_queue_size(&G_values)"));
}
