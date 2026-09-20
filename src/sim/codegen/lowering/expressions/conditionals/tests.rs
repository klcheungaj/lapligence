//! Plans use the immediate element, not the terminal packed leaf or initializer.
use super::*;
use crate::core::db::{AggregateLayout, TypeId};
use crate::core::model::TypeInfo;

fn atom(width: u32, two_state: bool) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(1),
        name: "element".into(),
        two_state,
        info: TypeInfo {
            kind: "logic".into(),
            width: Some(width),
            signed: false,
            type_name: None,
        },
        shape: TypeShape::PackedAtom { ranges: vec![] },
    }
}

fn array(element: TypeDescriptor, dimensions: Vec<(i32, i32)>) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(2),
        name: "array".into(),
        two_state: element.two_state,
        info: TypeInfo::default(),
        shape: TypeShape::FixedArray {
            dimensions,
            element: Box::new(element),
        },
    }
}

fn member(name: &str, descriptor: TypeDescriptor, initializer: u64) -> AggregateMember {
    AggregateMember {
        name: name.into(),
        ty: descriptor.info.clone(),
        two_state: descriptor.two_state,
        packed_ranges: vec![],
        aggregate: None,
        descriptor,
        initializer: Some(ValueData::UInt(initializer)),
    }
}

#[test]
fn array_conditional_plan_keeps_whole_immediate_rows() {
    let descriptor = array(atom(7, false), vec![(2, 1), (-1, 0)]);
    let (width, default) = array_merge_default(&descriptor).unwrap();
    assert_eq!(width, 28);
    assert_eq!(default.width, 14);
    assert_eq!(default.x, vec![0x3fff]);
    let nested = array(array(atom(7, false), vec![(-1, 0)]), vec![(2, 1)]);
    let (nested_width, nested_default) = array_merge_default(&nested).unwrap();
    assert_eq!(nested_width, width);
    assert_eq!(nested_default, default);
}

#[test]
fn array_conditional_defaults_ignore_explicit_member_initializers() {
    let descriptor = TypeDescriptor {
        id: TypeId(3),
        name: "record".into(),
        two_state: false,
        info: TypeInfo::default(),
        shape: TypeShape::Aggregate(AggregateLayout {
            kind: AggregateKind::UnpackedStruct,
            type_identity: None,
            type_id: None,
            members: vec![
                member("state", atom(8, true), 7),
                member("data", atom(8, false), 0xa5),
            ],
        }),
    };
    let storage = Codegen::fixed_descriptor_default(&descriptor).unwrap();
    assert_eq!(storage.bits, vec![0x07a5]);
    assert_eq!(storage.x, vec![0]);
    let (width, default) = array_merge_default(&array(descriptor, vec![(0, 1)])).unwrap();
    assert_eq!(width, 32);
    assert_eq!(default.width, 16);
    assert_eq!(default.bits, vec![0]);
    assert_eq!(default.x, vec![0x00ff]);
    assert_eq!(default.z, vec![0]);
}

#[test]
fn array_conditional_plan_defaults_two_state_elements_to_zero() {
    let (width, default) = array_merge_default(&array(atom(65, true), vec![(-1, -2)])).unwrap();
    assert_eq!(width, 130);
    assert_eq!(default.width, 65);
    assert_eq!(default.bits, vec![0, 0]);
    assert_eq!(default.x, vec![0, 0]);
}

#[test]
fn array_conditional_plan_rejects_empty_dimensions_and_nonfixed_elements() {
    assert!(array_merge_default(&array(atom(8, false), vec![])).is_err());
    let mut element = atom(8, false);
    element.shape = TypeShape::String;
    assert!(array_merge_default(&array(element, vec![(0, 1)])).is_err());
}
