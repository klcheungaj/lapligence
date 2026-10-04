use super::sim_cli;
use std::path::Path;

const SUITE: &str = "feature_completion/rtl_016";

/// Expected stderr with `{source}` standing for the fixture's absolute path,
/// which runtime tagged-access reports carry as their source location.
fn source_stderr(fixture: &str, template: &str) -> String {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    template.replace("{source}", &super::sim_harness::source_display(&source))
}

/// Run a fixture whose run-time errors make it exit 1. All its results are on
/// stderr, interleaved with the reports; compare that stream exactly in both
/// HDL optimizer modes and on every available value backend.
fn assert_runtime_errors(fixture: &str, template: &str) {
    let expected = source_stderr(fixture, template);
    let gmp = std::env::var("LLG_TEST_GMP_ROOT").unwrap_or_default();
    for optimized in [false, true] {
        for (backend, kernel) in [
            ("legacy", "portable"),
            ("compact", "portable"),
            ("compact", "gmp"),
        ] {
            if kernel == "gmp" && gmp.is_empty() {
                eprintln!("BLOCKED GMP parity: set LLG_TEST_GMP_ROOT");
                continue;
            }
            let controls = [
                ("LLG_VALUE_BACKEND", backend),
                ("LLG_COMPACT_KERNELS", kernel),
                ("GMP_ROOT", gmp.as_str()),
            ];
            let output = sim_cli::invoke_with_env(SUITE, fixture, optimized, &[], &controls, &[]);
            let label = format!("{fixture}, {backend}/{kernel}, optimized={optimized}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
            assert!(output.stdout.is_empty(), "{label}: {output:?}");
            assert_eq!(stderr, expected, "{label}");
        }
    }
}

#[test]
fn unpacked_payload_categories_keep_member_types() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_016/unpacked_payloads.out");
    sim_cli::run_case(SUITE, "unpacked_payloads", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "unpacked_payloads", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "unpacked_payloads", expected);
}

#[test]
fn unpacked_tagged_unions_cross_formals_ports_and_storage() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_016/unpacked_contexts.out");
    sim_cli::run_case(SUITE, "unpacked_contexts", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "unpacked_contexts", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "unpacked_contexts", expected);
}

#[test]
fn active_member_selects_read_and_write_only_that_member() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_016/member_selects.out");
    sim_cli::run_case(SUITE, "member_selects", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "member_selects", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "member_selects", expected);
}

#[test]
fn inactive_member_selects_report_and_store_nothing() {
    assert_runtime_errors(
        "wrong_tag_selects",
        "llg: runtime error: access to inactive tagged-union member A at {source}:32:21\n\
         llg: runtime error: access to inactive tagged-union member A at {source}:33:22\n\
         llg: runtime error: access to inactive tagged-union member A at {source}:34:9\n\
         llg: runtime error: access to inactive tagged-union member A at {source}:35:9\n\
         llg: runtime error: access to inactive tagged-union member A at {source}:36:9\n\
         llg: runtime error: access to inactive tagged-union member Row at {source}:38:21\n\
         llg: runtime error: access to inactive tagged-union member Row at {source}:39:9\n\
         bit=x part=xxxx row=xx calls=2\n\
         p B=ff u Count=7\n\
         llg: $finish at time 1000 at tb:44:9\n",
    );
    sim_cli::reject_case_with_args(SUITE, "wrong_tag_selects", "member Row at", &[]);
}

#[test]
fn tagged_patterns_bind_unpacked_payloads_in_scope() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_016/patterns.out");
    sim_cli::run_case(SUITE, "patterns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "patterns", expected, &[], &[]);
}

#[test]
fn undefined_tags_follow_the_enclosing_case_mode() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_016/case_modes.out");
    sim_cli::run_case(SUITE, "case_modes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "case_modes", expected, &[], &[]);
}

#[test]
fn largest_finite_payload_below_capacity_keeps_one_owner() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_016/capacity_boundary.out");
    sim_cli::run_case(SUITE, "capacity_boundary", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "capacity_boundary", expected, &[], &[]);
}

#[test]
fn q03_same_tag_witness() {
    // Adopted FND-002 witness for L-F02-09-02 (SV2009 4.9.4, 10.4.2, 11.9).
    let stderr = "llg: $finish at time 1000 at tb:7:1\n";
    sim_cli::run_case(SUITE, "q03_same_tag_witness", "7\n", stderr, &[]);
    sim_cli::run_case_backend_parity(SUITE, "q03_same_tag_witness", "7\n", &[], &[]);
}

#[test]
fn q03_member_nbas_commit_when_their_member_is_active() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_016/q03_stable.out");
    sim_cli::run_case(SUITE, "q03_stable", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "q03_stable", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "q03_stable", expected);
}

