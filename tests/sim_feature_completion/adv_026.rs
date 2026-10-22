//! ADV-026: `$sreadmemb` / `$sreadmemh` (IEEE 1800-2009 D.14, IEEE
//! 1364-2001 C.13) as built-in tasks. The strings are loaded by the parser of
//! the memory files, so most checks compare a string-sourced load with the
//! `$readmem*` load of the equivalent file image, and every expected value is
//! also derived by hand in the fixture readme. Positive cases run in both
//! optimizer modes on the legacy, compact/portable and compact/GMP value
//! backends; `.v` fixtures also run in both editions.
use super::{sim_cli, sim_harness};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Output;

const SUITE: &str = "feature_completion/adv_026";

const EDITIONS: [&str; 2] = ["v2001", "sv2009"];

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_026/",
            $name
        ))
    };
}

macro_rules! data {
    ($name:literal) => {
        ($name, fixture!($name))
    };
}

fn stdout_text(label: &str, output: &Output) -> String {
    assert!(
        output.status.success(),
        "{label}: {}",
        sim_cli::observed_output(output)
    );
    String::from_utf8(llg::ffi::platform::native_text_to_lf(output.stdout.clone()))
        .unwrap_or_else(|error| panic!("{label}: stdout is not UTF-8: {error}"))
}

/// The memory-load diagnostics on stderr, in order, without the `llg: `
/// prefix.
fn memory_diagnostics(output: &Output) -> Vec<String> {
    let stderr = llg::ffi::platform::native_text_to_lf(output.stderr.clone());
    String::from_utf8_lossy(&stderr)
        .lines()
        .filter_map(|line| line.strip_prefix("llg: "))
        .filter(|line| {
            ["warning: ", "error: "].iter().any(|severity| {
                line.strip_prefix(severity).is_some_and(|rest| {
                    rest.starts_with("memory file ") || rest.starts_with("memory string ")
                })
            })
        })
        .map(str::to_owned)
        .collect()
}

/// The part of a diagnostic after its label, with the source named
/// uniformly, so a file load and a string load compare equal when they report
/// the same thing.
fn normalized(diagnostic: &str) -> String {
    let (severity, rest) = diagnostic
        .split_once(": ")
        .unwrap_or_else(|| panic!("diagnostic without a category: {diagnostic}"));
    let message = rest
        .split_once("`: ")
        .unwrap_or_else(|| panic!("diagnostic without a label: {diagnostic}"))
        .1;
    format!(
        "{severity}: {}",
        message.replacen("memory string ", "memory file ", 1)
    )
}

/// The label between the backquotes of a diagnostic.
fn diagnostic_label(diagnostic: &str) -> &str {
    let rest = &diagnostic[diagnostic.find('`').expect("a label") + 1..];
    &rest[..rest.find('`').expect("a closing backquote")]
}

/// Line and column (1-based) of the `$sreadmem*` call on the line of
/// `source` that ends with the comment `// <tag>`, and its task name.
fn tagged_call(source: &str, tag: &str) -> (String, usize, usize) {
    let suffix = format!("// {tag}");
    let (index, line) = source
        .lines()
        .enumerate()
        .find(|(_, line)| line.trim_end().ends_with(&suffix))
        .unwrap_or_else(|| panic!("no call tagged {tag}"));
    let column = line.find("$sreadmem").expect("a $sreadmem call");
    (line[column..column + 10].to_owned(), index + 1, column + 1)
}

/// Whether `label` (`$sreadmemh at tb:<line>:<column>`) points at a
/// `$sreadmem*` call of the same name in `source`.
fn assert_label_is_a_call(source: &str, label: &str) {
    let (name, place) = label
        .split_once(" at tb:")
        .unwrap_or_else(|| panic!("unexpected label {label}"));
    let (line, column) = place
        .split_once(':')
        .unwrap_or_else(|| panic!("unexpected label {label}"));
    let line: usize = line.parse().expect("a line number");
    let column: usize = column.parse().expect("a column number");
    let text = source.lines().nth(line - 1).expect("a source line");
    assert!(
        text[column - 1..].starts_with(name),
        "label {label} does not point at the call: {text}"
    );
}

