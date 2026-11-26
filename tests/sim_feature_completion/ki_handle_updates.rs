//! Nonblocking assignments to handles, arrays of them and parts of static
//! class properties
//! ([readme](../fixtures/sim/feature_completion/ki_handle_updates/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_handle_updates";

#[test]
fn built_in_handles_are_nonblocking_targets() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "kinds",
        include_str!("../fixtures/sim/feature_completion/ki_handle_updates/kinds.out"),
        &[],
        &[],
    );
}

#[test]
fn delayed_updates_of_built_in_handles_wake_their_waiters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "delays",
        include_str!("../fixtures/sim/feature_completion/ki_handle_updates/delays.out"),
        &[],
        &[],
    );
}

#[test]
fn event_controlled_updates_reach_elements_of_handle_arrays() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "elements",
        include_str!("../fixtures/sim/feature_completion/ki_handle_updates/elements.out"),
        &[],
        &[],
    );
}

#[test]
fn event_controlled_updates_reach_elements_of_string_arrays() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "strings",
        include_str!("../fixtures/sim/feature_completion/ki_handle_updates/strings.out"),
        &[],
        &[],
    );
}

#[test]
fn updates_of_parts_of_static_properties_end_with_their_issuer() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "static_ends",
        include_str!("../fixtures/sim/feature_completion/ki_handle_updates/static_ends.out"),
        &[],
        &[],
    );
}

#[test]
fn a_handle_in_an_object_is_no_nonblocking_target() {
    sim_cli::reject_case(
        SUITE,
        "neg_class_property",
        "neg_class_property.sv:9:5: [CODEGEN-002] nonblocking assignment to class property `next` in `tb` is illegal (IEEE 1800-2009 6.21",
    );
}

#[test]
fn operands_are_evaluated_like_those_of_a_packed_element() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "operand_order",
        include_str!("../fixtures/sim/feature_completion/ki_handle_updates/operand_order.out"),
        &[],
        &[],
    );
}
