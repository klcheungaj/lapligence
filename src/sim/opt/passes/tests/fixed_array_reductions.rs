//! Walkers visit both the captured receiver and the lexical map, not just storage.
use super::*;
use crate::sim::ir::{IrContainerReduction, IrFixedArrayReduction, IrFixedArrayReductionSource};

fn reduction(source: IrExpr, value: IrExpr) -> IrExpr {
    let (width, signed) = (value.width, value.signed);
    IrExpr::new(IrExprKind::FixedArrayReduce(Box::new(IrFixedArrayReduction {
        source: IrFixedArrayReductionSource::Value(Box::new(source)),
        operation: IrContainerReduction::Sum,
        left: 0,
        right: 1,
        element_width: 8,
        element_signed: false,
        element_two_state: false,
        item_name: "item".into(),
        index_name: "index".into(),
        value,
    })), width, signed, None)
}

#[test]
fn fixed_array_reduction_walkers_keep_receiver_and_map_capture_reads() {
    let mut model = IrModel::new("fold_reads".into(), 1).unwrap();
    for (i, width) in [16, 8, 8].into_iter().enumerate() {
        model.signals.push(IrSignal::new(format!("G_{i}"), None, IrType::packed(width, false).unwrap(), None).unwrap());
    }
    let receiver = IrExpr::new(IrExprKind::SigRead(0), 16, false, None);
    let capture = IrExpr::new(IrExprKind::SigRead(1), 8, false, None);
    model.processes.push(IrProcess::new("p".into(), "p".into(), IrShape::RunOnce, vec![],
        vec![assign(IrLhs::Whole(2), reduction(receiver, capture))]));
    model.validate().unwrap();
    mark_unused_storage(&mut model, None);
    assert!(model.signals.iter().all(|signal| !signal.omit));
    let mut reads = 0;
    walk_model_exprs_mut(&mut model, &mut |expr| {
        if matches!(expr.kind, IrExprKind::SigRead(_)) { reads += 1; }
    });
    assert_eq!(reads, 2);
}

#[test]
fn fixed_array_reduction_optimizes_children_without_folding_away_iterations() {
    let source = IrExpr::new(IrExprKind::Convert { a: Box::new(konst(0x0203, 16)) }, 16, false, None);
    let body = IrExpr::new(IrExprKind::Convert { a: Box::new(konst(1, 8)) }, 8, false, None);
    let mut expr = reduction(source, body);
    ident_expr(&mut expr);
    fold_expr(&mut expr);
    let IrExprKind::FixedArrayReduce(plan) = &expr.kind else {
        panic!("fold must remain symbolic");
    };
    let IrFixedArrayReductionSource::Value(source) = &plan.source else {
        unreachable!();
    };
    assert!(matches!(source.kind, IrExprKind::Const(_)));
    assert!(matches!(plan.value.kind, IrExprKind::Const(_)));
    assert_eq!(expr.width, 8);
}

#[test]
fn fixed_array_reduction_keeps_calls_in_a_constant_map_and_in_its_receiver() {
    let call = |width| IrExpr::new(IrExprKind::CallFn(Box::new(crate::sim::ir::IrCallExpr::new(
        0, vec![], IrDepth::PROC, false,
    ))), width, false, None);
    let mut expr = reduction(call(16), call(8));
    ident_expr(&mut expr);
    fold_expr(&mut expr);
    let IrExprKind::FixedArrayReduce(plan) = &expr.kind else {
        unreachable!();
    };
    let mut calls = 0;
    plan.expressions(&mut |expr| { if matches!(expr.kind, IrExprKind::CallFn(_)) { calls += 1; } });
    assert_eq!(calls, 2);
}
