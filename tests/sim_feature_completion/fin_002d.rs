//! FIN-002 step 4: the allocation failure matrix on generated models.
//!
//! Each `alloc_*` fixture exercises one runtime subsystem. Its model is
//! built once with the allocation fault injection of
//! `src/sim/rt/llg_alloc_fault.h` (`--cflags -DLLG_ALLOC_FAULT`) and then
//! run once per chosen allocation request with that request failing
//! (`LLG_ALLOC_FAULT=<n>`). Every such run must end in one of the defined
//! results below; a signal other than the abort, an uncatalogued line on
//! stderr (which is what a sanitizer or leak report is), output that is not
//! the beginning of the fault-free transcript, or a hang fails the test.
//!
//! | Subsystem | Fixture | Defined result of a failed request |
//! | --- | --- | --- |
//! | packed values, GMP kernels | `alloc_values` | fatal `SIM-031`, abort |
//! | strings | `alloc_strings` | fatal `SIM-031`, abort |
//! | dynamic arrays, queues, associative arrays | `alloc_containers` | fatal `SIM-031`, abort |
//! | class objects, collector | `alloc_classes` | fatal `SIM-031`, abort; a request of the collector's own bookkeeping is absorbed (the collection is put off) |
//! | processes, coroutine frames, forks | `alloc_processes` | fatal `SIM-031`, abort |
//! | mailboxes, semaphores, events | `alloc_sync` | fatal `SIM-031`, abort |
//! | constraint solver, random choices | `alloc_random` | fatal `SIM-031`, abort |
//! | covergroups, assertions, coverage database | `alloc_coverage` | fatal `SIM-031`, abort (after the last line when the database is written) |
//! | file I/O, memory images, scanning, plusargs | `alloc_files` | fatal `SIM-031`, abort |
//! | waveform dump | `alloc_waves` | error `SIM-018` and exit status 1: before time 0 when a signal cannot be registered, else the run completes without a usable dump; fatal `SIM-031` for the model's other requests |
//! | DPI-C imports and exports, VPI application | `alloc_foreign` | fatal `SIM-031`, abort (also inside a VPI routine or a call context) |
//!
//! A fault-free run of each model is compared with its `.out` first: the
//! industry-standard behaviour for the ten SystemVerilog-only fixtures, and
//! for `alloc_foreign` the lines derived in the fixture readme.
//!
//! The checked-in run fails a bounded number of requests per model (the
//! first and last ones and an even spread between them); with
//! `LLG_ALLOC_FAULT_ALL=1` it fails every request, and every request once
//! more in the mode where all later requests fail too. Commands and the
//! sanitizer lane: `tests/readme.md`, "Allocation fault injection".
use super::{sim_cli, sim_harness};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

const SUITE: &str = "feature_completion/fin_002d";
const RUN_TIMEOUT: Duration = Duration::from_secs(120);

/// Requests failed per model at each end of the run, and in between, when
/// `LLG_ALLOC_FAULT_ALL` does not ask for all of them.
const EDGE_REQUESTS: u64 = 24;
const SPREAD_REQUESTS: u64 = 160;

/// The runtime message ids of the defined results.
const ALLOCATION_ID: &str = "SIM-031";
const WAVEFORM_ID: &str = "SIM-018";

macro_rules! expected {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/fin_002d/",
            $name,
            ".out"
        ))
    };
}

/// The compiler flags of a fault injection build: the sanitizer flags of a
/// `$LLG_CFLAGS` lane stay in front of it (`--cflags` replaces the variable).
fn fault_cflags() -> String {
    match std::env::var("LLG_CFLAGS") {
        Ok(flags) if !flags.trim().is_empty() => format!("{flags} -DLLG_ALLOC_FAULT"),
        _ => "-DLLG_ALLOC_FAULT".to_owned(),
    }
}

/// The model `llg` built below `directory` (its default output directory).
fn model(directory: &Path) -> PathBuf {
    let name = format!("sim{}", std::env::consts::EXE_SUFFIX);
    let path = directory.join("build/sim/tb/build/bin").join(name);
    assert!(path.is_file(), "no built model at {}", path.display());
    path
}

/// The model of a fixture that `llg` built and ran in `built`'s directory.
fn built_model(fixture: &str, built: &(sim_harness::TempDir, Output)) -> PathBuf {
    assert_eq!(
        built.1.status.code(),
        Some(0),
        "{fixture}: {}",
        sim_cli::observed_output(&built.1)
    );
    model(built.0.path())
}

fn run(executable: &Path, directory: &Path, run_args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(executable);
    command
        .current_dir(directory)
        .args(run_args)
        .env_remove("LLG_ALLOC_FAULT")
        .env_remove("LLG_ALLOC_FAULT_COUNT")
        .envs(envs.iter().copied());
    sim_harness::run_command(&mut command, RUN_TIMEOUT)
        .unwrap_or_else(|error| panic!("{}: {error}", executable.display()))
}

