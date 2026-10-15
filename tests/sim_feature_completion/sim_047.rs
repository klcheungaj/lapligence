//! SIM-047: waveform output qualified against owned storage, aliases, the
//! storage pass and object lifetimes (IEEE 1364-2001 18.1-18.2, IEEE
//! 1800-2009 21.7.1-21.7.2). Every VCD is read by the independent reader in
//! `support/vcd.rs` and compared with traces derived by hand in the fixture
//! readme, in both optimizer modes; FST files go through GTKWave's reader.
use super::c_compiler;
use super::sim_cli;
use super::sim_harness;
use super::vcd::{self, Section};
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

const SUITE: &str = "feature_completion/sim_047";

/// The writer's reversible escape of one name component: ASCII letters,
/// digits and `_` stay, every other byte is `$` and two hex digits.
fn escape(component: &str) -> String {
    component
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                (byte as char).to_string()
            } else {
                format!("${byte:02X}")
            }
        })
        .collect()
}

/// A dotted VCD path from source components, escaping each one.
fn path(components: &[&str]) -> String {
    components
        .iter()
        .map(|component| escape(component))
        .collect::<Vec<_>>()
        .join(".")
}

fn assert_success(label: &str, output: &Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
}

/// Runtime stderr lines: compile reports and the `$finish` note removed.
fn runtime_stderr(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| !sim_harness::is_compile_report_line(line))
        .filter(|line| !line.starts_with("llg: $finish at time"))
        .map(str::to_owned)
        .collect()
}

fn read_vcd(label: &str, dir: &sim_harness::TempDir, file: &str) -> vcd::Vcd {
    let path = dir.path().join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{label}: read {}: {error}", path.display()));
    vcd::parse(&text).unwrap_or_else(|error| panic!("{label}: malformed VCD: {error}\n{text}"))
}

/// The end-of-step values of `name` and at most one ordinary record per
/// time step (S47-D2).
fn assert_steps(label: &str, vcd: &vcd::Vcd, name: &str, expected: &[(u64, &str)]) {
    let expected: Vec<(u64, String)> = expected
        .iter()
        .map(|(time, value)| (*time, (*value).to_owned()))
        .collect();
    assert_eq!(vcd.step_values(name), expected, "{label}: {name}");
    assert!(
        vcd.max_plain_records_per_step(name) <= 1,
        "{label}: {name} has more than one record in a time step"
    );
}

fn sorted(mut paths: Vec<String>) -> Vec<String> {
    paths.sort();
    paths
}

#[test]
fn vcd_reader_accepts_aliases_and_rejects_malformed_files() {
    const SAMPLE: &str = "$date\n  d\n$end\n$timescale 1ps $end\n\
        $scope module tb $end\n$var wire 4 ! a $end\n$var wire 1 \" b $end\n\
        $scope module u $end\n$var wire 4 ! a $end\n$var real 64 # r $end\n$upscope $end\n\
        $upscope $end\n$enddefinitions $end\n\
        #0\n$dumpvars\nb1 !\n0\"\nr1.5 #\n$end\n#5\nbx0 !\n1\"\n#7\n$dumpoff\nbx !\nx\"\nrNaN #\n$end\n";
    let vcd = vcd::parse(SAMPLE).expect("sample parses");
    assert_eq!(vcd.timescale.as_deref(), Some("1ps"));
    assert_eq!(vcd.paths(), ["tb.a", "tb.b", "tb.u.a", "tb.u.r"]);
    assert_eq!(vcd.alias_groups()["!"], ["tb.a", "tb.u.a"]);
    // Table 83: `1` and `0` extend with 0, `x` with x.
    assert_eq!(
        vcd.changes_of("tb.u.a"),
        [
            (0, "0001".to_owned()),
            (5, "xxx0".to_owned()),
            (7, "xxxx".to_owned())
        ]
    );
    assert_eq!(vcd.checkpoints(Section::DumpOff)[0].0, 7);
    assert_eq!(vcd.checkpoints(Section::DumpVars)[0].1.len(), 3);
    for (label, text) in [
        ("repeated time", SAMPLE.replace("#7", "#5")),
        ("undeclared code", SAMPLE.replace("1\"\n#7", "1$\n#7")),
        (
            "unclosed checkpoint",
            SAMPLE.trim_end_matches("$end\n").to_owned(),
        ),
        ("scalar for vector", SAMPLE.replace("bx0 !", "x!")),
        ("too wide", SAMPLE.replace("bx0 !", "b10101 !")),
        ("value in dumpoff", SAMPLE.replace("bx !", "b1 !")),
        (
            "alias width",
            SAMPLE.replace(
                "$var wire 4 ! a $end\n$var real",
                "$var wire 3 ! a $end\n$var real",
            ),
        ),
        (
            "bad scope type",
            SAMPLE.replace("$scope module u", "$scope struct u"),
        ),
        (
            "time in checkpoint",
            SAMPLE.replace("r1.5 #\n", "r1.5 #\n#1\n"),
        ),
    ] {
        assert!(vcd::parse(&text).is_err(), "{label} was accepted");
    }
}

