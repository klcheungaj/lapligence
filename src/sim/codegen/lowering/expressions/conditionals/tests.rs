//! Plans merge leaves with their default-uninitialized values, not initializers.
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

fn record(members: Vec<AggregateMember>) -> TypeDescriptor {
    TypeDescriptor {
        id: TypeId(4),
        name: "record".into(),
        two_state: false,
        info: TypeInfo::default(),
        shape: TypeShape::Aggregate(AggregateLayout {
            kind: AggregateKind::UnpackedStruct,
            type_identity: None,
            type_id: None,
            members,
        }),
    }
}

fn uniform(merge: &AggregateMerge) -> &IrConst {
    match &merge.leaves {
        AggregateLeaves::Uniform(default) => default,
        AggregateLeaves::Mixed(_) => panic!("expected uniform leaves"),
    }
}

fn mixed(merge: &AggregateMerge) -> &IrLeafMerge {
    match &merge.leaves {
        AggregateLeaves::Mixed(leaves) => leaves,
        AggregateLeaves::Uniform(_) => panic!("expected a record leaf layout"),
    }
}

#[test]
fn array_conditional_plan_merges_innermost_elements() {
    let descriptor = array(atom(7, false), vec![(2, 1), (-1, 0)]);
    let merge = aggregate_conditional_merge(&descriptor).unwrap().unwrap();
    assert_eq!(merge.width, 28);
    assert_eq!(uniform(&merge).width, 7);
    assert_eq!(uniform(&merge).x, vec![0x7f]);
    let nested = array(array(atom(7, false), vec![(-1, 0)]), vec![(2, 1)]);
    let nested = aggregate_conditional_merge(&nested).unwrap().unwrap();
    assert_eq!(nested.width, merge.width);
    assert_eq!(uniform(&nested), uniform(&merge));
}

#[test]
fn array_conditional_defaults_ignore_explicit_member_initializers() {
    let descriptor = record(vec![
        member("state", atom(8, true), 7),
        member("data", atom(8, false), 0xa5),
    ]);
    let storage = Codegen::fixed_descriptor_default(&descriptor).unwrap();
    assert_eq!(storage.bits, vec![0x07a5]);
    assert_eq!(storage.x, vec![0]);
    let merge = aggregate_conditional_merge(&array(descriptor, vec![(0, 1)]))
        .unwrap()
        .unwrap();
    assert_eq!(merge.width, 32);
    let leaves = mixed(&merge);
    assert_eq!(leaves.default.width, 16);
    assert_eq!(leaves.default.bits, vec![0]);
    assert_eq!(leaves.default.x, vec![0x00ff]);
    assert_eq!(leaves.default.z, vec![0]);
    assert_eq!(leaves.starts.bits, vec![0x0101]);
}

#[test]
fn array_conditional_plan_defaults_two_state_elements_to_zero() {
    let merge = aggregate_conditional_merge(&array(atom(65, true), vec![(-1, -2)]))
        .unwrap()
        .unwrap();
    assert_eq!(merge.width, 130);
    assert_eq!(uniform(&merge).width, 65);
    assert_eq!(uniform(&merge).bits, vec![0, 0]);
    assert_eq!(uniform(&merge).x, vec![0, 0]);
}

#[test]
fn array_conditional_plan_rejects_empty_dimensions_and_nonfixed_elements() {
    assert!(aggregate_conditional_merge(&array(atom(8, false), vec![])).is_err());
    let mut element = atom(8, false);
    element.shape = TypeShape::String;
    assert!(aggregate_conditional_merge(&array(element, vec![(0, 1)])).is_err());
    assert!(aggregate_conditional_merge(&atom(8, false))
        .unwrap()
        .is_none());
}

#[test]
fn structure_conditional_plan_marks_leaves_and_uninitialized_defaults() {
    let descriptor = record(vec![
        member("byte", atom(8, false), 0xa5),
        member("flag", atom(1, true), 1),
        member("nibble", atom(4, false), 2),
    ]);
    let merge = aggregate_conditional_merge(&descriptor).unwrap().unwrap();
    assert_eq!(merge.width, 13);
    let leaves = mixed(&merge);
    assert_eq!(leaves.default.x, vec![0x1fef]);
    assert_eq!(leaves.default.bits, vec![0]);
    assert_eq!(leaves.starts.bits, vec![0x31]);
}

#[test]
fn structure_conditional_plan_recurses_into_nested_records_and_arrays() {
    // { { logic [3:0] a; int i; } r; logic [1:0] t [0:1]; }
    let inner = record(vec![
        member("a", atom(4, false), 0),
        member("i", atom(32, true), 0),
    ]);
    let descriptor = record(vec![
        member("r", inner, 0),
        member("t", array(atom(2, false), vec![(0, 1)]), 0),
    ]);
    let merge = aggregate_conditional_merge(&descriptor).unwrap().unwrap();
    assert_eq!(merge.width, 40);
    let leaves = mixed(&merge);
    assert_eq!(leaves.starts.bits, vec![1 | 1 << 2 | 1 << 4 | 1 << 36]);
    assert_eq!(leaves.default.x, vec![0xf | 0xf << 36]);
    // Equal leaves with one default collapse to the stride merge.
    let pair = record(vec![
        member("a", atom(8, false), 0),
        member("b", array(atom(8, false), vec![(0, 2)]), 0),
    ]);
    let merge = aggregate_conditional_merge(&pair).unwrap().unwrap();
    assert_eq!((merge.width, uniform(&merge).width), (32, 8));
}

#[test]
fn structure_conditional_plan_rejects_native_members() {
    let mut native = atom(8, false);
    native.shape = TypeShape::String;
    let descriptor = record(vec![member("native", native, 0)]);
    assert!(aggregate_conditional_merge(&descriptor).is_err());
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
