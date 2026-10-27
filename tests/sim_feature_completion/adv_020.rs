//! ADV-020 (A01 groundwork): VPI header and symbol parity and the
//! multichannel-descriptor routines. `symbols.c` takes the address of every
//! routine IEEE 1800-2009 Annex K and Annex M declare, so it loads as a
//! run-time plugin (`LLG_VPI_PLUGIN`, a DLL linked against the model's
//! import library on Windows) and links as a static archive (`--dpi-lib`,
//! every host) only when the model defines all of them; `mcd.c` exercises
//! `vpi_mcd_*` on the `$fopen` table. Expected outputs are derived by hand in
//! the fixture readme from 38.2 and 38.24-38.29.
use super::sim_cli;
use super::sim_harness;

const SUITE: &str = "feature_completion/adv_020";

const SYMBOLS: &[&str] = &["--define-system-task", "task $symbols()"];
const MCD: &[&str] = &[
    "--define-system-task",
    "task $mcd(input integer m, input integer fd)",
    "--define-system-task",
    "task $after_close(input integer m)",
    "--define-system-task",
    "function integer $vopen()",
    "--define-system-task",
    "task $show_files()",
];

/// The run-time plugin route: `<stem>.c` built into a shared library (a DLL
/// linked against a model import library on Windows).
fn plugin(directory: &sim_harness::TempDir, stem: &str) -> Option<String> {
    super::sim_040::plugin_library(SUITE, directory, stem, &[])
}

#[test]
fn every_annex_k_and_m_routine_resolves_in_a_plugin() {
    let directory = sim_harness::TempDir::new("adv020").expect("temporary directory");
    let Some(library) = plugin(&directory, "symbols") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "symbols",
        include_str!("../fixtures/sim/feature_completion/adv_020/symbols.out"),
        SYMBOLS,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// The static archive needs no import library, so this route also covers a
/// Windows model linking every routine directly.
#[test]
fn every_annex_k_and_m_routine_links_from_a_static_archive() {
    let directory = sim_harness::TempDir::new("adv020-static").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "symbols", &[]);
    let mut args = SYMBOLS.to_vec();
    args.extend(["--dpi-lib", archive.as_str()]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "symbols",
        include_str!("../fixtures/sim/feature_completion/adv_020/symbols.out"),
        &args,
        &[],
    );
}

#[test]
fn mcd_routines_share_the_fopen_table() {
    let directory = sim_harness::TempDir::new("adv020-mcd").expect("temporary directory");
    let Some(library) = plugin(&directory, "mcd") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "mcd",
        include_str!("../fixtures/sim/feature_completion/adv_020/mcd.out"),
        MCD,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

const VALUE_ARRAY: &[&str] = &[
    "--define-system-task",
    "task $va_get()",
    "--define-system-task",
    "task $va_put()",
];

#[test]
fn value_arrays_read_and_write_sections_in_every_format() {
    let directory = sim_harness::TempDir::new("adv020-va").expect("temporary directory");
    let Some(library) = plugin(&directory, "value_array") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "value_array",
        include_str!("../fixtures/sim/feature_completion/adv_020/value_array.out"),
        VALUE_ARRAY,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

const FORCE_CALLBACKS: &[&str] = &[
    "--define-system-task",
    "task $watch()",
    "--define-system-task",
    "task $vpi_force()",
    "--define-system-task",
    "task $stale()",
];

#[test]
fn force_and_release_statements_run_their_callbacks() {
    let directory = sim_harness::TempDir::new("adv020-force").expect("temporary directory");
    let Some(library) = plugin(&directory, "force_callbacks") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "force_callbacks",
        include_str!("../fixtures/sim/feature_completion/adv_020/force_callbacks.out"),
        FORCE_CALLBACKS,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

const DISABLE: &[&str] = &[
    "--define-system-task",
    "task $watch_disable()",
    "--define-system-task",
    "task $inner()",
];

#[test]
fn disable_runs_cb_disable_of_blocks_tasks_and_enclosed_calls() {
    let directory = sim_harness::TempDir::new("adv020-disable").expect("temporary directory");
    let Some(library) = plugin(&directory, "disable") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "disable",
        include_str!("../fixtures/sim/feature_completion/adv_020/disable.out"),
        DISABLE,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

#[test]
fn delays_read_and_write_paths_checks_primitives_and_assignments() {
    let directory = sim_harness::TempDir::new("adv020-delays").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "delays", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "delays",
        include_str!("../fixtures/sim/feature_completion/adv_020/delays.out"),
        &[
            "--define-system-task",
            "task $delays(input integer stage)",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}