/// The diagnostic a string load of the call tagged `tag` reports.
fn string_diagnostic(source: &str, tag: &str, severity: &str, message: &str) -> String {
    let (name, line, column) = tagged_call(source, tag);
    format!("{severity}: memory string `{name} at tb:{line}:{column}`: {message}")
}

const MALFORMED: &str = "malformed or over-width memory token; load terminated";
const JUMP: &str =
    "address jump is outside the destination memory or selected range; load terminated";
const OUTSIDE: &str = "selected range includes an address outside the memory";
const WORD_OUTSIDE: &str = "memory word address is outside the destination memory; load terminated";
const TOO_FEW: &str = "memory string contains too few words for the selected range";
const MORE: &str = "memory string contains more words than the selected range";
const DIGITS: &str = "memory word has more digits than the element holds; load terminated";

/// For each pair of (file, string) lines `X file ...` / `X str  ...` of an
/// equivalence fixture's output, the words after the tag are equal.
fn assert_file_and_string_agree(label: &str, stdout: &str) {
    let lines: Vec<&str> = stdout.lines().collect();
    let mut pairs = 0;
    for window in lines.windows(2) {
        let (Some(file), Some(string)) = (
            window[0].split_once(" file "),
            window[1].split_once(" str  "),
        ) else {
            continue;
        };
        assert_eq!(file.0, string.0, "{label}: pair of tags");
        assert_eq!(file.1, string.1, "{label}: {} file vs string", file.0);
        pairs += 1;
    }
    assert!(pairs >= 6, "{label}: only {pairs} pairs in {stdout}");
}

// A01: several memories, ascending and descending bounds, sparse `@`
// addresses, X/Z digits and wide words; each loaded from strings and from the
// file image of the same strings, in both editions.
#[test]
fn strings_load_like_the_equivalent_file_image() {
    let source = fixture!("equivalence.v");
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "equivalence.v",
            &["--edition", edition],
            &[
                data!("a.hex"),
                data!("b.hex"),
                data!("f.hex"),
                data!("g.hex"),
                data!("d.hex"),
                data!("e.hex"),
                data!("h.bin"),
            ],
            &|label, output| {
                let stdout = stdout_text(label, output);
                assert_eq!(stdout, fixture!("equivalence.out"), "{label}: stdout");
                assert_file_and_string_agree(label, &stdout);
                // The word-count rules are the file rules: a hole after an
                // `@` address is reported by Verilog-2001 only, and a word
                // past the finish address by both.
                let found = memory_diagnostics(output);
                let mut expected: Vec<(&str, &str)> = Vec::new();
                if edition == "v2001" {
                    expected.push(("a.hex", "contains too few words for the selected range"));
                }
                expected.push(("f.hex", "contains more words than the selected range"));
                if edition == "v2001" {
                    expected.push(("d.hex", "contains too few words for the selected range"));
                    expected.push(("h.bin", "contains too few words for the selected range"));
                }
                assert_eq!(found.len(), expected.len() * 2, "{label}: {found:#?}");
                for (pair, (file, message)) in found.chunks(2).zip(&expected) {
                    assert_eq!(
                        pair[0],
                        format!("warning: memory file `{file}`: memory file {message}"),
                        "{label}"
                    );
                    assert_eq!(normalized(&pair[1]), normalized(&pair[0]), "{label}");
                    assert_label_is_a_call(source, diagnostic_label(&pair[1]));
                }
            },
        );
    }
}

