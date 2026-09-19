//! Owned-emitter regressions for the first Group 1 repair batch.
use super::*;

fn call(function: usize, args: Vec<IrCallArg>, width: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            function,
            args,
            IrDepth::PROC,
            false,
        ))),
        width,
        false,
        None,
    )
}

#[test]
fn callback_expansions_rename_all_internal_labels_and_gotos() {
    let mut model = IrModel::new("review_inline_labels".to_owned(), 1).unwrap();
    let ty = IrType::Packed {
        width: 32,
        signed: false,
        two_state: false,
    };
    model.funcs.push(IrFunc::new(
        "f_inner".to_owned(),
        Some(ty),
        vec![],
        vec![],
        vec![],
        vec![
            IrStmt::Block(vec![
                IrStmt::Goto("loop_exit".to_owned()),
                IrStmt::Label("loop_exit".to_owned()),
            ]),
            IrStmt::Return {
                value: Some(Box::new(number(7, 32))),
            },
        ],
    ));
    model.funcs.push(IrFunc::new(
        "f_outer".to_owned(),
        Some(ty),
        vec![],
        vec![],
        vec![],
        vec![IrStmt::Return {
            value: Some(Box::new(add(call(0, vec![], 32), call(0, vec![], 32), 32))),
        }],
    ));
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.read_only_callback = true;
    for _ in 0..2 {
        let value = frame.expression(&call(1, vec![], 32)).unwrap();
        frame.discard(value);
    }
    let labels: Vec<_> = frame
        .body()
        .lines()
        .filter_map(|line| line.trim().strip_suffix(": ;"))
        .collect();
    let unique: std::collections::HashSet<_> = labels.iter().copied().collect();
    assert_eq!(labels.len(), unique.len(), "duplicate C function-scoped label");
    assert_eq!(labels.iter().filter(|label| label.ends_with("_loop_exit")).count(), 4);
    for jump in frame.body().lines().filter_map(|line| {
        line.trim().strip_prefix("goto ").and_then(|line| line.strip_suffix(';'))
    }) {
        assert!(unique.contains(jump), "missing target for {jump}");
    }
    assert!(!frame.body().contains("goto loop_exit;"));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn real_callback_results_are_declared_in_the_callers_scope() {
    for shortreal in [false, true] {
        let mut model = IrModel::new("review_inline_real".to_owned(), 1).unwrap();
        let mut formal = IrFormal::new(false, 1, false).unwrap();
        formal.real = true;
        formal.shortreal = shortreal;
        model.funcs.push(IrFunc::new(
            "f_real".to_owned(),
            Some(IrType::Real { shortreal }),
            vec![formal],
            vec![],
            vec![],
            vec![IrStmt::Return {
                value: Some(Box::new(IrExpr::new(IrExprKind::FormalRead(0), 0, false, None))),
            }],
        ));
        let ctx = RCtx {
            model: &model,
            func: None,
            sampled: false,
            activation_label: None,
        };
        let mut frame = Frame::new(&ctx);
        frame.read_only_callback = true;
        let arg = IrExpr::new(IrExprKind::Const(IrConst::real(1.25)), 0, false, None);
        let result = frame.expression(&call(0, vec![IrCallArg::Val(arg)], 0)).unwrap();
        let declaration = format!("double {} = 0.0;", result.code);
        let source = frame.body();
        assert!(
            source.find(&declaration).unwrap() < source.find("{\n").unwrap(),
            "real escape owner was declared inside the inline block: {source}"
        );
        assert!(source.contains(&format!("{} = ", result.code)));
        frame.discard(result);
        assert!(frame.slots.iter().all(|used| !used));
    }
}
