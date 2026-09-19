//! Captured chain owners, masks and synchronous-input descriptors.
use super::*;

fn model() -> IrModel {
    let mut model = IrModel::new("packed_selection_test".to_owned(), 1).unwrap();
    model.arrays.push(IrArray::new(
        "memory".to_owned(), "memory".to_owned(), 16, false, vec![(0, 0)],
    ).unwrap());
    model
}

fn lhs() -> IrLhs {
    IrLhs::ArrayElem {
        arr: 0,
        indices: vec![number(0, 32)],
        elem_sel: IrElemSel::PackedChain(vec![
            IrPackedSelect { base: number(0, 32), width: 8 },
            IrPackedSelect { base: number(6, 32), width: 4 },
        ]),
    }
}

#[test]
fn packed_selection_owners_are_released_and_nba_captures_the_same_mask() {
    let model = model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let target = frame.target(&lhs()).unwrap();
    assert_eq!((target.width, target.signed), (4, false));
    // All array/selector operands have been reduced to borrowed-free scalars.
    assert!(frame.slots.iter().all(|used| !used));
    let value = frame.expression(&number(15, 4)).unwrap();
    frame.store(&target, value, true, "1").unwrap();
    frame.release_target(target);
    assert!(frame.slots.iter().all(|used| !used));
    let source = frame.body();
    assert_eq!(source.matches("sv4_select_plan_step(").count(), 2);
    assert_eq!(source.matches("sv4_select_plan_set(").count(), 2);
    assert!(source.contains("llg_nba_masked("));
    assert!(source.rfind("sv4_select_plan_step(").unwrap() < source.find("sv4_select_plan_set(").unwrap());
    assert!(source.rfind("sv4_select_plan_set(").unwrap() < source.find("llg_nba_masked(").unwrap());
    assert!(!source.contains("({{"));
}

#[test]
fn packed_selection_reads_default_the_parent_before_applying_inner_bounds() {
    let mut model = model();
    model.arrays[0].two_state = true;
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let target = frame.target(&lhs()).unwrap();
    let value = frame.read_target(&target);
    assert_eq!((value.width, value.signed), (4, false));
    frame.discard(value);
    frame.release_target(target);
    let source = frame.body();
    assert!(source.contains("sv4_select_plan_read("));
    assert!(source.contains("sv4_from_u64(0, 16, 0)"));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn packed_selection_input_uses_a_synchronous_plan_descriptor() {
    let model = model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let result = frame.file_input(&IrFileInput::ScanString {
        source: IrStringExpr::Literal(b"f".to_vec()),
        format: IrPlusArgText::Literal("%h".to_owned()),
        targets: vec![IrFileInputTarget::Packed {
            lhs: Box::new(lhs()), width: 4, signed: false, two_state: false,
        }],
    }).unwrap();
    frame.discard(result);
    let source = frame.body();
    assert!(source.contains(".kind = LLG_REF_PACKED_PLAN, .retained = &"));
    assert!(source.find("sv4_select_plan_step(").unwrap() < source.find("llg_string_scanf(").unwrap());
    assert!(frame.slots.iter().all(|used| !used));
}