/// Paths of `trace_values.sv`, with ports, nets, a real and array elements.
fn trace_values_paths() -> Vec<String> {
    let stage = |scope: &[&str]| {
        ["bus", "clk", "d", "drive", "q"]
            .iter()
            .map(|name| {
                let mut components = scope.to_vec();
                components.push(name);
                path(&components)
            })
            .collect::<Vec<_>>()
    };
    let mut paths = vec![
        path(&["tb", "clk"]),
        path(&["tb", "d"]),
        path(&["tb", "q0"]),
        path(&["tb", "q1"]),
        path(&["tb", "shared"]),
        path(&["tb", "level"]),
        path(&["tb", "glitch"]),
        path(&["tb", "mem[0]"]),
        path(&["tb", "mem[1]"]),
        // IEEE 1800-2009 27.6: the second generate construct of `tb` is
        // unnamed, so its block is `genblk2`.
        path(&["tb", "genblk2", "extra"]),
    ];
    // IEEE 1800-2009 27.4: loop instances are `g[0]` and `g[1]` of `tb`.
    paths.extend(stage(&["tb", "g[0]", "first", "s"]));
    paths.extend(stage(&["tb", "g[1]", "second", "s"]));
    sorted(paths)
}

fn check_trace_values(label: &str, dir: &sim_harness::TempDir, output: &Output) {
    assert_success(
        label,
        output,
        include_str!("../fixtures/sim/feature_completion/sim_047/trace_values.out"),
    );
    assert!(runtime_stderr(output).is_empty(), "{label}: {output:?}");
    let vcd = read_vcd(label, dir, "trace.vcd");
    assert_eq!(vcd.timescale.as_deref(), Some("1ps"), "{label}");
    assert!(vcd.date.is_some() && vcd.version.is_some(), "{label}");
    assert_eq!(vcd.paths(), trace_values_paths(), "{label}");
    // The inout port nets collapse into `shared` (one net), so the three
    // names share one identifier code; every other path has its own.
    let bus = sorted(vec![
        path(&["tb", "g[0]", "first", "s", "bus"]),
        path(&["tb", "g[1]", "second", "s", "bus"]),
        path(&["tb", "shared"]),
    ]);
    for (code, names) in vcd.alias_groups() {
        if names.len() > 1 {
            assert_eq!(names, bus, "{label}: alias group {code}");
        }
    }
    assert_eq!(vcd.var(&path(&["tb", "shared"])).width, 1);
    for var in &vcd.vars {
        let expected = if var.path == path(&["tb", "level"]) {
            ("real", 64)
        } else if var.path.ends_with(".d")
            || var.path.ends_with(".q")
            || var.path.ends_with("q0")
            || var.path.ends_with("q1")
        {
            ("wire", 4)
        } else if var.path.contains("mem") {
            ("wire", 8)
        } else {
            ("wire", 1)
        };
        assert_eq!(
            (var.kind.as_str(), var.width),
            expected,
            "{label}: {}",
            var.path
        );
    }
    // The dump starts at the end of step 0 (S47-D1): one $dumpvars
    // checkpoint holding every code once, with the step's final values.
    let checkpoints = vcd.checkpoints(Section::DumpVars);
    assert_eq!(checkpoints.len(), 1, "{label}");
    assert_eq!(checkpoints[0].0, 0, "{label}");
    assert_eq!(checkpoints[0].1.len(), vcd.alias_groups().len(), "{label}");
    assert_eq!(vcd.times, [0, 1000, 2000, 3000, 4000], "{label}");
    let clock = [(0, "0"), (1000, "1"), (2000, "0"), (3000, "1")];
    for name in [
        path(&["tb", "clk"]),
        path(&["tb", "genblk2", "extra"]),
        path(&["tb", "g[0]", "first", "s", "clk"]),
        path(&["tb", "g[1]", "second", "s", "clk"]),
    ] {
        assert_steps(label, &vcd, &name, &clock);
    }
    let d = [(0, "0011"), (2000, "1x0z")];
    assert_steps(label, &vcd, &path(&["tb", "d"]), &d);
    assert_steps(label, &vcd, &path(&["tb", "g[0]", "first", "s", "d"]), &d);
    assert_steps(
        label,
        &vcd,
        &path(&["tb", "g[1]", "second", "s", "d"]),
        &[(0, "1100"), (2000, "0x1x")],
    );
    let q0 = [(0, "xxxx"), (1000, "0011"), (3000, "1x0z")];
    let q1 = [(0, "xxxx"), (1000, "1100"), (3000, "0x1x")];
    assert_steps(label, &vcd, &path(&["tb", "q0"]), &q0);
    assert_steps(label, &vcd, &path(&["tb", "g[0]", "first", "s", "q"]), &q0);
    assert_steps(label, &vcd, &path(&["tb", "q1"]), &q1);
    assert_steps(label, &vcd, &path(&["tb", "g[1]", "second", "s", "q"]), &q1);
    assert_steps(
        label,
        &vcd,
        &path(&["tb", "shared"]),
        &[(0, "x"), (1000, "1"), (3000, "z")],
    );
    assert_steps(
        label,
        &vcd,
        &path(&["tb", "g[0]", "first", "s", "drive"]),
        &[(0, "1")],
    );
    assert_steps(
        label,
        &vcd,
        &path(&["tb", "g[1]", "second", "s", "drive"]),
        &[(0, "0")],
    );
    // The glitch returns to 0 within step 1000: one record of the final 0.
    assert_steps(
        label,
        &vcd,
        &path(&["tb", "glitch"]),
        &[(0, "0"), (1000, "0")],
    );
    assert_steps(
        label,
        &vcd,
        &path(&["tb", "level"]),
        &[(0, "0.5"), (2000, "0.33333333333333331")],
    );
    assert_steps(label, &vcd, &path(&["tb", "mem[0]"]), &[(0, "00000000")]);
    assert_steps(
        label,
        &vcd,
        &path(&["tb", "mem[1]"]),
        &[(0, "11111111"), (2000, "01011010")],
    );
}

