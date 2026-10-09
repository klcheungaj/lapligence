//! ADV-032: legacy constructs the user decision of 2026-10-08 leaves
//! unsupported must stop the run with one source-located diagnostic before C
//! generation. Expected diagnostics are written out literally from the
//! construct and family names in `docs/sim_features.md`. Procedural
//! `assign`/`deassign` is rejected in every form, local or hierarchical;
//! hierarchical continuous assignment and force/release, and the supported
//! `$q_*` subset, keep executing.
use super::{sim_cli, sim_harness};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/adv_032";
const BOTH: &[&str] = &["v2001", "sv2009"];

const MOS: &str = "MOS and resistive switch primitives";
const TRIREG: &str = "trireg charge storage";
const DIRECTIVE: &str = "charge and delay-mode directives";
const VCD: &str = "extended VCD port dumping";
const INSPECT: &str = "legacy driver and scope inspection tasks";
const PLA: &str = "legacy PLA tasks";
const QUEUE: &str = "stochastic queue form";
const ASSIGN: &str = "legacy procedural assign/deassign form";

fn fixture_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(name)
}

/// The `<line>:<col>: unsupported: <construct> (<family>) is not supported by
/// llg` tail of one expected `error:` line.
fn finding(line: u32, col: u32, construct: &str, family: &str) -> String {
    format!("{line}:{col}: unsupported: {construct} ({family}) is not supported by llg")
}

/// The frontend (Slang or the edition profile) rejects the fixture itself, so
/// the diagnostic is the frontend's and the run never reaches lowering.
fn frontend_rejects(fixture: &str, edition: &str, diagnostic: &str) {
    let output = sim_cli::invoke_with_env(SUITE, fixture, true, &["--edition", edition], &[], &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
    assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
    assert!(stderr.contains(diagnostic), "{fixture}: {stderr}");
}

#[test]
fn mos_and_resistive_switch_primitives_are_rejected() {
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "mos_primitives.v",
            &[
                finding(4, 8, "`nmos` primitive", MOS),
                finding(5, 8, "`pmos` primitive", MOS),
                finding(6, 9, "`rnmos` primitive", MOS),
                finding(7, 9, "`rpmos` primitive", MOS),
                finding(8, 8, "`cmos` primitive", MOS),
                finding(9, 9, "`rcmos` primitive", MOS),
            ],
            &["--edition", edition],
        );
    }
}

#[test]
fn trireg_nets_charge_strengths_and_decay_delays_are_rejected() {
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "trireg_nets.v",
            &[
                finding(1, 32, "`trireg` net `y`", TRIREG),
                finding(6, 10, "`trireg` net `t1`", TRIREG),
                finding(7, 18, "`trireg` net `t2`", TRIREG),
                finding(8, 30, "`trireg` net `t3`", TRIREG),
                finding(9, 25, "`trireg` net `t4`", TRIREG),
                finding(10, 10, "`trireg` net array `t5`", TRIREG),
            ],
            &["--edition", edition],
        );
    }
}

#[test]
fn charge_and_delay_mode_directives_are_rejected_not_ignored() {
    for edition in BOTH {
        let args = ["--edition", edition];
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "directive_default_decay_time.v",
            &[finding(
                1,
                1,
                "directive `` `default_decay_time``",
                DIRECTIVE,
            )],
            &args,
        );
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "directive_default_trireg_strength.v",
            &[finding(
                1,
                1,
                "directive `` `default_trireg_strength``",
                DIRECTIVE,
            )],
            &args,
        );
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "directive_delay_mode_distributed.v",
            &[finding(
                1,
                1,
                "directive `` `delay_mode_distributed``",
                DIRECTIVE,
            )],
            &args,
        );
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "directive_delay_mode_path.v",
            &[finding(1, 1, "directive `` `delay_mode_path``", DIRECTIVE)],
            &args,
        );
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "directive_delay_mode_unit.v",
            &[finding(1, 1, "directive `` `delay_mode_unit``", DIRECTIVE)],
            &args,
        );
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "directive_delay_mode_zero.v",
            &[finding(1, 1, "directive `` `delay_mode_zero``", DIRECTIVE)],
            &args,
        );
    }
}

