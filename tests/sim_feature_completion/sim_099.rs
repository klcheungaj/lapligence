//! SIM-099: the common verification gate. An integrated class-based
//! testbench drives a fixed RTL design through a virtual interface with
//! timed tasks, mailboxes, semaphores, constrained randomization,
//! randsequence, assertions, a covergroup with coverage queries, a DPI-C
//! reference model and a VPI value-change callback; step-2 fixtures pin the
//! cross-task contracts (collector roots held by DPI, VPI and assertion
//! state, selected write notifications, source mapping, shuffle stream
//! ownership and deterministic random replay); the rest are regressions of
//! defects the gate audit found. Every expected line is derived in the
//! fixture readme from IEEE 1800-2009. Positive cases run in both optimizer
//! modes on the legacy, compact/portable and compact/GMP value backends.
use super::{sim_cli, sim_harness};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_099";

/// Collector settings that collect at every safe point and keep condemned
/// objects poisoned, so a missed root stops the run (SIM-018).
const GC_STRESS: &[(&str, &str)] = &[("LLG_GC_STRESS", "1"), ("LLG_GC_VERIFY", "1")];

const RANDOMIZE_WARNING: &str = "llg: warning: randomize() at env:222:12: the constraints cannot \
                                 be satisfied; randomize returns 0";

/// `<stem>.c` of this suite built into a static archive for `--dpi-lib`
/// (every host; `$LLG_CFLAGS` instruments it in sanitizer runs).
fn archive(directory: &sim_harness::TempDir, stem: &str) -> String {
    super::sim_041::static_archive(SUITE, directory, stem, &[])
}

/// The `llg:` lines of a run's stderr, without lint reports.
fn llg_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| line.starts_with("llg: "))
        .map(str::to_owned)
        .collect()
}

/// A run that succeeded with exactly `expected` on stdout and, among its
/// `llg:` stderr lines, exactly `messages` besides the `$finish` report.
fn assert_run(label: &str, output: &Output, expected: &str, messages: &[&str]) {
    assert!(
        output.status.success(),
        "{label}: {}",
        sim_cli::observed_output(output)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
    let lines = llg_lines(output);
    let others: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|line| !line.starts_with("llg: $finish at time "))
        .collect();
    assert_eq!(others, messages, "{label}");
}

const TESTBENCH_MESSAGES: &[&str] = &[
    "llg: warning: repeat count in `tb.bus` is not a constant; evaluated at runtime",
    RANDOMIZE_WARNING,
];

/// Step 1: the integrated testbench with its DPI-C reference model and VPI
/// application linked as static archives (every host).
#[test]
fn integrated_testbench_with_linked_dpi_and_vpi() {
    let directory = sim_harness::TempDir::new("sim099-tb").expect("temporary directory");
    let dpi = archive(&directory, "testbench_dpi");
    let vpi = archive(&directory, "testbench_vpi");
    let expected = include_str!("../fixtures/sim/feature_completion/sim_099/testbench.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "testbench",
        &["--dpi-lib", &dpi, "--dpi-lib", &vpi],
        &|label, output| assert_run(label, output, expected, TESTBENCH_MESSAGES),
    );
}

/// Step 1: the same testbench with the DPI-C model as a shared companion
/// and the VPI application loaded at run time (`LLG_VPI_PLUGIN`).
#[test]
fn integrated_testbench_with_shared_companion_and_plugin() {
    let directory = sim_harness::TempDir::new("sim099-tb-shared").expect("temporary directory");
    let Some(dpi) =
        super::sim_040::try_shared_companion(SUITE, &directory, "testbench_dpi", "testbench", &[])
    else {
        return;
    };
    let dpi = dpi
        .unwrap_or_else(|error| panic!("testbench_dpi.c: {error}"))
        .link
        .to_string_lossy()
        .into_owned();
    let Some(vpi) = super::sim_040::plugin_library(SUITE, &directory, "testbench_vpi", &[]) else {
        return;
    };
    let expected = include_str!("../fixtures/sim/feature_completion/sim_099/testbench.out");
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "testbench",
        &["--vpi", "--dpi-lib", &dpi],
        &[("LLG_VPI_PLUGIN", &vpi)],
        &|label, output| assert_run(label, output, expected, TESTBENCH_MESSAGES),
    );
}