// A01 with SystemVerilog element types: two-state, signed, packed-struct and
// enumerated words, a two-dimensional memory and a slice.
#[test]
fn systemverilog_element_types_load_like_the_equivalent_file_image() {
    let source = fixture!("equivalence_sv.sv");
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "equivalence_sv",
        &[],
        &[
            data!("c.bin"),
            data!("i.hex"),
            data!("p.hex"),
            data!("n.hex"),
            data!("m.hex"),
            data!("v.hex"),
        ],
        &|label, output| {
            let stdout = stdout_text(label, output);
            assert_eq!(stdout, fixture!("equivalence_sv.out"), "{label}: stdout");
            assert_file_and_string_agree(label, &stdout);
            // The two-state memory reads x/z digits as 0 with one warning
            // (21.4.2) and five words fill five of six cells; a nine-digit
            // `int` word ends its load (S27-D2).
            let found = memory_diagnostics(output);
            assert_eq!(found.len(), 6, "{label}: {found:#?}");
            assert_eq!(
                found[0],
                "warning: memory file `c.bin`: X/Z memory data converted to a two-state element",
                "{label}"
            );
            assert_eq!(
                found[1],
                "warning: memory file `c.bin`: memory file contains too few words for the selected range",
                "{label}"
            );
            assert_eq!(normalized(&found[2]), normalized(&found[0]), "{label}");
            assert_eq!(normalized(&found[3]), normalized(&found[1]), "{label}");
            assert_label_is_a_call(source, diagnostic_label(&found[2]));
            assert_label_is_a_call(source, diagnostic_label(&found[3]));
            assert_eq!(
                found[4],
                format!("error: memory file `i.hex`: {DIGITS}"),
                "{label}"
            );
            assert_eq!(normalized(&found[5]), normalized(&found[4]), "{label}");
        },
    );
}

// Step 1: every storage form that `$readmemh` accepts is a memory_name of
// `$sreadmemh`.
#[test]
fn every_storage_form_loads_from_strings() {
    sim_cli::run_case_checked_matrix_with_files(SUITE, "contexts", &[], &[], &|label, output| {
        assert_eq!(
            stdout_text(label, output),
            fixture!("contexts.out"),
            "{label}: stdout"
        );
        assert_eq!(memory_diagnostics(output), Vec::<String>::new(), "{label}");
    });
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "contexts_v2001.v",
            &["--edition", edition],
            &[],
            &|label, output| {
                assert_eq!(
                    stdout_text(label, output),
                    fixture!("contexts_v2001.out"),
                    "{label}: stdout"
                );
                assert_eq!(memory_diagnostics(output), Vec::<String>::new(), "{label}");
            },
        );
    }
}

// Decisions S26-D16 and S26-D20: the strings of a call are one image; the end
// of a string separates tokens, a line comment ends with its string and a
// block comment continues into the next one.
#[test]
fn strings_form_one_image_with_a_token_boundary_between_them() {
    let source = fixture!("joining.v");
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "joining.v",
            &["--edition", edition],
            &[],
            &|label, output| {
                assert_eq!(
                    stdout_text(label, output),
                    fixture!("joining.out"),
                    "{label}: stdout"
                );
                let found = memory_diagnostics(output);
                // The holes after `@` addresses of J2 and J7 are Verilog-2001
                // warnings only.
                let expected: Vec<String> = if edition == "v2001" {
                    let at = |marker: &str| {
                        let (index, line) = source
                            .lines()
                            .enumerate()
                            .find(|(_, line)| line.contains(marker))
                            .unwrap_or_else(|| panic!("no line with {marker}"));
                        format!(
                            "warning: memory string `$sreadmemh at tb:{}:{}`: {TOO_FEW}",
                            index + 1,
                            line.find("$sreadmem").expect("a call") + 1
                        )
                    };
                    vec![at("show(\"J2\")"), at("show(\"J7\")")]
                } else {
                    Vec::new()
                };
                assert_eq!(found, expected, "{label}");
            },
        );
    }
}

/// The events of `text` grouped by time stamp: `[t] rest` lines.
fn by_time(text: &str) -> BTreeMap<u64, BTreeSet<String>> {
    let mut groups: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    for line in text.lines() {
        let rest = line.strip_prefix('[').expect("a time stamp");
        let (time, event) = rest.split_once("] ").expect("a closing bracket");
        groups
            .entry(time.parse().expect("a time"))
            .or_default()
            .insert(event.to_owned());
    }
    groups
}