#[test]
fn directives_in_inactive_conditional_branches_are_not_reported() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/adv_032/directive_inactive.out");
    for edition in BOTH {
        sim_cli::run_case_with_args(
            SUITE,
            "directive_inactive.v",
            expected,
            "llg: $finish at time 0 at tb:8:5\n",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn extended_vcd_port_dumping_is_rejected() {
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "dumpports.v",
            &[
                finding(5, 5, "system task `$dumpports`", VCD),
                finding(7, 5, "system task `$dumpportsoff`", VCD),
                finding(8, 5, "system task `$dumpportson`", VCD),
                finding(9, 5, "system task `$dumpportsall`", VCD),
                finding(10, 5, "system task `$dumpportslimit`", VCD),
                finding(11, 5, "system task `$dumpportsflush`", VCD),
            ],
            &["--edition", edition],
        );
    }
}

#[test]
fn driver_pattern_scale_and_scope_inspection_tasks_are_rejected() {
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "inspection_tasks.v",
            &[
                finding(9, 19, "system task `$getpattern`", INSPECT),
                finding(13, 9, "system task `$countdrivers`", INSPECT),
                finding(14, 9, "system task `$scale`", INSPECT),
                finding(15, 5, "system task `$scope`", INSPECT),
                finding(16, 5, "system task `$showscopes`", INSPECT),
                finding(17, 5, "system task `$showvars`", INSPECT),
                finding(18, 5, "system task `$showvars`", INSPECT),
            ],
            &["--edition", edition],
        );
    }
}

#[test]
fn pla_forms_for_every_gate_and_timing_are_rejected() {
    let names = [
        "$async$and$array",
        "$async$nand$plane",
        "$async$or$array",
        "$async$nor$plane",
        "$sync$and$array",
        "$sync$nand$plane",
        "$sync$or$array",
        "$sync$nor$plane",
    ];
    let findings: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(index, name)| finding(8 + index as u32, 5, &format!("system task `{name}`"), PLA))
        .collect();
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "pla_tasks.v",
            &findings,
            &["--edition", edition],
        );
    }
}

#[test]
fn stochastic_queue_outputs_beyond_whole_integer_variables_are_rejected() {
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "queue_add_array_element.v",
            &[finding(
                6,
                21,
                "`$q_add` status output that is not a whole packed integer variable",
                QUEUE,
            )],
            &["--edition", edition],
        );
    }
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "queue_remove_part_select.v",
            &[finding(
                8,
                18,
                "`$q_remove` job_id output that is not a whole packed integer variable",
                QUEUE,
            )],
            &["--edition", edition],
        );
    }
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "queue_exam_array_element.v",
            &[finding(
                6,
                19,
                "`$q_exam` stat_value output that is not a whole packed integer variable",
                QUEUE,
            )],
            &["--edition", edition],
        );
    }
}

/// One expected rejection per procedural `assign`/`deassign` statement, at the
/// statement's own position.
fn assign_findings(sites: &[(u32, u32, &str)]) -> Vec<String> {
    sites
        .iter()
        .map(|(line, col, stmt)| finding(*line, *col, &format!("procedural `{stmt}`"), ASSIGN))
        .collect()
}

