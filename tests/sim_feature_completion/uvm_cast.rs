//! Class handle forms a verification library relies on: `$cast` from any
//! source into any singular destination, handle functions with packed output
//! and `ref` formals, class-typed `?:`, comparisons of method results,
//! discarded pops and unformatted handles. Clauses and oracles are in the
//! fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/uvm_cast";

#[test]
fn call_sources() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "call_sources",
        include_str!("../fixtures/sim/feature_completion/uvm_cast/call_sources.out"),
        &[],
        &[],
    );
}

#[test]
fn element_targets() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "element_targets",
        include_str!("../fixtures/sim/feature_completion/uvm_cast/element_targets.out"),
        &[],
        &[],
    );
}

#[test]
fn string_values() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "string_values",
        include_str!("../fixtures/sim/feature_completion/uvm_cast/string_values.out"),
        &[],
        &[],
    );
}

#[test]
fn handle_outputs() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "handle_outputs",
        include_str!("../fixtures/sim/feature_completion/uvm_cast/handle_outputs.out"),
        &[],
        &[],
    );
}

#[test]
fn call_formals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "call_formals",
        include_str!("../fixtures/sim/feature_completion/uvm_cast/call_formals.out"),
        &[],
        &[],
    );
}

#[test]
fn handle_conditionals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "handle_conditionals",
        include_str!("../fixtures/sim/feature_completion/uvm_cast/handle_conditionals.out"),
        &[],
        &[],
    );
}

#[test]
fn discarded_pops() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "discarded_pops",
        include_str!("../fixtures/sim/feature_completion/uvm_cast/discarded_pops.out"),
        &[],
        &[],
    );
}

#[test]
fn unformatted_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "unformatted_handles",
        include_str!("../fixtures/sim/feature_completion/uvm_cast/unformatted_handles.out"),
        &[],
        &[],
    );
}
