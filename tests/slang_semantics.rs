use llg::ffi::slang::{
    self, CompileOptions, CompileRequest, ConstantValue, SemanticEdge, SemanticEdgeRole,
    SemanticKind, SemanticNode, SemanticOperation, SemanticTimeScale, SemanticTimeUnit, Source,
    Type, TypeKind, TypeMember, TypeRange, TypeRangeKind,
};

fn compile(source: &str) -> slang::Snapshot {
    let sources = [Source::compilation_unit("semantic-contract.sv", source)];
    let options = CompileOptions::default();
    let snapshot = slang::compile(&CompileRequest {
        sources: &sources,
        options: &options,
    })
    .expect("semantic source should compile");
    assert!(
        !snapshot.has_errors(),
        "semantic source produced blocking diagnostics: {:?}",
        snapshot.diagnostics
    );
    snapshot
}

fn node_by_id(snapshot: &slang::Snapshot, id: u64) -> &SemanticNode {
    snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.id == id)
        .expect("semantic edge target")
}

fn edges<'a>(snapshot: &'a slang::Snapshot, node: &SemanticNode) -> &'a [SemanticEdge] {
    let start = usize::try_from(node.edge_start).expect("edge start fits usize");
    let count = usize::try_from(node.edge_count).expect("edge count fits usize");
    snapshot
        .semantic_edges
        .get(start..start + count)
        .expect("semantic edge window")
}

fn edge_targets<'a>(
    snapshot: &'a slang::Snapshot,
    node: &SemanticNode,
    role: SemanticEdgeRole,
) -> Vec<&'a SemanticNode> {
    edges(snapshot, node)
        .iter()
        .filter(|edge| edge.role == role)
        .map(|edge| node_by_id(snapshot, edge.target_id))
        .collect()
}

fn type_by_id(snapshot: &slang::Snapshot, id: u64) -> &Type {
    snapshot
        .types
        .iter()
        .find(|ty| ty.id == id)
        .expect("referenced type")
}

#[test]
fn subroutine_bodies_have_exact_edges_and_missing_bodies_fail_lowering() {
    let mut snapshot = compile(
        "// llg-test-fixture: tests/slang_semantics.rs/subroutine-bodies\n\
         module top;\n\
         function automatic int calculate(input int x);\n\
           int local_value; local_value = x + 1; return local_value;\n\
         endfunction\n\
         initial $display(\"%0d\", calculate(2));\n\
         endmodule",
    );
    let function = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == SemanticKind::Subroutine && node.name == "calculate")
        .expect("captured function");
    let body_edges = edges(&snapshot, function)
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Body)
        .collect::<Vec<_>>();
    assert_eq!(
        body_edges.len(),
        1,
        "function must have one exact body edge"
    );
    let edge_start = function.edge_start as usize;
    let edge_end = edge_start + function.edge_count as usize;
    let body_id = body_edges[0].target_id;
    assert_eq!(node_by_id(&snapshot, body_id).kind, SemanticKind::Statement);
    let database = llg::core::db::Db::from_slang(&snapshot).expect("owned function graph");
    llg::sim::codegen::generate(&database).expect("captured body must lower");

    for edge in &mut snapshot.semantic_edges[edge_start..edge_end] {
        if edge.role == SemanticEdgeRole::Body {
            edge.role = SemanticEdgeRole::Child;
        }
    }
    let incomplete = llg::core::db::Db::from_slang(&snapshot).expect("body-less declaration graph");
    let error = llg::sim::codegen::generate(&incomplete)
        .err()
        .expect("lowering must not infer a missing body from child order");
    assert!(error.to_string().contains("without a body"), "{error}");
}

fn type_ranges<'a>(snapshot: &'a slang::Snapshot, ty: &Type) -> &'a [TypeRange] {
    let start = usize::try_from(ty.range_start).expect("range start fits usize");
    let count = usize::try_from(ty.range_count).expect("range count fits usize");
    snapshot
        .type_ranges
        .get(start..start + count)
        .expect("type range window")
}

fn type_members<'a>(snapshot: &'a slang::Snapshot, ty: &Type) -> &'a [TypeMember] {
    let start = usize::try_from(ty.member_start).expect("member start fits usize");
    let count = usize::try_from(ty.member_count).expect("member count fits usize");
    snapshot
        .type_members
        .get(start..start + count)
        .expect("type member window")
}

fn integer_payload(snapshot: &slang::Snapshot, node: &SemanticNode) -> Option<(bool, u64, u64)> {
    let id = usize::try_from(node.constant_id?).ok()?;
    match &snapshot.constants.get(id)?.value {
        ConstantValue::Integer {
            is_signed,
            bit_width,
            value_words,
            unknown_words,
        } if unknown_words.iter().all(|word| *word == 0) => {
            Some((*is_signed, *bit_width, *value_words.first()?))
        }
        _ => None,
    }
}

