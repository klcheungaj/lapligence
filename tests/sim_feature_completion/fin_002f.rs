//! FIN-002 step 4: the remaining rows of the failure matrix.
//!
//! | Row | Fixture | What ends, breaks or goes stale | Expected outcome |
//! | --- | --- | --- | --- |
//! | cancellation | `cancel_calls` | `disable`, `kill()`, `$finish`, `$fatal`, `$stop` in nested calls, randomize hooks, a coverage sample, after a clocking event | industry-standard behaviour |
//! | | `cancel_expressions` | the same in a function called from 62 kinds of operand | industry-standard behaviour |
//! | | `cancel_final` | the same in a `final` procedure | industry-standard behaviour |
//! | | `cancel_sequences` | the same in assertion actions, `expect` and a sequence event control | industry-standard behaviour |
//! | | `cancel_regions` | the same in program processes and in processes an NBA wakes | industry-standard behaviour |
//! | | `cancel_dpi`, `cancel_dpi_calls` | the same in DPI imports and exports, `disable fork` around them | LRM 35.9 (readme) |
//! | | `cancel_vpi_regions`, `cancel_vpi_statements` | `vpi_control(vpiFinish)`, removed callbacks in a callback of every region and reason | LRM 38.10 (readme) |
//! | stale handles | `null_handles` | null class, virtual interface, event, mailbox, semaphore, process handles | industry-standard behaviour |
//! | | `vpi_stale` | VPI handles of elements, objects, frames and threads that are gone, a pending event handle at the end, a write that makes more handles | LRM 37, 38 (readme) |
//! | | `vpi_handles` | chosen routines on live, released, dead and wild handles | no crash |
//! | | `foreign_chandle` | chandles whose C object was freed | the value is passed on unchanged (6.14) |
//! | storms | `event_storm` | many processes, NBAs, `#0` loops, gates, events, mailboxes at one time | a member of the set the readme derives |
//! | wrong types | `wrong_cast`, `wrong_plusargs` | values and objects that do not fit | industry-standard behaviour |
//! | | `wrong_readmem`, `wrong_mailbox`, `wrong_vpi_formats` | data, messages and formats of another type | LRM (readme) |
//! | | `reject_dpi_types`, `reject_object_types` | DPI signatures, virtual interfaces, mailboxes, `$cast`, coverage methods | exactly the marked lines are refused |
//!
//! SDF values are in `adv_012` and the `adversarial_inputs` corpora, the
//! language server row in `lsp_stdio::storms`.
use super::{fuzz, sim_041, sim_cli, sim_harness};
use std::collections::BTreeSet;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

const SUITE: &str = "feature_completion/fin_002f";
const RUN_TIMEOUT: Duration = Duration::from_secs(120);

/// Seeds of `vpi_handles` per run; `LLG_FUZZ_CASES` names a longer campaign.
const HANDLE_SEEDS: u64 = 24;

macro_rules! expected {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/fin_002f/",
            $name,
            ".out"
        ))
    };
}

