//! SIM-046: coverage queries, options, the coverage database and the
//! coverage control API (IEEE 1800-2009 §§19.7-19.11, 20.14, 40.3). Every
//! expected count and percentage is derived by hand in the fixture readme;
//! positive cases run in both optimizer modes on the legacy,
//! compact/portable and compact/GMP value backends.
use super::sim_cli;
use std::path::Path;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_046";

fn assert_stdout(label: &str, output: &Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
}

/// The `llg:` lines of a run's stderr other than the `$finish` report.
fn llg_messages(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| line.starts_with("llg: ") && !line.starts_with("llg: $finish"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn options_names_and_two_argument_queries_follow_19_7_and_19_8() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_046/options.out");
    sim_cli::run_case_checked_matrix(SUITE, "options", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert!(llg_messages(output).is_empty(), "{label}");
    });
}

/// The coverage databases left in a run directory, sorted.
fn database_files(directory: &Path) -> Vec<String> {
    let mut files = std::fs::read_dir(directory)
        .expect("run directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".db"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

/// Runs of one model in one directory, in both optimizer modes on every
/// value backend: `run(optimized, envs)` returns the databases left and the
/// outputs. Every run must succeed silently; the concatenated standard
/// output must equal `expected` and the directory must hold exactly `files`.
type SavedRuns<'a> = &'a dyn Fn(bool, &[(&str, &str)]) -> (Vec<String>, Vec<Output>);

fn check_saved_runs(case: &str, expected: &str, files: &[&str], run: SavedRuns) {
    for optimized in [false, true] {
        for (backend, controls) in sim_cli::backend_matrix() {
            let envs = controls
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect::<Vec<_>>();
            let (left, outputs) = run(optimized, &envs);
            let label = format!("{SUITE}/{case}, {backend}, optimized={optimized}");
            let mut stdout = String::new();
            for output in &outputs {
                assert!(
                    output.status.success(),
                    "{label}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(llg_messages(output).is_empty(), "{label}");
                stdout.push_str(&String::from_utf8_lossy(&output.stdout));
            }
            assert_eq!(stdout, expected, "{label}");
            assert_eq!(left, files, "{label}");
        }
    }
}

#[test]
fn saved_runs_load_and_merge_into_type_coverage() {
    // The third run sets no database name, so it saves nothing; the first
    // two leave exactly their files.
    check_saved_runs(
        "database",
        include_str!("../fixtures/sim/feature_completion/sim_046/database.out"),
        &["cov.db", "cov2.db"],
        &|optimized, envs| {
            let (directory, outputs) = sim_cli::run_case_sequence(
                SUITE,
                "database",
                optimized,
                envs,
                &[],
                &[(&[], &["+first"]), (&[], &["+second"]), (&[], &["+third"])],
            );
            (database_files(directory.path()), outputs)
        },
    );
}

#[test]
fn saved_crosses_and_instances_merge_across_runs() {
    check_saved_runs(
        "db_cross",
        include_str!("../fixtures/sim/feature_completion/sim_046/db_cross.out"),
        &["x.db", "y.db"],
        &|optimized, envs| {
            let (directory, outputs) = sim_cli::run_case_sequence(
                SUITE,
                "db_cross",
                optimized,
                envs,
                &[],
                &[(&[], &["+first"]), (&[], &["+second"]), (&[], &["+third"])],
            );
            (database_files(directory.path()), outputs)
        },
    );
}

/// FNV-1a 64, the checksum of the database format (readme.md).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

/// A database with `payload` (its sections, each framed by a u64 byte
/// length) and the header of `template` (its version, sections and schema
/// hashes), framed and checksummed as the format says.
fn database_with_payload(template: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut bytes = template[..32].to_vec();
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(payload);
    let checksum = fnv1a(&bytes);
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes
}

/// The single record of the `db_errors` database `good` (one type `g`, one
/// record of one automatic coverpoint without transitions): its bytes and
/// the offset of its first counter entry's bin index within them.
fn single_record(good: &[u8]) -> (Vec<u8>, usize) {
    assert_eq!(&good[..8], b"LLGCOVDB");
    assert_eq!(u32_at(good, 12), 1, "only the functional section");
    assert_eq!(u64_at(good, 32) as usize, good.len() - 48);
    let payload = &good[48..good.len() - 8];
    assert_eq!(u64_at(good, 40) as usize, payload.len());
    assert_eq!(u32_at(payload, 0), 1, "one type");
    let name = u32_at(payload, 4);
    assert_eq!(u32_at(payload, 8 + name), 1, "one record");
    let record = payload[12 + name..].to_vec();
    // hash, name, comment, five options, two flags, slots, items
    let mut at = 8;
    at += 4 + u32_at(&record, at);
    at += 4 + u32_at(&record, at);
    at += 40 + 2;
    assert_eq!(u32_at(&record, at), 0, "no argument slots");
    at += 4;
    assert_eq!(u32_at(&record, at), 1, "one coverpoint");
    at += 4;
    // weight, goal, at_least, has_at_least, auto_bin_max, comment, entries
    at += 24 + 1 + 8;
    at += 4 + u32_at(&record, at);
    assert_eq!(u64_at(&record, at), 1, "one counter entry");
    (record, at + 8)
}

/// A framed functional section of type `g` holding `records`.
fn functional_payload(records: &[&[u8]]) -> Vec<u8> {
    let mut body = 1u32.to_le_bytes().to_vec();
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(b'g');
    body.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for record in records {
        body.extend_from_slice(record);
    }
    let mut payload = (body.len() as u64).to_le_bytes().to_vec();
    payload.extend_from_slice(&body);
    payload
}

#[test]
fn corrupt_and_foreign_databases_change_nothing() {
    let error = |reason: &str| {
        format!(
            "llg: runtime error: $load_coverage_db at tb:21:40: coverage database `bad.db` was not loaded: {reason}; no coverage changed"
        )
    };
    for optimized in [false, true] {
        let label = format!("{SUITE}/db_errors, optimized={optimized}");
        let (directory, outputs) = sim_cli::run_case_sequence(
            SUITE,
            "db_errors",
            optimized,
            &[],
            &[],
            &[(&[], &["+save=good.db"])],
        );
        assert_stdout(&label, &outputs[0], "type 1 16 6.250000\n");
        let good = std::fs::read(directory.path().join("good.db")).expect("saved database");
        let (record, bin_at) = single_record(&good);
        // A valid second record under another name, then one whose counter
        // lies outside its 16 bins.
        let mut other = record.clone();
        let name_at = 12;
        assert_eq!(&other[name_at..name_at + 6], b"tb.g#1");
        other[name_at..name_at + 6].copy_from_slice(b"other1");
        other[bin_at..bin_at + 8].copy_from_slice(&5u64.to_le_bytes());
        let mut outside = record.clone();
        outside[bin_at..bin_at + 8].copy_from_slice(&16u64.to_le_bytes());
        let valid = database_with_payload(&good, &functional_payload(&[&record, &other]));
        let mut flipped = good.clone();
        flipped[58] ^= 1;
        let mut future = good.clone();
        future[8..12].copy_from_slice(&2u32.to_le_bytes());
        let mut unknown_section = good.clone();
        unknown_section[12] |= 4;
        let mut overlong = functional_payload(&[&record]);
        let frame = u64_at(&overlong, 0) + 1;
        overlong[..8].copy_from_slice(&frame.to_le_bytes());
        let mut trailing = functional_payload(&[&record]);
        trailing.push(0);
        let frame = u64_at(&trailing, 0) + 1;
        trailing[..8].copy_from_slice(&frame.to_le_bytes());
        let cases: Vec<(&str, Vec<u8>, String)> = vec![
            ("two valid records", valid, String::new()),
            (
                "truncated",
                good[..30].to_vec(),
                error("the file is not an llg coverage database"),
            ),
            (
                "flipped bit",
                flipped,
                error("the checksum does not match (the file is corrupt)"),
            ),
            (
                "future version",
                future,
                error("the database version is not supported"),
            ),
            (
                "unknown section",
                unknown_section,
                error("the database has unknown sections"),
            ),
            (
                "missing checksum byte",
                good[..good.len() - 1].to_vec(),
                error("the file length does not match its header"),
            ),
            (
                "section longer than the payload",
                database_with_payload(&good, &overlong),
                error("the database sections are malformed"),
            ),
            (
                "trailing section data",
                database_with_payload(&good, &trailing),
                error("the functional section has trailing data"),
            ),
            (
                "foreign magic",
                [b"NOTCOVDB".as_slice(), &good[8..]].concat(),
                error("the file is not an llg coverage database"),
            ),
            (
                "counter outside its bins",
                database_with_payload(&good, &functional_payload(&[&other, &outside])),
                error("a counter is outside its bins"),
            ),
        ];
        for (case, bytes, message) in cases {
            let (_rerun, outputs) = sim_cli::run_case_sequence(
                SUITE,
                "db_errors",
                optimized,
                &[],
                &[("bad.db", &bytes)],
                &[(&[], &["+load=bad.db"])],
            );
            let output = &outputs[0];
            let label = format!("{label}, {case}");
            if message.is_empty() {
                // Both records published: the second one is its own member.
                assert_stdout(&label, output, "type 2 32 6.250000\n");
                continue;
            }
            assert_eq!(output.status.code(), Some(1), "{label}");
            // Nothing was published: the earlier valid record is not counted.
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                "type 1 16 6.250000\n",
                "{label}"
            );
            assert_eq!(llg_messages(output), [message], "{label}");
        }
        // The same database read by a model whose coverpoint is narrower.
        let (_rerun, outputs) = sim_cli::run_case_sequence(
            SUITE,
            "db_errors",
            optimized,
            &[],
            &[("bad.db", &good)],
            &[(&["-G", "W=3"], &["+load=bad.db"])],
        );
        assert_eq!(outputs[0].status.code(), Some(1), "{label}");
        assert_eq!(
            String::from_utf8_lossy(&outputs[0].stdout),
            "type 1 8 12.500000\n",
            "{label}"
        );
        assert_eq!(
            llg_messages(&outputs[0]),
            [error("it was saved by a model with different covergroups")],
            "{label}"
        );
        // A missing database, and one in a directory that does not exist.
        let (rerun, outputs) = sim_cli::run_case_sequence(
            SUITE,
            "db_errors",
            optimized,
            &[],
            &[],
            &[(&[], &["+load=missing.db", "+save=nodir/x.db"])],
        );
        assert_eq!(outputs[0].status.code(), Some(1), "{label}");
        let messages = llg_messages(&outputs[0]);
        assert_eq!(messages.len(), 2, "{label}: {messages:?}");
        assert!(
            messages[0].starts_with(
                "llg: runtime error: $load_coverage_db at tb:21:40: coverage database `missing.db` was not loaded: "
            ),
            "{label}: {messages:?}"
        );
        assert!(
            messages[1].starts_with(
                "llg: runtime error: end of simulation: coverage database `nodir/x.db` was not saved: "
            ),
            "{label}: {messages:?}"
        );
        // Neither the database nor its temporary file was left behind.
        let left = std::fs::read_dir(rerun.path())
            .expect("run directory")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".db") || name.starts_with("nodir"))
            .collect::<Vec<_>>();
        assert!(left.is_empty(), "{label}: {left:?}");
    }
}

