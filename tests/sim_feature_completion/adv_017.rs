//! ADV-017: assertion and coverage native API object lifecycles (IEEE
//! 1800-2009 Clauses 39 and 40). Each fixture's C application is loaded as
//! a run-time plugin (`LLG_VPI_PLUGIN`, a DLL linked against the model
//! import library on Windows); `lifecycle` also links as a static archive
//! (`--dpi-lib`, every host). Expected outputs are derived by hand in the
//! fixture readme.
use super::sim_cli;
use super::sim_harness;

const SUITE: &str = "feature_completion/adv_017";

fn plugin(directory: &sim_harness::TempDir, stem: &str) -> Option<String> {
    super::sim_040::plugin_library(SUITE, directory, stem, &[])
}

const LIFECYCLE: &[&str] = &[
    "--define-system-task",
    "task $attach()",
    "--define-system-task",
    "task $ctl(input integer n)",
    "--define-system-task",
    "task $report()",
];

/// A01/A02: attempt callbacks on multiclock, future-global, checker-owned
/// and cover assertions agree with the HDL action counters and the 40.5.3
/// counters; a kill and a disabled pass action are told apart; a removed
/// record runs no more.
#[test]
fn assertion_callbacks_correlate_with_hdl_and_coverage_counters() {
    let directory = sim_harness::TempDir::new("adv017-life").expect("temporary directory");
    let Some(library) = plugin(&directory, "lifecycle") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "lifecycle",
        include_str!("../fixtures/sim/feature_completion/adv_017/lifecycle.out"),
        LIFECYCLE,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

#[test]
fn assertion_callbacks_link_from_a_static_archive() {
    let directory = sim_harness::TempDir::new("adv017-static").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "lifecycle", &[]);
    let mut args = LIFECYCLE.to_vec();
    args.extend(["--dpi-lib", archive.as_str()]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "lifecycle",
        include_str!("../fixtures/sim/feature_completion/adv_017/lifecycle.out"),
        &args,
        &[],
    );
}

/// A03: every 39.5 operation on concurrent, immediate, deferred and
/// sub-instance assertions, the callbacks it raises, and the specified
/// errors (step callbacks, wrong handles, after vpiAssertionSysEnd).
#[test]
fn vpi_control_drives_assertions_and_raises_their_callbacks() {
    let directory = sim_harness::TempDir::new("adv017-ctl").expect("temporary directory");
    let Some(library) = plugin(&directory, "control") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "control",
        include_str!("../fixtures/sim/feature_completion/adv_017/control.out"),
        &[
            "--define-system-task",
            "task $setup()",
            "--define-system-task",
            "task $ctl(input integer n)",
            "--define-system-task",
            "task $report()",
        ],
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// 39.3 static information of concurrent, immediate, deferred, unnamed and
/// checker-owned assertions; a callback that kills its own attempt and one
/// that removes itself.
#[test]
fn static_assertion_information_and_reentrant_callbacks() {
    let directory = sim_harness::TempDir::new("adv017-obj").expect("temporary directory");
    let Some(library) = plugin(&directory, "objects") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "objects",
        include_str!("../fixtures/sim/feature_completion/adv_017/objects.out"),
        &[
            "--define-system-task",
            "task $objects()",
            "--define-system-task",
            "task $summary()",
        ],
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// A01/A03: assertion relations (37.45-37.51), sequence and property
/// instances and declarations (37.47-37.50), failExpr and the callbacks of
/// an instance that is an assertion's whole property (39.4.2, A17-D4 and
/// A17-D5); a nested instance, a declaration body and a formal default are
/// specified errors.
#[test]
fn assertion_relations_reach_clocks_conditions_properties_and_instances() {
    let directory = sim_harness::TempDir::new("adv017-rel").expect("temporary directory");
    let Some(library) = plugin(&directory, "relations") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "relations",
        include_str!("../fixtures/sim/feature_completion/adv_017/relations.out"),
        &["--vpi", "--define-system-task", "task $relations()"],
        &[("LLG_VPI_PLUGIN", &library)],
    );
}