#[test]
fn vcd_header_aliases_values_and_times_follow_independent_traces() {
    for optimized in [true, false] {
        let (dir, output) =
            sim_cli::run_case_keeping_dir(SUITE, "trace_values", optimized, &[], &[]);
        check_trace_values(
            &format!("trace_values, optimized={optimized}"),
            &dir,
            &output,
        );
    }
}

#[test]
fn vcd_bytes_are_identical_on_every_value_backend() {
    let gmp = sim_harness::test_gmp_root();
    let mut reference: Option<String> = None;
    for (backend, kernel) in [
        ("legacy", "portable"),
        ("compact", "portable"),
        ("compact", "gmp"),
    ] {
        let envs = [
            ("LLG_DEV_VALUE_BACKEND", backend),
            ("LLG_DEV_COMPACT_KERNELS", kernel),
            ("GMP_ROOT", gmp.as_str()),
        ];
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "trace_values", true, &[], &envs);
        let label = format!("trace_values, {backend}/{kernel}");
        check_trace_values(&label, &dir, &output);
        let text = std::fs::read_to_string(dir.path().join("trace.vcd")).expect("read VCD");
        match &reference {
            None => reference = Some(text),
            Some(reference) => assert_eq!(&text, reference, "{label}"),
        }
    }
}

#[test]
fn dumpvars_calls_of_one_step_add_up_and_later_calls_are_ignored() {
    for optimized in [true, false] {
        let label = format!("targets, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "targets", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/targets.out"),
        );
        assert_eq!(
            runtime_stderr(&output),
            ["llg: waveform: warning: $dumpvars at time 1 ignored: the dump started at time 0"],
            "{label}"
        );
        let vcd = read_vcd(&label, &dir, "trace.vcd");
        // $dumpvars(1, tb.u1): u1's own variables, not u1.l; (0, tb.g[1].l):
        // that instance; (0, tb.mem, tb.b): every element and one variable.
        assert_eq!(
            vcd.paths(),
            sorted(vec![
                path(&["tb", "b"]),
                path(&["tb", "g[1]", "l", "i"]),
                path(&["tb", "g[1]", "l", "r"]),
                path(&["tb", "mem[0]"]),
                path(&["tb", "mem[1]"]),
                path(&["tb", "u1", "i"]),
                path(&["tb", "u1", "m"]),
            ]),
            "{label}"
        );
        assert_steps(&label, &vcd, &path(&["tb", "b"]), &[(0, "11"), (1, "10")]);
        assert_steps(
            &label,
            &vcd,
            &path(&["tb", "u1", "i"]),
            &[(0, "00"), (1, "01")],
        );
        assert_steps(
            &label,
            &vcd,
            &path(&["tb", "u1", "m"]),
            &[(0, "00"), (1, "01")],
        );
        assert_steps(
            &label,
            &vcd,
            &path(&["tb", "g[1]", "l", "i"]),
            &[(0, "01"), (1, "10")],
        );
        assert_steps(
            &label,
            &vcd,
            &path(&["tb", "g[1]", "l", "r"]),
            &[(0, "10"), (1, "01")],
        );
        assert_steps(&label, &vcd, &path(&["tb", "mem[0]"]), &[(0, "00010000")]);
        assert_steps(
            &label,
            &vcd,
            &path(&["tb", "mem[1]"]),
            &[(0, "00010001"), (1, "00100010")],
        );
    }
}

#[test]
fn dumpvars_levels_without_scopes_count_from_the_top_instance() {
    for optimized in [true, false] {
        let label = format!("depth_only, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "depth_only", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/depth_only.out"),
        );
        let vcd = read_vcd(&label, &dir, "trace.vcd");
        assert_eq!(vcd.paths(), ["tb.t"], "{label}");
        assert_steps(&label, &vcd, "tb.t", &[(0, "00"), (1, "11")]);
    }
}