// Step 3: a `$sreadmemh` into a watched memory wakes processes, updates
// continuous assignments and fires VPI callbacks exactly like `$readmemh` of
// the same image.
#[test]
fn string_loads_notify_like_file_loads() {
    let directory = sim_harness::TempDir::new("adv026-notify").expect("temporary directory");
    let vpi = super::sim_041::static_archive(SUITE, &directory, "notify", &[]);
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "notify",
        &["--dpi-lib", &vpi],
        &[data!("w1.hex"), data!("w2.hex")],
        &|label, output| {
            let stdout = stdout_text(label, output);
            let found = by_time(&stdout);
            assert_eq!(found, by_time(fixture!("notify.out")), "{label}");
            // The string load of a step reports what the file load of the
            // step before it reported, with `s` for `f` and `sums` for
            // `sumf`.
            for time in [2, 6] {
                let file: BTreeSet<String> = found[&time]
                    .iter()
                    .map(|event| {
                        event
                            .replacen("f w", "s w", 1)
                            .replace("totf", "tots")
                            .replace("sumf", "sums")
                    })
                    .collect();
                assert_eq!(file, found[&(time + 1)], "{label}: time {time}");
            }
            for quiet in [4, 5] {
                assert!(
                    !found.contains_key(&quiet),
                    "{label}: reloading the same image at {quiet} is no event"
                );
            }
            assert_eq!(memory_diagnostics(output), Vec::<String>::new(), "{label}");
        },
    );
}

// A02: diagnostics by category, no write outside the range or the memory.
#[test]
fn erroneous_strings_and_arguments_report_and_never_write_outside() {
    let source = fixture!("diagnostics.v");
    let cases: [(&str, &str, &str); 14] = [
        ("D1", "error", MALFORMED),
        ("D2", "error", MALFORMED),
        ("D3", "error", JUMP),
        ("D4", "error", JUMP),
        ("D5", "warning", TOO_FEW),
        (
            "D6",
            "error",
            "start address is unknown or outside the index range",
        ),
        ("D7", "error", WORD_OUTSIDE),
        ("D8", "warning", TOO_FEW),
        ("D9", "warning", MORE),
        ("D10", "warning", TOO_FEW),
        ("D11", "warning", TOO_FEW),
        ("D12", "error", MALFORMED),
        ("D13", "error", MALFORMED),
        ("D14", "error", WORD_OUTSIDE),
    ];
    let expected: Vec<String> = cases
        .iter()
        .map(|(tag, severity, message)| string_diagnostic(source, tag, severity, message))
        .collect();
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "diagnostics.v",
            &["--edition", edition],
            &[],
            &|label, output| {
                assert_eq!(
                    stdout_text(label, output),
                    fixture!("diagnostics.out"),
                    "{label}: stdout"
                );
                assert_eq!(memory_diagnostics(output), expected, "{label}");
            },
        );
    }
}

#[test]
fn erroneous_systemverilog_storage_reports_and_never_writes_outside() {
    let source = fixture!("diagnostics_sv.sv");
    let cases: [(&str, &str, &str); 12] = [
        ("E1", "error", DIGITS),
        ("S1", "error", WORD_OUTSIDE),
        ("S2", "warning", TOO_FEW),
        ("S3", "error", JUMP),
        ("R1", "error", OUTSIDE),
        ("R2", "error", WORD_OUTSIDE),
        ("R3", "error", WORD_OUTSIDE),
        ("R4", "warning", MORE),
        ("A1", "warning", MORE),
        (
            "W1",
            "warning",
            "X/Z memory data converted to a two-state element",
        ),
        ("W1", "warning", TOO_FEW),
        ("W2", "error", DIGITS),
    ];
    // S4 (a descending load of a slice) reports nothing.
    let expected: Vec<String> = cases
        .iter()
        .map(|(tag, severity, message)| string_diagnostic(source, tag, severity, message))
        .collect();
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "diagnostics_sv",
        &[],
        &[],
        &|label, output| {
            assert_eq!(
                stdout_text(label, output),
                fixture!("diagnostics_sv.out"),
                "{label}: stdout"
            );
            assert_eq!(memory_diagnostics(output), expected, "{label}");
        },
    );
}

