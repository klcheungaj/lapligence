//! The fold sees immediate elements and a self-determined map result.
use super::*;
use crate::core::db::{AggregateLayout, TypeId};
use crate::core::model::TypeInfo;

fn atom(width: u32, signed: bool, two_state: bool) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(1),
        name: "element".into(),
        info: TypeInfo {
            kind: "logic".into(),
            width: Some(width),
            signed,
            type_name: None,
        },
        two_state,
        shape: TypeShape::PackedAtom { ranges: vec![] },
    }
}

fn array(element: TypeDescriptor, dimensions: Vec<(i32, i32)>) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(2),
        name: "array".into(),
        info: TypeInfo::default(),
        two_state: element.two_state,
        shape: TypeShape::FixedArray {
            dimensions,
            element: Box::new(element),
        },
    }
}

#[test]
fn fixed_array_reduction_preserves_signed_and_two_state_elements() {
    let element = atom(65, true, true);
    let descriptor = array(element.clone(), vec![(-3, -1)]);
    let (left, right, actual) = reduction_element(&descriptor).unwrap();
    assert_eq!((left, right), (-3, -1));
    assert_eq!(actual, element);
    assert!(reduction_integral(&actual));
    assert_eq!(Codegen::fixed_descriptor_width(&actual), Some(65));
}

#[test]
fn fixed_array_reduction_peels_one_unpacked_dimension_not_the_leaf() {
    let descriptor = array(atom(7, true, false), vec![(3, 2), (-1, 1)]);
    let (left, right, row) = reduction_element(&descriptor).unwrap();
    assert_eq!((left, right), (3, 2));
    assert!(!reduction_integral(&row));
    assert!(!row.info.signed);
    assert_eq!(Codegen::fixed_descriptor_width(&row), Some(21));
    let (left, right, element) = reduction_element(&row).unwrap();
    assert_eq!((left, right), (-1, 1));
    assert!(element.info.signed);
    assert_eq!(Codegen::fixed_descriptor_width(&element), Some(7));
    let nested = array(array(atom(7, true, false), vec![(-1, 1)]), vec![(3, 2)]);
    let (_, _, row) = reduction_element(&nested).unwrap();
    assert_eq!(Codegen::fixed_descriptor_width(&row), Some(21));
}

#[test]
fn fixed_array_reduction_bounds_include_both_signed_extremes() {
    for bounds in [(i32::MIN, i32::MAX), (i32::MAX, i32::MIN), (-1, -1)] {
        let (left, right, _) = reduction_element(&array(atom(1, false, false), vec![bounds])).unwrap();
        assert_eq!((left, right), bounds);
        let count = i64::from(left).abs_diff(i64::from(right)) + 1;
        assert_eq!(count, if left == right { 1 } else { 1u64 << 32 });
    }
}

#[test]
fn fixed_array_reduction_distinguishes_packed_records_and_unpacked_maps() {
    let mut descriptor = atom(8, false, false);
    for kind in [AggregateKind::PackedStruct, AggregateKind::PackedUnion, AggregateKind::UnpackedStruct] {
        descriptor.shape = TypeShape::Aggregate(AggregateLayout {
            kind,
            type_identity: None,
            type_id: None,
            members: vec![],
        });
        assert_eq!(reduction_integral(&descriptor), !matches!(kind, AggregateKind::UnpackedStruct));
    }
}

#[test]
fn fixed_array_reduction_rejects_missing_dimensions_and_native_payloads() {
    assert!(reduction_element(&atom(8, false, false)).is_err());
    assert!(reduction_element(&array(atom(8, false, false), vec![])).is_err());
    let mut native = atom(8, false, false);
    native.shape = TypeShape::String;
    assert!(!reduction_integral(&native));
    assert_eq!(Codegen::fixed_descriptor_width(&native), None);
}

#[test]
fn fixed_array_reduction_assignment_context_does_not_widen_the_accumulator() {
    let expr = IrExpr::new(
        IrExprKind::FixedArrayReduce(Box::new(IrFixedArrayReduction {
            source: IrFixedArrayReductionSource::Array(0),
            operation: IrContainerReduction::Sum,
            left: 0,
            right: 1,
            element_width: 8,
            element_signed: false,
            element_two_state: false,
            item_name: "item".into(),
            index_name: "index".into(),
            value: IrExpr::new(IrExprKind::LocalRead("item".into()), 8, false, None),
        })),
        8,
        false,
        None,
    );
    let widened = apply_assignment_expression_width(expr.clone(), 64);
    assert_eq!(widened, expr);
    let converted = ir_to_storage(widened, 64, true, false).unwrap();
    assert_eq!(converted.width, 64);
    let IrExprKind::Convert { a } = converted.kind else {
        panic!("conversion must be outside the fold");
    };
    assert_eq!(*a, expr);
}
