//! SIM-043: VPI callbacks, simulation control and evaluator integration.
//! Each fixture has a checked-in C application, loaded as a run-time plugin
//! through `LLG_VPI_PLUGIN` (every host) or linked as a static archive with
//! `--dpi-lib` (every host). The expected outputs are derived by hand in the
//! fixture readme from IEEE 1800-2009 4.4, 4.10 and 38.36.
use super::{sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/sim_043";
const VPI: &[&str] = &["--vpi"];
const PEEK: &[&str] = &["--define-system-task", "function int $peek(input int v)"];

/// The run-time plugin route: `<stem>.c` built into a shared library (a DLL
/// linked against a model import library on Windows).
fn plugin(directory: &sim_harness::TempDir, stem: &str) -> Option<String> {
    super::sim_040::plugin_library(SUITE, directory, stem, &[])
}

/// A01: callbacks of every time reason, value changes and the start/end
/// actions interleave with Active, NBA, Observed, Reactive and Postponed
/// work in the Table 4-1 order; a callback removes itself and registers a
/// replacement while it runs.
#[test]
fn callbacks_trace_every_region_of_a_time_slot() {
    let directory = sim_harness::TempDir::new("sim043").expect("temporary directory");
    let Some(library) = plugin(&directory, "regions") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "regions",
        include_str!("../fixtures/sim/feature_completion/sim_043/regions.out"),
        VPI,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// A01: registration and removal inside running callbacks, handle lifetime
/// of one-shot callbacks and the rejected reasons.
#[test]
fn callbacks_register_and_remove_callbacks_while_running() {
    let directory = sim_harness::TempDir::new("sim043").expect("temporary directory");
    let Some(library) = plugin(&directory, "reentry") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "reentry",
        include_str!("../fixtures/sim/feature_completion/sim_043/reentry.out"),
        VPI,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// A02: read-only phases and Postponed `$strobe`/`$monitor` evaluators
/// cannot write; a read-write callback's write wakes ordinary downstream
/// Active and NBA work.
#[test]
fn read_only_phases_reject_writes_and_read_write_callbacks_schedule() {
    let directory = sim_harness::TempDir::new("sim043").expect("temporary directory");
    let Some(library) = plugin(&directory, "readonly") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "readonly",
        include_str!("../fixtures/sim/feature_completion/sim_043/readonly.out"),
        PEEK,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// A03: vpiStop resumes under the default policy, vpiFinish ends the run
/// after the callback returns, final procedures and cbEndOfSimulation run
/// once.
#[test]
fn control_from_callbacks_follows_the_stop_and_finish_lifecycle() {
    let directory = sim_harness::TempDir::new("sim043").expect("temporary directory");
    let Some(library) = plugin(&directory, "control") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "control",
        include_str!("../fixtures/sim/feature_completion/sim_043/control.out"),
        VPI,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// A03: with `--stop-policy exit` a callback's vpiStop suspends the run;
/// closing the model then runs cbEndOfSimulation exactly once and no final
/// procedure.
#[test]
fn exit_policy_stop_from_a_callback_ends_once_at_close() {
    let directory = sim_harness::TempDir::new("sim043-exit").expect("temporary directory");
    let Some(library) = plugin(&directory, "control") else {
        return;
    };
    let expected = "[0] start\n\
vpiReset -> 0 LLG_VPI_UNSUPPORTED\n\
[3] after delay 3: vpiStop -> 1\n\
[3] end call 1: vpiFinish -> 1, vpiStop -> 1\n\
late cbEndOfSimulation: handle none\n";
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "control",
        &["--vpi", "--stop-policy", "exit"],
        &[("LLG_VPI_PLUGIN", &library)],
        &|label, output| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{label}: {stderr}");
            assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
            assert_eq!(
                stderr.matches("$stop at time 3").count(),
                1,
                "{label}: {stderr}"
            );
            assert!(!stderr.contains("$finish"), "{label}: {stderr}");
        },
    );
}

/// A03 (S43-D10): stdout and the single `$stop`/`$finish` report of a
/// `start_control` run; `SIM043_START_CONTROL` picks the start request.
fn start_control_check<'a>(
    expected: &'a str,
    report: &'a str,
) -> impl Fn(&str, &std::process::Output) + 'a {
    move |label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert_eq!(stderr.matches(report).count(), 1, "{label}: {stderr}");
    }
}

/// The default resume policy reports the stop at time 0 and continues.
#[test]
fn start_of_simulation_stop_resumes_under_the_default_policy() {
    let directory = sim_harness::TempDir::new("sim043-start").expect("temporary directory");
    let Some(library) = plugin(&directory, "start_control") else {
        return;
    };
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "start_control",
        VPI,
        &[("LLG_VPI_PLUGIN", &library), ("SIM043_START_CONTROL", "stop")],
        &start_control_check(
            "vpiStop -> 1 none\nstart returns\n[0] hdl: initial\n[2] hdl: later\n[2] final\n[2] end\n",
            "$stop at time 0",
        ),
    );
}