/// The ids of the catalogued messages on `stderr`; a line that is not a
/// message (a sanitizer report, a C library complaint) fails the run.
fn message_ids(label: &str, stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .map(|line| {
            llg::core::report::parse(line)
                .unwrap_or_else(|| panic!("{label}: stderr line is not a message: {line}"))
                .id
        })
        .collect()
}

/// The requests to fail out of `1..=count`.
fn requests(count: u64) -> Vec<u64> {
    if std::env::var_os("LLG_ALLOC_FAULT_ALL").is_some()
        || count <= 2 * EDGE_REQUESTS + SPREAD_REQUESTS
    {
        return (1..=count).collect();
    }
    let step = (count - 2 * EDGE_REQUESTS).div_ceil(SPREAD_REQUESTS);
    let mut chosen: Vec<u64> = (1..=EDGE_REQUESTS).collect();
    chosen.extend(
        (EDGE_REQUESTS + 1..=count - EDGE_REQUESTS)
            .step_by(usize::try_from(step).expect("the step fits the host")),
    );
    chosen.extend(count - EDGE_REQUESTS + 1..=count);
    chosen
}

/// What a model may do besides the fatal allocation report.
#[derive(Clone, Copy, PartialEq)]
enum Also {
    Nothing,
    /// The waveform writer reports its own failures and the run goes on.
    WaveformError,
}

