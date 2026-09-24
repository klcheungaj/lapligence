//! Foreach follows the typed element chain, not flattened value descriptors.

use super::*;

fn ty(id: u64, kind: TypeKind, width: u64, element: Option<u64>) -> SlangType {
    SlangType {
        id,
        kind,
        is_signed: false,
        is_four_state: true,
        is_fixed_size: true,
        is_tagged: false,
        bit_width: width,
        display_name: "not used for dimension recovery".into(),
        element_type_id: element,
        index_type_id: None,
        range_start: 0,
        range_count: 0,
        member_start: 0,
        member_count: 0,
    }
}

fn ranged(mut ty: SlangType, index: u64) -> SlangType {
    ty.range_start = index;
    ty.range_count = 1;
    ty
}

fn project(
    types: &[SlangType],
    ranges: &[TypeRange],
    root: u64,
    count: usize,
) -> Result<Vec<Option<(i32, i32)>>, String> {
    SlangTypeProjector {
        constants: &[],
        types: types.iter().map(|ty| (ty.id, ty)).collect(),
        ranges,
        members: &[],
    }
    .foreach_dimensions(root, count)
}

fn range(left: i64, right: i64, kind: TypeRangeKind) -> TypeRange {
    TypeRange { left, right, kind }
}

#[test]
fn foreach_mixed_dimensions_follow_unpacked_then_packed_elements() {
    let types = [
        ty(0, TypeKind::Integral, 1, None),
        ranged(ty(1, TypeKind::PackedArray, 4, Some(0)), 0),
        ranged(ty(2, TypeKind::FixedUnpackedArray, 8, Some(1)), 1),
    ];
    let ranges = [
        range(3, 0, TypeRangeKind::Packed),
        range(0, 1, TypeRangeKind::Unpacked),
    ];
    assert_eq!(
        project(&types, &ranges, 2, 2).unwrap(),
        [Some((0, 1)), Some((3, 0))]
    );
    assert_eq!(project(&types, &ranges, 2, 1).unwrap(), [Some((0, 1))]);
    assert!(project(&types, &ranges, 2, 3)
        .unwrap_err()
        .contains("scalar"));
}

#[test]
fn foreach_mixed_dimensions_preserve_all_bounds_and_requested_prefixes() {
    let types = [
        ty(0, TypeKind::Integral, 1, None),
        ranged(ty(1, TypeKind::PackedArray, 2, Some(0)), 0),
        ranged(ty(2, TypeKind::PackedArray, 8, Some(1)), 1),
        ranged(ty(3, TypeKind::FixedUnpackedArray, 16, Some(2)), 2),
        ranged(ty(4, TypeKind::FixedUnpackedArray, 32, Some(3)), 3),
    ];
    let ranges = [
        range(1, 2, TypeRangeKind::Packed),
        range(3, 0, TypeRangeKind::Packed),
        range(-2, -1, TypeRangeKind::Unpacked),
        range(2, 1, TypeRangeKind::Unpacked),
    ];
    let expected = [Some((2, 1)), Some((-2, -1)), Some((3, 0)), Some((1, 2))];
    for count in 0..=expected.len() {
        assert_eq!(
            project(&types, &ranges, 4, count).unwrap(),
            expected[..count]
        );
    }
    assert_eq!(project(&types, &ranges, 2, 2).unwrap(), expected[2..]);
}

#[test]
fn foreach_mixed_dimensions_keep_implicit_integer_vectors_beneath_packed_arrays() {
    let types = [
        ty(0, TypeKind::Integral, 8, None),
        ranged(ty(1, TypeKind::PackedArray, 16, Some(0)), 0),
        ranged(ty(2, TypeKind::FixedUnpackedArray, 32, Some(1)), 1),
    ];
    let ranges = [
        range(1, 0, TypeRangeKind::Packed),
        range(0, 1, TypeRangeKind::Unpacked),
    ];
    assert_eq!(
        project(&types, &ranges, 2, 3).unwrap(),
        [Some((0, 1)), Some((1, 0)), Some((7, 0))]
    );
    assert!(project(&types, &ranges, 2, 4).is_err());
}

#[test]
fn foreach_mixed_dimensions_do_not_reuse_enum_base_bounds() {
    let types = [
        ty(0, TypeKind::Integral, 1, None),
        ranged(ty(1, TypeKind::PackedArray, 6, Some(0)), 0),
        ty(2, TypeKind::Enum, 6, Some(1)),
        ranged(ty(3, TypeKind::FixedUnpackedArray, 12, Some(2)), 1),
    ];
    let ranges = [
        range(9, 4, TypeRangeKind::Packed),
        range(1, 0, TypeRangeKind::Unpacked),
    ];
    assert_eq!(
        project(&types, &ranges, 3, 2).unwrap(),
        [Some((1, 0)), Some((5, 0))]
    );
    assert!(project(&types, &ranges, 3, 3).is_err());
}