/// The language error for an illegal target is the frontend's own located
/// diagnostic, never the unsupported-by-design message.
fn rejects_as_language_error(fixture: &str, edition: &str, diagnostic: &str) {
    for optimized in [false, true] {
        let output =
            sim_cli::invoke_with_env(SUITE, fixture, optimized, &["--edition", edition], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        assert!(stderr.contains(diagnostic), "{fixture}: {stderr}");
        assert!(
            !stderr.contains("is not supported by llg"),
            "{fixture}: illegal form reported as unsupported: {stderr}"
        );
    }
}

#[test]
fn procedural_assign_and_deassign_are_rejected_in_every_form() {
    // User decision 2026-10-09: procedural assign/deassign (V 9.3.1, SV
    // 10.6.1) is unsupported by design whatever the target; every statement
    // is reported, including the forms an earlier subset simulated.
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "assign_forms.v",
            &assign_findings(&[
                (14, 5, "assign"),
                (18, 5, "deassign"),
                (23, 5, "assign"),
                (27, 5, "deassign"),
                (28, 5, "deassign"),
                (31, 5, "assign"),
                (33, 5, "assign"),
                (35, 5, "deassign"),
                (36, 5, "assign"),
                (38, 5, "deassign"),
            ]),
            &["--edition", edition],
        );
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "assign_in_task.v",
            &assign_findings(&[(5, 7, "assign")]),
            &["--edition", edition],
        );
    }
    for (fixture, line) in [
        ("assign_struct.sv", 5),
        ("assign_unpacked_array.sv", 4),
        ("assign_string.sv", 4),
        ("assign_queue.sv", 4),
        ("assign_class_handle.sv", 7),
    ] {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            fixture,
            &assign_findings(&[(line, 5, "assign")]),
            &["--edition", "sv2009"],
        );
    }
}

#[test]
fn hierarchical_procedural_assign_targets_are_rejected_like_local_ones() {
    // Downward, multi-level, generate-if/for, instance-array, per-instance
    // child, upward, concatenated and real targets; the two `owner`
    // instances share their statements, which are reported once.
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "assign_hierarchical.v",
            &assign_findings(&[
                (15, 5, "assign"),
                (16, 8, "deassign"),
                (22, 5, "assign"),
                (23, 8, "deassign"),
                (47, 5, "assign"),
                (48, 5, "assign"),
                (49, 5, "assign"),
                (50, 5, "assign"),
                (51, 5, "assign"),
                (52, 5, "assign"),
                (53, 8, "deassign"),
                (54, 5, "deassign"),
                (55, 5, "deassign"),
                (56, 5, "deassign"),
                (57, 5, "deassign"),
                (58, 5, "deassign"),
            ]),
            &["--edition", edition],
        );
    }
    sim_cli::reject_case_with_error_lines(
        SUITE,
        "assign_hierarchical_root.sv",
        &assign_findings(&[
            (9, 5, "assign"),
            (10, 8, "deassign"),
            (25, 5, "assign"),
            (26, 5, "assign"),
            (27, 8, "deassign"),
            (28, 5, "deassign"),
        ]),
        &["--edition", "sv2009"],
    );
}

#[test]
fn illegal_procedural_assign_targets_stay_language_errors() {
    // V 9.3.1: only variables and their concatenations; a net or a select
    // reached through a hierarchical path is a frontend error at the target.
    let bad =
        "lvalue of procedural assign/deassign must be a variable or concatenation of variables";
    for edition in BOTH {
        rejects_as_language_error(
            "assign_hierarchical_net.v",
            edition,
            &format!("assign_hierarchical_net.v:12:12 {bad}"),
        );
        rejects_as_language_error(
            "assign_hierarchical_select.v",
            edition,
            &format!("assign_hierarchical_select.v:10:12 {bad}"),
        );
    }
}

#[test]
fn hierarchical_continuous_assignments_drive_other_instances_nets() {
    // Hand-derived from V 6.1/12.4: each net follows its own expression of
    // `src` (1 then 6); the undriven generate-for sibling stays z, the
    // upward driver is constant 5 and the two part-selects compose `b.n`.
    let expected =
        include_str!("../fixtures/sim/feature_completion/adv_032/hier_continuous_assign.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "hier_continuous_assign.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
    let expected =
        include_str!("../fixtures/sim/feature_completion/adv_032/hier_continuous_assign_root.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "hier_continuous_assign_root.sv",
        expected,
        &["--edition", "sv2009"],
        &[],
    );
    frontend_rejects(
        "hier_continuous_assign_root.sv",
        "v2001",
        "is not available in IEEE 2001",
    );
}

#[test]
fn a_continuously_assigned_child_variable_admits_no_other_driver() {
    // SV 6.5: the procedural initializer is a second driver; the run stops
    // with a located language error, not the unsupported-by-design message.
    rejects_as_language_error(
        "hier_continuous_assign_mixed.sv",
        "sv2009",
        "has both a continuous assignment at",
    );
}

#[test]
fn hierarchical_force_and_release_keep_their_lrm_semantics() {
    // Hand-derived from V 9.3.2: forced variables keep the forced value after
    // release, forced nets resume their drivers, a later bit force overrides
    // one bit of a whole-net force, and the upward force window is t=2..4.
    let expected =
        include_str!("../fixtures/sim/feature_completion/adv_032/hier_force_release.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "hier_force_release.v",
            expected,
            &["--edition", edition],
            &[],
        );
        // V 9.3.2: a select of a variable is not a force target.
        rejects_as_language_error(
            "hier_force_variable_select.v",
            edition,
            "hier_force_variable_select.v:10:11 lvalue of force/release must be a net, a variable, a constant select of a net, or a concatenation of these",
        );
    }
}

