//! SIM-027: memory-image views, file diagnostics and the Q02 resolution.

use std::process::Output;

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_027";

macro_rules! data {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!(
                "../fixtures/sim/feature_completion/sim_027/",
                $name
            )),
        )
    };
}

const EDITIONS: [&str; 2] = ["v2001", "sv2009"];

fn stdout_text(label: &str, output: &Output) -> String {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(llg::ffi::platform::native_text_to_lf(output.stdout.clone()))
        .unwrap_or_else(|error| panic!("{label}: stdout is not UTF-8: {error}"))
}

/// The memory-file diagnostics on stderr, in order, without the `llg: `
/// prefix: `warning: memory file ...` or `error: memory file ...`.
fn memory_diagnostics(output: &Output) -> Vec<String> {
    let stderr = llg::ffi::platform::native_text_to_lf(output.stderr.clone());
    String::from_utf8_lossy(&stderr)
        .lines()
        .filter_map(|line| line.strip_prefix("llg: "))
        .filter(|line| {
            line.starts_with("warning: memory file ") || line.starts_with("error: memory file ")
        })
        .map(str::to_owned)
        .collect()
}

/// A check for exact stdout and memory-file diagnostics, used on every value
/// backend and in both optimizer modes.
fn exact<'a>(expected: &'a str, diagnostics: &'a [&'a str]) -> impl Fn(&str, &Output) + 'a {
    move |label, output| {
        assert_eq!(stdout_text(label, output), expected, "{label}: stdout");
        assert_eq!(
            memory_diagnostics(output),
            diagnostics,
            "{label}: diagnostics"
        );
    }
}

fn more_words(file: &str) -> String {
    format!(
        "warning: memory file `{file}`: memory file contains more words than the selected range"
    )
}

fn too_few_words(file: &str) -> String {
    format!(
        "warning: memory file `{file}`: memory file contains too few words for the selected range"
    )
}

fn as_strs(lines: &[String]) -> Vec<&str> {
    lines.iter().map(String::as_str).collect()
}

// A03: every legal storage form, each with its own dump read back.
#[test]
fn every_storage_form_loads_and_dumps() {
    let warnings = [
        more_words("d70.mem"),
        more_words("d80.mem"),
        more_words("d10.mem"),
        more_words("d40.mem"),
        more_words("d50.mem"),
        more_words("d60.mem"),
        more_words("d70.mem"),
    ];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "contexts",
        &[],
        &[],
        &exact(
            include_str!("../fixtures/sim/feature_completion/sim_027/contexts.out"),
            &as_strs(&warnings),
        ),
    );
}

#[test]
fn verilog_2001_storage_forms_follow_declaration_order() {
    for (edition, expected) in [
        (
            "v2001",
            include_str!("../fixtures/sim/feature_completion/sim_027/contexts_v2001.2001.out"),
        ),
        (
            "sv2009",
            include_str!("../fixtures/sim/feature_completion/sim_027/contexts_v2001.2009.out"),
        ),
    ] {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "contexts_v2001.v",
            &["--edition", edition],
            &[],
            &exact(expected, &[]),
        );
    }
}

// A01: write -> read round trips per radix and direction, wide words,
// two-state words and X/Z digits.
#[test]
fn dumps_read_back_per_radix_direction_and_width() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "round_trip",
        &[],
        &[],
        &exact(
            include_str!("../fixtures/sim/feature_completion/sim_027/round_trip.out"),
            &[],
        ),
    );
}

#[test]
fn file_grammar_comments_digits_and_sparse_addresses() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "file_format",
        &[],
        &[data!("format.hex"), data!("format.bin")],
        &exact(
            include_str!("../fixtures/sim/feature_completion/sim_027/file_format.out"),
            &[],
        ),
    );
}