#[test]
fn captures_process_assignments_calls_delays_and_events() {
    let snapshot = compile(
        r#"
module top;
    timeunit 1ns;
    timeprecision 1ps;
    logic clk;
    logic value;
    event done;

    initial begin
        clk = 1'b0;
        value = 1'b0;
        #2 value <= 1'b1;
        -> done;
        @done $display("value=%0b", value);
    end

    always @(posedge clk)
        value <= value + 1'b1;
endmodule
"#,
    );

    let processes: Vec<_> = snapshot
        .semantic_nodes
        .iter()
        .filter(|node| node.kind == SemanticKind::Process)
        .collect();
    assert_eq!(processes.len(), 2);
    assert!(processes
        .iter()
        .all(|process| { !edge_targets(&snapshot, process, SemanticEdgeRole::Body).is_empty() }));

    let top = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == SemanticKind::Instance && node.is_top)
        .expect("top instance");
    assert_eq!(
        top.time_scale,
        Some(SemanticTimeScale {
            unit: SemanticTimeUnit::Nanoseconds,
            magnitude: 1,
            precision_unit: SemanticTimeUnit::Picoseconds,
            precision_magnitude: 1,
        })
    );

    let assignments: Vec<_> = snapshot
        .semantic_nodes
        .iter()
        .filter(|node| {
            node.kind == SemanticKind::Expression && node.operation == SemanticOperation::Assign
        })
        .collect();
    assert!(assignments
        .iter()
        .any(|assignment| !assignment.is_nonblocking));
    assert!(assignments
        .iter()
        .any(|assignment| assignment.is_nonblocking));
    assert!(assignments.iter().all(|assignment| {
        edge_targets(&snapshot, assignment, SemanticEdgeRole::Lhs).len() == 1
            && edge_targets(&snapshot, assignment, SemanticEdgeRole::Rhs).len() == 1
    }));

    let mut runtime_literals: Vec<_> = assignments
        .iter()
        .flat_map(|assignment| edge_targets(&snapshot, assignment, SemanticEdgeRole::Rhs))
        .filter_map(|rhs| integer_payload(&snapshot, rhs))
        .collect();
    runtime_literals.sort_unstable();
    assert_eq!(
        runtime_literals,
        vec![(false, 1, 0), (false, 1, 0), (false, 1, 1)]
    );

    let display = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == SemanticKind::SystemCall && node.name == "$display")
        .expect("system call semantic node");
    assert_eq!(
        edge_targets(&snapshot, display, SemanticEdgeRole::Argument).len(),
        2
    );

    let delay = snapshot
        .semantic_nodes
        .iter()
        .find(|node| {
            node.kind == SemanticKind::TimingControl
                && edge_targets(&snapshot, node, SemanticEdgeRole::Delay).len() == 1
        })
        .expect("delay timing control");
    assert!(delay.parent_id.is_some());

    let posedge = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == SemanticKind::TimingControl && node.is_posedge)
        .expect("posedge timing control");
    let event_expr = edge_targets(&snapshot, posedge, SemanticEdgeRole::Event);
    assert_eq!(event_expr.len(), 1);
    assert_eq!(event_expr[0].kind, SemanticKind::Expression);
    assert_eq!(
        event_expr[0]
            .target_id
            .map(|id| node_by_id(&snapshot, id).name.as_str()),
        Some("clk")
    );

    let named_event = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == SemanticKind::NamedEvent && node.name == "done")
        .expect("named event declaration");
    let trigger = snapshot
        .semantic_nodes
        .iter()
        .find(|node| {
            node.kind == SemanticKind::Statement
                && edge_targets(&snapshot, node, SemanticEdgeRole::Event)
                    .iter()
                    .any(|target| target.target_id == Some(named_event.id))
        })
        .expect("event trigger relationship");
    assert!(!trigger.is_nonblocking);
}

