//! SYN-038 source routes across module, generate, interface, and subroutine scopes.

use crate::sim_cli;

const FIXTURE_SOURCE: &str = include_str!("fixtures/sim/syn038_pairwise/scope_hierarchy_matrix.sv");
const EXPECTED_STDOUT: &str = "scope=if:10,6b,00 gen:20,a7 module:10 types:5c,22,88,44 op:a7,0,05,e5 return:a7 init:1000a700 event=1 latch=a7,10 ff=10\n";
const FOCAL_SOURCE_ANCHORS: &[&str] = &[
    "always_comb interface_hierarchy_read = $root.tb.child.value;",
    "always_comb interface_member_read = nested.member_value;",
    "return bus.member_value;",
    "automatic logic [7:0] automatic_hierarchical = child.value;",
    "static logic [7:0] static_hierarchical = child.default_value;",
    "automatic logic [7:0] automatic_interface = bus.member_value;",
    "static logic [7:0] static_interface = bus.default_member;",
    "always_comb module_comb_hierarchical = child.value;",
    "module_latch_interface = bus.member_value;",
    "module_latch_hierarchical = child.value;",
    "ff_hierarchical <= child.value;",
    "always @(bus.event_value)",
    "generated_hierarchical[g] = generated_child.value;",
    "generated_interface[g] = bus.member_value;",
    "nested_member_initial = bus.interface_member_read;",
    "enum_copy = bus.member_phase;",
    "integral_array_copy = bus.member_bytes;",
    "record_array_copy = bus.member_records;",
    "unpacked_record_copy = bus.member_record;",
    "conditional_copy = select_source ? bus.member_value : 8'h00;",
    "equality_copy = bus.member_value == member_value;",
    "cast_copy = byte_t'(bus.cast_source);",
    "pattern_copy = '{",
    "function_return_copy = read_interface_member();",
];

#[test]
fn source_routes_keep_their_lexical_scope_and_storage_identity() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/scope_hierarchy_matrix.sv\n"
    ));
    for anchor in FOCAL_SOURCE_ANCHORS {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost source-route anchor: {anchor}"
        );
    }
    for assertion in [
        "interface source type routes mismatch",
        "interface source operation or return mismatch",
        "hierarchical/interface declaration initializer mismatch",
        "scoped hierarchy source routes mismatch",
        "hierarchical/interface latch source mismatch",
        "hierarchical flip-flop source mismatch",
        "interface event source mismatch",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(assertion),
            "fixture lost immediate source oracle: {assertion}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "scope_hierarchy_matrix",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}