/// The C companion of a fixture.
#[derive(Clone, Copy)]
enum Companion {
    None,
    Dpi(&'static str),
    /// A VPI application and the prototypes of its system tasks.
    Vpi(&'static str, &'static [&'static str]),
}

impl Companion {
    /// The `llg` arguments that link the companion, built in `directory`.
    fn arguments(self, directory: &sim_harness::TempDir) -> Vec<String> {
        let (stem, prototypes): (_, &[&str]) = match self {
            Self::None => return Vec::new(),
            Self::Dpi(stem) => (stem, &[]),
            Self::Vpi(stem, prototypes) => (stem, prototypes),
        };
        let mut arguments = vec![
            "--dpi-lib".to_owned(),
            sim_041::static_archive(SUITE, directory, stem, &[]),
        ];
        if matches!(self, Self::Vpi(..)) {
            arguments.push("--vpi".to_owned());
        }
        for prototype in prototypes {
            arguments.extend(["--define-system-task".to_owned(), (*prototype).to_owned()]);
        }
        arguments
    }
}

/// Builds one fixture: the optimizer mode and the companion's arguments. The
/// tests call `sim_cli` themselves, with the name of their fixture.
type Build = fn(bool, &[&str]) -> (sim_harness::TempDir, Output);

/// Build a fixture in both optimizer modes and hand each model to `check`.
fn each_model(companion: Companion, build: Build, check: impl Fn(&str, &Path, &Path)) {
    let archives = sim_harness::TempDir::new("fin002f").expect("temporary directory");
    let arguments = companion.arguments(&archives);
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    for optimized in [true, false] {
        let (directory, output) = build(optimized, &arguments);
        let name = format!("sim{}", std::env::consts::EXE_SUFFIX);
        let model: PathBuf = directory.path().join("build/sim/tb/build/bin").join(name);
        let label = format!("optimized={optimized}");
        assert!(
            model.is_file(),
            "{label}: {}",
            sim_cli::observed_output(&output)
        );
        check(&label, &model, directory.path());
    }
}

fn run(model: &Path, directory: &Path, arguments: &[&str]) -> Output {
    let mut command = Command::new(model);
    command.current_dir(directory).args(arguments);
    sim_harness::run_command(&mut command, RUN_TIMEOUT)
        .unwrap_or_else(|error| panic!("{}: {error}", model.display()))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
}

/// The ids of the warnings and errors on stderr; a line that is not a
/// catalogued message (a sanitizer report) fails the run.
fn message_ids(label: &str, output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(|line| {
            llg::core::report::parse(line)
                .unwrap_or_else(|| panic!("{label}: stderr line is not a message: {line}"))
        })
        .filter(|message| message.severity != llg::core::report::Severity::Info)
        .map(|message| message.id)
        .collect()
}

/// One run of a table: the plusargs after `###`, the standard output, and
/// after `===` the exit status and the ids of the warnings and errors.
struct Section<'a> {
    arguments: Vec<&'a str>,
    stdout: &'a str,
    status: i32,
    ids: Vec<&'a str>,
}

fn sections(table: &str) -> Vec<Section<'_>> {
    let mut rest = table;
    let mut sections = Vec::new();
    while let Some(section) = rest.strip_prefix("### ") {
        let (header, body) = section.split_once('\n').expect("a header line");
        let end = if body.starts_with("=== ") {
            0
        } else {
            body.find("\n=== ").expect("an end line") + 1
        };
        let (stdout, end) = body.split_at(end);
        let (end, next) = end.split_once('\n').expect("a whole end line");
        let mut end = end.split(' ').skip(1);
        sections.push(Section {
            // An argument may hold a blank and no ` +`.
            arguments: header
                .match_indices(" +")
                .map(|(at, _)| at + 1)
                .chain([header.len() + 1])
                .scan(0, |start, next| {
                    let argument = &header[*start..next - 1];
                    *start = next;
                    Some(argument)
                })
                .collect(),
            stdout,
            status: end
                .next()
                .and_then(|status| status.parse().ok())
                .expect("a status"),
            ids: end.collect(),
        });
        rest = next;
    }
    assert!(
        rest.is_empty() && !sections.is_empty(),
        "no section: {rest}"
    );
    sections
}

fn run_table(table: &str, companion: Companion, build: Build) {
    let table = table.replace("\r\n", "\n");
    let sections = sections(&table);
    each_model(companion, build, |label, model, directory| {
        for section in &sections {
            let label = format!("{label}, {:?}", section.arguments);
            let output = run(model, directory, &section.arguments);
            assert_eq!(
                stdout(&output),
                section.stdout,
                "{label}: {}",
                sim_cli::observed_output(&output)
            );
            assert_eq!(output.status.code(), Some(section.status), "{label}");
            assert_eq!(message_ids(&label, &output), section.ids, "{label}");
        }
    });
}

#[test]
fn cancellation_in_calls_and_hooks() {
    run_table(
        expected!("cancel_calls"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_calls", optimized, arguments, &[])
        },
    );
}

#[test]
fn cancellation_in_the_operands_of_every_statement() {
    run_table(
        expected!("cancel_expressions"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_expressions", optimized, arguments, &[])
        },
    );
}

#[test]
fn cancellation_in_a_final_procedure() {
    run_table(
        expected!("cancel_final"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_final", optimized, arguments, &[])
        },
    );
}

#[test]
fn cancellation_in_assertions_and_sequences() {
    run_table(
        expected!("cancel_sequences"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_sequences", optimized, arguments, &[])
        },
    );
}

#[test]
fn cancellation_in_the_later_regions() {
    run_table(
        expected!("cancel_regions"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_regions", optimized, arguments, &[])
        },
    );
}

#[test]
fn cancellation_in_dpi_imports_and_exports() {
    run_table(
        expected!("cancel_dpi"),
        Companion::Dpi("cancel_dpi"),
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_dpi", optimized, arguments, &[])
        },
    );
    run_table(
        expected!("cancel_dpi_calls"),
        Companion::Dpi("cancel_dpi_calls"),
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_dpi_calls", optimized, arguments, &[])
        },
    );
}

