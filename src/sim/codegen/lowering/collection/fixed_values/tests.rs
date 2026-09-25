//! Descriptor offsets preserve nominal leaf types across nested fixed layouts.
use super::*;
use crate::core::db::{AggregateLayout, TypeId};
use crate::core::model::TypeInfo;

fn atom(id: u64, width: u32, signed: bool, two_state: bool) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(id),
        name: format!("leaf_{id}"),
        two_state,
        info: TypeInfo {
            kind: "logic".into(),
            width: Some(width),
            signed,
            type_name: None,
        },
        shape: TypeShape::PackedAtom { ranges: Vec::new() },
    }
}

fn array(id: u64, element: TypeDescriptor, dimensions: Vec<(i32, i32)>) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(id),
        name: format!("array_{id}"),
        two_state: element.two_state,
        info: TypeInfo::default(),
        shape: TypeShape::FixedArray {
            dimensions,
            element: Box::new(element),
        },
    }
}

fn aggregate(id: u64, kind: AggregateKind, members: Vec<(&str, TypeDescriptor)>) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(id),
        name: format!("aggregate_{id}"),
        two_state: members.iter().all(|(_, member)| member.two_state),
        info: TypeInfo::default(),
        shape: TypeShape::Aggregate(AggregateLayout {
            kind,
            type_identity: None,
            type_id: Some(TypeId(id)),
            members: members
                .into_iter()
                .map(|(name, descriptor)| AggregateMember {
                    name: name.to_owned(),
                    ty: descriptor.info.clone(),
                    two_state: descriptor.two_state,
                    packed_ranges: Vec::new(),
                    aggregate: None,
                    descriptor,
                    initializer: None,
                })
                .collect(),
        }),
    }
}

#[test]
fn nested_record_row_offsets_and_nominal_leaf_types_cover_limb_boundaries() {
    for width in [1, 7, 8, 31, 32, 33, 63, 64, 65, 129] {
        let leaf = atom(1, width, true, false);
        let row = array(2, leaf.clone(), vec![(3, 1)]);
        let flag = atom(3, 7, false, true);
        let record = aggregate(
            4,
            AggregateKind::UnpackedStruct,
            vec![
                ("head", atom(5, 1, false, false)),
                ("lanes", row),
                ("flag", flag.clone()),
            ],
        );
        let record_width = 3 * width + 8;
        let matrix = array(6, record, vec![(2, 1), (-1, 1)]);
        assert_eq!(fixed_width(&matrix), Some(6 * record_width));
        let coordinates = [(2, -1), (2, 0), (2, 1), (1, -1), (1, 0), (1, 1)];
        for (ordinal, (outer, inner)) in coordinates.into_iter().enumerate() {
            let record_base = (5 - ordinal as u32) * record_width;
            for (lane, displacement) in [(3, 2 * width), (2, width), (1, 0)] {
                let (actual, offset) = fixed_path_descriptor(
                    &matrix,
                    &[
                        AggregatePathPart::Index(outer),
                        AggregatePathPart::Index(inner),
                        AggregatePathPart::Member("lanes".into()),
                        AggregatePathPart::Index(lane),
                    ],
                )
                .unwrap();
                assert_eq!(offset, record_base + 7 + displacement);
                assert_eq!(
                    actual, leaf,
                    "shape-compatible leaves must not lose nominal identity"
                );
            }
            let (actual, offset) = fixed_path_descriptor(
                &matrix,
                &[
                    AggregatePathPart::Index(outer),
                    AggregatePathPart::Index(inner),
                    AggregatePathPart::Member("flag".into()),
                ],
            )
            .unwrap();
            assert_eq!(offset, record_base);
            assert_eq!(actual, flag);
        }
        assert!(fixed_path_descriptor(&matrix, &[AggregatePathPart::Index(0)]).is_none());
        assert!(fixed_path_descriptor(
            &matrix,
            &[AggregatePathPart::Index(2), AggregatePathPart::Index(2),]
        )
        .is_none());
    }
}

#[test]
fn equal_width_union_members_share_storage_but_keep_their_types() {
    for kind in [AggregateKind::PackedUnion, AggregateKind::UnpackedUnion] {
        let unsigned = atom(7, 129, false, false);
        let signed = atom(8, 129, true, false);
        let union = aggregate(
            9,
            kind,
            vec![("raw", unsigned.clone()), ("signed_view", signed.clone())],
        );
        assert_eq!(fixed_width(&union), Some(129));
        for (name, expected) in [("raw", unsigned), ("signed_view", signed)] {
            let (actual, offset) =
                fixed_path_descriptor(&union, &[AggregatePathPart::Member(name.into())]).unwrap();
            assert_eq!(offset, 0);
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn fixed_layout_capacity_and_native_leaf_gates_stay_separate() {
    let huge = array(10, atom(11, 129, false, false), vec![(0, 65535)]);
    assert_eq!(fixed_width_bits(&huge), Some(129 * 65536));
    assert!(fixed_width(&huge).is_none());
    let overflow = array(
        12,
        atom(13, 129, false, false),
        vec![(i32::MIN, i32::MAX); 3],
    );
    assert!(fixed_width_bits(&overflow).is_none());
    let mut native = atom(14, 129, false, false);
    native.shape = TypeShape::String;
    let mixed = aggregate(15, AggregateKind::UnpackedStruct, vec![("native", native)]);
    assert!(fixed_width_bits(&mixed).is_none());
}