#[test]
fn dynamic_objects_are_excluded_while_the_collector_reclaims_them() {
    for optimized in [true, false] {
        let label = format!("excluded, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(
            SUITE,
            "excluded",
            optimized,
            &[],
            &[("LLG_GC_STRESS", "1")],
        );
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/excluded.out"),
        );
        assert!(runtime_stderr(&output).is_empty(), "{label}: {output:?}");
        let vcd = read_vcd(&label, &dir, "trace.vcd");
        assert_eq!(vcd.paths(), ["tb.count", "tb.ratio", "tb.total"], "{label}");
        assert_eq!(vcd.var("tb.ratio").kind, "real");
        assert_eq!(vcd.var("tb.total").width, 32);
        assert_steps(
            &label,
            &vcd,
            "tb.count",
            &[
                (0, "0000"),
                (1000, "0001"),
                (2000, "0010"),
                (3000, "0011"),
                (4000, "0100"),
            ],
        );
        let total = |value: u32| format!("{value:032b}");
        let totals = [(0, 0), (1000, 4), (2000, 6), (3000, 12), (4000, 19)]
            .map(|(time, value)| (time, total(value)));
        let totals: Vec<(u64, &str)> = totals
            .iter()
            .map(|(time, value)| (*time, value.as_str()))
            .collect();
        assert_steps(&label, &vcd, "tb.total", &totals);
        assert_steps(
            &label,
            &vcd,
            "tb.ratio",
            &[
                (0, "0"),
                (1000, "1"),
                (2000, "1.5"),
                (3000, "3"),
                (4000, "4.75"),
            ],
        );
    }
}

#[test]
fn naming_storage_without_a_vcd_encoding_is_rejected() {
    let reason = "has no VCD/FST representation";
    sim_cli::reject_case(SUITE, "reject_string", reason);
    sim_cli::reject_case(SUITE, "reject_handle", reason);
    sim_cli::reject_case(SUITE, "reject_dynamic", reason);
    sim_cli::reject_case(SUITE, "reject_block_variable", reason);
}

#[test]
fn dump_controls_record_end_of_step_values() {
    for optimized in [true, false] {
        let label = format!("controls, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "controls", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/controls.out"),
        );
        assert_eq!(
            runtime_stderr(&output),
            ["llg: waveform: warning: $dumpfile(\"late.vcd\") ignored: the dump started at time 0"],
            "{label}"
        );
        assert!(
            !dir.path().join("ignored.vcd").exists() && !dir.path().join("late.vcd").exists(),
            "{label}: only the last name before the dump starts is written"
        );
        let vcd = read_vcd(&label, &dir, "trace.vcd");
        assert_eq!(vcd.paths(), ["tb.r", "tb.s", "tb.v"], "{label}");
        // $dumpon at 0 (pending) and the second $dumpoff and the $dumpall at
        // 3 (off) leave no checkpoint (S47-D5).
        assert_eq!(
            vcd.sections,
            [
                (0, Section::DumpVars),
                (2, Section::DumpOff),
                (4, Section::DumpOn),
                (5, Section::DumpAll)
            ],
            "{label}"
        );
        assert_eq!(vcd.times, [0, 1, 2, 4, 5, 6], "{label}");
        let v = |path: &str| vcd.var(path).code.clone();
        let (cv, cs, cr) = (v("tb.v"), v("tb.s"), v("tb.r"));
        let checkpoint = |values: &[(&String, &str)]| {
            values
                .iter()
                .map(|(code, value)| ((*code).clone(), (*value).to_owned()))
                .collect::<Vec<_>>()
        };
        let mut sections = Vec::new();
        for kind in [
            Section::DumpVars,
            Section::DumpOff,
            Section::DumpOn,
            Section::DumpAll,
        ] {
            for (_, mut values) in vcd.checkpoints(kind) {
                values.sort();
                sections.push(values);
            }
        }
        let mut expected = vec![
            checkpoint(&[(&cv, "0001"), (&cs, "0"), (&cr, "0")]),
            checkpoint(&[(&cv, "xxxx"), (&cs, "x"), (&cr, "NaN")]),
            checkpoint(&[(&cv, "0111"), (&cs, "1"), (&cr, "2.5")]),
            checkpoint(&[(&cv, "1000"), (&cs, "1"), (&cr, "2.5")]),
        ];
        for values in &mut expected {
            values.sort();
        }
        assert_eq!(sections, expected, "{label}");
        // Step 1 records 3 once; the final procedure's 9 is recorded at the
        // end time 6; nothing is recorded while the dump is off.
        assert_steps(
            &label,
            &vcd,
            "tb.v",
            &[
                (0, "0001"),
                (1, "0011"),
                (2, "xxxx"),
                (4, "1000"),
                (5, "1000"),
                (6, "1001"),
            ],
        );
        assert_eq!(
            vcd.changes_of("tb.v")[2..4],
            [(2, "0100".to_owned()), (2, "xxxx".to_owned())],
            "{label}: the step's value precedes the $dumpoff checkpoint"
        );
        assert_steps(
            &label,
            &vcd,
            "tb.s",
            &[(0, "0"), (1, "1"), (2, "x"), (4, "1"), (5, "0")],
        );
        assert_steps(
            &label,
            &vcd,
            "tb.r",
            &[(0, "0"), (2, "NaN"), (4, "2.5"), (5, "2.5")],
        );
    }
}

