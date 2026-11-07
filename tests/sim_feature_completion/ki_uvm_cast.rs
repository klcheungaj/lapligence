//! Class handle forms a verification library relies on: `$cast` from any
//! source into any singular destination, handle functions with packed output
//! and `ref` formals, class-typed `?:`, comparisons of method results,
//! discarded pops, unformatted handles, class string actuals, record locals,
//! branch handles, local array locators, `$bits` bounds and bare and
//! `this.c` constraint modes, and nested `$cast` destinations. Clauses and
//! oracles are in the fixture readme.
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

#[test]
fn string_contexts() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "string_contexts",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/string_contexts.out"),
        &[],
        &[],
    );
}

#[test]
fn nested_string_keys() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "nested_string_keys",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/nested_string_keys.out"),
        &[],
        &[],
    );
}

#[test]
fn function_fork_record() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "function_fork_record",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/function_fork_record.out"),
        &[],
        &[],
    );
}

#[test]
fn join_branch_detached_fork() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "join_branch_detached_fork",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_cast/join_branch_detached_fork.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn initializer_virtual_dispatch() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "initializer_virtual_dispatch",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_cast/initializer_virtual_dispatch.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn initializer_stop_reachable() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "initializer_stop_reachable",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_cast/initializer_stop_reachable.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn nested_destinations() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "nested_destinations",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/nested_destinations.out"),
        &[],
        &[],
    );
}

#[test]
fn nested_destination_stop() {
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "nested_destination_stop",
        &[],
        include_str!("../fixtures/sim/feature_completion/ki_uvm_cast/nested_destination_stop.out"),
        "$cast failed to assign an incompatible value at tb:13:15: the destination element does not exist",
    );
}