#[test]
fn foreach_mixed_dimensions_treat_packed_records_as_one_integral_dimension() {
    for kind in [
        TypeKind::PackedStruct,
        TypeKind::PackedUnion,
        TypeKind::Enum,
    ] {
        for width in [1, 8] {
            let types = [ty(0, kind, width, None)];
            assert_eq!(
                project(&types, &[], 0, 1).unwrap(),
                [Some((width as i32 - 1, 0))]
            );
            assert!(project(&types, &[], 0, 2).is_err());
        }
    }
}

#[test]
fn foreach_mixed_dimensions_preserve_explicit_singletons_and_int_boundaries() {
    let types = [
        ty(0, TypeKind::Integral, 1, None),
        ranged(ty(1, TypeKind::PackedArray, 1, Some(0)), 0),
        ranged(ty(2, TypeKind::FixedUnpackedArray, 1, Some(1)), 1),
    ];
    let ranges = [
        range(
            i64::from(i32::MIN),
            i64::from(i32::MIN),
            TypeRangeKind::Packed,
        ),
        range(
            i64::from(i32::MAX),
            i64::from(i32::MAX),
            TypeRangeKind::Unpacked,
        ),
    ];
    assert_eq!(
        project(&types, &ranges, 2, 2).unwrap(),
        [Some((i32::MAX, i32::MAX)), Some((i32::MIN, i32::MIN))]
    );
}

#[test]
fn foreach_mixed_dimensions_distinguish_runtime_bounds_from_static_bounds() {
    for kind in [
        TypeKind::DynamicArray,
        TypeKind::Queue,
        TypeKind::AssociativeArray,
    ] {
        let types = [
            ty(0, TypeKind::Integral, 1, None),
            ranged(ty(1, TypeKind::PackedArray, 4, Some(0)), 0),
            ty(2, kind, 0, Some(1)),
        ];
        let ranges = [range(3, 0, TypeRangeKind::Packed)];
        assert_eq!(
            project(&types, &ranges, 2, 2).unwrap(),
            [None, Some((3, 0))]
        );
    }
    let types = [ty(0, TypeKind::String, 0, None)];
    assert_eq!(project(&types, &[], 0, 1).unwrap(), [None]);
    assert!(project(&types, &[], 0, 2).is_err());
}

#[test]
fn foreach_mixed_dimensions_reject_extra_scalar_and_unpacked_record_dimensions() {
    for kind in [
        TypeKind::Integral,
        TypeKind::UnpackedStruct,
        TypeKind::UnpackedUnion,
    ] {
        let types = [
            ty(0, kind, 1, None),
            ranged(ty(1, TypeKind::FixedUnpackedArray, 2, Some(0)), 0),
        ];
        let ranges = [range(0, 1, TypeRangeKind::Unpacked)];
        assert_eq!(project(&types, &ranges, 1, 1).unwrap(), [Some((0, 1))]);
        assert!(project(&types, &ranges, 1, 2).is_err());
    }
}

#[test]
fn foreach_mixed_dimensions_reject_bounds_that_do_not_fit_int_indices() {
    for (left, right) in [(i64::MAX, 0), (0, i64::MIN)] {
        let types = [
            ty(0, TypeKind::Integral, 1, None),
            ranged(ty(1, TypeKind::PackedArray, 4, Some(0)), 0),
        ];
        let ranges = [range(left, right, TypeRangeKind::Packed)];
        assert!(project(&types, &ranges, 1, 1)
            .unwrap_err()
            .contains("bound"));
    }
}

#[test]
fn foreach_mixed_dimensions_reject_cycles_and_excessive_slot_counts() {
    let cyclic = [ranged(ty(0, TypeKind::FixedUnpackedArray, 1, Some(0)), 0)];
    let ranges = [range(0, 0, TypeRangeKind::Unpacked)];
    assert!(project(&cyclic, &ranges, 0, usize::MAX)
        .unwrap_err()
        .contains("cycle"));
    let integral = [ty(0, TypeKind::Integral, 32, None)];
    assert!(project(&integral, &[], 0, usize::MAX)
        .unwrap_err()
        .contains("count"));
}

#[test]
fn foreach_mixed_dimensions_reject_missing_type_links_and_malformed_ranges() {
    let missing = [ranged(ty(0, TypeKind::FixedUnpackedArray, 2, None), 0)];
    let ranges = [range(0, 1, TypeRangeKind::Unpacked)];
    assert!(project(&missing, &ranges, 0, 1).is_err());
    assert!(project(&missing, &ranges, 9, 1).is_err());
    let types = [
        ty(0, TypeKind::Integral, 1, None),
        ranged(ty(1, TypeKind::PackedArray, 2, Some(0)), 0),
    ];
    assert!(project(&types, &ranges, 1, 1).is_err());
    assert!(project(&types, &[], 1, 1).is_err());
    for width in [0, u64::MAX] {
        assert!(project(&[ty(0, TypeKind::PackedStruct, width, None)], &[], 0, 1).is_err());
    }
}
