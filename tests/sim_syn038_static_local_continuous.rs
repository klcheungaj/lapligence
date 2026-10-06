//! SYN-038 hierarchical continuous writes to static function locals.

use crate::sim_cli;

#[test]
fn static_function_local_accepts_continuous_variable_assignment() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "static_local_continuous",
        "state=1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn duplicate_static_local_continuous_drivers_are_rejected_by_codegen() {
    sim_cli::reject_case_with_args(
        "syn038_pairwise",
        "static_local_continuous",
        "semantic error: multiple continuous assignments to variable storage",
        &["--edition", "2009", "--define", "SYN038_DUPLICATE_DRIVER"],
    );
}
