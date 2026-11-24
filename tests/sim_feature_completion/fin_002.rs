//! FIN-002 steps 1 and 2: composed cross-feature scenarios. Six class-based
//! testbench designs (RTL with generate hierarchies, interfaces with
//! clocking blocks, classes, mailboxes, randomization, assertions, a
//! checker, covergroups, DPI-C and VPI applications, waveforms) and six
//! gate-level designs (cell netlists, UDPs with notifiers, strengths,
//! switch networks, specify paths with SDF annotation, timing checks, VPI
//! inspection) each run in both optimizer modes on the legacy,
//! compact/portable and compact/GMP value backends. The expected outputs
//! are industry-standard behaviour, or derived from IEEE 1800-2009 where
//! the fixture readme says so; lines of one time step compare as a set
//! because the order of processes within a time step is not defined (4.7).
use super::sim_047::fst_dump;
use super::sim_099::llg_lines;
use super::{sim_cli, sim_harness, vcd};
use std::process::Output;

const SUITE: &str = "feature_completion/fin_002";

/// The system tasks and functions of `shared_force_vpi.c`.
const FORCE_APP: &[&str] = &[
    "--define-system-task",
    "task $vforce(string path, int value)",
    "--define-system-task",
    "task $vrelease(string path)",
    "--define-system-task",
    "task $vput(string path, int value)",
    "--define-system-task",
    "function int $vget(string path)",
    "--define-system-task",
    "task $vwatch(string path)",
    "--define-system-task",
    "task $vreport()",
];

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/fin_002/",
            $name
        ))
    };
}

/// `<stem>.c` of this suite built into a static archive for `--dpi-lib`
/// (every host; `$LLG_CFLAGS` instruments it in sanitizer runs).
fn archive(directory: &sim_harness::TempDir, stem: &str) -> String {
    super::sim_041::static_archive(SUITE, directory, stem, &[])
}

/// The lines of a transcript grouped by time step, each group sorted. A
/// step starts at a line whose `[time]` prefix differs from the open
/// step's; lines without a prefix belong to the open step.
fn time_steps(text: &str) -> Vec<(String, Vec<String>)> {
    let mut steps: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        let time = line
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .map(|(time, _)| time);
        match (time, steps.last_mut()) {
            (Some(time), Some((open, lines))) if open == time => lines.push(line.to_owned()),
            (None, Some((_, lines))) => lines.push(line.to_owned()),
            (time, _) => steps.push((time.unwrap_or("").to_owned(), vec![line.to_owned()])),
        }
    }
    for (_, lines) in &mut steps {
        lines.sort();
    }
    steps
}

/// The lowering and run-time messages of a run, sorted, without the hint
/// that follows a timing violation report.
fn messages(output: &Output) -> Vec<String> {
    let mut lines: Vec<String> = llg_lines(output)
        .into_iter()
        .map(|line| match line.split_once(" (run with ") {
            Some((report, _)) => report.to_owned(),
            None => line,
        })
        .collect();
    lines.sort();
    lines
}

/// A run that succeeded with `expected` on stdout, compared per time step,
/// and exactly `expected_messages` as its lowering and run-time messages.
fn assert_scenario(label: &str, output: &Output, expected: &str, expected_messages: &[&str]) {
    assert!(
        output.status.success(),
        "{label}: {}",
        sim_cli::observed_output(output)
    );
    assert_eq!(
        time_steps(&String::from_utf8_lossy(&output.stdout)),
        time_steps(expected),
        "{label}"
    );
    let mut sorted = expected_messages.to_vec();
    sorted.sort_unstable();
    assert_eq!(messages(output), sorted, "{label}");
}

/// The check of [`assert_scenario`] for a checked-matrix run.
fn scenario<'a>(
    expected: &'a str,
    expected_messages: &'a [&'a str],
) -> impl Fn(&str, &Output) + 'a {
    move |label, output| assert_scenario(label, output, expected, expected_messages)
}

/// `--dpi-lib` arguments for `archives` followed by `rest`.
fn linked<'a>(archives: &'a [String], rest: &[&'a str]) -> Vec<&'a str> {
    let mut args = Vec::new();
    for archive in archives {
        args.extend(["--dpi-lib", archive.as_str()]);
    }
    args.extend_from_slice(rest);
    args
}

/// Testbench scenario 1: a VPI force, release and deposit on variables that
/// a clocking block samples and an assertion clocked by it checks.
#[test]
fn testbench_vpi_force_under_clocking_sample() {
    let directory = sim_harness::TempDir::new("fin002-force").expect("temporary directory");
    let archives = [archive(&directory, "shared_force_vpi")];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "tb_force_clocking",
        &linked(&archives, FORCE_APP),
        &[],
        &scenario(fixture!("tb_force_clocking.out"), &[]),
    );
}