#[test]
fn definition_only_options_and_partial_counts_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_option_definition",
        "neg_option_definition.sv:10:27 the 'per_instance' option cannot be modified outside of the covergroup definition",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_auto_bin_max",
        "neg_auto_bin_max.sv:10:30 the 'auto_bin_max' option cannot be modified outside of the covergroup definition",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_type_strobe",
        "neg_type_strobe.sv:10:27 the 'strobe' option cannot be modified outside of the covergroup definition",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_option_compound",
        "neg_option_compound.sv:11:5: option.weight can only be written by a plain blocking assignment (llg limitation)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_coverage_scope",
        "neg_coverage_scope.sv:10:68 expected module instance",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_count_args",
        "neg_count_args.sv:12:9: `get_inst_coverage` takes either no arguments or both covered and total bin arguments (IEEE 1800-2009 19.8)",
    );
}

/// The assertion section of a database: per item, in table order,
/// (attempts, successes, vacuous, failures, disabled, killed).
fn assertion_counters(bytes: &[u8]) -> Vec<[u64; 6]> {
    assert_eq!(&bytes[..8], b"LLGCOVDB");
    let sections = u32_at(bytes, 12);
    assert_ne!(sections & 2, 0, "an assertion section");
    let mut at = 40;
    if sections & 1 != 0 {
        at += 8 + u64_at(bytes, at) as usize;
    }
    let length = u64_at(bytes, at) as usize;
    let items = u32_at(bytes, at + 8);
    assert_eq!(length, 4 + items * 48, "section length");
    let first = at + 12;
    (0..items)
        .map(|item| std::array::from_fn(|counter| u64_at(bytes, first + item * 48 + counter * 8)))
        .collect()
}

