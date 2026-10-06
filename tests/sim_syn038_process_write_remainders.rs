//! SYN-038 remaining operation/write and process/write paths through the public simulator CLI.

use crate::sim_cli;

const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/process_write_remainders.sv");

const FOCAL_SOURCE_ANCHORS: &[&str] = &[
    "eq_nba <= source == 8'h3c;",
    "inside_nba <= source inside {8'h3c};",
    "cast_nba <= byte_t'(source);",
    "pattern_nba <= '{source[7],source[6],source[5],source[4],source[3],source[2],source[1],source[0]};",
    "comb_nba <= source + 8'h03;",
    "always_comb begin",
    "latch_nba <= source + 8'h04;",
    "always_latch if (enable) begin",
    "function automatic logic [7:0] const_ref_read(const ref logic [7:0] value);",
    "assign const_ref_value = const_ref_read(source);",
];

#[test]
fn remaining_process_write_paths_match_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/process_write_remainders.sv\n"
    ));
    for anchor in FOCAL_SOURCE_ANCHORS {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost focal source anchor: {anchor}"
        );
    }
    for assertion in [
        "if (eq_nba !== 1 || inside_nba !== 1 || cast_nba !== 8'h3c",
        "if (eq_nba !== 0 || inside_nba !== 0 || cast_nba !== 8'h5a",
        "if (eq_nba !== 0 || inside_nba !== 0 || cast_nba !== 8'h21",
        "if (const_ref_value !== 8'hda)",
        "if (const_ref_value !== 8'ha1)",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(assertion),
            "fixture lost destination assertion: {assertion}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "process_write_remainders",
        "ops=0,0,21,21 comb=24 latch=25 const_ref=a1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