/// Testbench scenario 2: DPI-C exports disable and kill class methods that
/// wait, one of them inside an import task.
#[test]
fn testbench_dpi_exports_disable_waiting_class_methods() {
    let directory = sim_harness::TempDir::new("fin002-disable").expect("temporary directory");
    let archives = [archive(&directory, "tb_dpi_disable_dpi")];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "tb_dpi_disable",
        &linked(&archives, &[]),
        &[],
        &scenario(fixture!("tb_dpi_disable.out"), &[]),
    );
}

/// Testbench scenario 3: assertion action blocks sample covergroups; the
/// checker and its module twin give the same transcript.
#[test]
fn testbench_assertion_actions_sample_covergroups() {
    for args in [&[][..], &["--define", "CHECKER_AS_MODULE"]] {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "tb_cov_assert",
            args,
            &[],
            &scenario(fixture!("tb_cov_assert.out"), &[]),
        );
    }
}

/// Testbench scenario 4: randomized packets through a mailbox whose
/// consumer `disable fork` kills.
#[test]
fn testbench_randomized_packets_survive_a_killed_consumer() {
    sim_cli::run_case_checked_matrix_with_files(SUITE, "tb_rand_mailbox_kill", &[], &[], &scenario(fixture!("tb_rand_mailbox_kill.out"), &[
            "WARNING: [SIM-011] randomize() at tb:260:13: the constraints cannot be satisfied; \
           randomize returns 0",
        ]));
}

/// Testbench scenario 5: a virtual interface rebound under pending waits.
#[test]
fn testbench_virtual_interface_swapped_under_pending_waits() {
    let directory = sim_harness::TempDir::new("fin002-vif").expect("temporary directory");
    let archives = [
        archive(&directory, "tb_vif_swap_dpi"),
        archive(&directory, "shared_force_vpi"),
    ];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "tb_vif_swap",
        &linked(&archives, FORCE_APP),
        &[],
        &scenario(fixture!("tb_vif_swap.out"), &[]),
    );
}

/// Testbench scenario 6: a program-block environment with a C reference
/// model and a VPI application that stalls the driver from a callback.
#[test]
fn testbench_program_environment_with_c_model_and_vpi_stall() {
    let directory = sim_harness::TempDir::new("fin002-fifo").expect("temporary directory");
    let archives = [
        archive(&directory, "tb_fifo_env_dpi"),
        archive(&directory, "tb_fifo_env_vpi"),
    ];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "tb_fifo_env",
        &linked(&archives, &[]),
        &[],
        &scenario(fixture!("tb_fifo_env.out"), &[]),
    );
}

const CELLS_VIOLATIONS: &[&str] = &[
    "WARNING: gl_cells_sdf.sv:98:5: [SIM-015] timing violation in tb.dut.f0 at time 25000 \
     (1ps): $setup(D:24960, posedge CK &&& ...:25000, 600)",
    "WARNING: gl_cells_sdf.sv:99:5: [SIM-015] timing violation in tb.dut.f1 at time 45300 \
     (1ps): $hold(posedge CK &&& ...:45000, D:45300, 450)",
    "WARNING: gl_cells_sdf.sv:100:5: [SIM-015] timing violation in tb.dut.f1 at time 62800 \
     (1ps): $width(negedge RN:62000, posedge RN:62800, 1000, 0)",
    "WARNING: gl_cells_sdf.sv:100:5: [SIM-015] timing violation in tb.dut.f0 at time 62800 \
     (1ps): $width(negedge RN:62000, posedge RN:62800, 1200, 0)",
];

/// Gate-level scenario 1: an annotated cell netlist whose setup, hold and
/// width violations toggle notifiers, from `$sdf_annotate` and from `--sdf`.
#[test]
fn gate_level_cells_with_sdf_and_notifiers() {
    let files = [("gl_cells_sdf.sdf", fixture!("gl_cells_sdf.sdf"))];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "gl_cells_sdf",
        &[],
        &files,
        &scenario(fixture!("gl_cells_sdf.out"), CELLS_VIOLATIONS),
    );
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "gl_cells_sdf",
        &["--define", "CLI_SDF", "--sdf", "tb.dut=gl_cells_sdf.sdf"],
        &files,
        &scenario(fixture!("gl_cells_sdf.out"), CELLS_VIOLATIONS),
    );
}

