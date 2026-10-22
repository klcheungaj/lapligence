//! ADV-016: `randsequence` grammar execution. Oracles come from IEEE
//! 1800-2009 18.17 as quoted in the fixture readme: the clause's own examples
//! and outcome lists, exact values for deterministic grammars, chi-square
//! bounds for weights, and structural relations (order, balance, replay) for
//! random grammars. No expected output holds a generator-dependent trace; the
//! replay checksum is compared across optimizer modes and value backends only.
use std::cell::RefCell;
use std::process::Output;

use super::sim_cli;

const SUITE: &str = "feature_completion/adv_016";

/// Exit status 1 after exactly `expected` on stdout, with every diagnostic
/// present the given number of times.
fn runtime_failure(label: &str, output: &Output, expected: &str, diagnostics: &[(&str, usize)]) {
    assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    for (diagnostic, count) in diagnostics {
        assert_eq!(
            stderr.matches(diagnostic).count(),
            *count,
            "{label}: {diagnostic}\n{stderr}"
        );
    }
}

// ── A01: the clause's examples, weights, replay and semantic oracles ─────────

#[test]
fn the_clause_examples_produce_their_listed_outcomes() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "lrm_examples",
        include_str!("../fixtures/sim/feature_completion/adv_016/lrm_examples.out"),
        &[],
        &[],
    );
}

/// Pearson chi-square against the weights with the p=0.001 critical value of
/// each set's degrees of freedom; seeds are fixed, so the result is
/// reproducible.
#[test]
fn rule_frequencies_stay_inside_the_predeclared_chi_square_bound() {
    let mut expected = String::new();
    for seed in 1..=3 {
        for set in [
            "lrm 3:2",
            "default 1:1:1",
            "mixed 5:1:2",
            "zero 0:1:3",
            "dynamic 1:1",
            "wide40 3:1",
            "recursive 1:3",
        ] {
            expected.push_str(&format!("{set} seed {seed}: ok\n"));
        }
    }
    expected.push_str("failures=0\n");
    let fixture = include_str!("../fixtures/sim/feature_completion/adv_016/frequency.out");
    assert_eq!(expected, fixture);
    sim_cli::run_case_backend_parity(SUITE, "frequency", fixture, &[], &[]);
}

/// The trace depends on the generator, so the oracle is a relation: the same
/// seed replays (also with waits while another process draws), another seed
/// differs, and every optimizer mode and value backend gives one checksum.
#[test]
fn a_seed_replays_the_same_trace_in_every_mode() {
    let outputs = RefCell::new(Vec::new());
    sim_cli::run_case_checked_matrix(SUITE, "seed_replay", &[], &|label, output: &Output| {
        assert!(output.status.success(), "{label}: {output:?}");
        outputs.borrow_mut().push((
            label.to_owned(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ));
    });
    let outputs = outputs.into_inner();
    let (first_label, first) = &outputs[0];
    let relations = "replay equal: yes\nother seed differs: yes\n\
                     timed replay with another process equal: yes\n";
    assert!(first.starts_with(relations), "{first_label}: {first}");
    assert!(
        first[relations.len()..].starts_with("checksum "),
        "{first_label}: {first}"
    );
    assert_eq!(outputs.len(), 6);
    for (label, output) in &outputs[1..] {
        assert_eq!(output, first, "{label} differs from {first_label}");
    }
}

#[test]
fn typed_arguments_and_results_pass_between_productions() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "results",
        include_str!("../fixtures/sim/feature_completion/adv_016/results.out"),
        &[],
        &[],
    );
}

#[test]
fn recursive_productions_keep_each_generation_apart() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "recursion",
        include_str!("../fixtures/sim/feature_completion/adv_016/recursion.out"),
        &[],
        &[],
    );
}

#[test]
fn rand_join_keeps_sequence_order_and_follows_the_ratio() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "rand_join",
        include_str!("../fixtures/sim/feature_completion/adv_016/rand_join.out"),
        &[],
        &[],
    );
}

#[test]
fn a_large_shared_production_returns_to_each_caller() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "big_production",
        include_str!("../fixtures/sim/feature_completion/adv_016/big_production.out"),
        &[],
        &[],
    );
}

// ── A02: control, contexts, suspension and kill ─────────────────────────────

#[test]
fn break_return_if_case_and_repeat_follow_the_clause() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "control",
        include_str!("../fixtures/sim/feature_completion/adv_016/control.out"),
        &[],
        &[],
    );
}

#[test]
fn randsequence_runs_in_functions_tasks_methods_and_always_blocks() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "contexts",
        include_str!("../fixtures/sim/feature_completion/adv_016/contexts.out"),
        &[],
        &[],
    );
}

/// Under the generated-model sanitizer lane (`LLG_CC=gcc LLG_CFLAGS=
/// '-fsanitize=address,undefined -DLLG_CO_DEBUG'`) this also shows that the
/// frames and strings of killed generations are released.
#[test]
fn a_suspended_production_can_be_killed_or_disabled() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "suspend_kill",
        include_str!("../fixtures/sim/feature_completion/adv_016/suspend_kill.out"),
        &[],
        &[],
    );
}

// ── A03: run-time errors and rejected sources ───────────────────────────────