// Decision S26-D17: a data argument may be any string expression (variable,
// parameter, concatenation, formatted string) or an integral value holding
// text, read each time the task runs, in both editions.
#[test]
fn string_expression_arguments_load_their_current_text() {
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "string_expressions",
        &[],
        &[],
        &|label, output| {
            assert_eq!(
                stdout_text(label, output),
                fixture!("string_expressions.out"),
                "{label}: stdout"
            );
            assert_eq!(memory_diagnostics(output), Vec::<String>::new(), "{label}");
        },
    );
    for edition in EDITIONS {
        sim_cli::run_case_checked_matrix_with_files(
            SUITE,
            "string_expressions_v2001.v",
            &["--edition", edition],
            &[],
            &|label, output| {
                assert_eq!(
                    stdout_text(label, output),
                    fixture!("string_expressions_v2001.out"),
                    "{label}: stdout"
                );
                // The `@` addresses of the second load leave holes, which are
                // a Verilog-2001 warning only.
                let expected: Vec<String> = if edition == "v2001" {
                    vec![format!(
                        "warning: memory string `$sreadmemh at tb:14:5`: {TOO_FEW}"
                    )]
                } else {
                    Vec::new()
                };
                assert_eq!(memory_diagnostics(output), expected, "{label}");
            },
        );
    }
}

// A02: wrong argument order, count and type, and memories that cannot be
// loaded, are rejected when the design is compiled, with their location.
#[test]
fn wrong_forms_are_rejected_at_compile_time() {
    sim_cli::reject_case(
        SUITE,
        "neg_order",
        "neg_order.sv:5:22 expression is not assignable",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_count",
        "neg_count.sv:4:11 too few arguments for '$sreadmemh'; expected 4 but 3 were provided",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_scalar",
        "neg_scalar.sv:4:28 invalid argument type 'int' to system task",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_net_memory",
        "neg_net_memory.sv:4:22 cannot assign to a net within a procedural context",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_memory",
        "neg_real_memory.sv:4:22: $sreadmemh does not support real or shortreal memory elements",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_string_elements",
        "neg_string_elements.sv:4:22: $sreadmemh requires packed integral or packed-struct \
         memory elements",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wildcard_index",
        "neg_wildcard_index.sv:5:22: $sreadmemh requires an associative array with an index of \
         integral type, not a wildcard `[*]` index",
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_slice_v2001.v",
        "neg_slice_v2001.v:4:22: $sreadmemh memory slices require SystemVerilog-2009",
        &["--edition", "v2001"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_multidimensional_v2001.v",
        "neg_multidimensional_v2001.v:4:22: $sreadmemh multidimensional memory views require \
         SystemVerilog-2009",
        &["--edition", "v2001"],
    );
}

// A03 with decision S99-D6: the task is a built-in. Every other test runs it
// with no VPI registration; a registration of the name stops a design that
// calls it, and is accepted when the design calls only `$sreadmemb`.
#[test]
fn registrations_of_sreadmem_follow_the_builtin_override_policy() {
    let directory = sim_harness::TempDir::new("adv026-override").expect("temporary directory");
    let vpi = super::sim_041::static_archive(SUITE, &directory, "sreadmem_override", &[]);
    sim_cli::run_case_checked_matrix(
        SUITE,
        "sreadmem_unused_registration",
        &["--dpi-lib", &vpi],
        &|label, output| {
            assert_eq!(
                stdout_text(label, output),
                fixture!("sreadmem_unused_registration.out"),
                "{label}"
            );
            assert!(
                !String::from_utf8_lossy(&output.stderr).contains("VPI"),
                "{label}: {}",
                sim_cli::observed_output(output)
            );
        },
    );
    // The resolved native spelling llg reports: on Windows `SUITE`'s `/`
    // would stay in a joined path.
    let source = llg::ffi::platform::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join("neg_sreadmem_override.sv"),
    )
    .expect("resolve the fixture");
    let diagnostic = format!(
        "llg: VPI error: a VPI application registers `$sreadmemh`, which llg implements as a \
         built-in (called at {}:7); llg does not support overriding built-in system tasks and \
         functions (IEEE 1800-2009 36.3.2)",
        source.display()
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_sreadmem_override",
        &diagnostic,
        &["--dpi-lib", &vpi],
    );
    let Some(plugin) = super::sim_040::plugin_library(SUITE, &directory, "sreadmem_override", &[])
    else {
        return;
    };
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            SUITE,
            "neg_sreadmem_override",
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
