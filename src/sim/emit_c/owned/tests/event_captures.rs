use super::*;

fn event_initial(event: IrEventRef) -> IrExpr {
    IrExpr::new(
        IrExprKind::ObjectQuery(Box::new(IrObjectQuery::EventCapture(event))),
        1,
        false,
        None,
    )
}

#[test]
fn fork_event_capture_copies_object_identity_and_binds_a_private_handle() {
    let model = IrModel::new("event_capture".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let storage = StorageRef::new(
        FrameId::new(0),
        0,
        StorageLifetime::Automatic,
        StorageOwnership::Owned,
    )
    .with_kind(StorageKind::Event);
    let initial = event_initial(IrEventRef::Formal(0));
    let mut parent = Frame::new(&ctx);
    parent.event_bindings.last_mut().unwrap().insert(
        super::super::events::event_formal_binding(0),
        "&a0".to_owned(),
    );
    let values = parent
        .prepare_captures(std::iter::once((storage, &initial)))
        .unwrap();
    parent.publish_captures("event_frame", values);
    let source = parent.body();
    assert!(source.contains("->object : NULL"));
    assert!(source.contains("llg_frame_capture_opaque("));
    assert!(!source.contains("llg_frame_capture_value("));

    let mut child = Frame::new(&ctx);
    child
        .bind_capture("child_event", storage, &initial, "frame")
        .unwrap();
    let event = IrEventRef::Captured("child_event".to_owned());
    assert!(child.event_address(&event).unwrap().starts_with('&'));
    let nested_initial = event_initial(event);
    let nested_values = child
        .prepare_captures(std::iter::once((storage, &nested_initial)))
        .unwrap();
    child.publish_captures("nested_frame", nested_values);
    let source = child.body();
    assert!(source.contains("llg_event_t "));
    assert!(source.contains("llg_frame_read_opaque(frame, 0u)"));
    assert!(source.contains("->object : NULL"));
}

#[test]
fn event_capture_rejects_numeric_sources_and_invalid_borrowing() {
    let model = IrModel::new("event_capture".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let owned = StorageRef::new(
        FrameId::new(0),
        0,
        StorageLifetime::Automatic,
        StorageOwnership::Owned,
    )
    .with_kind(StorageKind::Event);
    assert!(frame
        .prepare_captures(std::iter::once((owned, &number(1, 1))))
        .is_err());
    let borrowed = StorageRef::new(
        FrameId::new(0),
        0,
        StorageLifetime::Automatic,
        StorageOwnership::Borrowed,
    )
    .with_kind(StorageKind::Event);
    assert!(frame
        .prepare_captures(std::iter::once((
            borrowed,
            &event_initial(IrEventRef::Null)
        )))
        .is_err());
}

#[test]
fn joined_event_capture_borrows_the_handle_for_sibling_rebinding() {
    let model = IrModel::new("event_alias".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let storage = StorageRef::new(
        FrameId::new(0),
        0,
        StorageLifetime::Automatic,
        StorageOwnership::Borrowed,
    )
    .with_kind(StorageKind::Event);
    let initial = event_initial(IrEventRef::Formal(0));
    let mut parent = Frame::new(&ctx);
    parent.event_bindings.last_mut().unwrap().insert(
        super::super::events::event_formal_binding(0),
        "&a0".to_owned(),
    );
    let values = parent
        .prepare_captures(std::iter::once((storage, &initial)))
        .unwrap();
    parent.publish_captures("event_frame", values);
    assert!(parent.body().contains("= &a0;"));
    assert!(!parent.body().contains("->object"));
    let mut child = Frame::new(&ctx);
    child
        .bind_capture("child_event", storage, &initial, "frame")
        .unwrap();
    let address = child
        .event_address(&IrEventRef::Captured("child_event".to_owned()))
        .unwrap();
    assert!(child.body().contains("llg_event_t* "));
    assert!(!address.starts_with('&'));
}

#[test]
fn event_capture_validation_keeps_formals_in_the_initializer_only() {
    let model = IrModel::new("event_capture".to_owned(), 1).unwrap();
    let mut formal = IrFormal::new(false, 1, false).unwrap();
    formal.event = true;
    let function = IrFunc::new(
        "waiter".to_owned(),
        None,
        vec![formal],
        vec![],
        vec![],
        vec![],
    );
    let storage = StorageRef::new(
        FrameId::new(0),
        0,
        StorageLifetime::Automatic,
        StorageOwnership::Owned,
    )
    .with_kind(StorageKind::Event);
    let make_branch = |storage, initial, event| IrPreFn::CapturedBranch {
        c_name: "branch".to_owned(),
        frame: FrameId::new(0),
        captures: vec![IrCapture::new(storage, initial)],
        body: vec![IrStmt::EventTrigger { ev: event }],
    };
    let branch = make_branch(
        storage,
        event_initial(IrEventRef::Formal(0)),
        IrEventRef::Captured("event".to_owned()),
    );
    assert!(model.validate_pre_fn(&branch, Some(&function)).is_ok());
    let branch = make_branch(
        storage,
        event_initial(IrEventRef::Formal(0)),
        IrEventRef::Formal(0),
    );
    assert!(model
        .validate_pre_fn(&branch, Some(&function))
        .unwrap_err()
        .to_string()
        .contains("requires an input event formal"));
    let branch = make_branch(
        storage,
        number(1, 1),
        IrEventRef::Captured("event".to_owned()),
    );
    assert!(model.validate_pre_fn(&branch, Some(&function)).is_err());
    let branch = make_branch(
        storage.with_kind(StorageKind::Packed),
        event_initial(IrEventRef::Formal(0)),
        IrEventRef::Captured("event".to_owned()),
    );
    assert!(model.validate_pre_fn(&branch, Some(&function)).is_err());
}