/// Compare the fault-free run of the fault injection build `built` of
/// `fixture` with `expected` and check the result of every chosen failed
/// request.
fn check(
    fixture: &str,
    built: (sim_harness::TempDir, Output),
    run_args: &[&str],
    expected: &str,
    also: Also,
) {
    let executable = built_model(fixture, &built);
    let directory = built.0;
    let count_file = directory.path().join("alloc_fault_count");
    let count_path = count_file.to_string_lossy().into_owned();
    let clean = run(
        &executable,
        directory.path(),
        run_args,
        &[("LLG_ALLOC_FAULT_COUNT", &count_path)],
    );
    let clean_stdout = String::from_utf8_lossy(&clean.stdout).into_owned();
    let clean_stderr = String::from_utf8_lossy(&clean.stderr).into_owned();
    assert_eq!(clean.status.code(), Some(0), "{fixture}: {clean_stderr}");
    assert_eq!(clean_stdout, expected, "{fixture}: fault-free transcript");
    let clean_ids = message_ids(fixture, &clean_stderr);
    let count: u64 = std::fs::read_to_string(&count_file)
        .unwrap_or_else(|error| panic!("{fixture}: no request count: {error}"))
        .trim()
        .parse()
        .expect("the request count is a number");
    assert!(count > 0, "{fixture}: the model allocates nothing");

    let all = std::env::var_os("LLG_ALLOC_FAULT_ALL").is_some();
    let modes: &[&str] = if all { &["", "+"] } else { &[""] };
    let mut fatal = 0u64;
    for mode in modes {
        for request in requests(count) {
            let label = format!("{fixture}: LLG_ALLOC_FAULT={request}{mode}");
            let setting = format!("{request}{mode}");
            let output = run(
                &executable,
                directory.path(),
                run_args,
                &[("LLG_ALLOC_FAULT", &setting)],
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let ids = message_ids(&label, &stderr);
            assert!(
                clean_stdout.starts_with(stdout.as_ref()),
                "{label}: output is not the beginning of the fault-free transcript:\n{stdout}\n{stderr}"
            );
            let reported = ids.iter().filter(|id| *id == ALLOCATION_ID).count();
            if sim_harness::ended_by_abort(&output.status) {
                assert_eq!(
                    reported, 1,
                    "{label}: abort without one {ALLOCATION_ID}:\n{stderr}"
                );
                assert_eq!(
                    ids.last().map(String::as_str),
                    Some(ALLOCATION_ID),
                    "{label}: a message follows the fatal one:\n{stderr}"
                );
                fatal += 1;
                continue;
            }
            assert_eq!(
                reported, 0,
                "{label}: {ALLOCATION_ID} without abort:\n{stderr}"
            );
            if also == Also::WaveformError && ids.iter().any(|id| id == WAVEFORM_ID) {
                assert_eq!(output.status.code(), Some(1), "{label}:\n{stderr}");
                continue;
            }
            // Absorbed: the run is the fault-free one.
            assert_eq!(
                output.status.code(),
                Some(0),
                "{label}:\n{stdout}\n{stderr}"
            );
            assert_eq!(stdout, clean_stdout, "{label}:\n{stderr}");
            assert_eq!(ids, clean_ids, "{label}:\n{stderr}");
        }
    }
    // The model's own start-up alone makes more requests than this.
    assert!(
        fatal >= 8,
        "{fixture}: only {fatal} of {count} requests were fatal"
    );
}

#[test]
fn allocation_failure_values() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_values", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_values",
        built,
        &[],
        expected!("alloc_values"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_strings() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_strings", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_strings",
        built,
        &[],
        expected!("alloc_strings"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_containers() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_containers", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_containers",
        built,
        &[],
        expected!("alloc_containers"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_classes_and_collector() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_classes", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_classes",
        built,
        &[],
        expected!("alloc_classes"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_processes_and_frames() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_processes", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_processes",
        built,
        &[],
        expected!("alloc_processes"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_mailboxes_semaphores_events() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_sync", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_sync",
        built,
        &[],
        expected!("alloc_sync"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_randomization() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_random", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_random",
        built,
        &[],
        expected!("alloc_random"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_coverage_and_assertions() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_coverage", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_coverage",
        built,
        &[],
        expected!("alloc_coverage"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_files_memories_plusargs() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_files", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_files",
        built,
        &["+ratio=0.75", "+name=llg", "+wide=123456789abcdef", "+flag"],
        expected!("alloc_files"),
        Also::Nothing,
    );
}

#[test]
fn allocation_failure_waveform() {
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_waves", true, &["--cflags", &cflags], &[]);
    check(
        "alloc_waves",
        built,
        &[],
        expected!("alloc_waves"),
        Also::WaveformError,
    );
}

#[test]
fn allocation_failure_dpi_and_vpi() {
    let directory = sim_harness::TempDir::new("fin002d-foreign").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "alloc_foreign", &[]);
    let cflags = fault_cflags();
    let built = sim_cli::run_case_keeping_dir(
        SUITE,
        "alloc_foreign",
        true,
        &[
            "--cflags",
            &cflags,
            "--dpi-lib",
            &archive,
            "--define-system-task",
            "task $inspect(string path)",
            "--define-system-task",
            "task $watch(string path)",
            "--define-system-task",
            "task $poke(string path, int value)",
            "--define-system-task",
            "task $later(int delay)",
        ],
        &[],
    );
    check(
        "alloc_foreign",
        built,
        &[],
        expected!("alloc_foreign"),
        Also::Nothing,
    );
}

/// The objects of a fault injection build that still call the C library's
/// allocator by name: the wrapper's own unit and the vendored libraries the
/// readme lists as not covered. A runtime unit that does not include
/// `llg_msg.h` before its first allocation would show up here. The listing
/// is `nm`'s, so the check runs on Unix hosts and returns at once on Windows.
#[test]
fn allocation_fault_build_renames_every_runtime_allocation() {
    if cfg!(windows) {
        return;
    }
    let cflags = fault_cflags();
    let built =
        sim_cli::run_case_keeping_dir(SUITE, "alloc_waves", true, &["--cflags", &cflags], &[]);
    built_model("alloc_waves", &built);
    let project = built.0.path().join("build/sim/tb");
    let cache = std::fs::read_to_string(project.join("build/CMakeCache.txt"))
        .expect("the model's CMake cache");
    let archive = cache
        .lines()
        .find_map(|line| line.strip_prefix("LLG_RUNTIME_LIBRARY:"))
        .and_then(|rest| rest.split_once('='))
        .map(|(_, path)| PathBuf::from(path))
        .expect("the model links a runtime archive");
    let mut objects = vec![archive];
    let mut pending = vec![project.join("build")];
    while let Some(folder) = pending.pop() {
        for entry in std::fs::read_dir(&folder).expect("the model build tree") {
            let path = entry.expect("a build tree entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "o") {
                objects.push(path);
            }
        }
    }
    let mut command = Command::new("nm");
    command.args(["-A", "-u"]).args(&objects);
    let output = sim_harness::run_command(&mut command, RUN_TIMEOUT).expect("run nm");
    assert!(
        output.status.success(),
        "nm: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let listing = String::from_utf8_lossy(&output.stdout);
    let mut wrapped = 0;
    let mut direct: Vec<&str> = Vec::new();
    for line in listing.lines() {
        // `<archive>:<member>: U <symbol>`; Mach-O symbols have a leading `_`.
        let Some((object, symbol)) = line.rsplit_once(" U ") else {
            continue;
        };
        let symbol = symbol.trim();
        let symbol = symbol.strip_prefix('_').unwrap_or(symbol);
        let object = object.trim_end().trim_end_matches(':');
        let member = object.rsplit([':', '/']).next().unwrap_or(object);
        if symbol.starts_with("llg_alloc_fault_") {
            wrapped += 1;
        } else if matches!(symbol, "malloc" | "calloc" | "realloc") && !direct.contains(&member) {
            direct.push(member);
        }
    }
    assert!(
        wrapped > 20,
        "few renamed allocations ({wrapped}):\n{listing}"
    );
    // llg_rt.c holds the wrapper; memory.c is GMP's default allocator, which
    // a model replaces when it starts (compact values with GMP kernels only);
    // the rest writes FST dumps.
    const DIRECT: &[&str] = &[
        "fstapi.c.o",
        "gzlib.c.o",
        "gzread.c.o",
        "gzwrite.c.o",
        "llg_rt.c.o",
        "lz4.c.o",
        "memory.c.o",
        "zutil.c.o",
    ];
    assert!(direct.contains(&"llg_rt.c.o"), "{listing}");
    for member in direct {
        assert!(
            DIRECT.contains(&member),
            "{member} calls the C library's allocator directly:\n{listing}"
        );
    }
}
