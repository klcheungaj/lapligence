//! Parameters declared in procedural blocks (IEEE 1800-2009 A.2.8, 9.3.4).

use super::{sim_041, sim_cli, sim_harness};

const SUITE: &str = "feature_completion/ki_block_parameters";

#[test]
fn value_kinds_of_block_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "kinds",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/kinds.out"),
        &[],
        &[],
    );
}

#[test]
fn types_of_block_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "types",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/types.out"),
        &[],
        &[],
    );
}

#[test]
fn scopes_of_block_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "scopes",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/scopes.out"),
        &[],
        &[],
    );
}

#[test]
fn contexts_that_read_block_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "contexts",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/contexts.out"),
        &[],
        &[],
    );
}

#[test]
fn block_parameters_of_subroutines() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "subroutines",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/subroutines.out"),
        &[],
        &[],
    );
}

#[test]
fn block_parameters_per_instance() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "instances",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/instances.out"),
        &[],
        &[],
    );
}

#[test]
fn hierarchical_names_of_block_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "hierarchy",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/hierarchy.out"),
        &[],
        &[],
    );
}

#[test]
fn block_parameters_of_the_2001_edition() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "verilog.v",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/verilog.out"),
        &["--edition", "v2001"],
        &[],
    );
}

#[test]
fn vpi_reads_block_parameters() {
    let directory = sim_harness::TempDir::new("ki-block-parameters").expect("temporary directory");
    let archive = sim_041::static_archive(SUITE, &directory, "vpi", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "vpi",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/vpi.out"),
        &[
            "--define-system-task",
            "$show",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn defparams_of_block_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "defparam",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/defparam.out"),
        &[],
        &[],
    );
}

/// The final value decides the type of a parameter declared without one.
#[test]
fn defparam_values_give_untyped_parameters_their_types() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "defparam_types",
        include_str!("../fixtures/sim/feature_completion/ki_block_parameters/defparam_types.out"),
        &[],
        &[],
    );
}

#[test]
fn illegal_defparams_of_local_parameters_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_defparam_localparam",
        "neg_defparam_localparam.sv:6:12 'L' is a localparam",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_defparam_generate",
        "neg_defparam_generate.sv:6:12 'P' is a localparam",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_defparam_package",
        "neg_defparam_package.sv:8:12 'P' is a localparam",
    );
}

#[test]
fn illegal_uses_of_block_parameters_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_out_of_scope",
        "neg_out_of_scope.sv:4:21 use of undeclared identifier 'K'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_assigned",
        "neg_assigned.sv:4:7 constant 'K' is not modifiable",
    );
}