#[test]
fn controls_before_the_first_dumpvars_have_no_effect() {
    for optimized in [true, false] {
        let label = format!("pending, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "pending", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/pending.out"),
        );
        let vcd = read_vcd(&label, &dir, "trace.vcd");
        assert_eq!(
            vcd.sections,
            [
                (1, Section::DumpVars),
                (1, Section::DumpOff),
                (2, Section::DumpOn)
            ],
            "{label}"
        );
        assert_eq!(
            vcd.changes_of("tb.v"),
            [
                (1, "0011".to_owned()),
                (1, "xxxx".to_owned()),
                (2, "0100".to_owned())
            ],
            "{label}"
        );

        let label = format!("no_dumpvars, optimized={optimized}");
        let (dir, output) =
            sim_cli::run_case_keeping_dir(SUITE, "no_dumpvars", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/no_dumpvars.out"),
        );
        assert!(runtime_stderr(&output).is_empty(), "{label}: {output:?}");
        for file in ["trace.vcd", "dump.vcd"] {
            assert!(!dir.path().join(file).exists(), "{label}: {file} written");
        }
    }
}

#[test]
fn dump_file_names_without_the_fst_extension_are_vcd() {
    for optimized in [true, false] {
        let label = format!("dump_extension, optimized={optimized}");
        let (dir, output) =
            sim_cli::run_case_keeping_dir(SUITE, "dump_extension", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/dump_extension.out"),
        );
        let vcd = read_vcd(&label, &dir, "verilog.dump");
        assert_steps(&label, &vcd, "tb.v", &[(0, "0"), (1, "1")]);
    }
}

#[test]
fn an_unwritable_dump_file_is_reported_and_the_run_still_completes() {
    for optimized in [true, false] {
        let label = format!("open_error, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "open_error", optimized, &[], &[]);
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            include_str!("../fixtures/sim/feature_completion/sim_047/open_error.out"),
            "{label}"
        );
        let stderr = runtime_stderr(&output);
        assert_eq!(stderr.len(), 1, "{label}: one report: {stderr:?}");
        assert!(
            stderr[0].starts_with("llg: waveform: cannot open `missing_directory/trace.vcd`: "),
            "{label}: {stderr:?}"
        );
        assert!(!dir.path().join("missing_directory").exists(), "{label}");
    }
}

#[test]
fn dump_limit_keeps_the_declarations_and_appends_a_comment() {
    for optimized in [true, false] {
        let label = format!("limit, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "limit", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/limit.out"),
        );
        let vcd = read_vcd(&label, &dir, "trace.vcd");
        assert_eq!(vcd.paths(), ["tb.a", "tb.b"], "{label}");
        assert!(vcd.changes.is_empty() && vcd.times.is_empty(), "{label}");
        assert_eq!(
            vcd.comments,
            [(0, "dump limit reached".to_owned())],
            "{label}"
        );
    }
}

/// The host command that copies `from` to `to` for `$system`.
fn copy_command(from: &str, to: &str) -> String {
    if cfg!(windows) {
        format!("COPY=\"copy /Y {from} {to}\"")
    } else {
        format!("COPY=\"cp {from} {to}\"")
    }
}

#[test]
fn a_flushed_dump_is_readable_while_the_simulation_runs() {
    let copy = copy_command("trace.vcd", "mid.vcd");
    for optimized in [true, false] {
        let label = format!("flush_read vcd, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(
            SUITE,
            "flush_read",
            optimized,
            &["--define", "FILE=\"trace.vcd\"", "--define", &copy],
            &[("LLG_ALLOW_SYSTEM", "1")],
        );
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/flush_read.out"),
        );
        let mid = read_vcd(&label, &dir, "mid.vcd");
        let end = read_vcd(&label, &dir, "trace.vcd");
        // Every change up to the $dumpflush at time 2 was in the file.
        let before: Vec<_> = end
            .changes
            .iter()
            .filter(|change| change.time <= 2)
            .collect();
        assert_eq!(mid.changes.iter().collect::<Vec<_>>(), before, "{label}");
        assert_steps(
            &label,
            &end,
            "tb.a",
            &[(0, "0000"), (1, "0001"), (2, "0010"), (3, "0011")],
        );
    }
    // FST keeps its hierarchy and block index until close, so the copy is
    // not yet readable; the flush still writes the value blocks.
    let copy = copy_command("trace.fst", "mid.fst");
    let (dir, output) = sim_cli::run_case_keeping_dir(
        SUITE,
        "flush_read",
        true,
        &["--define", "FILE=\"trace.fst\"", "--define", &copy],
        &[("LLG_ALLOW_SYSTEM", "1")],
    );
    assert_success(
        "flush_read fst",
        &output,
        include_str!("../fixtures/sim/feature_completion/sim_047/flush_read.out"),
    );
    let mid = std::fs::metadata(dir.path().join("mid.fst")).expect("flushed FST copy");
    assert!(mid.len() > 0, "flushed FST copy is empty");
    let end = fst_dump(dir.path(), "trace.fst").expect("final FST");
    assert_eq!(
        end.changes_of("tb.a"),
        [
            (0, "0000".to_owned()),
            (1, "0001".to_owned()),
            (2, "0010".to_owned()),
            (3, "0011".to_owned())
        ]
    );
}

