//! Members, elements and selects of static class properties
//! ([readme](../fixtures/sim/feature_completion/ki_static_members/readme.md)).

use super::{c_compiler, sim_cli, sim_harness};

const SUITE: &str = "feature_completion/ki_static_members";

#[test]
fn packed_members_are_read_and_written_alone() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "packed",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/packed.out"),
        &[],
        &[],
    );
}

#[test]
fn unpacked_members_are_read_and_written_alone() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "unpacked",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/unpacked.out"),
        &[],
        &[],
    );
}

#[test]
fn tagged_members_are_checked() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "tagged",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/tagged.out"),
        &[],
        &[],
    );
}

#[test]
fn members_are_actuals_and_assignment_targets() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "uses",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/uses.out"),
        &[],
        &[],
    );
}

#[test]
fn members_wake_their_waiters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "waits",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/waits.out"),
        &[],
        &[],
    );
}

#[test]
fn members_are_in_implicit_sensitivity() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "implicit",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/implicit.out"),
        &[],
        &[],
    );
}

#[test]
fn members_are_named_from_every_scope() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "scopes",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/scopes.out"),
        &[],
        &[],
    );
}

#[test]
fn random_members_are_constrained() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "random",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/random.out"),
        &[],
        &[],
    );
}

#[test]
fn random_members_reached_through_handles_are_variables() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "random_reach",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/random_reach.out"),
        &[],
        &[],
    );
}

#[test]
fn vpi_finds_members_of_static_properties() {
    let directory = sim_harness::TempDir::new("ki-static-members").expect("temporary directory");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/sim/{SUITE}/vpi_members.c"));
    let archive = c_compiler::static_archive(&source, directory.path(), "vpi_members", &[])
        .expect("VPI archive");
    let archive = archive.to_string_lossy().into_owned();
    sim_cli::run_case_backend_parity(
        SUITE,
        "vpi_members",
        include_str!("../fixtures/sim/feature_completion/ki_static_members/vpi_members.out"),
        &[
            "--define-system-task",
            "task $show()",
            "--define-system-task",
            "task $write_all()",
            "--dpi-lib",
            &archive,
        ],
        &[],
    );
}

#[test]
fn unsupported_forms_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_effects",
        "neg_effects.sv:15:5: [CODEGEN-001] the object expression in front of static property `v` calls a subroutine or assigns, which is not supported: name the property through its class",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_interface_queue",
        "neg_interface_queue.sv:12:11: [CODEGEN-001] unsupported object method receiver",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_random_record",
        "neg_random_record.sv:12:15: [CODEGEN-003] randomization: random property `rec` of class `st` is not a packed integral value",
    );
}
