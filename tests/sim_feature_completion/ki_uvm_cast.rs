//! Class handle forms a verification library relies on: `$cast` from any
//! source into any singular destination, handle functions with packed output
//! and `ref` formals, class-typed `?:`, comparisons of method results,
//! discarded pops, unformatted handles, class string actuals, record locals,
//! branch handles, local array locators, `$bits` bounds and `this.c`
//! constraint modes. Clauses and oracles are in the
//! fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/ki_uvm_cast";

#[test]
fn call_sources() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "call_sources",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/call_sources.out"),
        &[],
        &[],
    );
}

#[test]
fn element_targets() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "element_targets",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/element_targets.out"),
        &[],
        &[],
    );
}

#[test]
fn string_values() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "string_values",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/string_values.out"),
        &[],
        &[],
    );
}

#[test]
fn handle_outputs() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "handle_outputs",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/handle_outputs.out"),
        &[],
        &[],
    );
}

#[test]
fn call_formals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "call_formals",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/call_formals.out"),
        &[],
        &[],
    );
}

#[test]
fn handle_conditionals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "handle_conditionals",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/handle_conditionals.out"),
        &[],
        &[],
    );
}

#[test]
fn discarded_pops() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "discarded_pops",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/discarded_pops.out"),
        &[],
        &[],
    );
}

#[test]
fn unformatted_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "unformatted_handles",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/unformatted_handles.out"),
        &[],
        &[],
    );
}

#[test]
fn class_string_actuals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_string_actuals",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/class_string_actuals.out"),
        &[],
        &[],
    );
}

#[test]
fn subroutine_records() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "subroutine_records",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/subroutine_records.out"),
        &[],
        &[],
    );
}

#[test]
fn function_fork_locals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "function_fork_locals",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/function_fork_locals.out"),
        &[],
        &[],
    );
}

#[test]
fn fixed_local_locators() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "fixed_local_locators",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/fixed_local_locators.out"),
        &[],
        &[],
    );
}

#[test]
fn bits_replication() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "bits_replication",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/bits_replication.out"),
        &[],
        &[],
    );
}

#[test]
fn constraint_scope() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "constraint_scope",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/constraint_scope.out"),
        &[],
        &[],
    );
}

#[test]
fn handle_call_arguments() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "handle_call_arguments",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/handle_call_arguments.out"),
        &[],
        &[],
    );
}