/// Steps 1 and 2: collecting at every safe point changes nothing; the
/// mailboxes, semaphores, covergroup owner and suspended activations keep
/// every object the testbench still uses reachable.
#[test]
fn integrated_testbench_under_collector_stress() {
    let directory = sim_harness::TempDir::new("sim099-tb-gc").expect("temporary directory");
    let dpi = archive(&directory, "testbench_dpi");
    let vpi = archive(&directory, "testbench_vpi");
    let expected = include_str!("../fixtures/sim/feature_completion/sim_099/testbench.out");
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "testbench",
        &["--dpi-lib", &dpi, "--dpi-lib", &vpi],
        GC_STRESS,
        &|label, output| assert_run(label, output, expected, TESTBENCH_MESSAGES),
    );
}

/// Step 2: objects reachable only from a pending deferred-assertion action,
/// from an activation that called C which allocated through an export, and
/// from activations woken by a VPI callback's write survive collections.
#[test]
fn collector_roots_held_by_dpi_vpi_and_assertions() {
    let directory = sim_harness::TempDir::new("sim099-gc").expect("temporary directory");
    let library = archive(&directory, "gc_roots");
    let expected = include_str!("../fixtures/sim/feature_completion/sim_099/gc_roots.out");
    for envs in [&[][..], GC_STRESS] {
        sim_cli::run_case_checked_matrix_with_env(
            SUITE,
            "gc_roots",
            &["--dpi-lib", &library],
            envs,
            &|label, output| assert_run(&format!("{label} {envs:?}"), output, expected, &[]),
        );
    }
}

/// Lines grouped by their `[time]` prefix, each group sorted: the order of
/// processes and callbacks within one time step is not defined (4.7).
fn time_step_sets(text: &str) -> Vec<(u64, Vec<String>)> {
    let mut groups: Vec<(u64, Vec<String>)> = Vec::new();
    for line in text.lines() {
        let time = line
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .and_then(|(time, _)| time.parse::<u64>().ok())
            .unwrap_or_else(|| panic!("line without a time prefix: {line}"));
        match groups.last_mut() {
            Some((last, lines)) if *last == time => lines.push(line.to_owned()),
            _ => groups.push((time, vec![line.to_owned()])),
        }
    }
    for (_, lines) in &mut groups {
        lines.sort();
    }
    groups
}

/// Step 2: selected writes through a virtual interface notify exactly the
/// waiters, continuous assignments and VPI callbacks whose operand changed.
#[test]
fn selected_writes_notify_waiters_assignments_and_callbacks() {
    let directory = sim_harness::TempDir::new("sim099-select").expect("temporary directory");
    let library = archive(&directory, "selected_writes");
    let expected = time_step_sets(include_str!(
        "../fixtures/sim/feature_completion/sim_099/selected_writes.out"
    ));
    sim_cli::run_case_checked_matrix(
        SUITE,
        "selected_writes",
        &["--dpi-lib", &library],
        &|label, output| {
            assert!(output.status.success(), "{label}: {output:?}");
            assert_eq!(
                time_step_sets(&String::from_utf8_lossy(&output.stdout)),
                expected,
                "{label}"
            );
        },
    );
}

/// Step 2: run-time reports name the line of the package function, class
/// method, randomize call and interface assertion that produced them, in
/// public CLI runs and in models generated after the frontend snapshot and
/// the owned database are destroyed.
#[test]
fn reports_keep_their_source_positions() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_099/source_map.out");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("source_map.sv");
    let source = source.to_string_lossy().into_owned();
    let assertion = format!("llg: assertion assert failed: {source}:16:3 (always_low)");
    let messages = [
        "llg: severity error: checks_pkg:10:10: not positive: -2",
        "llg: severity warning: limited_c:23:5: limited_c warning 1",
        "llg: warning: randomize() at tb:40:10: the constraints cannot be satisfied; randomize \
         returns 0",
        assertion.as_str(),
    ];
    sim_cli::run_case_checked_matrix(SUITE, "source_map", &[], &|label, output| {
        assert_run(label, output, expected, &messages)
    });
    let stderr = messages
        .iter()
        .filter(|line| !line.starts_with("llg: warning: "))
        .map(|line| format!("{line}\n"))
        .collect::<String>()
        + "llg: $finish at time 6 at tb:45:5\n";
    let warnings: Vec<&str> = messages
        .iter()
        .filter_map(|line| line.strip_prefix("llg: warning: "))
        .collect();
    sim_cli::run_compile_opts_after_db_drop_with_warnings(
        SUITE,
        "source_map",
        llg::core::compile::CompileOpts {
            files: vec![source.clone()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        &stderr,
        &warnings,
    );
}

/// Step 2: shuffle() draws from the calling thread and randomize() from the
/// object, also when post_randomize() shuffles.
#[test]
fn shuffle_and_randomize_keep_their_stream_owners() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "shuffle_ownership",
        include_str!("../fixtures/sim/feature_completion/sim_099/shuffle_ownership.out"),
        &[],
        &[],
    );
}

