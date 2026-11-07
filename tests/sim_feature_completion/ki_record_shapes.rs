//! Record storage forms agree: a record with a member array of 16 cells (one
//! column) and with 3 cells (one leaf per cell) prints the same for every
//! shape of record transfer in a covering subset of the sources and targets
//! (`gen_record_shapes.py`, [readme](../fixtures/sim/feature_completion/ki_record_shapes/readme.md)).
//! The full grid is the ignored `sim_record_shape_grid` test.
//!
//! The default tier runs the optimized mode only (user decision 2026-11-07,
//! an exception to the both-modes rule in `tests/AGENTS.md`): the oracle is
//! the other storage form within the same mode, not the unoptimized run.
//!
//! The oracle is the other form: the same checked-in file builds with `-D N=16`
//! and `-D N=3`, and the per-cell form is the long-validated one.
//! The runs pass `--no-index-checks`: a row such as `rf(q[0], q.pop_back())`
//! binds a `ref` to an element the pop removed, and the comparison is of the
//! text's result at both sizes, not of the default stop (S07-D15).

use super::sim_cli;
use std::process::{Command, Output};

const SUITE: &str = "feature_completion/ki_record_shapes";
const WIDE: &[&str] = &["--no-index-checks", "-D", "N=16"];
const NARROW: &[&str] = &["--no-index-checks", "-D", "N=3"];

fn assert_same_at_both_sizes(fixture: &str, wide: &Output, narrow: &Output) {
    for (cells, output) in [(16, wide), (3, narrow)] {
        assert_eq!(
            output.status.code(),
            Some(0),
            "{fixture} at {cells} cells: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let wide = String::from_utf8_lossy(&wide.stdout);
    let narrow = String::from_utf8_lossy(&narrow.stdout);
    assert!(
        wide.lines().any(|line| line.starts_with("row ")),
        "{fixture} printed no row"
    );
    assert_eq!(wide, narrow, "{fixture}: 16 and 3 cells print differently");
}

#[test]
fn string_member_records_print_the_same_at_16_and_3_cells() {
    {
        let optimized = true;
        let (_dir, wide) = sim_cli::run_case_keeping_dir(SUITE, "shapes_str", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_str", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_str", &wide, &narrow);
        let (_dir, wide) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_str_nba", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_str_nba", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_str_nba", &wide, &narrow);
    }
}

#[test]
fn packed_member_records_print_the_same_at_16_and_3_cells() {
    {
        let optimized = true;
        let (_dir, wide) = sim_cli::run_case_keeping_dir(SUITE, "shapes_pk", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_pk", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_pk", &wide, &narrow);
        let (_dir, wide) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_pk_nba", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_pk_nba", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_pk_nba", &wide, &narrow);
    }
}

#[test]
fn mixed_member_records_print_the_same_at_16_and_3_cells() {
    {
        let optimized = true;
        let (_dir, wide) = sim_cli::run_case_keeping_dir(SUITE, "shapes_mix", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_mix", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_mix", &wide, &narrow);
        let (_dir, wide) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_mix_nba", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_mix_nba", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_mix_nba", &wide, &narrow);
    }
}

#[test]
fn nested_records_print_the_same_at_16_and_3_cells() {
    {
        let optimized = true;
        let (_dir, wide) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_nest", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_nest", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_nest", &wide, &narrow);
        let (_dir, wide) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_nest_nba", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_nest_nba", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_nest_nba", &wide, &narrow);
    }
}

#[test]
fn real_member_records_print_the_same_at_16_and_3_cells() {
    {
        let optimized = true;
        let (_dir, wide) = sim_cli::run_case_keeping_dir(SUITE, "shapes_rl", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_rl", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_rl", &wide, &narrow);
        let (_dir, wide) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_rl_nba", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_rl_nba", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_rl_nba", &wide, &narrow);
    }
}

#[test]
fn string_array_records_print_the_same_at_16_and_3_cells() {
    {
        let optimized = true;
        let (_dir, wide) = sim_cli::run_case_keeping_dir(SUITE, "shapes_st", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_st", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_st", &wide, &narrow);
        let (_dir, wide) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_st_nba", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_st_nba", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_st_nba", &wide, &narrow);
    }
}

#[test]
fn handle_array_records_print_the_same_at_16_and_3_cells() {
    {
        let optimized = true;
        let (_dir, wide) = sim_cli::run_case_keeping_dir(SUITE, "shapes_obj", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_obj", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_obj", &wide, &narrow);
        let (_dir, wide) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_obj_nba", optimized, WIDE, &[]);
        let (_dir, narrow) =
            sim_cli::run_case_keeping_dir(SUITE, "shapes_obj_nba", optimized, NARROW, &[]);
        assert_same_at_both_sizes("shapes_obj_nba", &wide, &narrow);
    }
}

#[test]
fn component_record_shapes_are_generated() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = crate::sim_harness::run_command(
        Command::new("python3")
            .arg("-B")
            .arg(root.join(
                "tests/fixtures/sim/feature_completion/ki_record_shapes/gen_record_shapes.py",
            ))
            .arg("check"),
        std::time::Duration::from_secs(60),
    )
    .expect("record shape generator requires Python 3");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