#[test]
fn cancellation_in_vpi_callbacks() {
    run_table(
        expected!("cancel_vpi_regions"),
        Companion::Vpi(
            "cancel_vpi",
            &["task $arm", "function int $fn(input int x)"],
        ),
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_vpi_regions", optimized, arguments, &[])
        },
    );
    run_table(
        expected!("cancel_vpi_statements"),
        Companion::Vpi("cancel_vpi", &["task $arm"]),
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "cancel_vpi_statements", optimized, arguments, &[])
        },
    );
}

#[test]
fn null_handles_in_every_operation() {
    run_table(
        expected!("null_handles"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "null_handles", optimized, arguments, &[])
        },
    );
}

#[test]
fn vpi_handles_of_objects_that_ended() {
    each_model(
        Companion::Vpi("vpi_stale", &["task $step(input string s)"]),
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "vpi_stale", optimized, arguments, &[])
        },
        |label, model, directory| {
            let output = run(model, directory, &[]);
            assert_eq!(
                stdout(&output),
                expected!("vpi_stale").replace("\r\n", "\n"),
                "{label}"
            );
            assert_eq!(output.status.code(), Some(0), "{label}");
            assert!(message_ids(label, &output).is_empty(), "{label}");
        },
    );
}

#[test]
fn vpi_routines_survive_every_handle() {
    each_model(
        Companion::Vpi(
            "vpi_handles",
            &[
                "task $step(input int n)",
                "function int $value(input int x)",
            ],
        ),
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "vpi_handles", optimized, arguments, &[])
        },
        |label, model, directory| {
            let seeds = fuzz::cases(HANDLE_SEEDS).map(|seed| format!("+seed={seed}"));
            for argument in seeds.chain(["+sweep".to_owned()]) {
                let label = format!("{label}, {argument}");
                let output = run(model, directory, &[&argument]);
                let text = stdout(&output);
                assert!(text.ends_with("ends\n"), "{label}: {text}");
                assert_eq!(text.contains("swept\n"), argument == "+sweep", "{label}");
                assert_eq!(output.status.code(), Some(0), "{label}");
                message_ids(&label, &output);
            }
        },
    );
}

#[test]
fn chandles_are_passed_on_unread() {
    let directory = sim_harness::TempDir::new("fin002f").expect("temporary directory");
    let arguments = Companion::Dpi("foreign_chandle").arguments(&directory);
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    sim_cli::run_case_backend_parity(
        SUITE,
        "foreign_chandle",
        expected!("foreign_chandle"),
        &arguments,
        &[],
    );
}

/// The accepted values of one line of `event_storm`: its words that are
/// numbers, each in its range, and what else relates them.
struct Accepted {
    label: &'static str,
    ranges: Vec<RangeInclusive<i64>>,
    relation: fn(&[i64], i64) -> bool,
}

/// The sets `readme.md` derives for `n` processes of every kind.
fn accepted_lines(n: i64) -> Vec<Accepted> {
    let line = |label, ranges: &[RangeInclusive<i64>], relation| Accepted {
        label,
        ranges: ranges.to_vec(),
        relation,
    };
    let any: fn(&[i64], i64) -> bool = |_, _| true;
    vec![
        line("race ran", &[0..=n - 1, n..=n], any),
        line(
            "nonblocking changes",
            &[0..=n - 1, 1000..=999 + n, 1..=n],
            |value, _| value[1] == 1000 + value[0],
        ),
        line("events", &[n..=n, 0..=n, n..=n, n..=n], any),
        line(
            "gates parity changes",
            &[0x7f..=0x7f, 1..=1, 1..=9],
            |value, _| value[2] % 2 == 1,
        ),
        line("zero sample", &[5 * n..=5 * n, n..=n, n..=2 * n], any),
        line(
            "mailbox got distinct left taken polled",
            &[n..=3 * n, n..=3 * n, 0..=2, 0..=n, 0..=3 * n],
            |value, n| {
                let [got, distinct, left, taken, polled] = *value else {
                    return false;
                };
                got == distinct
                    && got == taken + polled
                    && (left == 0 || taken == n)
                    && (got + left == 3 * n || left == 2)
            },
        ),
        line(
            "semaphore peak entered all back",
            &[1..=3, n..=n, 1..=1],
            any,
        ),
        line(
            "disable done finished killed",
            &[0..=n, 0..=n, 0..=n],
            |value, n| value[0] == value[1] && value[1] + value[2] == n,
        ),
        line("join any all", &[0..=n, n..=n], any),
        line("strobe", std::slice::from_ref(&(1000..=1000)), any),
    ]
}

