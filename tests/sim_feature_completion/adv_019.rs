//! ADV-019 families 1 and 2. The C walkers print the typespec tree of each
//! declaration (37.23); read, find, watch and write structure and union
//! members (37.17, 37.24); and walk the structure objects of a design:
//! ports, interface ports, processes, continuous assignments, tasks and
//! functions, primitives, instance arrays and named events (37.5-37.15,
//! 37.25, 37.31, 37.37, 37.43, 37.58). All link as static archives with
//! `--dpi-lib`, so they run on every host. Expected outputs are derived by
//! hand in the fixture readme.
use super::sim_cli;
use super::sim_harness;

const SUITE: &str = "feature_completion/adv_019";

#[test]
fn typespec_trees_name_typedefs_ranges_members_and_enum_constants() {
    let directory = sim_harness::TempDir::new("adv019-types").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "typespecs", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "typespecs",
        include_str!("../fixtures/sim/feature_completion/adv_019/typespecs.out"),
        &[
            "--define-system-task",
            "task $typespecs()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn structure_and_union_members_read_watch_and_write_their_fields() {
    let directory = sim_harness::TempDir::new("adv019-members").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "members", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "members",
        include_str!("../fixtures/sim/feature_completion/adv_019/members.out"),
        &[
            "--define-system-task",
            "task $members()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn structure_walker_reaches_ports_processes_assignments_primitives_and_arrays() {
    let directory = sim_harness::TempDir::new("adv019-structure").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "structure", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "structure",
        include_str!("../fixtures/sim/feature_completion/adv_019/structure.out"),
        &[
            "--define-system-task",
            "task $structure()",
            "--define-system-task",
            "task $fire()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn module_paths_and_timing_checks_walk_their_terminals_and_conditions() {
    let directory = sim_harness::TempDir::new("adv019-timing").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "timing", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "timing",
        include_str!("../fixtures/sim/feature_completion/adv_019/timing.out"),
        &[
            "--define-system-task",
            "task $timing()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}
