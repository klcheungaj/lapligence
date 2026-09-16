//! Structural checks and an opt-in end-to-end C-toolchain smoke test.
use super::*;
use crate::sim::execution::ExecutionModel;

fn number(value: u64, width: u32) -> IrExpr {
    let count = ((width + 63) / 64) as usize;
    let mut bits = vec![0; count];
    bits[0] = value;
    IrExpr::new(IrExprKind::Const(IrConst {
        bits, x: vec![0; count], z: vec![0; count], width,
        signed: false, real: None, fill: None,
    }), width, false, None)
}

fn add(a: IrExpr, b: IrExpr, width: u32) -> IrExpr {
    IrExpr::new(IrExprKind::Bin { op: IrBinOp::Add, a: Box::new(a), b: Box::new(b) }, width, false, None)
}

fn numeric_model() -> IrModel {
    let mut model = IrModel::new("owned_numeric".to_owned(), 1).unwrap();
    let ty = IrType::Packed { width: 65, signed: false, two_state: false };
    model.signals.push(IrSignal::new("G_value".to_owned(), None, ty, None).unwrap());
    let formal = IrExpr::new(IrExprKind::FormalRead(0), 65, false, None);
    model.funcs.push(IrFunc::new("f_increment".to_owned(), Some(ty),
        vec![IrFormal::new(false, 65, false).unwrap()], Vec::new(), Vec::new(),
        vec![IrStmt::Return { value: Some(Box::new(add(formal, number(1, 65), 65))) }]));
    let read = IrExpr::new(IrExprKind::SigRead(0), 65, false, None);
    let call = IrExpr::new(IrExprKind::CallFn(Box::new(IrCallExpr::new(
        0, vec![IrCallArg::Val(read.clone())], IrDepth::PROC, false))), 65, false, None);
    model.processes.push(IrProcess::new("p_numeric".to_owned(), "numeric".to_owned(),
        IrShape::RunOnce, Vec::new(), vec![
            IrStmt::Assign { lhs: IrLhs::Whole(0), rhs: number(7, 65), nba: false },
            IrStmt::Repeat { count: number(1000, 32), body: vec![
                IrStmt::Assign { lhs: IrLhs::Whole(0), rhs: call, nba: false },
            ] },
            IrStmt::Display { fmt: "\"%0d\"".to_owned(), args: vec![(read, false)],
                newline: true, default_radix: IrDisplayRadix::Decimal },
        ]));
    model.spawns.push("p_numeric".to_owned());
    model
}

#[test]
fn expressions_sequence_owners_and_reuse_slots() {
    let model = IrModel::new("test".to_owned(), 1).unwrap();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let expression = add(add(number(3, 65), number(4, 65), 65), number(5, 65), 65);
    for _ in 0..100 {
        let result = frame.expression(&expression).unwrap();
        frame.discard(result);
        assert!(frame.slots.iter().all(|used| !used));
    }
    assert_eq!(frame.slots.len(), 2);
    assert!(frame.prologue().contains("llg_value_scope_begin(2)"));
    assert!(!frame.body().contains("({"));
    assert!(!frame.body().contains("sv4_add(sv4_"));
    assert!(frame.body().contains("sv4_destroy(&_llg_t[1])"));
}

#[test]
fn conditional_fill_arms_are_owned_and_context_sized() {
    let model = IrModel::new("test".to_owned(), 1).unwrap();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    let expression = IrExpr::new(IrExprKind::Mux {
        sel: Box::new(number(1, 1)),
        a: Box::new(IrExpr::new(IrExprKind::Fill(1), 1, false, Some(1))),
        b: Box::new(number(0, 65)),
    }, 65, false, None);
    let result = frame.expression(&expression).unwrap();
    frame.discard(result);
    assert!(frame.body().contains("sv4_fill(1, 65, 0)"));
    assert!(frame.body().contains("sv4_mux("));
    assert!(frame.body().contains("sv4_move("));
    assert!(!frame.body().contains("({"));
}

#[test]
fn model_has_dynamic_start_close_and_no_width_abi() {
    let execution = ExecutionModel::lower(numeric_model()).unwrap();
    let source = super::super::model::render(&execution).unwrap();
    assert!(source.contains("#define LLG_MODEL_VALUE_ABI 3"));
    assert!(source.contains("sv4_t G_value = SV4_EMPTY;"));
    assert!(source.contains("sv4_t _llg_returned = sv4_clone("));
    assert!(source.contains("int llg_model_start("));
    assert!(source.contains("if (llg_rt_is_suspended()) return 2;"));
    assert!(source.contains("llg_rt_cleanup();\n    llg_model_storage_destroy();"));
    assert!(source.contains("#ifndef LLG_MODEL_NO_MAIN"));
    assert!(!source.contains("LLG_MODEL_MAX_WIDTH"));
    assert!(!source.contains("({"));
}

#[test]
fn unmigrated_opaque_expression_is_rejected_not_rendered() {
    let mut model = numeric_model();
    model.processes[0].body = vec![IrStmt::Assign { lhs: IrLhs::Whole(0), nba: false,
        rhs: IrExpr::new(IrExprKind::Verbatim {
            code: "old_untracked_result()".to_owned(), width: 65, signed: false,
        }, 65, false, None),
    }];
    let execution = ExecutionModel::lower(model).unwrap();
    let error = super::super::model::render(&execution).unwrap_err().to_string();
    assert!(error.contains("opaque C expressions"), "{error}");
    assert!(error.contains("no C model was emitted"), "{error}");
}

#[test]
fn declaration_calls_remain_rejected_outside_coroutine_context() {
    let model = numeric_model();
    let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
    let mut frame = Frame::new(&ctx);
    frame.allow_calls = false;
    let call = IrExpr::new(IrExprKind::CallFn(Box::new(IrCallExpr::new(
        0, vec![IrCallArg::Val(number(3, 65))], IrDepth::PROC, false))), 65, false, None);
    assert!(frame.expression(&call).err().unwrap().contains("declaration initialization"));
}

#[test]
#[ignore = "requires the Rust project build, a C compiler, and CMake"]
fn structured_owned_model_executes_numeric_loop() {
    use std::process::Command;
    let execution = ExecutionModel::lower(numeric_model()).unwrap();
    let source = super::super::model::render(&execution).unwrap();
    let directory = std::env::temp_dir().join(format!("llg-owned-numeric-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let binary = crate::sim::build::build_model_cmake(&directory, &[("model.c", &source)]).unwrap();
    let result = Command::new(&binary).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "1007");
    std::fs::remove_dir_all(directory).unwrap();
}