/// The text the reader probe prints for an FST file.
#[derive(Debug, Default)]
struct FstDump {
    /// (path, handle, length, alias)
    vars: Vec<(String, u32, u32, bool)>,
    end: u64,
    activity: Vec<(u64, u8)>,
    changes: Vec<(u64, u32, String)>,
}

impl FstDump {
    fn handle(&self, path: &str) -> u32 {
        self.vars
            .iter()
            .find(|var| var.0 == path)
            .unwrap_or_else(|| panic!("{path} missing from {:?}", self.vars))
            .1
    }

    fn changes_of(&self, path: &str) -> Vec<(u64, String)> {
        let handle = self.handle(path);
        self.changes
            .iter()
            .filter(|change| change.1 == handle)
            .map(|change| (change.0, change.2.clone()))
            .collect()
    }
}

const FST_DUMP_PROBE: &str = r#"#include "fstapi.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static void on_change(void* user, uint64_t time, fstHandle handle,
                      const unsigned char* value) {
    (void)user;
    printf("change %llu %u %s\n", (unsigned long long)time, (unsigned)handle,
           (const char*)value);
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    void* reader = fstReaderOpen(argv[1]);
    if (!reader) return 3;
    char path[4096] = "";
    size_t lengths[64];
    int depth = 0;
    struct fstHier* item;
    while ((item = fstReaderIterateHier(reader)) != NULL) {
        if (item->htyp == FST_HT_SCOPE && depth < 64) {
            lengths[depth++] = strlen(path);
            if (path[0]) strcat(path, ".");
            strcat(path, item->u.scope.name);
        } else if (item->htyp == FST_HT_UPSCOPE && depth > 0) {
            path[lengths[--depth]] = 0;
        } else if (item->htyp == FST_HT_VAR) {
            printf("var %s.%s %u %u %d\n", path, item->u.var.name,
                   (unsigned)item->u.var.handle, (unsigned)item->u.var.length,
                   (int)item->u.var.is_alias);
        }
    }
    printf("end %llu\n", (unsigned long long)fstReaderGetEndTime(reader));
    uint32_t activity = fstReaderGetNumberDumpActivityChanges(reader);
    for (uint32_t i = 0; i < activity; i++)
        printf("activity %llu %d\n",
               (unsigned long long)fstReaderGetDumpActivityChangeTime(reader, i),
               (int)fstReaderGetDumpActivityChangeValue(reader, i));
    fstReaderSetFacProcessMaskAll(reader);
    fstReaderIterBlocks(reader, on_change, NULL, NULL);
    fstReaderClose(reader);
    return 0;
}
"#;