#[test]
fn unbounded_recursion_stops_at_the_depth_limit() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_016/depth_limit.out");
    sim_cli::run_case_checked_matrix(SUITE, "depth_limit", &[], &|label, output| {
        runtime_failure(
            label,
            output,
            expected,
            &[(
                "depth_limit.sv:10:47: randsequence recursion exceeds the depth limit of 10000 \
                 active generations",
                1,
            )],
        );
    });
}

#[test]
fn invalid_counts_and_ratios_are_located_runtime_errors() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_016/invalid_runtime.out");
    sim_cli::run_case_checked_matrix(SUITE, "invalid_runtime", &[], &|label, output| {
        runtime_failure(
            label,
            output,
            expected,
            &[
                (
                    "invalid_runtime.sv:15:7: all rule weights are zero; nothing is generated",
                    2,
                ),
                (
                    "invalid_runtime.sv:14:29: randsequence repeat count is negative or unknown",
                    1,
                ),
                (
                    "invalid_runtime.sv:16:16: rand join expression is not a real number in 0.0 \
                     to 1.0; 0.5 is used",
                    1,
                ),
            ],
        );
    });
}

/// A negative weight in a recursive production ends the run at once,
/// without final procedures (S16-D3).
#[test]
fn a_negative_weight_ends_the_run() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_016/invalid_weight.out");
    sim_cli::run_case_checked_matrix(SUITE, "invalid_weight", &[], &|label, output| {
        runtime_failure(
            label,
            output,
            expected,
            &[
                (
                    "invalid_weight.sv:13:7: the weight of rule 1 is negative",
                    1,
                ),
                ("randsequence weight is negative", 1),
            ],
        );
    });
}

#[test]
fn unsupported_grammars_are_located_compile_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_formal",
        "neg_output_formal.sv:7:5: formal `o` is not an input; llg supports only input \
         production arguments",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_join_duplicate",
        "neg_join_duplicate.sv:4:24: production `a` is interleaved more than once",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_join_reach",
        "neg_join_reach.sv:5:22: production `a` generates `b`, which the same rand join \
         interleaves",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_join_recursive",
        "neg_join_recursive.sv:5:22: production `a` is recursive or generates a recursive \
         production",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_recursive_handle_frame",
        "neg_recursive_handle_frame.sv:11:7: `h` has a type llg cannot save across a \
         recursive generation",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_recursive_string_array_frame",
        "neg_recursive_string_array_frame.sv:6:12: `w` has a type llg cannot save across a \
         recursive generation",
    );
}

#[test]
fn illegal_rules_weights_and_results_are_rejected_by_the_frontend() {
    sim_cli::reject_case(
        SUITE,
        "neg_continue",
        "neg_continue.sv:6:16 statement can only be used inside a loop",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_void_return_value",
        "neg_void_return_value.sv:5:11 value of type 'int' cannot be assigned to type 'void'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_missing_return_value",
        "neg_missing_return_value.sv:6:15 must provide a return value for non-void production",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_result_type",
        "neg_result_type.sv:8:26 value of type 'C' cannot be assigned to type 'int'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_weight",
        "neg_real_weight.sv:4:18 expression type 'real' is not integral",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_repeat_count",
        "neg_real_repeat_count.sv:4:20 expression type 'real' is not integral",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_undeclared_production",
        "neg_undeclared_production.sv:4:14 use of undeclared identifier 'missing'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_void_result_read",
        "neg_void_result_read.sv:5:32 use of undeclared identifier 'a'",
    );
}

/// Every generation of the same source writes the same C. The order of a
/// rule's implicit result variables once followed the frontend's allocation
/// addresses, which change between runs; several generations make a
/// coincidental match unlikely.
#[test]
fn generated_model_is_identical_across_generations() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("implicit_order.sv");
    for (optimized, generations) in [(true, 6), (false, 3)] {
        let mut first: Option<Vec<(String, Vec<u8>)>> = None;
        for generation in 0..generations {
            let directory =
                super::sim_harness::TempDir::new("implicit_order").expect("generation directory");
            let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_llg"));
            command.current_dir(directory.path()).args([
                "--top",
                "tb",
                "--gen-only",
                "--out-dir",
                "gen",
            ]);
            if !optimized {
                command.arg("--no-opt");
            }
            let output = command.arg(&source).output().expect("run llg");
            assert!(output.status.success(), "{output:?}");
            let model = directory.path().join("gen/sim/tb");
            let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&model)
                .unwrap_or_else(|error| panic!("{}: {error}", model.display()))
                .map(|entry| {
                    let path = entry.expect("model entry").path();
                    let name = path.file_name().unwrap().to_string_lossy().into_owned();
                    (name, std::fs::read(&path).unwrap_or_default())
                })
                .collect();
            files.sort();
            assert!(files.iter().any(|(name, _)| name == "model.c"), "{files:?}");
            match &first {
                None => first = Some(files),
                Some(first) => {
                    for ((name, bytes), (first_name, first_bytes)) in files.iter().zip(first) {
                        assert_eq!(name, first_name);
                        assert!(
                            bytes == first_bytes,
                            "{name} differs in generation {generation} (optimized={optimized})"
                        );
                    }
                    assert_eq!(files.len(), first.len());
                }
            }
        }
    }
}

#[test]
fn implicit_result_variables_of_many_types() {
    sim_cli::run_case(
        SUITE,
        "implicit_order",
        include_str!("../fixtures/sim/feature_completion/adv_016/implicit_order.out"),
        "llg: $finish at time 0 at tb:19:5\n",
        &[],
    );
}