/// A line's words and its numbers; the gates print theirs in hexadecimal.
fn words_and_numbers(line: &str) -> (String, Vec<i64>) {
    let mut words = Vec::new();
    let mut numbers = Vec::new();
    for word in line.split(' ') {
        let radix = if words == ["gates"] && numbers.is_empty() {
            16
        } else {
            10
        };
        match i64::from_str_radix(word, radix) {
            Ok(number) => numbers.push(number),
            Err(_) => words.push(word),
        }
    }
    (words.join(" "), numbers)
}

#[test]
fn simultaneous_events_end_in_an_accepted_state() {
    for n in [24, 200] {
        for optimized in [true, false] {
            let parameter = format!("N={n}");
            let (_directory, output) = sim_cli::run_case_keeping_dir(
                SUITE,
                "event_storm",
                optimized,
                &["-G", &parameter],
                &[],
            );
            let label = format!("event_storm, {parameter}, optimized={optimized}");
            let text = stdout(&output);
            assert_eq!(output.status.code(), Some(0), "{label}: {text}");
            let accepted = accepted_lines(n);
            assert_eq!(text.lines().count(), accepted.len(), "{label}: {text}");
            for (line, accepted) in text.lines().zip(accepted) {
                let (words, numbers) = words_and_numbers(line);
                assert_eq!(words, accepted.label, "{label}: {line}");
                assert!(
                    numbers.len() == accepted.ranges.len()
                        && accepted
                            .ranges
                            .iter()
                            .zip(&numbers)
                            .all(|(range, number)| range.contains(number))
                        && (accepted.relation)(&numbers, n),
                    "{label}: {line}"
                );
            }
        }
    }
}

#[test]
fn casts_of_values_that_do_not_fit() {
    run_table(
        expected!("wrong_cast"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "wrong_cast", optimized, arguments, &[])
        },
    );
}

#[test]
fn plusargs_that_do_not_fit_their_conversion() {
    run_table(
        expected!("wrong_plusargs"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "wrong_plusargs", optimized, arguments, &[])
        },
    );
}

#[test]
fn memory_images_that_do_not_fit() {
    run_table(
        expected!("wrong_readmem"),
        Companion::None,
        |optimized, arguments| {
            sim_cli::run_case_keeping_dir(SUITE, "wrong_readmem", optimized, arguments, &[])
        },
    );
}

#[test]
fn mailbox_messages_of_other_types() {
    sim_cli::run_case_backend_parity(SUITE, "wrong_mailbox", expected!("wrong_mailbox"), &[], &[]);
}

#[test]
fn vpi_value_formats_of_every_object() {
    let directory = sim_harness::TempDir::new("fin002f").expect("temporary directory");
    let arguments = Companion::Vpi("wrong_vpi_formats", &[]).arguments(&directory);
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    sim_cli::run_case_backend_parity(
        SUITE,
        "wrong_vpi_formats",
        expected!("wrong_vpi_formats"),
        &arguments,
        &[],
    );
}

/// The lines of `source` with a `// wrong: <id>` mark are exactly the lines
/// `llg` reported an error for, with that id.
fn check_marked_lines(source: &str, output: &Output) {
    let marked: BTreeSet<(usize, &str)> = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| Some((index + 1, line.split_once("// wrong: ")?.1.trim())))
        .collect();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let messages: Vec<_> = stderr
        .lines()
        .filter_map(llg::core::report::parse)
        .filter(|message| message.severity == llg::core::report::Severity::Error)
        .collect();
    let reported: BTreeSet<(usize, &str)> = messages
        .iter()
        .map(|message| {
            let location = message.location.as_deref().expect("a located error");
            let line = location
                .rsplit(':')
                .nth(1)
                .and_then(|line| line.parse().ok())
                .unwrap_or_else(|| panic!("no line in {location}"));
            (line, message.id.as_str())
        })
        .collect();
    assert_eq!(reported, marked, "{stderr}");
}

#[test]
fn dpi_signatures_of_wrong_types_are_refused() {
    let (_directory, output) =
        sim_cli::run_case_keeping_dir(SUITE, "reject_dpi_types", true, &[], &[]);
    check_marked_lines(
        include_str!("../fixtures/sim/feature_completion/fin_002f/reject_dpi_types.sv"),
        &output,
    );
}

#[test]
fn objects_of_wrong_types_are_refused() {
    let (_directory, output) =
        sim_cli::run_case_keeping_dir(SUITE, "reject_object_types", true, &[], &[]);
    check_marked_lines(
        include_str!("../fixtures/sim/feature_completion/fin_002f/reject_object_types.sv"),
        &output,
    );
}
