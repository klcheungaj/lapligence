use super::{ir_to_explicit_cast_storage, parse_decimal_real_literal};
use crate::sim::codegen::lowering::ir_to_storage;
use crate::sim::ir::{
    IrExpr, IrExprKind, IrLhs, IrModel, IrProcess, IrShape, IrSignal, IrStmt, IrType,
};
use crate::sim::opt::{self, OptConfig};

#[test]
fn explicit_cast_materializes_unsized_fill_at_target_width() {
    let fill = IrExpr::new(IrExprKind::Fill(1), 1, false, Some(1));
    let cast = ir_to_explicit_cast_storage(fill, 1, false, false).unwrap();

    assert_eq!(cast.width(), 1);
    assert_eq!(cast.fill(), None);
    assert!(matches!(
        cast.kind(),
        IrExprKind::Const(value)
            if value.bits() == [1]
                && value.x_mask() == [0]
                && value.z_mask() == [0]
    ));
}

#[test]
fn folded_real_comparison_accepts_only_decimal_numeric_tokens() {
    assert_eq!(parse_decimal_real_literal("1.5"), Some(1.5));
    assert_eq!(parse_decimal_real_literal("-2_000e-3"), Some(-2.0));
    assert_eq!(parse_decimal_real_literal("42"), Some(42.0));
    for rejected in ["NaN", "inf", "+inf", "1.", ".5", "1__0", "1e", "8'h1"] {
        assert_eq!(parse_decimal_real_literal(rejected), None, "{rejected}");
    }
}

#[test]
fn narrow_fill_cast_stays_materialized_when_assigned_wider() {
    let build = || {
        let fill = IrExpr::new(IrExprKind::Fill(1), 1, false, Some(1));
        let cast = ir_to_explicit_cast_storage(fill, 1, false, false).unwrap();
        let rhs = ir_to_storage(cast, 8, false, false).unwrap();
        let mut model = IrModel::new("cast".to_string(), 1).unwrap();
        model.signals.push(
            IrSignal::new(
                "value".to_string(),
                None,
                IrType::packed(8, false).unwrap(),
                None,
            )
            .unwrap(),
        );
        model.processes.push(IrProcess::new(
            "proc".to_string(),
            "cast.initial".to_string(),
            IrShape::RunOnce,
            Vec::new(),
            vec![IrStmt::Assign {
                lhs: IrLhs::Whole(0),
                rhs,
                nba: false,
            }],
        ));
        model
    };

    for config in [OptConfig::none(), OptConfig::default()] {
        let mut model = build();
        opt::run_ir(&mut model, &config);
        let IrStmt::Assign { rhs, .. } = &model.processes[0].body[0] else {
            panic!("optimizer replaced the cast assignment")
        };
        assert_eq!(rhs.fill(), None);
        if let IrExprKind::Const(value) = rhs.kind() {
            assert_eq!(value.width(), 8);
            assert_eq!(value.bits(), [1]);
            assert_eq!(value.fill(), None);
        }
    }
}
