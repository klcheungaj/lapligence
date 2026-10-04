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

#[test]
fn structure_conditional_plan_keeps_member_boundaries_and_uninitialized_defaults() {
    let descriptor = TypeDescriptor {
        id: TypeId(4),
        name: "record".into(),
        two_state: false,
        info: TypeInfo::default(),
        shape: TypeShape::Aggregate(AggregateLayout {
            kind: AggregateKind::UnpackedStruct,
            type_identity: None,
            type_id: None,
            members: vec![
                member("byte", atom(8, false), 0xa5),
                member("flag", atom(1, true), 1),
                member("nibble", atom(4, false), 2),
            ],
        }),
    };
    let members = structure_merge_members(&descriptor).unwrap();
    assert_eq!(members.len(), 3);
    assert_eq!((members[0].offset, members[0].width), (0, 4));
    assert_eq!(members[0].default.x, vec![0xf]);
    assert_eq!((members[1].offset, members[1].width), (4, 1));
    assert_eq!(members[1].default.bits, vec![0]);
    assert_eq!((members[2].offset, members[2].width), (5, 8));
    assert_eq!(members[2].default.x, vec![0xff]);
}

#[test]
fn structure_conditional_plan_rejects_native_members() {
    let mut native = atom(8, false);
    native.shape = TypeShape::String;
    let descriptor = TypeDescriptor {
        id: TypeId(5),
        name: "record".into(),
        two_state: false,
        info: TypeInfo::default(),
        shape: TypeShape::Aggregate(AggregateLayout {
            kind: AggregateKind::UnpackedStruct,
            type_identity: None,
            type_id: None,
            members: vec![member("native", native, 0)],
        }),
    };
    assert!(structure_merge_members(&descriptor).is_err());
}

fn predicate_literal(value: ValueData, size: i32) -> ExprKind {
    ExprKind::Constant {
        const_type: if matches!(&value, ValueData::Real(_)) {
            ConstantType::Real
        } else {
            ConstantType::Binary
        },
        value,
        size,
        source: crate::core::db::ConstantSource::NotCaptured,
        time_scale: None,
    }
}

fn predicate_database(first: ExprKind, second: ExprKind, left: ExprKind, right: ExprKind) -> Db {
    use crate::core::db::{ConditionalPredicate, Node, PredicateClause};
    let predicate = ConditionalPredicate {
        clauses: vec![
            PredicateClause {
                expression: NodeId(1),
                pattern: None,
            },
            PredicateClause {
                expression: NodeId(2),
                pattern: None,
            },
        ],
    };
    let root = ExprKind::Conditional {
        predicate,
        if_true: NodeId(3),
        if_false: NodeId(4),
    };
    let nodes = [root, first, second, left, right]
        .into_iter()
        .map(|expression| Node {
            kind: NodeKind::Expr(expression),
            children: Vec::new(),
            parent: None,
            name: String::new(),
            full_name: "".into(),
            file: None,
            line: 0,
            col: 0,
            end_line: 0,
            end_col: 0,
        })
        .collect();
    Db::from_test_nodes("predicate", nodes, vec![], std::collections::HashMap::new()).unwrap()
}

#[test]
fn sequential_predicate_constant_evaluation_skips_unreached_clauses_and_arms() {
    let db = predicate_database(
        predicate_literal(ValueData::Bin("0".into()), 1),
        ExprKind::Other,
        ExprKind::Other,
        predicate_literal(ValueData::UInt(0xa6), 8),
    );
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    assert_eq!(cg.eval_bits(NodeId(0)).unwrap().to_u64(), Some(0xa6));
    let Val::Bits(value) = cg.eval_decl_value(NodeId(0)).unwrap() else {
        panic!("packed conditional expected");
    };
    assert_eq!(value.to_u64(), Some(0xa6));
}

#[test]
fn sequential_predicate_constant_evaluation_stops_at_ambiguity_before_false() {
    for second in [
        predicate_literal(ValueData::Bin("0".into()), 1),
        ExprKind::Other,
    ] {
        let db = predicate_database(
            predicate_literal(ValueData::Bin("z".into()), 1),
            second,
            predicate_literal(ValueData::UInt(0xa5), 8),
            predicate_literal(ValueData::UInt(0xa6), 8),
        );
        let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
        let cg = Codegen::new(&semantic);
        let expected = val_from_value_data(&ValueData::Bin("101001xx".into()), 8).unwrap();
        let Val::Bits(expected) = expected else {
            panic!("packed oracle expected");
        };
        assert_eq!(cg.eval_bits(NodeId(0)).unwrap().bits, expected.bits);
        let Val::Bits(value) = cg.eval_decl_value(NodeId(0)).unwrap() else {
            panic!("packed conditional expected");
        };
        assert_eq!(value.bits, expected.bits);
    }
}

#[test]
fn sequential_predicate_constant_evaluation_handles_real_truth_and_ambiguous_results() {
    let db = predicate_database(
        predicate_literal(ValueData::Bin("1".into()), 1),
        predicate_literal(ValueData::Real(0.25), 0),
        predicate_literal(ValueData::UInt(0xa5), 8),
        ExprKind::Other,
    );
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    assert_eq!(
        Codegen::new(&semantic)
            .eval_bits(NodeId(0))
            .unwrap()
            .to_u64(),
        Some(0xa5)
    );
    for right in [predicate_literal(ValueData::Real(3.5), 0), ExprKind::Other] {
        let is_poison = matches!(&right, ExprKind::Other);
        let db = predicate_database(
            predicate_literal(ValueData::Bin("x".into()), 1),
            ExprKind::Other,
            predicate_literal(ValueData::Real(2.5), 0),
            right,
        );
        let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
        let result = Codegen::new(&semantic).eval_decl_value(NodeId(0));
        if is_poison {
            assert!(
                result.is_err(),
                "both ambiguous alternatives must be evaluated"
            );
        } else {
            assert!(matches!(result, Ok(Val::Real(value)) if value == 0.0));
        }
    }
}
