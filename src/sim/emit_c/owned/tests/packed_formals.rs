//! Activation-relative member plans must work without model-global cells.
use super::*;

fn member(root: IrLhs, two_state: bool) -> IrLhs {
    IrLhs::PackedSelect {
        target: Box::new(root),
        steps: vec![IrPackedSelect {
            base: number(0, 32),
            width: 8,
        }],
        signed: false,
        two_state,
    }
}

fn local_root(name: &str) -> IrLhs {
    IrLhs::WholeRef {
        addr: format!("&{name}"),
        width: 16,
        signed: false,
        two_state: false,
        shortreal: false,
    }
}

fn read_member() -> IrExpr {
    IrExpr::new(
        IrExprKind::PartSel {
            base: Box::new(IrExpr::new(IrExprKind::FormalRead(0), 16, false, None)),
            left: 7,
            right: 0,
        },
        8,
        false,
        None,
    )
}

#[test]
fn callback_member_writes_and_nested_reads_use_private_owners() {
    let mut model = IrModel::new("formal_callback".to_owned(), 1).unwrap();
    model.funcs.push(IrFunc::new(
        "f_member".to_owned(),
        Some(IrType::Packed {
            width: 8,
            signed: false,
            two_state: false,
        }),
        vec![IrFormal::new(false, 16, false).unwrap()],
        vec![],
        vec![],
        vec![
            IrStmt::Assign {
                lhs: member(local_root("a0"), false),
                rhs: number(7, 8),
                nba: false,
            },
            IrStmt::Return {
                value: Some(Box::new(read_member())),
            },
        ],
    ));
    let expression = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(number(0x1234, 16))],
            IrDepth::PROC,
            false,
        ))),
        8,
        false,
        None,
    );
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.read_only_callback = true;
    for _ in 0..2 {
        let result = frame.expression(&expression).unwrap();
        frame.discard(result);
    }
    assert!(model.signals.is_empty());
    assert_eq!(frame.body().matches("sv4_select_plan_set(").count(), 2);
    assert!(!frame.body().contains("sv4_clone(&a0)"));
    assert!(!frame.body().contains("llg_ba("));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn input_member_plan_resolves_the_materialized_input_cell() {
    let model = IrModel::new("formal_private".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.local("a0", 16, false, false, None).unwrap();
    let address = frame.lookup("a0").unwrap().address;
    let target = frame.target(&member(local_root("a0"), false)).unwrap();
    assert_eq!(target.binding.address, address);
    assert_eq!(target.width, 8);
    assert!(target.binding.automatic);
    let value = frame.expression(&number(0x5a, 8)).unwrap();
    frame.store(&target, value, false, "0").unwrap();
    frame.release_target(target);
    assert!(!frame.body().contains("sv4_clone(&a0)"));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn packed_ref_member_updates_via_the_original_reference_descriptor() {
    let mut model = IrModel::new("formal_reference".to_owned(), 1).unwrap();
    let mut formal = IrFormal::new(true, 16, false).unwrap();
    formal.mode = IrFormalMode::Ref;
    model.funcs.push(IrFunc::new(
        "f_ref".to_owned(),
        None,
        vec![formal],
        vec![],
        vec![],
        vec![],
    ));
    let ctx = RCtx {
        model: &model,
        func: Some(&model.funcs[0]),
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let root = IrLhs::Ref {
        addr: "r0".to_owned(),
        width: 16,
        signed: false,
        two_state: false,
        const_ref: false,
        bit: None,
    };
    let target = frame.target(&member(root, false)).unwrap();
    let value = frame.expression(&number(1, 8)).unwrap();
    frame.store(&target, value, false, "0").unwrap();
    frame.release_target(target);
    assert!(frame.body().contains("llg_ref_read(r0)"));
    assert!(frame.body().contains("llg_ref_write_masked(r0,"));
    assert!(frame.body().contains("sv4_select_plan_set("));
    assert!(!frame.body().contains("sv4_clone(&r0)"));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn two_state_member_conversion_precedes_read_modify_write_selection() {
    let model = IrModel::new("formal_union".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.local("a0", 16, false, false, None).unwrap();
    let target = frame.target(&member(local_root("a0"), true)).unwrap();
    let read = frame.read_target(&target);
    frame.discard(read);
    frame.release_target(target);
    let body = frame.body();
    assert!(body.find("sv4_to_two_state(").unwrap() < body.find("sv4_select_plan_read(").unwrap());
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn whole_signed_member_reads_restore_the_descriptor_sign() {
    let model = IrModel::new("formal_signed".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.local("a0", 16, false, false, None).unwrap();
    let mut lhs = member(local_root("a0"), false);
    if let IrLhs::PackedSelect { signed, .. } = &mut lhs {
        *signed = true;
    }
    let target = frame.target(&lhs).unwrap();
    let read = frame.read_target(&target);
    assert!(read.signed);
    frame.discard(read);
    frame.release_target(target);
    assert!(frame.body().contains("sv4_cast("));
    assert!(frame.slots.iter().all(|used| !used));
}