/// Step 2 / A03: restoring saved thread and object states replays every
/// random source; the generator-dependent checksum is the same in every
/// optimizer mode, value backend and collector setting.
#[test]
fn random_sources_replay_deterministically() {
    let outputs = RefCell::new(Vec::new());
    for envs in [&[][..], GC_STRESS] {
        sim_cli::run_case_checked_matrix_with_env(
            SUITE,
            "random_replay",
            &[],
            envs,
            &|label, output| {
                assert!(output.status.success(), "{label}: {output:?}");
                outputs.borrow_mut().push((
                    format!("{label} {envs:?}"),
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                ));
            },
        );
    }
    let outputs = outputs.into_inner();
    assert_eq!(outputs.len(), 12);
    let (first_label, first) = &outputs[0];
    let relations = "replay equal: 1\ncontinuation differs: 1\n\
                     randc cycle complete: 1\n";
    assert!(first.starts_with(relations), "{first_label}: {first}");
    let checksum = &first[relations.len()..];
    assert!(
        checksum.starts_with("checksum ") && checksum.lines().count() == 1,
        "{first_label}: {first}"
    );
    for (label, output) in &outputs[1..] {
        assert_eq!(output, first, "{label} differs from {first_label}");
    }
}

/// Regression: built-in functions in void casts run as functions instead of
/// becoming VPI task calls that fail for want of a registration.
#[test]
fn void_cast_builtins_run_as_functions() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "void_casts",
        include_str!("../fixtures/sim/feature_completion/sim_099/void_casts.out"),
        &[],
        &[],
    );
}

/// Regression: a user-defined VPI function in a void cast is called as a
/// function (its calltf runs once and its value is discarded).
#[test]
fn void_cast_vpi_function_runs_its_calltf() {
    let directory = sim_harness::TempDir::new("sim099-void-vpi").expect("temporary directory");
    let Some(library) =
        super::sim_040::plugin_library("feature_completion/sim_042", &directory, "systf", &[])
    else {
        return;
    };
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "void_vpi_function",
        &[
            "--vpi",
            "--define-system-task",
            "function logic signed [11:0] $sum(input logic [7:0] a, input logic signed [11:0] b)",
        ],
        &[("LLG_VPI_PLUGIN", &library)],
        &|label, output| {
            assert_run(
                label,
                output,
                include_str!("../fixtures/sim/feature_completion/sim_099/void_vpi_function.out"),
                &[],
            )
        },
    );
}

/// Regression: semaphore methods on a semaphore selected through class
/// handles.
#[test]
fn semaphores_selected_through_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "handle_semaphores",
        include_str!("../fixtures/sim/feature_completion/sim_099/handle_semaphores.out"),
        &[],
        &[],
    );
}

/// Regression: whole and selected, blocking and nonblocking writes through a
/// virtual-interface formal of an automatic task.
#[test]
fn virtual_interface_formal_writes() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "vif_formal_writes",
        include_str!("../fixtures/sim/feature_completion/sim_099/vif_formal_writes.out"),
        &[],
        &[],
    );
}