/// Build GTKWave's reader (the bundled libfst and zlib, as generated models
/// link them) into `dir` and dump `file` through it.
fn fst_dump(dir: &Path, file: &str) -> Result<FstDump, String> {
    let reader = dir.join("fst_reader");
    std::fs::create_dir_all(reader.join("zlib")).map_err(|error| error.to_string())?;
    for (name, source) in llg::sim::rt::waveform_sources() {
        std::fs::write(reader.join(name), source).map_err(|error| error.to_string())?;
    }
    std::fs::write(reader.join("fst_dump.c"), FST_DUMP_PROBE).map_err(|error| error.to_string())?;
    let compiler = c_compiler::host_c_compiler();
    let zlib = llg::sim::rt::waveform_sources()
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| name.starts_with("zlib/") && name.ends_with(".c"));
    let executable = reader.join(if cfg!(windows) {
        "fst_dump.exe"
    } else {
        "fst_dump"
    });
    let compile = sim_harness::run_command(
        Command::new(&compiler)
            .current_dir(&reader)
            .args([
                "-std=c11",
                "-DFST_CONFIG_INCLUDE=\"fst_config.h\"",
                "-DZ_PREFIX",
                "-DZ_HAVE_UNISTD_H",
                "-I.",
                "-Izlib",
                "fst_dump.c",
                "fstapi.c",
                "fastlz.c",
                "lz4.c",
            ])
            .args(zlib)
            .args(["-lm", "-o"])
            .arg(&executable),
        Duration::from_secs(120),
    )?;
    if !compile.status.success() {
        return Err(format!(
            "compile FST reader with `{compiler}`: {}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let output = sim_harness::run_command(
        Command::new(&executable).arg(dir.join(file)),
        Duration::from_secs(60),
    )?;
    if !output.status.success() {
        return Err(format!("FST reader rejected {file}: {:?}", output.status));
    }
    let mut dump = FstDump::default();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let number = |text: &str| {
            text.parse::<u64>()
                .map_err(|error| format!("{line}: {error}"))
        };
        match fields.as_slice() {
            ["var", path, handle, length, alias] => dump.vars.push((
                (*path).to_owned(),
                number(handle)? as u32,
                number(length)? as u32,
                *alias == "1",
            )),
            ["end", end] => dump.end = number(end)?,
            ["activity", time, value] => dump.activity.push((number(time)?, number(value)? as u8)),
            ["change", time, handle, value] => {
                dump.changes
                    .push((number(time)?, number(handle)? as u32, (*value).to_owned()))
            }
            _ => return Err(format!("unexpected reader line `{line}`")),
        }
    }
    Ok(dump)
}

#[test]
fn fst_values_activity_and_size_limit_through_the_official_reader() {
    for optimized in [true, false] {
        let label = format!("fst_trace, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "fst_trace", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/fst_trace.out"),
        );
        let full =
            fst_dump(dir.path(), "trace.fst").unwrap_or_else(|error| panic!("{label}: {error}"));
        let mut paths: Vec<&str> = full.vars.iter().map(|var| var.0.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(paths, ["tb.r", "tb.s", "tb.v"], "{label}");
        assert_eq!(full.end, 206, "{label}");
        // $dumpvars at 0, $dumpoff at 2, $dumpon at 4.
        assert_eq!(full.activity, [(0, 1), (2, 0), (4, 1)], "{label}");
        // End-of-step values; the $dumpon checkpoint at 4 holds 04 and the
        // step ends with 05; from 6 on the value counts up once per step.
        let mut v = vec![(0, 0x01u32), (1, 0x03), (4, 0x04), (4, 0x05)];
        v.extend((6..=205).map(|time| (time, time as u32)));
        let v: Vec<(u64, String)> = v
            .into_iter()
            .map(|(time, value)| (time, format!("{value:08b}")))
            .collect();
        assert_eq!(full.changes_of("tb.v"), v, "{label}");
        assert_eq!(
            full.changes_of("tb.s"),
            [
                (0, "0".to_owned()),
                (1, "1".to_owned()),
                (4, "1".to_owned())
            ],
            "{label}"
        );
        let r: Vec<(u64, f64)> = full
            .changes_of("tb.r")
            .into_iter()
            .map(|(time, value)| (time, value.parse().expect("real value")))
            .collect();
        assert_eq!(r, [(0, 0.25), (4, 0.5)], "{label}");

        // A size limit stops the dump at a block boundary; the file still
        // opens with the whole hierarchy and a prefix of the values.
        let (dir, output) = sim_cli::run_case_keeping_dir(
            SUITE,
            "fst_trace",
            optimized,
            &["--define", "LIMIT=400"],
            &[],
        );
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/fst_trace.out"),
        );
        let limited =
            fst_dump(dir.path(), "trace.fst").unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!(limited.vars, full.vars, "{label}");
        assert!(limited.end < full.end, "{label}: {}", limited.end);
        assert!(
            !limited.changes.is_empty() && full.changes.starts_with(&limited.changes),
            "{label}: limited changes are not a prefix"
        );
    }
}

#[test]
fn storage_the_design_never_uses_is_dumped_with_its_value() {
    for optimized in [true, false] {
        let label = format!("pruning, optimized={optimized}");
        let (dir, output) = sim_cli::run_case_keeping_dir(SUITE, "pruning", optimized, &[], &[]);
        assert_success(
            &label,
            &output,
            include_str!("../fixtures/sim/feature_completion/sim_047/pruning.out"),
        );
        let vcd = read_vcd(&label, &dir, "trace.vcd");
        assert_eq!(
            vcd.paths(),
            [
                "tb.counter",
                "tb.from_sink",
                "tb.never_used",
                "tb.read_only",
                "tb.table_mem$5B0$5D",
                "tb.table_mem$5B1$5D",
                "tb.u.constant_out",
                "tb.u.unused_in",
                "tb.undriven",
                "tb.unused_real",
                "tb.written_only",
            ],
            "{label}"
        );
        let count = [(0, "0000"), (1, "0001"), (2, "0010"), (3, "0011")];
        assert_steps(&label, &vcd, "tb.counter", &count);
        assert_steps(&label, &vcd, "tb.u.unused_in", &count);
        assert_steps(
            &label,
            &vcd,
            "tb.written_only",
            &[(0, "xxxx"), (1, "0001"), (2, "0010"), (3, "0011")],
        );
        assert_steps(&label, &vcd, "tb.never_used", &[(0, "xxxx")]);
        assert_steps(&label, &vcd, "tb.read_only", &[(0, "0101")]);
        assert_steps(&label, &vcd, "tb.undriven", &[(0, "zzzz")]);
        assert_steps(&label, &vcd, "tb.from_sink", &[(0, "1010")]);
        assert_steps(&label, &vcd, "tb.u.constant_out", &[(0, "1010")]);
        assert_steps(&label, &vcd, "tb.unused_real", &[(0, "0")]);
        assert_steps(&label, &vcd, "tb.table_mem$5B0$5D", &[(0, "xxxxxxxx")]);
    }
}

