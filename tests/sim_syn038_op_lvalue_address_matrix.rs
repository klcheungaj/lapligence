//! SYN-038 operation syntax participates in selected lvalue address projection.

use crate::sim_cli;

const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/op_lvalue_address_matrix.sv");
const EXPECTED_STDOUT: &str =
    "conditional=a2,b6,0 equality=a5,b4,1 cast=b6,b4,1 pattern=10110110,d4,1\n";
const FOCAL_SOURCE_ANCHORS: &[&str] = &[
    "conditional_slice[choose ? KEY1 : KEY0][4 +: 4]",
    "{conditional_concat[choose ? KEY1 : KEY0], conditional_concat[KEY2]}",
    "'{conditional_pattern[choose ? KEY1 : KEY0], conditional_pattern[KEY2]}",
    "equality_field[choose == 1'b1].value",
    "equality_slice[choose inside {1'b1}][4 +: 4]",
    "{equality_concat[choose == 1'b1], equality_concat[2'b10]}",
    "'{equality_pattern[choose inside {1'b1}], equality_pattern[2'b10]}",
    "cast_field[key_t'(choose)].value",
    "cast_slice[key_t'(choose)][4 +: 4]",
    "{cast_concat[key_t'(choose)], cast_concat[KEY2]}",
    "'{cast_pattern[key_t'(choose)], cast_pattern[KEY2]}",
    "pattern_element[key_t'{1'b0, choose}]",
    "{pattern_concat[key_t'{1'b0, choose}], pattern_concat[KEY2]}",
    "'{pattern_lvalue[key_t'{1'b0, choose}], pattern_lvalue[KEY2]}",
];

#[test]
fn operation_lvalues_use_the_selected_address_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/op_lvalue_address_matrix.sv\n"
    ));
    for anchor in FOCAL_SOURCE_ANCHORS {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost selected-address source anchor: {anchor}"
        );
    }
    for assertion in [
        "conditional row slice low address",
        "conditional concatenation high address",
        "conditional positional pattern high address",
        "equality field high address",
        "inside row slice high address",
        "equality concatenation high address",
        "inside positional pattern high address",
        "cast field high address",
        "cast row slice high address",
        "cast concatenation high address",
        "cast positional pattern high address",
        "assignment pattern element high address",
        "assignment pattern concatenation high address",
        "assignment pattern positional pattern high address",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(assertion),
            "fixture lost immediate oracle: {assertion}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "op_lvalue_address_matrix",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}