#[test]
fn q03_retag_witness() {
    // Adopted FND-002 witness for L-F02-09-02, now with its resolved oracle
    // (SV2009 4.9.4, 10.4.2, 7.3.2, 11.9).
    assert_runtime_errors(
        "q03_retag_witness",
        "llg: runtime error: nonblocking write to tagged-union member A at {source}:9:15 found an inactive tag at commit\n\
         0100000010\n\
         llg: $finish at time 1000 at tb:10:1\n",
    );
    sim_cli::reject_case_with_args(SUITE, "q03_retag_witness", "at commit", &[]);
}

#[test]
fn q03_single_process_retags_fail_the_commit_check() {
    assert_runtime_errors(
        "q03_retag_kinds",
        "llg: runtime error: nonblocking write to tagged-union member A at {source}:52:9 found an inactive tag at commit\n\
         llg: runtime error: nonblocking write to tagged-union member A at {source}:54:9 found an inactive tag at commit\n\
         llg: runtime error: nonblocking write to tagged-union member A at {source}:56:9 found an inactive tag at commit\n\
         llg: runtime error: nonblocking write to tagged-union member A at {source}:59:9 found an inactive tag at commit\n\
         llg: runtime error: nonblocking write to tagged-union member A at {source}:60:9 found an inactive tag at commit\n\
         llg: runtime error: nonblocking write to tagged-union member P at {source}:63:9 found an inactive tag at commit\n\
         llg: runtime error: nonblocking write to tagged-union member Data at {source}:65:9 found an inactive tag at commit\n\
         llg: runtime error: nonblocking write to tagged-union member Record at {source}:67:9 found an inactive tag at commit\n\
         other B=52\n\
         to_void V\n\
         narrower C=3\n\
         whole_first B=54\n\
         cells A=06 B=17\n\
         inner Q=8\n\
         outer Halt\n\
         wide Count=-2\n\
         llg: runtime error: nonblocking write to tagged-union member A at {source}:69:9 found an inactive tag at commit\n\
         delayed B=15\n\
         llg: $finish at time 4000 at tb:82:9\n",
    );
    sim_cli::reject_case_with_args(SUITE, "q03_retag_kinds", "member Record at", &[]);
}

#[test]
fn q03_interprocess_races_end_in_the_retagged_value() {
    // SV 4.6-4.7 leave the order of the processes woken by `go` open, so the
    // oracle is a set: each target ends as tag B with payload 55. The
    // blocking retag yields exactly one report, at issue or at commit, for
    // the member NBA; the whole-variable NBA race yields zero or one.
    let blocking_issue = source_stderr(
        "q03_race",
        "llg: runtime error: access to inactive tagged-union member A at {source}:18:18\n",
    );
    let blocking_commit = source_stderr(
        "q03_race",
        "llg: runtime error: nonblocking write to tagged-union member A at {source}:18:18 found an inactive tag at commit\n",
    );
    let queued_commit = source_stderr(
        "q03_race",
        "llg: runtime error: nonblocking write to tagged-union member A at {source}:20:18 found an inactive tag at commit\n",
    );
    let results =
        "blocking_target B=55\nqueued_target B=55\nllg: $finish at time 2000 at tb:32:9\n";
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(SUITE, "q03_race", optimized, &[], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "optimized={optimized}: {output:?}"
        );
        let reports = stderr
            .strip_suffix(results)
            .unwrap_or_else(|| panic!("optimized={optimized}: results: {stderr}"));
        let mut lines = reports.split_inclusive('\n').collect::<Vec<_>>();
        let blocking = lines
            .iter()
            .position(|line| *line == blocking_issue || *line == blocking_commit)
            .unwrap_or_else(|| panic!("optimized={optimized}: blocking report: {stderr}"));
        lines.remove(blocking);
        assert!(
            lines.is_empty() || lines == [queued_commit.as_str()],
            "optimized={optimized}: unexpected reports: {stderr}"
        );
    }
    sim_cli::reject_case_with_args(SUITE, "q03_race", "tagged-union member A", &[]);
}

#[test]
fn pattern_bindings_end_with_their_statement() {
    sim_cli::reject_case(
        SUITE,
        "neg_binding_scope",
        "use of undeclared identifier 'n'",
    );
}

#[test]
fn tagged_member_expressions_require_the_member_type() {
    sim_cli::reject_case(SUITE, "neg_member_value", "no implicit conversion");
}

#[test]
fn native_payloads_remain_outside_finite_tagged_storage() {
    sim_cli::reject_case(
        SUITE,
        "neg_native_payload",
        "tagged union `value` in `tb` requires a fixed packed representation",
    );
}

#[test]
fn oversized_payloads_are_rejected_rather_than_flattened() {
    sim_cli::reject_case(
        SUITE,
        "neg_oversized_payload",
        "fixed value payload is 1048577 bits; packed value capacity is 1048575 bits",
    );
}

#[test]
fn oversized_whole_pattern_sources_are_rejected_rather_than_flattened() {
    sim_cli::reject_case(
        SUITE,
        "neg_oversized_pattern",
        "conditional whole-value pattern source in `tb` fixed value payload is 1600008 bits",
    );
}