/// Generate, build and run one fixture through the library with `opts`,
/// returning the dump text and stdout.
fn library_run(
    source: &Path,
    tag: &str,
    opts: llg::sim::opt::OptConfig,
) -> Result<(String, String), String> {
    let dir = sim_harness::TempDir::new(tag)?;
    let output = llg::core::compile::compile_checked(&llg::core::compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .map_err(|error| format!("compile: {error}"))?;
    let db =
        llg::core::db::Db::from_slang(&output.snapshot).map_err(|error| format!("db: {error}"))?;
    let generated = llg::sim::codegen::generate_from_db_with_codegen_options(
        &db,
        &llg::sim::codegen::CodegenOptions {
            optimization: opts,
            ..Default::default()
        },
    )
    .map_err(|error| format!("codegen: {error}"))?;
    let exe =
        llg::sim::build::build_model_cmake(dir.path(), &[("model.c", generated.model_c.as_str())])
            .map_err(|error| format!("cmake: {error}"))?;
    let run = sim_harness::run_command(
        Command::new(&exe).current_dir(dir.path()),
        Duration::from_secs(120),
    )?;
    if !run.status.success() {
        return Err(format!("run: {}", String::from_utf8_lossy(&run.stderr)));
    }
    let vcd = std::fs::read_to_string(dir.path().join("trace.vcd"))
        .map_err(|error| format!("read VCD: {error}"))?;
    Ok((vcd, String::from_utf8_lossy(&run.stdout).into_owned()))
}

/// Generated model text with and without the unused-storage pass.
fn models_with_and_without_storage_pass(source: &Path) -> Result<(String, String), String> {
    let output = llg::core::compile::compile_checked(&llg::core::compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .map_err(|error| format!("compile: {error}"))?;
    let db =
        llg::core::db::Db::from_slang(&output.snapshot).map_err(|error| format!("db: {error}"))?;
    let generate = |optimization| {
        llg::sim::codegen::generate_from_db_with_codegen_options(
            &db,
            &llg::sim::codegen::CodegenOptions {
                optimization,
                ..Default::default()
            },
        )
        .map(|generated| generated.model_c)
        .map_err(|error| format!("codegen: {error}"))
    };
    let kept = llg::sim::opt::OptConfig {
        unused_storage: false,
        ..llg::sim::opt::OptConfig::default()
    };
    Ok((
        generate(llg::sim::opt::OptConfig::default())?,
        generate(kept)?,
    ))
}

/// Step 6: the unused-storage pass never changes a dump. Every VCD fixture
/// of this task and of the waveform suite is generated with and without the
/// pass; differing models are built and run, and their dumps must match.
#[test]
fn the_storage_pass_never_changes_a_dump() {
    // The public CLI acceptance of the task fixture this sweep covers.
    let (_dir, output) = sim_cli::run_case_keeping_dir(SUITE, "pruning", true, &[], &[]);
    assert!(output.status.success(), "{output:?}");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim");
    let mut sources = Vec::new();
    for stem in ["trace_values", "targets", "excluded", "controls", "pruning"] {
        sources.push(root.join(SUITE).join(format!("{stem}.sv")));
    }
    for stem in [
        "aliases",
        "controls",
        "depth",
        "named",
        "partitioned_nets",
        "true_net_alias",
        "unlimited",
    ] {
        sources.push(root.join("waveform").join(format!("{stem}.sv")));
    }
    let mut differing = 0;
    for source in &sources {
        let label = source.display().to_string();
        let (pruned, kept) = models_with_and_without_storage_pass(source)
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        if pruned == kept {
            continue;
        }
        differing += 1;
        let kept_config = llg::sim::opt::OptConfig {
            unused_storage: false,
            ..llg::sim::opt::OptConfig::default()
        };
        let with = library_run(source, "sim047-pruned", llg::sim::opt::OptConfig::default())
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        let without = library_run(source, "sim047-kept", kept_config)
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!(with, without, "{label}: the storage pass changed the dump");
    }
    eprintln!(
        "storage-pass differential: {differing} of {} models differ",
        sources.len()
    );
}