/// Gate-level scenario 2: strengths and net resolution on a pad bus, read
/// through `%v` and through VPI strength values.
#[test]
fn gate_level_strength_resolution_on_a_pad_bus() {
    let directory = sim_harness::TempDir::new("fin002-strength").expect("temporary directory");
    let archives = [archive(&directory, "gl_strength_bus_vpi")];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "gl_strength_bus",
        &linked(
            &archives,
            &["--define-system-task", "task $strength(string path)"],
        ),
        &[],
        &scenario(fixture!("gl_strength_bus.out"), &[]),
    );
}

/// Gate-level scenario 3: bidirectional and resistive switch networks.
#[test]
fn gate_level_switch_networks() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "gl_switch_network",
        &[],
        &[],
        &scenario(fixture!("gl_switch_network.out"), &[]),
    );
}

const CHECK_VIOLATIONS: &[&str] = &[
    "WARNING: gl_timing_checks.sv:46:5: [SIM-015] timing violation in tb.dut at time 10000 \
     (1ps): $setup(D:9700, posedge CK &&& ...:10000, 500)",
    "WARNING: gl_timing_checks.sv:46:5: [SIM-015] timing violation in tb.dut at time 20200 \
     (1ps): $hold(posedge CK &&& ...:20000, D:20200, 300)",
    "WARNING: gl_timing_checks.sv:47:5: [SIM-015] timing violation in tb.dut at time 32300 \
     (1ps): $recovery(posedge RN:32000, posedge CK:32300, 600)",
    "WARNING: gl_timing_checks.sv:47:5: [SIM-015] timing violation in tb.dut at time 44200 \
     (1ps): $removal(posedge RN:44200, posedge CK:44000, 400)",
    "WARNING: gl_timing_checks.sv:51:5: [SIM-015] timing violation in tb.dut at time 55000 \
     (1ps): $width(posedge CK:54000, negedge CK:55000, 2000, 0)",
    "WARNING: gl_timing_checks.sv:52:5: [SIM-015] timing violation in tb.dut at time 68000 \
     (1ps): $period(posedge CK:62000, posedge CK:68000, 8000)",
    "WARNING: gl_timing_checks.sv:50:5: [SIM-015] timing violation in tb.dut at time 80600 \
     (1ps): $width(negedge RN:80000, posedge RN:80600, 1000, 0)",
    "WARNING: gl_timing_checks.sv:48:5: [SIM-015] timing violation in tb.dut at time 90400 \
     (1ps): $recovery(posedge SN:90000, posedge CK:90400, 600)",
];

/// Gate-level scenario 4: one violation of each timing check toggles the
/// notifier and the flip-flop recovers after a reset.
#[test]
fn gate_level_timing_checks_toggle_notifiers() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "gl_timing_checks",
        &[],
        &[],
        &scenario(fixture!("gl_timing_checks.out"), CHECK_VIOLATIONS),
    );
}

/// Gate-level scenario 4 under the timing check switches: without timing
/// checks or without notifiers no notifier toggles, so the flip-flop never
/// goes to x; without messages the transcript is unchanged.
#[test]
fn gate_level_timing_check_switches() {
    let quiet = |label: &str, output: &Output| {
        assert!(
            output.status.success(),
            "{label}: {}",
            sim_cli::observed_output(output)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("notifier") && !stdout.contains("q=x"),
            "{label}: {stdout}"
        );
        assert!(
            stdout.ends_with("[113.500] violations=0 q=0\n"),
            "{label}: {stdout}"
        );
    };
    for optimized in [false, true] {
        let run = |arg: &str| {
            sim_cli::run_case_keeping_dir(SUITE, "gl_timing_checks", optimized, &[arg], &[]).1
        };
        let label = format!("--no-timing-checks, optimized={optimized}");
        let output = run("--no-timing-checks");
        quiet(&label, &output);
        assert_eq!(messages(&output), [""; 0], "{label}");

        let label = format!("--no-notifiers, optimized={optimized}");
        let output = run("--no-notifiers");
        quiet(&label, &output);
        let mut reports = CHECK_VIOLATIONS.to_vec();
        reports.sort_unstable();
        assert_eq!(messages(&output), reports, "{label}");

        let label = format!("--no-timing-check-messages, optimized={optimized}");
        let output = run("--no-timing-check-messages");
        assert_scenario(&label, &output, fixture!("gl_timing_checks.out"), &[]);
    }
}

