use super::*;
use crate::sim::ir::{IrExpr, IrExprKind, IrModel, IrPreFn, IrStmt};

#[test]
fn detached_fragments_reject_missing_storage_before_rendering() {
    let model = IrModel::new("empty".to_owned(), 1).unwrap();
    let ctx = RCtx {
        model: &model,
        func: None,
    };
    let expression = IrExpr::try_new(IrExprKind::SigRead(7), 1, false, None).unwrap();
    let error = match render_expr(&ctx, &expression) {
        Ok(_) => panic!("missing signal was rendered"),
        Err(error) => error,
    };
    assert!(matches!(error, EmitError::InvalidIr(_)));
    assert!(std::error::Error::source(&error).is_some());

    let statement = IrStmt::If {
        cond: expression.clone(),
        then_: Vec::new(),
        els: None,
    };
    assert!(matches!(
        render_stmt(&ctx, &statement),
        Err(EmitError::InvalidIr(_))
    ));
    let helper = IrPreFn::MonEval {
        c_name: "monitor".to_owned(),
        args: vec![expression],
    };
    assert!(matches!(
        render_pre_fn(&ctx, &helper),
        Err(EmitError::InvalidIr(_))
    ));
}

#[test]
fn model_render_rejects_invalid_internal_precision() {
    let mut model = IrModel::new("empty".to_owned(), 1).unwrap();
    model.precision_ps = 0;
    assert!(matches!(render(&model), Err(EmitError::InvalidIr(_))));
}

#[test]
fn output_temporary_uses_its_declared_formal_after_c_argument_reordering() {
    use crate::sim::ir::{
        IrCallArg, IrCallExpr, IrConst, IrDepth, IrFormal, IrFunc, IrLhs, IrModelParts, IrType,
    };
    let function = IrFunc::new(
        "mixed".to_owned(),
        Some(IrType::Packed {
            width: 1,
            signed: false,
        }),
        vec![
            IrFormal::new(false, 8, false).unwrap(),
            IrFormal::new(true, 16, true).unwrap(),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let model = IrModel::from_parts(
        "mixed".to_owned(),
        1,
        IrModelParts {
            funcs: vec![function],
            ..Default::default()
        },
    )
    .unwrap();
    let input = IrExpr::try_new(
        IrExprKind::Const(
            IrConst::packed(vec![1], Vec::new(), Vec::new(), 8, false, None).unwrap(),
        ),
        8,
        false,
        None,
    )
    .unwrap();
    let call = IrCallExpr::new(
        0,
        vec![
            IrCallArg::OutTemp {
                name: "_out".to_owned(),
                init: None,
                writeback: Box::new(IrLhs::WholeRef {
                    addr: "&target".to_owned(),
                    width: 16,
                    signed: true,
                }),
            },
            IrCallArg::Val(input),
        ],
        IrDepth::PROC,
        false,
    );
    let expression = IrExpr::try_new(IrExprKind::CallFn(Box::new(call)), 1, false, None).unwrap();
    let rendered = render_expr(
        &RCtx {
            model: &model,
            func: None,
        },
        &expression,
    )
    .unwrap();
    assert!(
        rendered.code.contains("sv4_t _out = sv4_x(16, 1)"),
        "{}",
        rendered.code
    );
}
