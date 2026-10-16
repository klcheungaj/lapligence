//! SIM-042: commonly used VPI object and value access. Every fixture has a
//! checked-in C application, loaded as a run-time plugin through
//! `LLG_VPI_PLUGIN` (every host) or linked as a static archive with
//! `--dpi-lib` (every host). The expected outputs are
//! derived by hand in the fixture readme from IEEE 1800-2009 clauses 36-38.
use super::{sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/sim_042";

const OBJECTS: &[&str] = &["--define-system-task", "task $walk()"];
const VALUES: &[&str] = &[
    "--define-system-task",
    "task $formats()",
    "--define-system-task",
    "task $schedule()",
    "--define-system-task",
    "task $late()",
    "--define-system-task",
    "task $force_some()",
    "--define-system-task",
    "task $release_some()",
];
const SYSTF: &[&str] = &[
    "--define-system-task",
    "function logic signed [11:0] $sum(input logic [7:0] a, input logic signed [11:0] b)",
    "--define-system-task",
    "function int $noret()",
    "--define-system-task",
    "function real $half(input real x)",
    "--define-system-task",
    "function time $big()",
    "--define-system-task",
    "task $noop()",
    "--define-system-task",
    "task $args(input logic [7:0] a, input logic [7:0] b, input logic [3:0] c, \
     input logic [2:0] d, input logic [23:0] e, input real f, input logic [7:0] g)",
    "--define-system-task",
    "task $count()",
    "--define-system-task",
    "task $late_put()",
];
const ERRORS: &[&str] = &[
    "--define-system-task",
    "task $errors()",
    "--define-system-task",
    "task $keep(input logic [7:0] x)",
    "--define-system-task",
    "task $use_kept()",
    "--define-system-task",
    "task $stress()",
];

/// The run-time plugin route: `<stem>.c` built into a shared library (a DLL
/// linked against a model import library on Windows).
fn plugin(directory: &sim_harness::TempDir, stem: &str, flags: &[&str]) -> Option<String> {
    super::sim_040::plugin_library(SUITE, directory, stem, flags)
}

#[test]
fn objects_traverse_hierarchy_and_write_selected_destinations() {
    let directory = sim_harness::TempDir::new("sim042").expect("temporary directory");
    let Some(library) = plugin(&directory, "objects", &[]) else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "objects",
        include_str!("../fixtures/sim/feature_completion/sim_042/objects.out"),
        OBJECTS,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

#[test]
fn values_cover_every_format_and_put_mode() {
    let directory = sim_harness::TempDir::new("sim042").expect("temporary directory");
    let Some(library) = plugin(&directory, "values", &[]) else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "values",
        include_str!("../fixtures/sim/feature_completion/sim_042/values.out"),
        VALUES,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

#[test]
fn system_functions_follow_the_return_conventions() {
    let directory = sim_harness::TempDir::new("sim042").expect("temporary directory");
    let Some(library) = plugin(&directory, "systf", &[]) else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "systf",
        include_str!("../fixtures/sim/feature_completion/sim_042/systf.out"),
        SYSTF,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

#[test]
fn errors_report_levels_instead_of_guessed_success() {
    let directory = sim_harness::TempDir::new("sim042").expect("temporary directory");
    let Some(library) = plugin(&directory, "errors", &[]) else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "errors",
        include_str!("../fixtures/sim/feature_completion/sim_042/errors.out"),
        ERRORS,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// The application linked as a static archive runs its own startup table
/// (`vlog_startup_routines`) without `LLG_VPI_PLUGIN`, on every host.
#[test]
fn static_archive_application_runs_its_startup_table() {
    let directory = sim_harness::TempDir::new("sim042-static").expect("temporary directory");
    let objects = super::sim_041::static_archive(SUITE, &directory, "objects", &[]);
    let mut args = OBJECTS.to_vec();
    args.extend(["--dpi-lib", objects.as_str()]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "objects",
        include_str!("../fixtures/sim/feature_completion/sim_042/objects.out"),
        &args,
        &[],
    );
    let systf = super::sim_041::static_archive(SUITE, &directory, "systf", &[]);
    let mut args = SYSTF.to_vec();
    args.extend(["--dpi-lib", systf.as_str()]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "systf",
        include_str!("../fixtures/sim/feature_completion/sim_042/systf.out"),
        &args,
        &[],
    );
}

/// A call whose name the application never registered stops the model at
/// startup with a VPI error (38.37 registration; A03).
#[test]
fn unregistered_call_stops_at_startup() {
    let directory = sim_harness::TempDir::new("sim042-missing").expect("temporary directory");
    let Some(library) = plugin(&directory, "errors", &[]) else {
        return;
    };
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "missing",
        &["--define-system-task", "task $absent()"],
        &[("LLG_VPI_PLUGIN", &library)],
        &|label, output| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
            assert!(output.stdout.is_empty(), "{label}: {output:?}");
            assert!(
                stderr.contains("llg: VPI error: no VPI registration for `$absent` (called at "),
                "{label}: {stderr}"
            );
            assert!(stderr.contains("missing.sv:6)"), "{label}: {stderr}");
        },
    );
}

/// `char` is unsigned on the AArch64 Linux ABI. The application and the
/// model built with `-funsigned-char` (MSVC `/J`) must read and write the
/// same values.
#[test]
fn values_are_unchanged_when_char_is_unsigned() {
    let directory = sim_harness::TempDir::new("sim042-uchar").expect("temporary directory");
    let Some(library) = plugin(&directory, "values", &["-funsigned-char"]) else {
        return;
    };
    let cflags = super::sim_040::unsigned_char_cflags();
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            SUITE,
            "values",
            optimized,
            VALUES,
            &[("LLG_CFLAGS", &cflags), ("LLG_VPI_PLUGIN", &library)],
            &[],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            include_str!("../fixtures/sim/feature_completion/sim_042/values.out")
        );
    }
}

/// A02: the generated model owns every name, shape and value the bridge
/// serves, so registration and function results work after the native
/// frontend is destroyed, in both optimizer modes.
#[test]
fn systf_results_survive_frontend_destruction() {
    use llg::core::compile;
    use llg::sim;
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let directory = sim_harness::TempDir::new("sim042-frontend").expect("temporary directory");
    let Some(library) = plugin(&directory, "systf", &[]) else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(format!("tests/fixtures/sim/{SUITE}/systf.sv"));
    let prototypes = SYSTF
        .iter()
        .skip(1)
        .step_by(2)
        .map(|prototype| (*prototype).to_owned())
        .collect();
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        system_subroutines: prototypes,
        ..Default::default()
    })
    .expect("systf fixture should compile");
    let database =
        llg::core::db::Db::from_slang(&compiled.snapshot).expect("owned database import");
    drop(compiled);
    let value_config = sim::value_backend::ValueConfig::from_env().expect("value selection");
    for (label, optimization) in [
        ("optimized", sim::opt::OptConfig::default()),
        ("unoptimized", sim::opt::OptConfig::none()),
    ] {
        let generated = sim::codegen::generate_from_db_with_codegen_options(
            &database,
            &sim::codegen::CodegenOptions {
                optimization,
                value_config,
                ..Default::default()
            },
        )
        .expect("systf model should lower");
        let model = directory.path().join(label);
        std::fs::create_dir_all(&model).expect("model directory");
        let executable = sim::build::build_model_cmake_with_opts(
            &model,
            &[("model.c", generated.model_c.as_str())],
            &sim::build::CmakeBuildOpts {
                value_config,
                ..Default::default()
            },
        )
        .expect("systf model should build");
        let mut command = std::process::Command::new(&executable);
        command.env("LLG_VPI_PLUGIN", &library).current_dir(&model);
        let output = sim_harness::run_command(&mut command, std::time::Duration::from_secs(60))
            .expect("generated model should run");
        assert!(output.status.success(), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            include_str!("../fixtures/sim/feature_completion/sim_042/systf.out"),
            "{label}"
        );
    }
}
