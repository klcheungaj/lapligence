//! Selections of records of integral members held as one signal per leaf
//! ([readme](../fixtures/sim/feature_completion/small_record_selects/readme.md)):
//! each constant or run-time selection reads and writes its own leaf.

use super::sim_cli;
use super::sim_harness;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/small_record_selects";

#[test]
fn constant_and_run_time_selections_of_leaf_records() {
    sim_cli::run_case(
        SUITE,
        "leaf_selects",
        include_str!("../fixtures/sim/feature_completion/small_record_selects/leaf_selects.out"),
        "",
        &[],
    );
}

#[test]
fn invalid_indices_into_leaf_record_members_read_defaults_and_drop_writes() {
    sim_cli::run_case_with_args(
        SUITE,
        "leaf_selects_invalid",
        include_str!(
            "../fixtures/sim/feature_completion/small_record_selects/leaf_selects_invalid.out"
        ),
        "",
        &[],
        &["--no-index-checks"],
    );
}

#[test]
fn forced_constant_element_of_a_leaf_record_member_array() {
    sim_cli::run_case(
        SUITE,
        "leaf_selects_force",
        include_str!(
            "../fixtures/sim/feature_completion/small_record_selects/leaf_selects_force.out"
        ),
        "",
        &[],
    );
}

#[test]
fn subroutine_arrays_named_like_record_members_are_their_own_storage() {
    for fixture in ["member_name_alias", "member_name_alias_packed"] {
        let expected = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/sim")
                .join(SUITE)
                .join(format!("{fixture}.out")),
        )
        .expect("expected output");
        sim_cli::run_case(SUITE, fixture, &expected, "", &[]);
    }
}

#[test]
fn leaf_record_selections_name_their_leaf_in_the_model() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/small_record_selects/leaf_selects_shape.sv");
    for optimized in [true, false] {
        let directory = sim_harness::TempDir::new("small-record-shape").expect("scratch");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .args(["--gen-only", "--top", "tb", "--out-dir"])
            .arg(directory.path());
        if !optimized {
            command.arg("--no-opt");
        }
        let output = sim_harness::run_command(command.arg(&source), Duration::from_secs(60))
            .expect("generate leaf record model");
        assert!(output.status.success(), "{output:?}");
        let model =
            std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model source");
        // Joining leaves into one value, or writing through a composite
        // reference with a run-time selection plan, costs a merge of every
        // leaf per access.
        for joined in [
            "LLG_REF_COMPOSITE",
            "llg_ref_write_masked",
            "sv4_concat",
            "sv4_select_plan",
        ] {
            assert!(
                !model.contains(joined),
                "optimized={optimized}: model joins leaves ({joined})"
            );
        }
    }
    sim_cli::run_case(
        SUITE,
        "leaf_selects_shape",
        include_str!(
            "../fixtures/sim/feature_completion/small_record_selects/leaf_selects_shape.out"
        ),
        "",
        &[],
    );
}
