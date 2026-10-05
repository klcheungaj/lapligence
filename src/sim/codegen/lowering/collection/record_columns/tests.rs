//! Column-layout selection uses the declared shape, never storage spelling.
use super::*;
use crate::core::db::{AggregateLayout, TypeId};
use crate::core::model::TypeInfo;

fn atom(width: u32, two_state: bool) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(u64::from(width)),
        name: format!("logic [{}:0]", width - 1),
        two_state,
        info: TypeInfo {
            kind: "logic".into(),
            width: Some(width),
            signed: false,
            type_name: None,
        },
        shape: TypeShape::PackedAtom { ranges: Vec::new() },
    }
}

fn array(element: TypeDescriptor, dimensions: Vec<(i32, i32)>) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(1000),
        name: "array".into(),
        two_state: element.two_state,
        info: TypeInfo::default(),
        shape: TypeShape::FixedArray {
            dimensions,
            element: Box::new(element),
        },
    }
}

fn record(members: Vec<(&str, TypeDescriptor)>) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(2000),
        name: "record".into(),
        two_state: false,
        info: TypeInfo::default(),
        shape: TypeShape::Aggregate(AggregateLayout {
            kind: AggregateKind::UnpackedStruct,
            type_identity: Some("record".into()),
            type_id: Some(TypeId(2000)),
            members: members
                .into_iter()
                .map(|(name, descriptor)| AggregateMember {
                    name: name.to_owned(),
                    ty: descriptor.info.clone(),
                    two_state: descriptor.two_state,
                    packed_ranges: Vec::new(),
                    aggregate: None,
                    initializer: None,
                    descriptor,
                })
                .collect(),
        }),
    }
}

#[test]
fn dense_member_arrays_keep_leaf_layout_and_large_ones_use_columns() {
    let dense = record(vec![
        ("a", array(atom(8, false), vec![(0, 4095)])),
        ("t", atom(4, false)),
    ]);
    assert!(!record_column_layout(&dense));
    let large = record(vec![
        ("a", array(atom(8, false), vec![(0, 4096)])),
        ("t", atom(4, false)),
    ]);
    assert!(record_column_layout(&large));
    // Two dimensions count their product, and nesting is transparent.
    let nested = record(vec![(
        "inner",
        record(vec![("m", array(atom(1, true), vec![(63, 0), (0, 64)]))]),
    )]);
    assert!(record_column_layout(&nested));
}

#[test]
fn records_beyond_packed_capacity_use_columns_without_large_arrays() {
    let wide = record(vec![
        ("w", array(atom(1024, false), vec![(0, 2047)])),
        ("t", atom(8, true)),
    ]);
    assert!(record_column_layout(&wide));
    let leaves_only = record(vec![
        ("x", atom(600_000, false)),
        ("y", atom(600_000, false)),
    ]);
    assert!(record_column_layout(&leaves_only));
    assert!(!record_column_layout(&atom(8, false)));
}
