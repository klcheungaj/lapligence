//! Source-review regressions. Runtime-only probes cannot check emitted C types.
use super::*;

fn reference_model() -> IrModel {
    let mut model = IrModel::new("review_references".to_owned(), 1).unwrap();
    let mut formal = IrFormal::new(true, 65, false).unwrap();
    formal.mode = IrFormalMode::Ref;
    model.funcs.push(IrFunc::new("f_ref".to_owned(), None,
        vec![formal], vec![], vec![], vec![]));
    model
}

#[test]
fn forwarded_reference_resolution_uses_the_formal_descriptor() {
    let model = reference_model();
    let ctx = RCtx { model: &model, func: Some(&model.funcs[0]), sampled: false,
        activation_label: None };
    let frame = Frame::new(&ctx);
    assert_eq!(frame.reference_address("r0").unwrap(), "r0");
    assert!(frame.reference_address("r1").is_err());
    assert!(frame.reference_address("arbitrary_C_text").is_err());
}

#[test]
fn reference_bit_write_passes_one_captured_native_index() {
    let model = reference_model();
    let ctx = RCtx { model: &model, func: Some(&model.funcs[0]), sampled: false,
        activation_label: None };
    let mut frame = Frame::new(&ctx);
    let lhs = IrLhs::Ref { addr: "r0".to_owned(), width: 1, signed: false,
        two_state: false, const_ref: false, bit: Some(Box::new(number(64, 96))) };
    let target = frame.target(&lhs).unwrap();
    let index = match &target.selection {
        Some(super::super::stores::Selection::Bit(index)) => index.clone(),
        _ => panic!("reference bit target must capture a native index"),
    };
    let value = frame.expression(&number(1, 1)).unwrap();
    frame.store(&target, value, false, "0").unwrap();
    frame.release_target(target);
    let source = frame.body();
    assert_eq!(source.matches("sv4_to_index(").count(), 1);
    assert!(source.contains(&format!("uint64_t {index} = sv4_to_index(")));
    assert!(source.contains(&format!("llg_ref_write_bit(r0, {index}, _llg_t[")));
    assert!(!source.contains(&format!("sv4_from_u64({index},")));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn addressable_real_local_is_heap_backed_and_lexically_owned() {
    let model = IrModel::new("review_real".to_owned(), 1).unwrap();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.begin_block(&[]);
    frame.local("result", 0, false, false, None).unwrap();
    let local = frame.lookup("result").unwrap();
    assert!(local.automatic);
    assert!(!local.address.starts_with('&'));
    assert!(frame.body().contains("llg_value_scope_begin_object(sizeof(double), NULL)"));
    assert!(frame.body().contains(&format!("double* {} = (double*)llg_value_scope_object(", local.address)));
    assert!(frame.body().contains(&format!("*{} = 0.0;", local.address)));
    frame.end_block();
    assert!(frame.body().contains("llg_value_scopes_end_since("));
    assert!(frame.lookup("result").is_none());
}
