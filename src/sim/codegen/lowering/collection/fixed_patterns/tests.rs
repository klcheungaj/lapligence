//! The owned count is applied once; operand slots are not node identities.
use super::*;
use crate::core::db::{ConstantSource, ConstantType, Node};

#[derive(Default)]
struct Ast {
    nodes: Vec<Node>,
}

impl Ast {
    fn add(&mut self, kind: NodeKind) -> NodeId {
        let id = NodeId::from_index(self.nodes.len());
        self.nodes.push(Node {
            kind,
            children: Vec::new(),
            parent: None,
            name: String::new(),
            full_name: String::new(),
            file: None,
            line: 0,
            col: 0,
            end_line: 0,
            end_col: 0,
        });
        id
    }

    fn constant(&mut self, value: ValueData, size: i32) -> NodeId {
        self.add(NodeKind::Expr(ExprKind::Constant {
            value,
            size,
            const_type: ConstantType::Integer,
            source: ConstantSource::NotCaptured,
            time_scale: None,
        }))
    }

    fn pattern(&mut self, op: Operation, operands: Vec<NodeId>, reordered: bool) -> NodeId {
        self.add(NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            reordered,
            assignment: false,
        }))
    }

    fn database(self) -> Db {
        Db::from_test_nodes("patterns", self.nodes, Vec::new(), HashMap::new()).unwrap()
    }
}

#[test]
fn replicated_operands_preserve_interleaved_occurrences_and_order() {
    for reordered in [false, true] {
        let mut ast = Ast::default();
        let count = ast.constant(ValueData::UInt(3), 32);
        let a = ast.constant(ValueData::UInt(1), 7);
        let b = ast.constant(ValueData::UInt(2), 7);
        let mut operands = vec![count, a, b, a];
        if reordered {
            operands.reverse();
        }
        let pattern = ast.pattern(Operation::MultiAssignmentPattern, operands, reordered);
        let db = ast.database();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
        let cg = Codegen::new(&semantic);
        assert_eq!(
            cg.assignment_pattern_operands("tb", pattern)
                .unwrap()
                .unwrap(),
            vec![a, b, a, a, b, a, a, b, a],
        );
    }
}

#[test]
fn replication_expands_only_the_current_dimension() {
    let mut ast = Ast::default();
    let two = ast.constant(ValueData::UInt(2), 32);
    let three = ast.constant(ValueData::UInt(3), 32);
    let a = ast.constant(ValueData::UInt(9), 65);
    let inner = ast.pattern(Operation::MultiAssignmentPattern, vec![three, a], false);
    let outer = ast.pattern(Operation::MultiAssignmentPattern, vec![two, inner], false);
    let db = ast.database();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    assert_eq!(
        cg.assignment_pattern_operands("tb", outer).unwrap(),
        Some(vec![inner, inner])
    );
    assert_eq!(
        cg.assignment_pattern_operands("tb", inner).unwrap(),
        Some(vec![a, a, a])
    );
}

#[test]
fn plain_patterns_keep_shared_positions_without_replicating_again() {
    let mut ast = Ast::default();
    let a = ast.constant(ValueData::UInt(1), 1);
    let pattern = ast.pattern(Operation::AssignmentPattern, vec![a, a, a], false);
    let db = ast.database();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    assert_eq!(
        cg.assignment_pattern_operands("tb", pattern).unwrap(),
        Some(vec![a, a, a])
    );
    assert_eq!(cg.assignment_pattern_operands("tb", a).unwrap(), None);
}

#[test]
fn invalid_replication_counts_fail_before_expansion() {
    for (value, size, expected) in [
        (ValueData::UInt(0), 32, "must be positive"),
        (
            ValueData::Vector {
                bit_width: 4,
                is_signed: true,
                value_words: vec![15],
                unknown_words: vec![0],
            },
            4,
            "must be positive",
        ),
        (ValueData::Bin("x".to_owned()), 1, "count is unknown"),
        (ValueData::UInt(u64::MAX), 64, "too large"),
        (
            ValueData::UInt(u64::from(LLG_MAX_WIDTH) + 1),
            64,
            "too many elements",
        ),
    ] {
        let mut ast = Ast::default();
        let count = ast.constant(value, size);
        let a = ast.constant(ValueData::UInt(1), 1);
        let operands = if size == 64 && expected == "too large" {
            vec![count, a, a]
        } else {
            vec![count, a]
        };
        let pattern = ast.pattern(Operation::MultiAssignmentPattern, operands, false);
        let db = ast.database();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
        let cg = Codegen::new(&semantic);
        let error = cg.assignment_pattern_operands("tb", pattern).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn malformed_replication_requires_count_and_elements() {
    for missing_count in [false, true] {
        let mut ast = Ast::default();
        let count = ast.constant(ValueData::UInt(1), 32);
        let operands = if missing_count { vec![] } else { vec![count] };
        let pattern = ast.pattern(Operation::MultiAssignmentPattern, operands, false);
        let db = ast.database();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
        let cg = Codegen::new(&semantic);
        let error = cg.assignment_pattern_operands("tb", pattern).unwrap_err();
        assert!(
            error.contains(if missing_count {
                "has no count"
            } else {
                "has no elements"
            }),
            "{error}"
        );
    }
}

#[test]
fn component_packed_pattern_elements_keep_immediate_identity() {
    let snapshot = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            "packed_keys.sv",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/sim/feature_completion/rtl_004/vector_keys.sv"
            )),
        )],
        &Default::default(),
    )
    .unwrap();
    let db = Db::from_slang(&snapshot.snapshot).unwrap();
    drop(snapshot);
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    let mut found = false;
    for (index, node) in db.nodes().iter().enumerate() {
        if matches!(
            &node.kind,
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                ..
            })
        ) {
            let id = NodeId::from_index(index);
            let element = db.packed_pattern_element(id).unwrap();
            let descriptor = db.type_descriptor(id).unwrap();
            let TypeShape::PackedAtom { ranges } = &descriptor.shape else {
                continue;
            };
            let range = ranges.first().unwrap();
            let bounds = (range.left as i32, range.right as i32);
            let result = cg.p30_pattern_level("tb", id, bounds);
            assert!(result.is_ok(), "{element:?}: {result:?}; {:?}", node.kind);
            assert_eq!(
                result.unwrap().len() as u128,
                range.left.abs_diff(range.right) + 1
            );
            found = true;
        }
    }
    assert!(found);
}
