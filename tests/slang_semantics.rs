use llg::core::db::{EventTriggerTiming, NodeKind, StmtKind};
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

#[test]
fn navigation_capture_shares_repeated_bodies_and_generate_declarations() {
    let mut counts = Vec::new();
    for repetitions in [1, 2048] {
        let source = format!(
            "module leaf(input logic clk, output logic data);\n\
             always_comb data = clk; endmodule\n\
             module top(input logic clk);\n\
             for (genvar i = 0; i < {repetitions}; i++) begin : g\n\
             logic data; leaf u(.clk(clk), .data(data)); end\n\
             endmodule\n"
        );
        let options = CompileOptions {
            library_units: true,
            limits: slang::Limits {
                max_semantic_nodes: 1000,
                ..Default::default()
            },
            ..Default::default()
        };
        let snapshot = slang::compile(&CompileRequest {
            sources: &[Source::compilation_unit("/virtual/navigation.sv", &source)],
            options: &options,
        })
        .expect("navigation capture stays within the source-sized node budget");
        assert!(snapshot
            .semantic_nodes
            .iter()
            .any(|node| node.name == "leaf" && node.kind == SemanticKind::Definition));
        assert!(snapshot
            .semantic_nodes
            .iter()
            .all(|node| node.kind != SemanticKind::Expression
                && node.kind != SemanticKind::Statement));
        let reference = snapshot
            .lexical_tokens
            .iter()
            .find(|token| {
                token.text == "clk"
                    && token.range.is_some_and(|range| {
                        range.start == source.find("= clk").unwrap() as u64 + 2
                    })
            })
            .expect("clk reference token");
        let target = node_by_id(
            &snapshot,
            reference.semantic_id.expect("exact reference binding"),
        );
        assert_eq!(target.name, "clk");
        assert_eq!(
            target.range.unwrap().start,
            source.find("clk,").unwrap() as u64
        );
        counts.push(snapshot.semantic_nodes.len());
    }
    assert_eq!(
        counts[0], counts[1],
        "source snapshot must not grow with generate iteration count"
    );
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
fn captures_nonblocking_event_trigger_mode_and_delay_in_owned_db() {
    let snapshot = compile(
        r#"
module top;
    event done;
    initial begin
        ->> #2 done;
    end
endmodule
"#,
    );
    let db = llg::core::db::Db::from_slang(&snapshot).expect("owned event-trigger graph");
    let trigger = db
        .node_ids()
        .find_map(|id| match db.node_kind(id) {
            NodeKind::Stmt(StmtKind::EventTrigger {
                blocking: false,
                timing:
                    Some(EventTriggerTiming::Delay {
                        expression: delay, ..
                    }),
                ..
            }) => Some(*delay),
            _ => None,
        })
        .expect("owned DB retains nonblocking trigger delay");
    assert!(matches!(db.node_kind(trigger), NodeKind::Expr(_)));
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
fn connection_actual_tokens_use_canonical_source_identity() {
    use llg::ffi::slang::{LanguageEdition, LexicalKind, LexicalRole};

    let source = r#"
module child(input clk, input din, output dout);
    assign dout = clk & din;
endmodule
module top(input clk, input din, output dout);
    wire local_net;
    reg local_var;
    assign local_net = clk;
    initial local_var = 0;
    child u_ports(.clk(clk), .din(din), .dout(dout));
    child u_locals(.clk(local_net), .din(local_var), .dout());
endmodule
"#;
    let cases = [
        (
            ".clk(clk)", ".clk(", "input clk", "input ", "clk",
            LexicalKind::Port, SemanticKind::Port, SemanticKind::Net,
        ),
        (
            ".din(din)", ".din(", "input din", "input ", "din",
            LexicalKind::Port, SemanticKind::Port, SemanticKind::Net,
        ),
        (
            ".dout(dout)", ".dout(", "output dout", "output ", "dout",
            LexicalKind::Port, SemanticKind::Port, SemanticKind::Net,
        ),
        (
            ".clk(local_net)", ".clk(", "wire local_net", "wire ", "local_net",
            LexicalKind::Net, SemanticKind::Net, SemanticKind::Net,
        ),
        (
            ".din(local_var)", ".din(", "reg local_var", "reg ", "local_var",
            LexicalKind::Variable, SemanticKind::Variable, SemanticKind::Variable,
        ),
    ];

    for edition in [
        LanguageEdition::Verilog2001,
        LanguageEdition::SystemVerilog2009,
    ] {
        let sources = [Source::compilation_unit("connection-identity.v", source)];
        let options = CompileOptions {
            edition,
            ..Default::default()
        };
        let snapshot = slang::compile(&CompileRequest {
            sources: &sources,
            options: &options,
        })
        .expect("connection source should compile");
        assert!(
            !snapshot.has_errors(),
            "edition={edition:?}: {:?}",
            snapshot.diagnostics
        );

        for (connection, prefix, declaration, decl_prefix, name, kind, target_kind, storage_kind) in
            cases
        {
            let start =
                (source.find(connection).expect("connection spelling") + prefix.len()) as u64;
            let declaration_start =
                (source.rfind(declaration).expect("parent declaration") + decl_prefix.len()) as u64;
            let mut tokens = snapshot.lexical_tokens.iter().filter(|token| {
                token.range.is_some_and(|range| range.start == start)
                    && !token.is_missing
                    && !token.is_skipped
            });
            let token = tokens.next().expect("one lexical actual token");
            assert!(tokens.next().is_none(), "duplicate actual token: {connection}");
            assert_eq!(token.text, name);
            assert_eq!(token.role, LexicalRole::ConnectionActual);
            assert_eq!(token.kind, kind, "wrong lexical kind: {connection}");
            let target = node_by_id(&snapshot, token.semantic_id.expect("bound actual"));
            assert_eq!(target.name, name);
            assert_eq!(target.kind, target_kind, "wrong source identity: {connection}");
            assert_eq!(
                target.range.expect("declaration range").start,
                declaration_start,
                "actual must stay in the parent scope: {connection}"
            );

            // Lexical normalization must not rewrite the simulator's
            // expression targets from storage symbols to port symbols.
            let expression = snapshot
                .semantic_nodes
                .iter()
                .find(|node| {
                    node.kind == SemanticKind::Expression
                        && node.target_id.is_some()
                        && node.range.is_some_and(|range| range.start == start)
                })
                .expect("actual's semantic reference");
            let storage = node_by_id(&snapshot, expression.target_id.unwrap());
            assert_eq!(storage.kind, storage_kind, "storage changed: {connection}");
            assert_eq!(storage.range.unwrap().start, declaration_start);
        }
    }
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

#[test]
fn owned_scope_names_keep_interface_and_dump_metadata_without_executable_placeholders() {
    let snapshot = compile(
        r#"
interface bus;
    logic data;
    modport reader(input data);
endinterface
module consumer(bus.reader b);
    wire value = b.data;
endmodule
module bare_consumer(bus b);
    wire value = b.data;
endmodule
module top;
    bus b();
    consumer c(.b(b.reader));
    bare_consumer d(.b(b));
    initial begin
        $dumpvars(0, top);
        #1 $finish(0);
    end
endmodule
"#,
    );
    let db = llg::core::db::Db::from_slang(&snapshot).expect("owned scope metadata");
    let coverage = llg::sim::semantic::SemanticModel::from_db(&db).simulation_coverage();
    let scopes: Vec<_> = db
        .node_ids()
        .filter(|id| {
            matches!(
                db.node_kind(*id),
                NodeKind::Expr(llg::core::db::ExprKind::ScopeRef { .. })
            )
        })
        .collect();
    assert!(
        scopes.len() >= 3,
        "bare interface, modport, and dump scope must be captured"
    );
    for id in scopes {
        assert_eq!(
            coverage[id.index()].class,
            llg::sim::semantic::SimulationNodeClass::ElaborationConsumed,
            "scope expression {} is not an executable value: {:?}",
            id.index(),
            db.node(id),
        );
    }
}