/// The `assertions` run's counters, in elaboration order (readme.md).
const ASSERTION_COUNTERS: [[u64; 6]; 12] = [
    [4, 3, 0, 1, 0, 0], // checks::check.pk
    [5, 2, 2, 1, 0, 0], // tb.u1.p
    [5, 2, 0, 0, 0, 0], // tb.u1.c
    [5, 1, 4, 0, 0, 0], // tb.u1.s
    [7, 2, 2, 0, 2, 1], // tb.u2.p
    [7, 2, 0, 0, 2, 1], // tb.u2.c
    [7, 0, 3, 0, 2, 2], // tb.u2.s
    [8, 3, 0, 0, 0, 0], // tb.cs
    [4, 3, 0, 1, 0, 0], // tb.im
    [4, 1, 0, 0, 0, 0], // tb.ic
    [4, 1, 0, 0, 0, 0], // tb.dc
    [1, 0, 0, 1, 0, 0], // tb.im2
];

#[test]
fn assertion_coverage_counts_selects_and_merges_across_runs() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_046/assertions.out");
    let doubled = ASSERTION_COUNTERS.map(|row| row.map(|count| 2 * count));
    for optimized in [false, true] {
        for (backend, controls) in sim_cli::backend_matrix() {
            let envs = controls
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect::<Vec<_>>();
            let (directory, outputs) = sim_cli::run_case_sequence(
                SUITE,
                "assertions",
                optimized,
                &envs,
                &[],
                &[(&[], &[]), (&[], &["+merge"])],
            );
            let label = format!("{SUITE}/assertions, {backend}, optimized={optimized}");
            let mut stdout = String::new();
            for output in &outputs {
                assert!(
                    output.status.success(),
                    "{label}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(llg_messages(output).is_empty(), "{label}");
                stdout.push_str(&String::from_utf8_lossy(&output.stdout));
            }
            assert_eq!(stdout, expected, "{label}");
            let first = std::fs::read(directory.path().join("acov.db")).expect("acov.db");
            assert_eq!(assertion_counters(&first), ASSERTION_COUNTERS, "{label}");
            // The second run merged the first run's counters, then counted
            // the same run again.
            let second = std::fs::read(directory.path().join("acov2.db")).expect("acov2.db");
            assert_eq!(assertion_counters(&second), doubled, "{label}");
        }
    }
}