/// G5 / IEEE 1800-2009 16.15.6: a concurrent assertion reached in
/// procedural code queues one instance per execution on its process; the
/// queue is flushed when the process resumes from an event control and by a
/// disable, and matured instances begin at their leading clock (at once when
/// it ticked in the time step). Lines of one time step compare as a set.
#[test]
fn procedural_concurrent_assertions_queue_flush_and_mature() {
    let expected = time_step_sets(include_str!(
        "../fixtures/sim/feature_completion/sim_099/procedural_assertions.out"
    ));
    sim_cli::run_case_checked_matrix(SUITE, "procedural_assertions", &[], &|label, output| {
        assert!(output.status.success(), "{label}: {output:?}");
        assert_eq!(
            time_step_sets(&String::from_utf8_lossy(&output.stdout)),
            expected,
            "{label}"
        );
    });
}

/// G5 / IEEE 1800-2009 16.15.6.1: llg keeps no values with a queued
/// instance, so const casts and automatic variables in a procedural
/// concurrent assertion stop compilation at their position, and so does an
/// inferred `edge` clock.
#[test]
fn procedural_assertion_forms_without_exact_semantics_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_procedural_automatic",
        "neg_procedural_automatic.sv:9:47: automatic variable `i` in a procedural concurrent \
         assertion takes its value when the assertion is queued (IEEE 1800-2009 16.15.6.1); llg \
         does not capture queued values",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_procedural_const",
        "neg_procedural_const.sv:9:45: a const cast in a procedural concurrent assertion takes \
         its value when the assertion is queued (IEEE 1800-2009 16.15.6.1); llg does not capture \
         queued values",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_procedural_edge",
        "neg_procedural_edge.sv:8:5: the clock inferred from the procedure (`edge` of the \
         expression at line 6, column 17) cannot clock a concurrent assertion in llg; give the \
         assertion a posedge or negedge clock (IEEE 1800-2009 16.15.6)",
    );
}

/// G1 / IEEE 1800-2009 25.9: elements of unpacked array members read and
/// written (blocking and nonblocking, whole or through a packed select)
/// through virtual-interface handles reach the array of the instance bound
/// when the access executes.
#[test]
fn virtual_interface_array_members() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "vif_array_members",
        include_str!("../fixtures/sim/feature_completion/sim_099/vif_array_members.out"),
        &[],
        &[],
    );
}

/// G1: the array-member forms not dispatched on the bound instance stop
/// compilation at their position.
#[test]
fn virtual_interface_array_member_limits_are_located() {
    sim_cli::reject_case(
        SUITE,
        "neg_vif_array_whole",
        "neg_vif_array_whole.sv:11:12: unpacked array member `mem` of virtual interface \
         `bus_if` is supported only through element selects (`mem[i]`, `mem[i][b]`) that are \
         read or assigned with `=` or `<=`; a whole-array or slice use, or a use in an event \
         control or wait, is not supported in `tb`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_vif_array_update",
        "neg_vif_array_update.sv:10:5: a compound assignment to an element of an unpacked array \
         member through a virtual interface is not supported in `tb`; use `=` or `<=`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_vif_array_increment",
        "neg_vif_array_increment.sv:10:5: this write to an element of an unpacked array member \
         through a virtual interface is not supported in `tb`; assign it with `=` or `<=`",
    );
}

/// G4 / IEEE 1800-2009 32.9, D.4, D.7: string literals of `string` formals
/// reach a VPI application as string constants and scope actuals as module
/// handles, for built-ins without a lowering and user system tasks alike; a
/// string variable stops compilation at its position.
#[test]
fn vpi_string_and_scope_arguments() {
    let directory = sim_harness::TempDir::new("sim099-vpi-args").expect("temporary directory");
    let vpi = archive(&directory, "vpi_string_scope_args");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "vpi_string_scope_args",
        &[
            "--define-system-task",
            "task $show(string s, logic [7:0] b)",
            "--dpi-lib",
            &vpi,
        ],
        &|label, output| {
            assert_run(
                label,
                output,
                include_str!(
                    "../fixtures/sim/feature_completion/sim_099/vpi_string_scope_args.out"
                ),
                &[],
            )
        },
    );
    sim_cli::reject_case(
        SUITE,
        "neg_vpi_string_variable",
        "neg_vpi_string_variable.sv:5:18: argument of VPI system task `$input`: a string \
         variable or expression is not supported (only string literals reach VPI applications) \
         in `tb`",
    );
}

