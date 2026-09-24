//! Selected typed values consumed from interface and generate lexical scopes.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const FIXTURE_SOURCE: &str = include_str!("fixtures/sim/syn038_pairwise/typed_scope_remainders.sv");
const FOCAL_SOURCE_ANCHORS: &[&str] = &[
    "SYN038-GAP-TY-enum__HC-interface",
    "enum_copy = enum_source;",
    "SYN038-GAP-TY-packed_struct__HC-interface",
    "packed_copy = packed_source;",
    "SYN038-GAP-TY-fixed_array_integral__HC-interface",
    "array_copy = array_source;",
    "SYN038-GAP-TY-untagged_packed_union__HC-generate",
    "union_copy = union_source;",
    "SYN038-GAP-TY-unpacked_record__HC-generate",
    "record_copy = record_source;",
];

#[test]
fn typed_values_keep_their_outer_type_in_interface_and_generate_scopes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/typed_scope_remainders.sv\n"
    ));
    for anchor in FOCAL_SOURCE_ANCHORS {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost typed lexical-scope anchor: {anchor}"
        );
    }
    for assertion in [
        "TY-enum/HC-interface mismatch",
        "TY-packed_struct/HC-interface mismatch",
        "TY-fixed_array_integral/HC-interface mismatch",
        "TY-untagged_packed_union/HC-generate mismatch",
        "TY-unpacked_record/HC-generate mismatch",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(assertion),
            "fixture lost immediate typed-value oracle: {assertion}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "typed_scope_remainders",
        "tyhc=a,2b,c,5a,d\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