#[test]
fn captures_packed_unpacked_and_aggregate_type_shape() {
    let snapshot = compile(
        r#"
typedef struct packed {
    logic [3:0] high;
    bit [1:0] low;
} packed_record_t;

typedef struct {
    logic [7:0] data;
    int code;
} unpacked_record_t;

module top;
    logic [2:0][7:0] packed_array;
    logic [7:0] memory [1:0][3:2];
    packed_record_t packed_record;
    unpacked_record_t unpacked_record;
endmodule
"#,
    );

    let variable_type = |name: &str| {
        let variable = snapshot
            .semantic_nodes
            .iter()
            .find(|node| node.kind == SemanticKind::Variable && node.name == name)
            .unwrap_or_else(|| panic!("variable `{name}`"));
        type_by_id(
            &snapshot,
            variable
                .type_id
                .unwrap_or_else(|| panic!("type for `{name}`")),
        )
    };

    let mut packed = variable_type("packed_array");
    let mut packed_ranges = Vec::new();
    while packed.kind == TypeKind::PackedArray {
        packed_ranges.extend_from_slice(type_ranges(&snapshot, packed));
        packed = type_by_id(
            &snapshot,
            packed.element_type_id.expect("packed array element type"),
        );
    }
    assert_eq!(
        packed_ranges
            .iter()
            .map(|range| (range.kind, range.left, range.right))
            .collect::<Vec<_>>(),
        vec![(TypeRangeKind::Packed, 2, 0), (TypeRangeKind::Packed, 7, 0),]
    );

    let mut memory = variable_type("memory");
    let mut unpacked_ranges = Vec::new();
    while memory.kind == TypeKind::FixedUnpackedArray {
        unpacked_ranges.extend_from_slice(type_ranges(&snapshot, memory));
        memory = type_by_id(
            &snapshot,
            memory.element_type_id.expect("unpacked array element type"),
        );
    }
    assert_eq!(
        unpacked_ranges
            .iter()
            .map(|range| (range.kind, range.left, range.right))
            .collect::<Vec<_>>(),
        vec![
            (TypeRangeKind::Unpacked, 1, 0),
            (TypeRangeKind::Unpacked, 3, 2),
        ]
    );
    assert_eq!(memory.kind, TypeKind::PackedArray);
    assert_eq!(
        type_ranges(&snapshot, memory),
        [TypeRange {
            left: 7,
            right: 0,
            kind: TypeRangeKind::Packed,
        }]
    );

    let packed_record = variable_type("packed_record");
    assert_eq!(packed_record.kind, TypeKind::PackedStruct);
    let packed_members = type_members(&snapshot, packed_record);
    assert_eq!(
        packed_members
            .iter()
            .map(|member| (member.name.as_str(), member.bit_offset, member.bit_width))
            .collect::<Vec<_>>(),
        vec![("high", 2, 4), ("low", 0, 2)]
    );

    let unpacked_record = variable_type("unpacked_record");
    assert_eq!(unpacked_record.kind, TypeKind::UnpackedStruct);
    let unpacked_members = type_members(&snapshot, unpacked_record);
    assert_eq!(
        unpacked_members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        vec!["data", "code"]
    );
    assert_eq!(
        type_by_id(&snapshot, unpacked_members[0].type_id).bit_width,
        8
    );
    assert_eq!(
        type_by_id(&snapshot, unpacked_members[1].type_id).bit_width,
        32
    );
}

#[test]
fn captures_port_declarations_actuals_and_per_port_connections() {
    let snapshot = compile(
        r#"
module child(input logic data_in, output logic data_out);
    assign data_out = data_in;
endmodule

module top;
    logic source;
    logic sink;
    child u_child(.data_in(source), .data_out(sink));
endmodule
"#,
    );

    let instance = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == SemanticKind::Instance && node.name == "u_child")
        .expect("child instance");
    let declarations: Vec<_> = edges(&snapshot, instance)
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Declaration)
        .collect();
    let actuals: Vec<_> = edges(&snapshot, instance)
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Actual)
        .collect();
    assert_eq!(declarations.len(), 2);
    assert_eq!(actuals.len(), 2);

    for declaration in declarations {
        let port = node_by_id(&snapshot, declaration.target_id);
        assert_eq!(port.kind, SemanticKind::Port);
        let actual = actuals
            .iter()
            .find(|actual| actual.index == declaration.index)
            .map(|actual| node_by_id(&snapshot, actual.target_id))
            .expect("actual paired by port index");
        assert_eq!(
            edge_targets(&snapshot, port, SemanticEdgeRole::HighConnection),
            vec![actual]
        );
        assert_eq!(
            edge_targets(&snapshot, port, SemanticEdgeRole::LowConnection).len(),
            1
        );

        let actual_declaration = actual
            .target_id
            .map(|id| node_by_id(&snapshot, id).name.as_str())
            .expect("actual expression target");
        match port.name.as_str() {
            "data_in" => {
                assert!(port.is_input);
                assert!(!port.is_output);
                assert_eq!(actual_declaration, "source");
            }
            "data_out" => {
                assert!(port.is_output);
                assert!(!port.is_input);
                assert_eq!(actual_declaration, "sink");
            }
            other => panic!("unexpected port `{other}`"),
        }
    }
}