/// A01: the built-ins no other fixture called.
#[test]
fn remaining_builtins_execute() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "builtins",
        include_str!("../fixtures/sim/feature_completion/sim_099/builtins.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "global_sampled",
        include_str!("../fixtures/sim/feature_completion/sim_099/global_sampled.out"),
        &[],
        &[],
    );
}

/// A02: the clock-resolution built-ins llg cannot lower and a discarded
/// string result stop the run at their source position instead of reaching
/// VPI dispatch; the view direction check still covers selects.
#[test]
fn unsupported_builtin_forms_are_located_rejections() {
    sim_cli::reject_case(
        SUITE,
        "neg_global_clock",
        "neg_global_clock.sv:7:22: built-in system subroutine `$global_clock` (IEEE 1800-2009 \
         14.14) is not implemented by llg",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_inferred_clock",
        "neg_inferred_clock.sv:6:7: built-in system subroutine `$inferred_clock` (IEEE \
         1800-2009 16.15.7) is not implemented by llg",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_void_string",
        "neg_void_string.sv:5:11: the discarded string result of `$sformatf` in statement \
         position is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_view_input_select",
        "neg_view_input_select.sv:12:15 cannot assign to input port 'data'",
    );
    for fixture in [
        "neg_global_clock",
        "neg_inferred_clock",
        "neg_void_string",
        "neg_view_input_select",
    ] {
        let output = sim_cli::invoke_with_env(SUITE, fixture, true, &[], &[], &[]);
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("VPI"),
            "{fixture}: {}",
            sim_cli::observed_output(&output)
        );
    }
}

/// A02 / IEEE 1800-2009 36.3.2: a built-in without a llg lowering (the
/// informative Annex D services) stays on the VPI path. A registered
/// application provides it; with none, the run stops at startup naming the
/// call instead of running it as a no-op.
#[test]
fn builtins_without_lowering_dispatch_to_vpi() {
    let directory = sim_harness::TempDir::new("sim099-annex").expect("temporary directory");
    let vpi = archive(&directory, "annex_override");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "annex_override",
        &["--dpi-lib", &vpi],
        &|label, output| {
            assert_run(
                label,
                output,
                include_str!("../fixtures/sim/feature_completion/sim_099/annex_override.out"),
                &[],
            )
        },
    );
    let unregistered = |fixture: &str, name: &str| {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join(format!("{fixture}.sv"));
        format!(
            "llg: VPI error: no VPI registration for `{name}` (called at {}:7)",
            source.display()
        )
    };
    sim_cli::reject_case(
        SUITE,
        "neg_unregistered_annex_task",
        &unregistered("neg_unregistered_annex_task", "$sreadmemh"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_unregistered_annex_function",
        &unregistered("neg_unregistered_annex_function", "$reset_count"),
    );
}

/// G3 / IEEE 1800-2009 36.3.2, decision S99-D6: a VPI registration under
/// the name of a built-in the design calls stops the run at startup, naming
/// the built-in and its call, instead of being silently ignored, for linked
/// applications and `LLG_VPI_PLUGIN` plugins alike; a registration of a
/// built-in the design does not call is accepted.
#[test]
fn vpi_registrations_of_called_builtins_stop_at_startup() {
    let directory = sim_harness::TempDir::new("sim099-override").expect("temporary directory");
    let vpi = archive(&directory, "builtin_override");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "builtin_override",
        &["--dpi-lib", &vpi],
        &|label, output| {
            assert_run(
                label,
                output,
                include_str!("../fixtures/sim/feature_completion/sim_099/builtin_override.out"),
                &[],
            )
        },
    );
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("neg_builtin_override.sv");
    let diagnostic = format!(
        "llg: VPI error: a VPI application registers `$random`, which llg implements as a \
         built-in (called at {}:6); llg does not support overriding built-in system tasks and \
         functions (IEEE 1800-2009 36.3.2)",
        source.display()
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_builtin_override",
        &diagnostic,
        &["--dpi-lib", &vpi],
    );
    let Some(plugin) = super::sim_040::plugin_library(SUITE, &directory, "builtin_override", &[])
    else {
        return;
    };
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            SUITE,
            "neg_builtin_override",
            optimized,
            &[],
            &[("LLG_VPI_PLUGIN", &plugin)],
            &[],
        );
        let seen = sim_cli::observed_output(&output);
        assert_eq!(output.status.code(), Some(1), "plugin: {seen}");
        assert!(output.stdout.is_empty(), "plugin: {seen}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&diagnostic),
            "plugin: {seen}"
        );
    }
}