/// Gate-level scenario 5: an annotated scan chain whose x travels through
/// UDPs, inspected through VPI. The order of the primitives an iteration
/// returns is not defined, so the per-step comparison sorts them.
#[test]
fn gate_level_scan_chain_with_vpi_inspection() {
    let directory = sim_harness::TempDir::new("fin002-scan").expect("temporary directory");
    let archives = [archive(&directory, "gl_scan_chain_vpi")];
    sim_cli::run_case_checked_matrix_with_files(SUITE, "gl_scan_chain", &linked(
            &archives,
            &["--define-system-task", "task $inspect(string path)"],
        ), &[("gl_scan_chain.sdf", fixture!("gl_scan_chain.sdf"))], &scenario(fixture!("gl_scan_chain.out"), &[
            "WARNING: gl_scan_chain.sv:70:5: [SIM-015] timing violation in tb.dut.ff[0] at time \
             60000 (1ps): $setup(SI:59800, posedge CK &&& ...:60000, 400)",
        ]));
}

const PAD_VIOLATIONS: &[&str] = &[
    "WARNING: gl_pad_force.sv:53:5: [SIM-015] timing violation in tb.cap at time 80000 \
       (1ps): $setup(D:79700, posedge CK:80000, 500)",
];

/// Gate-level scenario 6: a pad net with pulse limits, a driver conflict
/// and a setup violation, forced and released through VPI.
#[test]
fn gate_level_pad_forced_through_vpi() {
    let directory = sim_harness::TempDir::new("fin002-pad").expect("temporary directory");
    let archives = [archive(&directory, "shared_force_vpi")];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "gl_pad_force",
        &linked(&archives, FORCE_APP),
        &[],
        &scenario(fixture!("gl_pad_force.out"), PAD_VIOLATIONS),
    );
}

/// Step 2 boundary: the gate-level features llg does not simulate stop with
/// their own diagnostics, never as a silent no-op.
#[test]
fn unsupported_gate_level_features_stop_with_diagnostics() {
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "neg_gl_trireg",
        "ELAB-012",
        Some("7:19"),
        "`trireg` net `store` (trireg charge storage) is not supported by llg",
    );
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "neg_gl_mos",
        "ELAB-011",
        Some("8:8"),
        "`nmos` primitive (MOS and resistive switch primitives) is not supported by llg",
    );
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "neg_gl_dumpports",
        "ELAB-014",
        Some("17:5"),
        "system task `$dumpports` (extended VCD port dumping) is not supported by llg",
    );
    sim_cli::run_case_stopping_at_runtime(
        SUITE,
        "neg_gl_reset",
        &[],
        "",
        "SIM-016",
        "VPI error: no VPI registration for `$reset`",
    );
}

/// Defect regression (found by scenario 1): a release through VPI
/// re-establishes the continuous assignments of the released variable.
#[test]
fn vpi_release_reestablishes_continuous_assignments() {
    let directory = sim_harness::TempDir::new("fin002-release").expect("temporary directory");
    let archives = [archive(&directory, "shared_force_vpi")];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "vpi_release_reestablishes",
        &linked(&archives, FORCE_APP),
        &[],
        &scenario(fixture!("vpi_release_reestablishes.out"), &[]),
    );
}

/// The release rule (decision CS-D3) where the portable case does not
/// reach: releases in a function, a class method and a final procedure, a
/// keyed array element, a real variable and a release made by the driver's
/// own function while it is evaluated.
#[test]
fn releases_repeat_continuous_drivers_in_place() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "release_in_place",
        &[],
        &[],
        &scenario(fixture!("release_in_place.out"), &[]),
    );
}

/// Releases where the instances of a module, the iterations of a generate
/// loop and the port connections share one generated driver body: each
/// release repeats the driver of its own instance with that instance's
/// operands, parameter and index.
#[test]
fn releases_repeat_the_driver_of_their_own_instance() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "release_shared_instances",
        &[],
        &[],
        &scenario(fixture!("release_shared_instances.out"), &[]),
    );
}

/// The value a `%v` strength display or a binary display stands for in a
/// waveform: the logic value of each bit.
fn logic_value(display: &str) -> String {
    match display {
        "HiZ" => "z".to_owned(),
        strength if strength.len() == 3 && !strength.starts_with(['0', '1', 'x', 'z']) => {
            strength[2..].to_ascii_lowercase()
        }
        strength
            if strength.len() == 3 && strength.as_bytes()[..2].iter().all(u8::is_ascii_digit) =>
        {
            strength[2..].to_ascii_lowercase()
        }
        bits => bits.to_owned(),
    }
}

/// The `(time, value)` changes `name=` reports in a transcript whose time
/// prefixes are nanoseconds with three decimals, in picoseconds.
fn transcript_changes(transcript: &str, name: &str) -> Vec<(u64, String)> {
    let key = format!("] {name}=");
    transcript
        .lines()
        .filter_map(|line| {
            let (time, value) = line.strip_prefix('[')?.split_once(&key)?;
            let (whole, fraction) = time.split_once('.')?;
            let time = whole.parse::<u64>().ok()? * 1000 + fraction.parse::<u64>().ok()?;
            Some((time, logic_value(value)))
        })
        .collect()
}