#[test]
fn supported_stochastic_queue_subset_keeps_executing() {
    // V 17.6: status 1 is queue-full, 3 is queue-empty; stat codes 1 and 3
    // are the current and longest length; FIFO removal returns 5 then 7.
    let expected = include_str!("../fixtures/sim/feature_completion/adv_032/queue_supported.out");
    for edition in BOTH {
        sim_cli::run_case_with_args(
            SUITE,
            "queue_supported.v",
            expected,
            "llg: $finish at time 0 at tb:17:5\n",
            &[],
            &["--edition", edition],
        );
    }
}

/// Every rejected construct still lints (diagnostics, not a crash): the lint
/// pass runs on the owned database before lowering and ignores these forms.
#[test]
fn lint_only_accepts_files_with_unsupported_constructs() {
    for fixture in [
        "mos_primitives.v",
        "trireg_nets.v",
        "directive_default_decay_time.v",
        "directive_delay_mode_zero.v",
        "dumpports.v",
        "inspection_tasks.v",
        "pla_tasks.v",
        "queue_add_array_element.v",
        "assign_in_task.v",
        "assign_struct.sv",
    ] {
        let output = sim_cli::invoke_with_env(SUITE, fixture, true, &["--lint-only"], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{fixture}: {stderr}");
        assert!(!stderr.contains("panicked"), "{fixture}: {stderr}");
    }
}

fn model_c_exists(out_dir: &Path) -> bool {
    out_dir.join("sim").exists()
}

#[test]
fn gen_only_also_stops_before_emission() {
    let out_dir = sim_harness::TempDir::new("adv032-gen").expect("output directory");
    let out = out_dir.path().to_string_lossy().into_owned();
    let output = sim_cli::invoke_with_env(
        SUITE,
        "mos_primitives.v",
        true,
        &["--gen-only", "--out-dir", &out],
        &[],
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(!model_c_exists(out_dir.path()));
}

// PLI 1.0 TF/ACC: llg ships no veriuser.h/acc_user.h and no routines, so a
// library naming `veriusertfs` or a `tf_*`/`acc_*` routine is refused when
// the model build links it.

fn build_with_library(library: &Path) -> Result<(), llg::sim::build::BuildError> {
    let directory = sim_harness::TempDir::new("adv032-pli").expect("model directory");
    let opts = llg::sim::build::CmakeBuildOpts {
        dpi_libraries: vec![library.to_path_buf()],
        ..Default::default()
    };
    llg::sim::build::generate_model_sources_with_opts(
        directory.path(),
        &[(
            "model.c",
            "#define LLG_MODEL_VALUE_ABI 5\n#define LLG_MODEL_VALUE_BACKEND 1\n#define LLG_MODEL_COMPACT_KERNELS 1\nint main(void) { return 0; }\n",
        )],
        &opts,
    )
}

#[test]
fn libraries_naming_pli_1_0_symbols_are_refused_before_the_model_builds() {
    let directory = sim_harness::TempDir::new("adv032-symbols").expect("library directory");
    for (index, (bytes, symbol)) in [
        (&b"\x7fELF\0veriusertfs\0"[..], "veriusertfs"),
        (b"\0_tf_getp\0", "_tf_getp"),
        (b"\0acc_next_net\0acc_initialize\0", "acc_next_net"),
        (b"\0__imp_tf_dofinish\0", "__imp_tf_dofinish"),
    ]
    .into_iter()
    .enumerate()
    {
        let library = directory.path().join(format!("lib{index}.a"));
        std::fs::write(&library, bytes).expect("write library stand-in");
        let error = build_with_library(&library).expect_err("PLI 1.0 symbol must be refused");
        let message = error.to_string();
        assert!(
            matches!(error, llg::sim::build::BuildError::InvalidDpiLibrary { .. })
                && message.contains(&format!(
                    "unsupported: the PLI 1.0 TF/ACC interface (`{symbol}`) is not supported by llg"
                )),
            "{message}"
        );
    }
}

#[test]
fn libraries_with_only_vpi_or_unrelated_names_are_not_refused() {
    let directory = sim_harness::TempDir::new("adv032-clean").expect("library directory");
    let library = directory.path().join("libclean.a");
    std::fs::write(
        &library,
        b"\0vlog_startup_routines\0vpi_register_systf\0acc_total\0tf_helper\0my_tf_getp\0",
    )
    .expect("write library stand-in");
    build_with_library(&library).expect("VPI and unrelated names pass validation");
}

#[test]
fn cli_refuses_a_linked_object_that_uses_pli_1_0() {
    if !cfg!(unix) {
        eprintln!("SKIP: shared PLI fixture build is only enabled on Unix hosts");
        return;
    }
    // Control: the host design alone runs, so the refusal below comes from
    // the linked library.
    sim_cli::run_case(
        SUITE,
        "pli_host.v",
        "host ran\n",
        "llg: $finish at time 0 at tb:4:5\n",
        &[],
    );
    let directory = sim_harness::TempDir::new("adv032-cli").expect("library directory");
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    for (source, symbol) in [
        ("pli_tf_user.c", "tf_getp"),
        ("pli_registration.c", "veriusertfs"),
        ("pli_acc_user.c", "acc_initialize"),
    ] {
        let library = directory.path().join(format!("lib{symbol}.so"));
        let mut build = Command::new(&compiler);
        build
            .args(["-shared", "-fPIC"])
            .arg(fixture_file(source))
            .arg("-o")
            .arg(&library);
        match sim_harness::run_command(&mut build, Duration::from_secs(30)) {
            Ok(result) if result.status.success() => {}
            other => {
                eprintln!("SKIP: C compiler cannot build the PLI fixture: {other:?}");
                return;
            }
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .current_dir(directory.path())
            .args(["--top", "tb", "--dpi-lib"])
            .arg(&library)
            .arg(fixture_file("pli_host.v"));
        let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
            .expect("llg should start");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{source}: {stderr}");
        assert!(output.stdout.is_empty(), "{source}: {output:?}");
        assert!(
            stderr.contains(&format!(
                "unsupported: the PLI 1.0 TF/ACC interface (`{symbol}`) is not supported by llg"
            )),
            "{source}: {stderr}"
        );
    }
}

fn db_of(fixture: &str) -> llg::core::db::Db {
    let out = llg::core::compile::compile_checked(&llg::core::compile::CompileOpts {
        files: vec![fixture_file(fixture).to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
    let db = llg::core::db::Db::from_slang(&out.snapshot).expect("owned database");
    drop(out);
    db
}

/// Owned-capture check (`component_`: no generated model): the database keeps
/// the active directives and their positions after the snapshot is gone, and
/// skips directives in an inactive branch.
#[test]
fn component_db_keeps_active_charge_directives_with_positions() {
    let db = db_of("directive_delay_mode_path.v");
    let seen: Vec<_> = db
        .legacy_directives()
        .iter()
        .map(|directive| (directive.name.as_str(), directive.line, directive.column))
        .collect();
    assert_eq!(seen, [("delay_mode_path", 1, 1)]);
    assert!(db.legacy_directives()[0]
        .file
        .ends_with("directive_delay_mode_path.v"));
    assert!(db_of("directive_inactive.v").legacy_directives().is_empty());
    assert!(db_of("assign_forms.v").legacy_directives().is_empty());
}
