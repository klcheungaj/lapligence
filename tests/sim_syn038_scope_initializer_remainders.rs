//! SYN-038 hierarchy-context and initializer-source remainder paths.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/scope_initializer_remainders.sv");

const FOCAL_VECTORS: &[&str] = &[
    "HC=subroutine, IN=constant_declaration; gap SYN038-GAP-HC-subroutine__IN-constant_declaration",
    "HC=generate, IN=constant_declaration; gap SYN038-GAP-HC-generate__IN-constant_declaration",
    "HC=generate, IN=automatic_local; gap SYN038-GAP-HC-generate__IN-automatic_local",
    "HC=generate, IN=static_local; gap SYN038-GAP-HC-generate__IN-static_local",
    "HC=generate, IN=runtime_declaration; gap SYN038-GAP-HC-generate__IN-runtime_declaration",
    "HC=interface, IN=constant_declaration; gap SYN038-GAP-HC-interface__IN-constant_declaration",
    "HC=interface, IN=automatic_local; gap SYN038-GAP-HC-interface__IN-automatic_local",
    "HC=interface, IN=static_local; gap SYN038-GAP-HC-interface__IN-static_local",
];

#[test]
fn scope_initializer_remainders_match_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/scope_initializer_remainders.sv\n"
    ));
    for vector in FOCAL_VECTORS {
        assert!(
            FIXTURE_SOURCE.contains(vector),
            "fixture lost focal vector or gap ID: {vector}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "scope_initializer_remainders",
        "subroutine=a1\ngenerate=11,22,33,44\ninterface=51,62,73\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