// A01/A03: erroneous and out-of-range files never write outside the memory
// and report the category the text prescribes.
#[test]
fn invalid_files_never_write_outside_the_selected_memory() {
    let jump = |file: &str| {
        format!("error: memory file `{file}`: address jump is outside the destination memory or selected range; load terminated")
    };
    let malformed = |file: &str| {
        format!(
            "error: memory file `{file}`: malformed or over-width memory token; load terminated"
        )
    };
    let range =
        "error: memory file `short.hex`: selected range includes an address outside the memory"
            .to_owned();
    let diagnostics = [
        "warning: memory file `missing.hex`: open for reading failed: No such file or directory"
            .to_owned(),
        malformed("bad_digit.hex"),
        jump("outside_task.hex"),
        jump("outside_memory.hex"),
        malformed("unknown_address.hex"),
        more_words("long.hex"),
        too_few_words("short.hex"),
        range.clone(),
        range,
        more_words("long.hex"),
        jump("outside_task.hex"),
    ];
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "invalid_data.v",
            &["--edition", edition],
            &[
                data!("bad_digit.hex"),
                data!("outside_task.hex"),
                data!("outside_memory.hex"),
                data!("unknown_address.hex"),
                data!("long.hex"),
                data!("short.hex"),
            ],
            &|label, output| {
                assert_eq!(
                    stdout_text(label, output),
                    include_str!("../fixtures/sim/feature_completion/sim_027/invalid_data.out"),
                    "{label}: stdout"
                );
                let found = memory_diagnostics(output);
                // The host's wording of a missing file differs; the category
                // and the file do not.
                assert!(
                    found.first().is_some_and(|line| line.starts_with(
                        "warning: memory file `missing.hex`: open for reading failed"
                    )),
                    "{label}: {found:#?}"
                );
                assert_eq!(found[1..], diagnostics[1..], "{label}: diagnostics");
            },
        );
    }
}

// Q02 resolution: settled cases are conformance tests with hand-derived
// expected values.
#[test]
fn q02_words_follow_unsized_literal_extension_and_truncation() {
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "q02_tokens.v",
            &["--edition", edition],
            &[data!("tokens.hex"), data!("tokens.bin")],
            &exact(
                include_str!("../fixtures/sim/feature_completion/sim_027/q02_tokens.out"),
                &[],
            ),
        );
    }
}

#[test]
fn q02_two_state_words_read_unknown_digits_as_zero() {
    let diagnostics = [
        "warning: memory file `tokens.hex`: X/Z memory data converted to a two-state element",
        "warning: memory file `tokens.bin`: X/Z memory data converted to a two-state element",
    ];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "q02_two_state",
        &[],
        &[data!("tokens.hex"), data!("tokens.bin")],
        &exact(
            include_str!("../fixtures/sim/feature_completion/sim_027/q02_two_state.out"),
            &diagnostics,
        ),
    );
}

#[test]
fn q02_selected_views_apply_the_same_word_rules() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "q02_views",
        &[],
        &[data!("tokens.hex")],
        &exact(
            include_str!("../fixtures/sim/feature_completion/sim_027/q02_views.out"),
            &[],
        ),
    );
}

#[test]
fn q02_range_intersections_terminate_or_warn_as_prescribed() {
    let diagnostics = [
        "error: memory file `far_address.hex`: address jump is outside the destination memory or selected range; load terminated".to_owned(),
        too_few_words("two_words.hex"),
        more_words("three_words.hex"),
    ];
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "q02_bounds.v",
            &["--edition", edition],
            &[
                data!("far_address.hex"),
                data!("two_words.hex"),
                data!("three_words.hex"),
            ],
            &exact(
                include_str!("../fixtures/sim/feature_completion/sim_027/q02_bounds.out"),
                &as_strs(&diagnostics),
            ),
        );
    }
}