/// SV source without `//` and `/* */` comments, so a name mentioned only in
/// prose does not count as called.
fn without_sv_comments(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("//") {
            rest = after.find('\n').map_or("", |end| &after[end..]);
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after.find("*/").map_or("", |end| &after[end + 2..]);
            result.push(' ');
        } else {
            let next = rest.chars().next().unwrap_or(' ');
            result.push(next);
            rest = &rest[next.len_utf8()..];
        }
    }
    result
}

/// Every `$name` token of the positive SV fixtures below `directory`
/// (`neg_`/`bad_` sources only exercise rejections).
fn called_system_names(directory: &Path, names: &mut BTreeSet<String>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry.expect("fixture directory entry").path();
        if path.is_dir() {
            called_system_names(&path, names);
            continue;
        }
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let source = [".sv", ".v", ".svh"]
            .iter()
            .any(|extension| file.ends_with(extension));
        if !source || file.starts_with("neg_") || file.starts_with("bad_") {
            continue;
        }
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {file}: {error}"));
        let text = without_sv_comments(&String::from_utf8_lossy(&bytes));
        let mut rest = text.as_str();
        while let Some(start) = rest.find('$') {
            let token = &rest[start..];
            let end = token[1..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
                .map_or(token.len(), |end| end + 1);
            if end > 1 {
                names.insert(token[..end].to_owned());
            }
            rest = &token[end.max(1)..];
        }
    }
}

/// A01: every built-in system subroutine the frontend admits is either called
/// by a positive simulator fixture or classified in `builtin_inventory.tsv`,
/// and the not-implemented rows are exactly the names lowering rejects at
/// their source position, so no built-in exists only as a registry name.
#[test]
fn builtin_inventory_is_complete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let slang_names = fs::read_to_string(root.join("vendor/slang/scripts/systemnames.txt"))
        .expect("vendor/slang/scripts/systemnames.txt (check out the Slang submodule)");
    let builtins: BTreeSet<&str> = slang_names
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| name.starts_with('$'))
        .collect();
    assert!(builtins.len() > 200, "Slang system names were not found");

    let mut rows = BTreeMap::new();
    for line in include_str!("../fixtures/sim/feature_completion/sim_099/builtin_inventory.tsv")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let [name, status, reference] = fields.as_slice() else {
            panic!("inventory row needs name, status and reference: {line:?}");
        };
        assert!(
            [
                "not-implemented",
                "vpi-only",
                "advanced",
                "by-design",
                "edition"
            ]
            .contains(status),
            "unknown inventory status in {line:?}"
        );
        assert!(
            !reference.is_empty(),
            "inventory row without reference: {line:?}"
        );
        assert!(builtins.contains(name), "{name} is not a Slang built-in");
        assert!(
            rows.insert(*name, *status).is_none(),
            "{name} is listed twice"
        );
    }

    let mut called = BTreeSet::new();
    called_system_names(&root.join("tests/fixtures/sim"), &mut called);
    let mut problems = Vec::new();
    for name in &builtins {
        match (called.contains(*name), rows.contains_key(name)) {
            (false, false) => problems.push(format!(
                "{name}: add a positive fixture that calls it, or classify it in \
                 builtin_inventory.tsv"
            )),
            (true, true) => problems.push(format!(
                "{name}: a positive fixture calls it; remove its builtin_inventory.tsv row"
            )),
            _ => {}
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));

    let lowering =
        fs::read_to_string(root.join("src/sim/codegen/lowering/expressions/system_functions.rs"))
            .expect("system_functions.rs");
    let table = lowering
        .split_once("fn unimplemented_builtin_reference")
        .and_then(|(_, rest)| rest.split_once("\n}\n"))
        .map(|(body, _)| body)
        .expect("unimplemented_builtin_reference table");
    let rejected: BTreeSet<&str> = table
        .split('"')
        .filter(|literal| literal.starts_with('$'))
        .collect();
    let not_implemented: BTreeSet<&str> = rows
        .iter()
        .filter(|(_, status)| **status == "not-implemented")
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(rejected, not_implemented);
}
