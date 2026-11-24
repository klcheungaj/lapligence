//! Parameters declared in procedural blocks (IEEE 1800-2009 A.2.8, 9.3.4).

use super::{sim_041, sim_cli, sim_harness};

const SUITE: &str = "feature_completion/ki_block_parameters";

macro_rules! case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            sim_cli::run_case_backend_parity(
                SUITE,
                $name,
                include_str!(concat!(
                    "../fixtures/sim/feature_completion/ki_block_parameters/",
                    $name,
                    ".out"
                )),
                &[],
                &[],
            );
        }
    };
}

case!(value_kinds_of_block_parameters, "kinds");
case!(types_of_block_parameters, "types");
case!(scopes_of_block_parameters, "scopes");
case!(contexts_that_read_block_parameters, "contexts");
case!(block_parameters_of_subroutines, "subroutines");
case!(block_parameters_per_instance, "instances");
case!(hierarchical_names_of_block_parameters, "hierarchy");

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
