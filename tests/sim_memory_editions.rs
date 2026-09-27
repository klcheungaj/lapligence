//! Native-runtime and public CLI coverage for memory-file tasks.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn signed_address_spellings_and_hex_boundaries_select_the_same_cells() {
    let words = "@-9 aa @-09 bb @-0009 cc @-7 17 @-8 18 @-f 1f @-80 80 @-81 81\n";
    let expected = "asc=17,18,cc,1f,80,81 desc=17,18,cc,1f,80,81\n";
    for edition in ["2001", "2009"] {
        let warning = "llg: memory file `signed.mem`: memory file contains too few words for the selected range\n";
        let stderr = if edition == "2001" {
            format!("{warning}{warning}llg: simulation ended without $finish (no processes remain) at time 0\n")
        } else {
            "llg: simulation ended without $finish (no processes remain) at time 0\n".to_owned()
        };
        sim_cli::run_case_with_files(
            "memory_editions",
            "signed_address",
            expected,
            &stderr,
            &[],
            &["--edition", edition],
            &[("signed.mem", words)],
        );
    }
}

#[test]
fn signed_address_overflow_and_out_of_range_stop_before_writing() {
    for edition in ["2001", "2009"] {
        for (word, diagnostic) in [
            ("@-8000000000000001", "address jump is not a known index"),
            ("@8000000000000000", "address jump is not a known index"),
            ("@-fffffffffffffffff", "address jump is not a known index"),
            (
                "@-100",
                "address jump is outside the destination memory or selected range; load terminated",
            ),
            (
                "@-8000000000000000",
                "address jump is outside the destination memory or selected range; load terminated",
            ),
            (
                "@7fffffffffffffff",
                "address jump is outside the destination memory or selected range; load terminated",
            ),
        ] {
            let contents = format!("@-9 aa {word} bb\n");
            let stderr = format!(
                "llg: memory file `signed.mem`: {diagnostic}\nllg: memory file `signed.mem`: {diagnostic}\nllg: simulation ended without $finish (no processes remain) at time 0\n"
            );
            sim_cli::run_case_with_files(
                "memory_editions",
                "signed_address",
                "asc=00,00,aa,00,00,00 desc=00,00,aa,00,00,00\n",
                &stderr,
                &[],
                &["--edition", edition],
                &[("signed.mem", &contents)],
            );
        }
    }
}

#[test]
fn native_enum_memory_overflow_is_rejected_before_cast() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = sim_harness::TempDir::new("native-enum-memory-overflow")
        .expect("create native probe directory");
    let probe = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/review_bundle/native/native_probe.c"),
    )
    .expect("read native enum probe");
    let executable =
        llg::sim::build::build_model_cmake(dir.path(), &[("native_probe.c", probe.as_str())])
            .expect("native runtime probe should compile");
    std::fs::write(
        dir.path().join("enum_overflow.hex"),
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/sim/review_bundle/enum_overflow.hex"),
        )
        .expect("read enum memory input"),
    )
    .expect("write enum memory input");
    let mut command = Command::new(executable);
    command.current_dir(dir.path()).arg("enum_overflow.hex");
    let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
        .expect("native enum runtime probe should run");
    assert!(
        output.status.success(),
        "native probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "enum_after_read: 1 0"),
        "invalid and following enum words must leave memory unchanged: {stdout:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr,
        "llg: memory file `enum_overflow.hex`: numeric memory data does not fit the enum base type; load terminated\n"
    );
}

#[test]
fn omitted_memory_range_keeps_the_selected_edition_order() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "default_order",
        "m1=11 m0=22\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2001"],
        &[("words.mem", "11\n22\n")],
    );
    sim_cli::run_case_with_files(
        "memory_editions",
        "default_order",
        "m1=22 m0=11\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("words.mem", "11\n22\n")],
    );
}

#[test]
fn start_only_memory_range_uses_the_edition_default_finish() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "start_only",
        "m3=00 m2=aa m1=bb m0=00\n",
        concat!(
            "llg: memory file `words.mem`: memory file contains too few words for the selected range\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2001"],
        &[("words.mem", "aa\nbb\n")],
    );
    sim_cli::run_case_with_files(
        "memory_editions",
        "start_only",
        "m3=bb m2=aa m1=00 m0=00\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("words.mem", "aa\nbb\n")],
    );
}

#[test]
fn explicit_descending_range_and_address_jumps_preserve_source_direction() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "explicit_descending",
        "m0=00 m1=33 m2=22 m3=11\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("words.mem", "11\n22\n33\n")],
    );
    sim_cli::run_case_with_files(
        "memory_editions",
        "address_jump",
        "m0=c3 m1=b2 m2=d4 m3=a1\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("jumps.mem", "@3\na1\n@1\nb2\nc3\n@2\nd4\n")],
    );
}