#[test]
fn coverage_control_status_values_follow_table_40_1() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_046/control.out");
    sim_cli::run_case_checked_matrix(SUITE, "control", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert!(llg_messages(output).is_empty(), "{label}");
    });
    // Unavailable kinds and a bad type save nothing.
    let (directory, outputs) =
        sim_cli::run_case_sequence(SUITE, "control", false, &[], &[], &[(&[], &[])]);
    assert_stdout("control files", &outputs[0], expected);
    assert!(directory.path().join("c.db").exists());
    assert!(!directory.path().join("never.db").exists());
}

#[test]
fn failed_assertion_merges_change_no_counter() {
    let error = |name: &str, reason: &str| {
        format!(
            "llg: runtime error: $coverage_merge at tb:15:29: coverage database `{name}` was not merged: {reason}; no coverage changed"
        )
    };
    let one = vec![[1, 1, 0, 0, 0, 0]; 2];
    for optimized in [false, true] {
        let label = format!("{SUITE}/merge_errors, optimized={optimized}");
        let (directory, outputs) = sim_cli::run_case_sequence(
            SUITE,
            "merge_errors",
            optimized,
            &[],
            &[],
            &[
                (&[], &["+save=good.db"]),
                (&[], &["+merge=good.db", "+save=two.db"]),
            ],
        );
        assert_stdout(&label, &outputs[0], "merged 0/2\nsave 1\n");
        assert_stdout(&label, &outputs[1], "merge 1\nmerged 2/2\nsave 1\n");
        let good = std::fs::read(directory.path().join("good.db")).expect("good.db");
        assert_eq!(assertion_counters(&good), one, "{label}");
        let two = std::fs::read(directory.path().join("two.db")).expect("two.db");
        assert_eq!(
            assertion_counters(&two),
            vec![[2, 2, 0, 0, 0, 0]; 2],
            "{label}"
        );
        // A functional-only database, from a model without assertion items.
        let (functional_run, _) = sim_cli::run_case_sequence(
            SUITE,
            "db_errors",
            optimized,
            &[],
            &[],
            &[(&[], &["+save=functional.db"])],
        );
        let functional =
            std::fs::read(functional_run.path().join("functional.db")).expect("functional.db");
        let mut flipped = good.clone();
        flipped[60] ^= 1;
        // The right hash and checksum, but an item table of one item.
        let mut short = 52u64.to_le_bytes().to_vec();
        short.extend_from_slice(&1u32.to_le_bytes());
        for count in [1u64, 1, 0, 0, 0, 0] {
            short.extend_from_slice(&count.to_le_bytes());
        }
        let short = database_with_payload(&good, &short);
        // (case, compile options, database, stdout, run-time error)
        type MergeCase<'a> = (&'a str, &'a [&'a str], Vec<u8>, &'a str, Option<String>);
        let cases: Vec<MergeCase> = vec![
            (
                "another design",
                &["-G", "N=3"],
                good.clone(),
                "merge -1\nmerged 0/3\nsave 1\n",
                Some(error(
                    "bad.db",
                    "it was saved by a model with different assertions",
                )),
            ),
            (
                "flipped bit",
                &[],
                flipped,
                "merge -1\nmerged 0/2\nsave 1\n",
                Some(error(
                    "bad.db",
                    "the checksum does not match (the file is corrupt)",
                )),
            ),
            (
                "short item table",
                &[],
                short,
                "merge -1\nmerged 0/2\nsave 1\n",
                Some(error(
                    "bad.db",
                    "the assertion section does not match its item table",
                )),
            ),
            (
                "no assertion section",
                &[],
                functional,
                "merge 0\nmerged 0/2\nsave 1\n",
                None,
            ),
        ];
        for (case, options, bytes, stdout, message) in cases {
            let label = format!("{label}, {case}");
            let (rerun, outputs) = sim_cli::run_case_sequence(
                SUITE,
                "merge_errors",
                optimized,
                &[],
                &[("bad.db", &bytes)],
                &[(options, &["+merge=bad.db", "+save=after.db"])],
            );
            let output = &outputs[0];
            assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
            match message {
                Some(message) => {
                    assert_eq!(output.status.code(), Some(1), "{label}");
                    assert_eq!(llg_messages(output), [message], "{label}");
                }
                None => {
                    assert!(output.status.success(), "{label}");
                    assert!(llg_messages(output).is_empty(), "{label}");
                }
            }
            // The run's own counters only: nothing was merged.
            if options.is_empty() {
                let after = std::fs::read(rerun.path().join("after.db")).expect("after.db");
                assert_eq!(assertion_counters(&after), one, "{label}");
            }
        }
        // A database of another kind merges nothing (40.3.2.4 NOCOV); a
        // missing one is an error for every kind.
        let (_rerun, outputs) = sim_cli::run_case_sequence(
            SUITE,
            "merge_errors",
            optimized,
            &[],
            &[("good.db", &good)],
            &[
                (&[], &["+toggle=good.db"]),
                (&[], &["+merge=missing.db", "+toggle=missing.db"]),
            ],
        );
        assert_stdout(&label, &outputs[0], "toggle 0\nmerged 0/2\n");
        assert_eq!(outputs[1].status.code(), Some(1), "{label}");
        assert_eq!(
            String::from_utf8_lossy(&outputs[1].stdout),
            "merge -1\ntoggle -1\nmerged 0/2\n",
            "{label}"
        );
        let messages = llg_messages(&outputs[1]);
        assert_eq!(messages.len(), 2, "{label}: {messages:?}");
        for (message, site) in messages.iter().zip(["tb:15:29", "tb:17:30"]) {
            assert!(
                message.starts_with(&format!(
                    "llg: runtime error: $coverage_merge at {site}: coverage database `missing.db` was not merged: "
                )),
                "{label}: {message}"
            );
        }
    }
}

#[test]
fn final_procedures_query_and_save_live_coverage() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_046/final_query.out");
    sim_cli::run_case_checked_matrix(SUITE, "final_query", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert!(llg_messages(output).is_empty(), "{label}");
    });
}

/// Step 3: assertion handles (39.3), their coverage properties and coverage
/// control through VPI (40.5) for an application loaded at run time.
#[test]
fn vpi_reads_and_controls_assertion_coverage() {
    let directory = super::sim_harness::TempDir::new("sim046-vpi").expect("temporary directory");
    let Some(library) = super::sim_040::plugin_library(SUITE, &directory, "vpi_coverage", &[])
    else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "vpi_coverage",
        include_str!("../fixtures/sim/feature_completion/sim_046/vpi_coverage.out"),
        &["--define-system-task", "task $vpi_cov(input int phase)"],
        &[("LLG_VPI_PLUGIN", &library)],
    );
}