/// With `--stop-policy exit` the stop suspends before any process runs;
/// closing the model runs cbEndOfSimulation once and no final procedure.
#[test]
fn start_of_simulation_stop_suspends_before_the_run_under_exit_policy() {
    let directory = sim_harness::TempDir::new("sim043-start").expect("temporary directory");
    let Some(library) = plugin(&directory, "start_control") else {
        return;
    };
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "start_control",
        &["--vpi", "--stop-policy", "exit"],
        &[
            ("LLG_VPI_PLUGIN", &library),
            ("SIM043_START_CONTROL", "stop"),
        ],
        &start_control_check(
            "vpiStop -> 1 none\nstart returns\n[0] end\n",
            "$stop at time 0",
        ),
    );
}

/// A finish with a level outside 0-2 ends the run before time 0 processes:
/// final procedures and cbEndOfSimulation run at 0.
#[test]
fn start_of_simulation_finish_ends_before_the_run() {
    let directory = sim_harness::TempDir::new("sim043-start").expect("temporary directory");
    let Some(library) = plugin(&directory, "start_control") else {
        return;
    };
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "start_control",
        VPI,
        &[
            ("LLG_VPI_PLUGIN", &library),
            ("SIM043_START_CONTROL", "finish"),
        ],
        &start_control_check(
            "vpiFinish level 7 -> 1 none\nstart returns\n[0] final\n[0] end\n",
            "$finish at time 0",
        ),
    );
}

/// The trace application linked as a static archive, which also selects the
/// catalog without `--vpi`.
#[test]
fn static_archive_application_traces_regions() {
    let directory = sim_harness::TempDir::new("sim043-static").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "regions", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "regions",
        include_str!("../fixtures/sim/feature_completion/sim_043/regions.out"),
        &["--dpi-lib", archive.as_str()],
        &[],
    );
}

/// Without `--vpi`, `--dpi-lib` or a user system task call site the model
/// carries no catalog: an application loaded at run time still receives
/// callbacks, finds no objects and is told how to rebuild.
#[test]
fn catalog_is_emitted_only_when_an_application_can_use_it() {
    let directory = sim_harness::TempDir::new("sim043-catalog").expect("temporary directory");
    let Some(library) = plugin(&directory, "catalog") else {
        return;
    };
    for (args, found) in [(&[][..], false), (VPI, true)] {
        sim_cli::run_case_checked_matrix_with_env(
            SUITE,
            "catalog",
            args,
            &[("LLG_VPI_PLUGIN", &library)],
            &|label, output| {
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(output.status.success(), "{label}: {stderr}");
                let expected = format!(
                    "tb.v {}\n[2] after delay\n[3] hdl: v=9\n",
                    if found { "found" } else { "missing" }
                );
                assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
                assert_eq!(
                    stderr.contains("built without the VPI object catalog"),
                    !found,
                    "{label}: {stderr}"
                );
            },
        );
    }
}

/// The gate at generation: no catalog rows or VPI storage retention unless
/// `CodegenOptions::vpi` is set or the design calls a user system task.
#[test]
fn generated_model_omits_the_catalog_by_default() {
    use llg::core::compile;
    use llg::sim;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(format!("tests/fixtures/sim/{SUITE}/regions.sv"));
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("regions fixture should compile");
    let database =
        llg::core::db::Db::from_slang(&compiled.snapshot).expect("owned database import");
    let generate = |vpi| {
        sim::codegen::generate_from_db_with_codegen_options(
            &database,
            &sim::codegen::CodegenOptions {
                vpi,
                ..Default::default()
            },
        )
        .expect("regions model should lower")
        .model_c
    };
    let without = generate(false);
    let with = generate(true);
    assert!(without.contains("static llg_vpi_model_object_t* const llg_vpi_objects = NULL;"));
    assert!(!without.contains("\"tb.mem\""));
    assert!(with.contains("static llg_vpi_model_object_t llg_vpi_objects[] = {"));
    assert!(with.contains("\"tb.mem\""));
    // Expression and statement objects (ADV-019 families 3 and 4) are
    // generated only for VPI models.
    assert!(without.contains("llg_vpi_code_tables = NULL;"));
    assert!(!without.contains("llg_vpi_code_rows"));
    assert!(with.contains("static const llg_vpi_code_t llg_vpi_code_rows[] = {"));
    assert!(without.len() < with.len());
}
