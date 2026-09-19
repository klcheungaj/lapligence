//! Coordinate normalization before C emission; runtime clipping has a separate oracle.
use super::*;
use crate::sim::opt::{run_ir, OptConfig};

fn folded_offset(index: IrExpr, range: PackedRange, stride: u32, back: u32) -> i128 {
    let value = packed_lsb(index, range, stride, back).unwrap();
    let mut model = IrModel::new("coordinate_test".to_owned(), 1).unwrap();
    model.signals.push(
        IrSignal::new(
            "result".to_owned(),
            None,
            IrType::Packed {
                width: value.width,
                signed: true,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    model.processes.push(IrProcess::new(
        "p".to_owned(),
        "p".to_owned(),
        IrShape::RunOnce,
        Vec::new(),
        vec![IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: value,
            nba: false,
        }],
    ));
    run_ir(
        &mut model,
        &OptConfig {
            fold_constants: true,
            identities: false,
            prune_branches: false,
            unused_storage: false,
        },
    );
    let IrStmt::Assign { rhs, .. } = &model.processes[0].body[0] else {
        panic!("assignment lost")
    };
    let IrExprKind::Const(constant) = &rhs.kind else {
        panic!("offset did not fold: {rhs:?}")
    };
    assert!(constant.width < 128 && constant.signed);
    assert!(constant.x.iter().chain(&constant.z).all(|limb| *limb == 0));
    let low = u128::from(constant.bits.first().copied().unwrap_or(0));
    let high = u128::from(constant.bits.get(1).copied().unwrap_or(0)) << 64;
    let raw = low | high;
    let extended = if raw & (1u128 << (constant.width - 1)) != 0 {
        raw | (u128::MAX << constant.width)
    } else {
        raw
    };
    extended as i128
}

#[test]
fn packed_coordinates_preserve_direction_and_negative_values() {
    let ascending = PackedRange { left: 0, right: 7 };
    let descending = PackedRange { left: 7, right: 0 };
    // +: on ascending, -: on descending subtract count-1 from physical base.
    assert_eq!(folded_offset(lhs_integer_expr(2), ascending, 1, 2), 3);
    assert_eq!(folded_offset(lhs_integer_expr(4), ascending, 1, 0), 3);
    assert_eq!(folded_offset(lhs_integer_expr(2), descending, 1, 0), 2);
    assert_eq!(folded_offset(lhs_integer_expr(4), descending, 1, 2), 2);
    assert_eq!(
        folded_offset(
            lhs_integer_expr(-1),
            PackedRange { left: -2, right: 5 },
            1,
            2
        ),
        4
    );
    assert_eq!(
        folded_offset(
            lhs_integer_expr(0),
            PackedRange { left: 12, right: 5 },
            1,
            2
        ),
        -7
    );
    assert_eq!(
        folded_offset(lhs_integer_expr(1), PackedRange { left: 3, right: 0 }, 8, 0),
        8
    );
    assert_eq!(
        folded_offset(lhs_integer_expr(2), PackedRange { left: 0, right: 3 }, 8, 0),
        8
    );
}

#[test]
fn packed_coordinates_never_truncate_unsigned_indices_or_expand_fill_one() {
    let range = PackedRange { left: 7, right: 0 };
    assert_eq!(
        folded_offset(lhs_integer_expr(i128::from(u64::MAX)), range, 8, 0),
        i128::from(u64::MAX) * 8
    );
    let one = IrExpr::new(
        IrExprKind::Const(IrConst::packed(vec![1], vec![], vec![], 1, false, Some(1)).unwrap()),
        1,
        false,
        Some(1),
    );
    assert_eq!(folded_offset(one, range, 1, 0), 1);
    let huge = packed_lsb(lhs_integer_expr(i128::MAX), range, 8, 0).unwrap();
    assert!(huge.width > 128 && huge.signed);
}

#[test]
fn packed_dimensions_keep_residual_element_stride() {
    let (_, stride) = packed_dimension(64, Some(PackedRange { left: 1, right: 0 })).unwrap();
    assert_eq!(stride, 32);
    let (_, stride) = packed_dimension(stride, Some(PackedRange { left: 3, right: 0 })).unwrap();
    assert_eq!(stride, 8);
    assert_eq!(packed_selection_width(2, stride).unwrap(), 16);
    let (range, stride) = packed_dimension(32, None).unwrap();
    assert_eq!((range.left, range.right, stride), (31, 0, 1));
    assert!(packed_dimension(1, None).is_err());
    assert!(packed_dimension(9, Some(PackedRange { left: 3, right: 0 })).is_err());
    assert!(packed_selection_width(0, 8).is_err());
    assert!(packed_selection_width(u128::MAX, 8).is_err());
    assert!(packed_selection_width(u128::from(LLG_MAX_WIDTH) + 1, 1).is_err());
}
