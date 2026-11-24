//! `string` parameters as arguments of user-defined system tasks (IEEE
//! 1800-2009 37.26, 38.15).

use super::{sim_041, sim_cli, sim_harness};

const SUITE: &str = "feature_completion/ki_vpi_string_parameters";

#[test]
fn system_task_arguments_are_string_parameters() {
    let directory =
        sim_harness::TempDir::new("ki-vpi-string-parameters").expect("temporary directory");
    let archive = sim_041::static_archive(SUITE, &directory, "arguments", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "arguments",
        include_str!("../fixtures/sim/feature_completion/ki_vpi_string_parameters/arguments.out"),
        &[
            "--define-system-task",
            "$show",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}
