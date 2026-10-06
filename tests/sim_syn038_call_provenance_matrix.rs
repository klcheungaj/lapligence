//! SYN-038 function-result and fixed-array-reduction provenance through calls,
//! storage, hierarchy, interface members, declarations, ports, and processes.

use crate::sim_cli;

const FIXTURE_SOURCE: &str = include_str!("fixtures/sim/syn038_pairwise/call_provenance_matrix.sv");

#[test]
fn call_provenance_paths_match_in_both_optimizer_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/call_provenance_matrix.sv\n"
    ));
    for anchor in [
        "selector ? make_byte() : 8'h44",
        "make_byte() == 8'h12",
        "int'(make_byte())",
        "'{make_byte(), 8'h03}",
        "return make_byte();",
        "observe_task(make_byte())",
        "automatic byte_t automatic_initialized = make_byte();",
        "automatic_target = make_byte();",
        "static_target = make_byte();",
        "value = make_byte();",
        "automatic byte_t automatic_total = module_lanes.sum();",
        "static byte_t static_total = make_lanes().sum();",
        "automatic_reduction_receiver = automatic_lanes.sum();",
        "static_reduction_receiver = static_lanes.sum();",
        "static byte_t static_initialized = constant_byte();",
        "typedef logic [constant_width()-1:0] function_width_t;",
        "typedef logic [CONSTANT_LANES.sum()-1:0] reduction_width_t;",
        "localparam bit_byte_pair_t DEFAULT_LANES = '{8'd0, 8'd0};",
        "byte_t parameter_array_reduction = DEFAULT_LANES.sum();",
        "DEFAULT_LANES[0] !== 8'd0",
        "module call_provenance_override_child",
        "parameter logic [7:0] lanes [0:1] = '{8'h21, 8'h43}",
        "selected = lanes[index];",
        ".lanes('{8'h51, 8'h62})",
        ".lanes('{8'h73, 8'h84})",
        "overridden_left_value !== 8'h51 || overridden_right_value !== 8'h84",
        "byte_t runtime_reduction = make_lanes().sum();",
        "localparam byte_t CONSTANT_REDUCTION = CONSTANT_LANES.sum();",
        ".value(module_lanes.sum())",
        "module_lanes.sum() == 8'd5",
        "int'(module_lanes.sum())",
        "byte_pair_t'{module_lanes.sum(), 8'd7}",
        "return values.sum();",
        "make_lanes().sum()",
        "child.make_byte()",
        "child.lanes.sum()",
        "bus.member_result = make_byte()",
        "bus.lanes.sum()",
        "always @(make_byte())",
        "assign continuous_reduction = module_lanes.sum();",
        "always_comb combinational_reduction = module_lanes.sum();",
        "always_latch begin\n        if (latch_gate)\n            latch_function_value = constant_byte();",
        "always_latch begin\n        if (latch_gate)\n            latch_reduction_value = module_lanes.sum();",
        "always_ff @(posedge clock)\n        sequential_reduction <= module_lanes.sum();",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "missing focal source: {anchor}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "call_provenance_matrix",
        "calls=13,1,19,21,18 task=13,12 routes=12,12,5a,12 reductions=6,1,6,12,05,05,0d,09 processes=12,05,05,1 parameter=0 overrides=51,84\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