// A02: same-slot readers satisfy the update rule in any permitted order.
#[test]
fn q02_readers_of_loaded_words_wake_once_in_any_permitted_order() {
    let mut expected = vec![
        "caller=11,22,33,00",
        "reader0=11",
        "reader0b=11",
        "reader2=33",
        "tick=1",
        "done",
    ];
    expected.sort_unstable();
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "wakeup.v",
            &["--edition", edition],
            &[data!("wake.hex")],
            &|label, output| {
                let stdout = stdout_text(label, output);
                let lines: Vec<&str> = stdout.lines().collect();
                let mut sorted = lines.clone();
                sorted.sort_unstable();
                assert_eq!(sorted, expected, "{label}: each line exactly once");
                let position = |line: &str| lines.iter().position(|found| *found == line);
                assert!(
                    position("caller=11,22,33,00") < position("tick=1"),
                    "{label}: the caller prints before it changes `tick`: {lines:#?}"
                );
                assert_eq!(lines.last(), Some(&"done"), "{label}: {lines:#?}");
                assert!(memory_diagnostics(output).is_empty(), "{label}");
            },
        );
    }
}

#[test]
fn image_loads_wake_only_the_changed_words() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_027/wakeup_storage.out");
    let mut woken: Vec<&str> = expected
        .lines()
        .filter(|line| !line.starts_with("done"))
        .collect();
    woken.sort_unstable();
    let done = expected.lines().last().expect("done line");
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "wakeup_storage",
        &[],
        &[data!("wake.hex")],
        &|label, output| {
            let stdout = stdout_text(label, output);
            let lines: Vec<&str> = stdout.lines().collect();
            assert_eq!(lines.last(), Some(&done), "{label}: {lines:#?}");
            let mut found = lines[..lines.len() - 1].to_vec();
            found.sort_unstable();
            assert_eq!(found, woken, "{label}: woken processes");
        },
    );
}

// A03: a selector with side effects in memory_name runs once per call, on
// the copy path and in place, for loads and dumps (S27-D13).
#[test]
fn memory_name_is_evaluated_once() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "single_evaluation",
        &[],
        &[],
        &exact(
            include_str!("../fixtures/sim/feature_completion/sim_027/single_evaluation.out"),
            &as_strs(&[more_words("words.mem")]),
        ),
    );
}

// A03: members of a structure mixing two- and four-state members keep X/Z
// in the four-state member and read it as 0 in the two-state one.
#[test]
fn mixed_state_structure_members_round_trip() {
    let converted =
        "warning: memory file `digits.mem`: X/Z memory data converted to a two-state element";
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "mixed_state_struct",
        &[],
        &[],
        &exact(
            include_str!("../fixtures/sim/feature_completion/sim_027/mixed_state_struct.out"),
            &[converted, converted, converted],
        ),
    );
}

// A03 nearest-illegal and unsupported forms are rejected before a model is
// built.
#[test]
fn neg_memory_names_that_are_not_variable_memories() {
    let invalid_argument = "invalid argument type";
    sim_cli::reject_case(SUITE, "neg_real_memory", invalid_argument);
    sim_cli::reject_case(SUITE, "neg_automatic_real_memory", invalid_argument);
    sim_cli::reject_case(SUITE, "neg_single_word", invalid_argument);
    sim_cli::reject_case(SUITE, "neg_string_elements", invalid_argument);
    sim_cli::reject_case(
        SUITE,
        "neg_net_memory",
        "cannot assign to a net within a procedural context",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_const_ref",
        "cannot assign to read-only variable 'mem'",
    );
}

#[test]
fn neg_unsupported_resizable_forms() {
    sim_cli::reject_case(
        SUITE,
        "neg_associative",
        "$readmemh does not support associative arrays in `tb`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_queue_slice",
        "$readmemh requires a whole dynamic array or queue, not a selection of one, in `tb`",
    );
}

#[test]
fn neg_verilog_2001_memory_file_limits() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_writemem_v2001.v",
        "`$writememh` is not available in IEEE 2001",
        &["--edition", "v2001"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_multidim_v2001.v",
        "$readmemh multidimensional memory views require SystemVerilog-2009 in `tb`",
        &["--edition", "v2001"],
    );
}
