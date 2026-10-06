//! SYN-038 hierarchy, process, call-result, and constant-elaboration remainders.

use crate::sim_cli;

const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/hierarchy_process_call_remainders.sv");

const FOCAL_VECTORS: &[&str] = &[
    "HC=generate, CP=fixed_array_reduction; gap SYN038-GAP-HC-generate__CP-fixed_array_reduction",
    "HC=interface, CP=function; gap SYN038-GAP-HC-interface__CP-function",
    "HC=generate, PC=always_latch; gap SYN038-GAP-HC-generate__PC-always_latch",
    "HC=interface, HR=child_port; gap SYN038-GAP-HC-interface__HR-child_port",
    "CO=constant_elaboration, HC=interface; gap SYN038-GAP-CO-constant_elaboration__HC-interface",
    "CO=constant_elaboration, HC=subroutine; gap SYN038-GAP-CO-constant_elaboration__HC-subroutine",
    "CO=constant_elaboration, HC=generate; gap SYN038-GAP-CO-constant_elaboration__HC-generate",
];

#[test]
fn hierarchy_process_call_remainders_match_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/hierarchy_process_call_remainders.sv\n"
    ));
    for vector in FOCAL_VECTORS {
        assert!(
            FIXTURE_SOURCE.contains(vector),
            "fixture lost focal vector or gap ID: {vector}"
        );
    }
    for anchor in [
        "reduction_value = lanes.sum();",
        "function_result = produce_value();",
        "always_latch begin",
        "latch_value = source;",
        "child_value_if nested(source);",
        "typedef logic [constant_scope_cfg::FUNCTION_WIDTH-1:0] local_t;",
        "typedef logic [constant_scope_cfg::GENERATE_WIDTH-1:0] generated_t;",
        "typedef logic [constant_scope_cfg::INTERFACE_WIDTH-1:0] member_t;",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost focal source anchor: {anchor}"
        );
    }
    for assertion in [
        "generated reduction receiver or result mismatch",
        "generated process or reduction mismatch",
        "hierarchy, function, or constant elaboration mismatch",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(assertion),
            "fixture lost focal value assertion: {assertion}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "hierarchy_process_call_remainders",
        "reduction=07 function=a6 latch=91 child_port=5c const=5,7,2a\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