#[test]
fn truncated_data_and_two_state_conversion_keep_prior_values_and_diagnose() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "truncated",
        "m0=7f m1=b1\n",
        concat!(
            "llg: memory file `short.mem`: memory file contains too few words for the selected range\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2009"],
        &[("short.mem", "7f\n")],
    );
    sim_cli::run_case_with_files(
        "memory_editions",
        "two_state",
        "m0=1000 m1=1010\n",
        concat!(
            "llg: memory file `unknown.mem`: X/Z memory data converted to a two-state element\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2009"],
        &[("unknown.mem", "1x0z\n1010\n")],
    );
}

#[test]
fn invalid_address_terminates_after_already_loaded_values() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "invalid_address",
        "m0=00 m1=aa m2=02 m3=03\n",
        concat!(
            "llg: memory file `bad.mem`: address jump is outside the destination memory or selected range; load terminated\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2009"],
        &[("bad.mem", "@1\naa\n@3\nbb\n")],
    );
}

#[test]
fn enum_memory_data_stops_at_the_first_non_member() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "enum_invalid",
        "m0=00 m1=01\n",
        concat!(
            "llg: memory file `enum.mem`: memory data value is not a member of the enum; load terminated\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2009"],
        &[("enum.mem", "00\n02\n")],
    );
}

#[test]
fn enum_memory_overflow_is_rejected_before_truncation() {
    let cases = [
        (
            std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/sim/review_bundle/enum_overflow.hex"),
            )
            .expect("read original enum overflow input"),
            "AFTER_ENUM_LOAD 1 0\n",
        ),
        (
            std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/sim/review_bundle/enum_overflow_after_valid.hex"),
            )
            .expect("read enum overflow input after a valid word"),
            "AFTER_ENUM_LOAD 0 0\n",
        ),
    ];
    for (words, expected) in cases {
        sim_cli::run_case_with_files(
            "review_bundle",
            "r08_enum_readmem_overflow",
            expected,
            concat!(
                "llg: memory file `enum_overflow.hex`: numeric memory data does not fit the enum base type; load terminated\n",
                "llg: $finish at time 0 at tb:10:5\n",
            ),
            &[],
            &["--edition", "2009"],
            &[("enum_overflow.hex", words.as_str())],
        );
    }
}

#[test]
fn enum_memory_range_check_preserves_packed_truncation_and_signed_values() {
    let overflow = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/review_bundle/enum_overflow.hex"),
    )
    .expect("read enum overflow input");
    let signed = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/review_bundle/signed_enum.hex"),
    )
    .expect("read signed enum input");
    let signed_nonextension = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/review_bundle/signed_enum_nonextension.hex"),
    )
    .expect("read signed enum non-extension input");
    let unknown = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/review_bundle/enum_unknown.hex"),
    )
    .expect("read enum X/Z input");
    sim_cli::run_case_with_files(
        "review_bundle",
        "r08_memory_readmem_controls",
        concat!(
            "PACKED_CONTROL 0 1\n",
            "SIGNED_ENUM_CONTROL -1\n",
            "SIGNED_ENUM_AFTER_NONEXTENSION -1 0\n",
            "TWO_STATE_ENUM_CONTROL 0 0\n",
        ),
        concat!(
            "llg: memory file `signed_enum_nonextension.hex`: numeric memory data does not fit the enum base type; load terminated\n",
            // SV 21.4.2: `1x` becomes 0x10 before the enum range check, so its
            // known high digit overflows and the load stops with the memory unchanged.
            "llg: memory file `enum_unknown.hex`: numeric memory data does not fit the enum base type; load terminated\n",
            "llg: $finish at time 0 at tb:17:5\n",
        ),
        &[],
        &["--edition", "2009"],
        &[
            ("enum_overflow.hex", overflow.as_str()),
            ("signed_enum.hex", signed.as_str()),
            (
                "signed_enum_nonextension.hex",
                signed_nonextension.as_str(),
            ),
            ("enum_unknown.hex", unknown.as_str()),
        ],
    );
}

#[test]
fn wide_enum_memory_uses_entry_count_for_emitted_table() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "enum_wide",
        "PASS wide enum memory\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &[],
        &[("enum_wide.mem", "1\n2\n3\n4\n")],
    );

    let run = sim_harness::run_generated_sim_with_files(
        include_str!("fixtures/sim/memory_editions/enum_wide.sv"),
        "tb",
        "enum_wide_model_source",
        &[("enum_wide.mem", "1\n2\n3\n4\n")],
    )
    .expect("wide enum memory simulation");
    assert_eq!(run.stdout, "PASS wide enum memory\n");
    let declaration = run
        .model_c
        .lines()
        .find(|line| line.contains("sv4_t _llg_memory_enum_values"))
        .expect("generated enum table declaration");
    assert!(declaration.contains("[4]"), "{declaration}");
}

#[test]
fn memory_file_tasks_accept_multidimensional_arrays() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "multidim_2009",
        "",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("bad.mem", "11\n22\n33\n44\n")],
    );
}

#[test]
fn multidimensional_memory_views_remain_a_systemverilog_feature() {
    sim_cli::reject_case_with_args(
        "memory_editions",
        "multidim_2009",
        "multidimensional memory views require SystemVerilog-2009",
        &["--edition", "2001"],
    );
}
