use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_013";

#[test]
fn disjoint_members_cells_and_ranges_keep_separate_writers() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/member_writers.out");
    sim_cli::run_case(SUITE, "member_writers", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "member_writers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "member_writers", expected);
}

#[test]
fn comb_follows_function_reads_and_selectors() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/transitive_reads.out");
    sim_cli::run_case(SUITE, "transitive_reads", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "transitive_reads", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "transitive_reads", expected);
}

#[test]
fn nested_ref_ports_carry_reads_and_owned_writes() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/ref_ports.out");
    sim_cli::run_case(SUITE, "ref_ports", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "ref_ports", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "ref_ports", expected);
}

#[test]
fn closed_latch_holds_and_notifies_nobody() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/closed_latch.out");
    sim_cli::run_case(SUITE, "closed_latch", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "closed_latch", expected, &[], &[]);
}

#[test]
fn comb_and_latch_run_once_at_time_zero_after_procedures_start() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/time_zero.out");
    sim_cli::run_case(
        SUITE,
        "time_zero",
        expected,
        "",
        &["combinational always process in `tb` reads no signals; evaluating once at time 0"],
    );
    sim_cli::run_case_backend_parity(SUITE, "time_zero", expected, &[], &[]);
}

#[test]
fn data_changes_alone_do_not_wake_always_ff() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/ff_wake.out");
    sim_cli::run_case(SUITE, "ff_wake", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "ff_wake", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "ff_wake", expected);
}

#[test]
fn overrides_are_not_competing_writers() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/override_writers.out");
    sim_cli::run_case(SUITE, "override_writers", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "override_writers", expected, &[], &[]);
}

#[test]
fn unchanged_results_do_not_notify() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/changed_only.out");
    sim_cli::run_case(SUITE, "changed_only", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "changed_only", expected, &[], &[]);
}

#[test]
fn descriptor_arrays_in_processes_without_flattening() {
    use super::sim_harness;
    use std::process::Command;
    use std::time::Duration;

    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_013/descriptor_processes.out");
    sim_cli::run_case(SUITE, "descriptor_processes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_processes", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "descriptor_processes", expected);

    // Per-cell sensitivity or a flattened row copy would grow the model with
    // the 65,537-cell extent; descriptor transport keeps it bounded.
    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("rtl013-descriptor").expect("scratch");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_013/descriptor_processes.sv");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .arg(source)
            .args(["--top", "tb", "--gen-only", "--out-dir"]);
        command.arg(directory.path());
        if !optimized {
            command.arg("--no-opt");
        }
        let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
            .expect("generate descriptor model");
        assert!(output.status.success(), "{output:?}");
        let model =
            std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model source");
        assert!(
            model.len() < 200_000,
            "unexpected model size: {}",
            model.len()
        );
    }
}

#[test]
fn strings_wake_implicit_sensitivity() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_013/string_sensitivity.out");
    sim_cli::run_case(SUITE, "string_sensitivity", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "string_sensitivity", expected, &[], &[]);
    let witness = include_str!("../fixtures/sim/feature_completion/rtl_013/native_comb.out");
    sim_cli::run_case(
        SUITE,
        "native_comb",
        witness,
        "llg: $finish at time 1000 at tb:7:1\n",
        &[],
    );
}

#[test]
fn pruned_branches_keep_wake_sources() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/optimizer_wakes.out");
    sim_cli::run_case(SUITE, "optimizer_wakes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "optimizer_wakes", expected, &[], &[]);
}

#[test]
fn generated_parameterized_stages_compose() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_013/composition.out");
    sim_cli::run_case(SUITE, "composition", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "composition", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "composition", expected);
}

#[test]
fn neg_timing_forks_and_event_controls() {
    sim_cli::reject_case(
        SUITE,
        "neg_comb_delay",
        "statements that pass time are not allowed in this context",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_comb_fork",
        "fork-join is not allowed in always_comb procedure",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_ff_two_events",
        "always_ff procedure must have one and only one event control",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_ff_intra_event",
        "always_ff procedure must have one and only one event control",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_ff_task_timing",
        "task 't' called from 'always_ff' procedure cannot contain blocking timing controls",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_latch_task_event",
        "task 't' called from 'always_latch' procedure cannot contain blocking timing controls",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_ff_wait",
        "always_ff process `tb` cannot contain a timing control at",
    );
}

#[test]
fn neg_overlapping_writers() {
    sim_cli::reject_case(
        SUITE,
        "neg_ff_member_initial",
        "process `tb.always_ff` has multiple writers for `tb.s.a`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_comb_cell_overlap",
        "process `tb.always_comb` has multiple writers for `tb.m[1]`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_ff_continuous_cell",
        "variable storage `tb.m[2]` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_called_writer",
        "process `tb.always_comb` has multiple writers for `tb.g`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_ref_port_writers",
        "process `tb.a.always_ff` has multiple writers for `tb.x`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_hier_writer",
        "process `tb.s.always_ff` has multiple writers for `tb.x`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_member_ref_writer",
        "process `tb.u.always_ff` has multiple writers for `tb.s.a`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wide_row_procedural",
        "has multiple writers for `tb.two[1][0] through tb.two[1][65536]`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wide_row_overlap",
        "variable storage `tb.two[1][0] through tb.two[1][65536]` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wide_row_port",
        "variable storage `tb.two[0][0] through tb.two[0][65536]` has both a continuous assignment",
    );
}