/// The records of `changes` after time 0 (the initial values are a dump
/// checkpoint, not changes the design printed).
fn after_start(changes: Vec<(u64, String)>) -> Vec<(u64, String)> {
    changes.into_iter().filter(|change| change.0 > 0).collect()
}

/// The same run dumped as VCD and as FST: both hold, for the nets the
/// design prints, exactly the changes of the transcript, including the
/// VPI force and release of the pad and the x pulse of the receiver path.
#[test]
fn waveforms_record_the_gate_level_run() {
    let directory = sim_harness::TempDir::new("fin002-wave").expect("temporary directory");
    let archives = [archive(&directory, "shared_force_vpi")];
    let expected = fixture!("gl_pad_force.out");
    for optimized in [false, true] {
        let dump = |file: &str| {
            let mut args = linked(&archives, FORCE_APP);
            args.extend(["--wave", file]);
            let (run, output) =
                sim_cli::run_case_keeping_dir(SUITE, "gl_pad_force", optimized, &args, &[]);
            let label = format!("gl_pad_force {file}, optimized={optimized}");
            assert_scenario(&label, &output, expected, PAD_VIOLATIONS);
            run
        };
        let run = dump("pad.vcd");
        let text = std::fs::read_to_string(run.path().join("pad.vcd")).expect("VCD file");
        let vcd = vcd::parse(&text).unwrap_or_else(|error| panic!("pad.vcd: {error}"));
        let run = dump("pad.fst");
        let fst = fst_dump(run.path(), "pad.fst").unwrap_or_else(|error| panic!("{error}"));
        for (path, name) in [("tb.pad", "pad"), ("tb.y", "y"), ("tb.q", "q")] {
            let printed = transcript_changes(expected, name);
            assert!(!printed.is_empty(), "{name}");
            let label = format!("{path}, optimized={optimized}");
            assert_eq!(after_start(vcd.changes_of(path)), printed, "VCD {label}");
            assert_eq!(after_start(fst.changes_of(path)), printed, "FST {label}");
        }
    }
}

/// The testbench run of scenario 1 dumped as VCD and as FST: the forced,
/// released and deposited values of the sampled variable are in both, at
/// the times the transcript reports, in the order the VPI callback saw.
#[test]
fn waveforms_record_the_testbench_run() {
    let directory = sim_harness::TempDir::new("fin002-wave-tb").expect("temporary directory");
    let archives = [archive(&directory, "shared_force_vpi")];
    let expected = fixture!("tb_force_clocking.out");
    // `vpi changes=<n>: <values>`: every value of tb.bus.dout after x.
    let seen: Vec<String> = expected
        .lines()
        .find_map(|line| line.strip_prefix("vpi changes=")?.split_once(": "))
        .expect("the callback report")
        .1
        .split(' ')
        .map(|value| format!("{:08b}", value.parse::<u8>().expect("a byte")))
        .collect();
    for optimized in [false, true] {
        let dump = |file: &str| {
            let mut args = linked(&archives, FORCE_APP);
            args.extend(["--wave", file]);
            let (run, output) =
                sim_cli::run_case_keeping_dir(SUITE, "tb_force_clocking", optimized, &args, &[]);
            let label = format!("tb_force_clocking {file}, optimized={optimized}");
            assert_scenario(&label, &output, expected, &[]);
            run
        };
        let run = dump("tb.vcd");
        let text = std::fs::read_to_string(run.path().join("tb.vcd")).expect("VCD file");
        let vcd = vcd::parse(&text).unwrap_or_else(|error| panic!("tb.vcd: {error}"));
        let run = dump("tb.fst");
        let fst = fst_dump(run.path(), "tb.fst").unwrap_or_else(|error| panic!("{error}"));
        let label = format!("optimized={optimized}");
        let changes = after_start(vcd.changes_of("tb.bus.dout"));
        let values: Vec<&str> = changes.iter().map(|change| change.1.as_str()).collect();
        assert_eq!(values, seen, "{label}");
        // The force at 58 and the release at 68 (1 ns time scale).
        assert!(changes.contains(&(58, format!("{:08b}", 99))), "{label}");
        assert!(changes.contains(&(68, format!("{:08b}", 26))), "{label}");
        assert_eq!(
            after_start(fst.changes_of("tb.bus.dout")),
            changes,
            "{label}"
        );
    }
}
