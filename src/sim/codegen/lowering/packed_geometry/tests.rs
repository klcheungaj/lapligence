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
    let outer = PackedSelectDim::new(64, Some(PackedRange { left: 1, right: 0 })).unwrap();
    assert_eq!(outer.stride, 32);
    let inner =
        PackedSelectDim::new(outer.stride, Some(PackedRange { left: 3, right: 0 })).unwrap();
    assert_eq!(inner.stride, 8);
    assert_eq!(inner.indexed_width(2).unwrap(), 16);
    let atom = PackedSelectDim::new(32, None).unwrap();
    assert_eq!((atom.range.left, atom.range.right, atom.stride), (31, 0, 1));
    assert!(PackedSelectDim::new(1, None).is_err());
    assert!(PackedSelectDim::new(9, Some(PackedRange { left: 3, right: 0 })).is_err());
    assert!(inner.indexed_width(0).is_err());
    assert!(atom.indexed_width(LLG_MAX_WIDTH + 1).is_err());
    assert!(inner.indexed_width(u32::MAX).is_err());
}

/// Element labels map to whole elements of the outer dimension: hand-derived
/// from IEEE 1800-2009 7.4.5 for `logic [3:0][7:0]`, `logic [0:3][7:0]` and
/// `logic [4:1][3:0]`.
#[test]
fn range_selects_cover_whole_elements_in_both_directions() {
    let descending = PackedSelectDim::new(32, Some(PackedRange { left: 3, right: 0 })).unwrap();
    assert_eq!(descending.part(3, 2).unwrap(), (16, 16));
    assert_eq!(descending.part(1, 0).unwrap(), (0, 16));
    assert_eq!(descending.element(2).unwrap(), (16, 8));
    assert_eq!(descending.indexed(2, 2, false).unwrap(), (16, 16));
    assert_eq!(descending.indexed(2, 2, true).unwrap(), (8, 16));
    // Out-of-range labels keep their offsets so runtime clipping can apply.
    assert_eq!(descending.indexed(3, 2, false).unwrap(), (24, 16));
    assert_eq!(descending.indexed(0, 2, true).unwrap(), (-8, 16));
    assert!(descending.part(2, 3).is_err());

    let ascending = PackedSelectDim::new(32, Some(PackedRange { left: 0, right: 3 })).unwrap();
    assert_eq!(ascending.part(0, 1).unwrap(), (16, 16));
    assert_eq!(ascending.element(3).unwrap(), (0, 8));
    assert_eq!(ascending.indexed(1, 2, false).unwrap(), (8, 16));
    assert_eq!(ascending.indexed(2, 2, true).unwrap(), (8, 16));
    assert!(ascending.part(1, 0).is_err());

    let offset = PackedSelectDim::new(16, Some(PackedRange { left: 4, right: 1 })).unwrap();
    assert_eq!(offset.part(3, 2).unwrap(), (4, 8));
    assert_eq!(offset.indexed(4, 2, true).unwrap(), (8, 8));
    assert_eq!(offset.element_offset(0), Some(-1));

    // A one-dimensional vector keeps one bit per label.
    let vector = PackedSelectDim::new(8, Some(PackedRange { left: 7, right: 0 })).unwrap();
    assert_eq!(vector.part(5, 2).unwrap(), (2, 4));
    assert_eq!(vector.indexed(1, 3, false).unwrap(), (1, 3));
}
