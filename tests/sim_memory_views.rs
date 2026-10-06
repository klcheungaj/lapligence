//! CLI acceptance tests for fixed multidimensional memory views.

use std::process::Command;
use std::time::Duration;

use crate::sim_cli;
use crate::sim_harness;

#[test]
fn multidimensional_read_walks_rows_and_honors_outer_address_jumps() {
    sim_cli::run_case_with_files(
        "memory_views",
        "multidim_read",
        "m0_2=12 m0_3=13 m1_2=22 m1_3=23\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "sv2009"],
        &[("multi.mem", "@1\n22\n23\n@0\n12\n13\n")],
    );
}

#[test]
fn selected_rows_and_slices_preserve_unselected_cells() {
    sim_cli::run_case_with_files(
        "memory_views",
        "selected_row",
        "r0=aa r1=bb r2=cc untouched=ee\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "sv2009"],
        &[("row.mem", "aa\nbb\ncc\n")],
    );
    sim_cli::run_case_with_files(
        "memory_views",
        "selected_slice",
        "s0=41 s1=42 other=ee\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "sv2009"],
        &[("slice.mem", "41\n42\n")],
    );
}

#[test]
fn partial_final_row_keeps_unread_subwords_unchanged() {
    sim_cli::run_case_with_files(
        "memory_views",
        "partial_row",
        "r0=11 r1=22 r2=ee untouched=ee\n",
        concat!(
            "llg: memory file `partial.mem`: memory file contains too few words for the selected range\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "sv2009"],
        &[("partial.mem", "11\n22\n")],
    );
}

#[test]
fn reversed_negative_ranges_and_writer_round_trip_keep_row_order() {
    sim_cli::run_case_with_files(
        "memory_views",
        "negative_ranges",
        "m-2-1=11 m-2-2=12 m-1-1=21 m-1-2=22\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "sv2009"],
        &[("negative.mem", "11\n12\n21\n22\n31\n32\n")],
    );
    sim_cli::run_case_with_files(
        "memory_views",
        "negative_outer_jump",
        "m-2-0=21 m-2-1=22 m-1-0=11 m-1-1=12\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "sv2009"],
        &[("negative_jump.mem", "@-1\n11\n12\n@-2\n21\n22\n")],
    );
    sim_cli::run_case_with_files(
        "memory_views",
        "writer_round_trip",
        "w0=11 w1=12 w2=21 w3=22\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "sv2009"],
        &[],
    );
    sim_cli::run_case_with_files(
        "memory_views",
        "packed_struct",
        "s0=12 s1=34\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "sv2009"],
        &[("struct.mem", "12\n34\n")],
    );
}

#[test]
fn unsupported_memory_view_shapes_fail_at_lowering() {
    sim_cli::reject_case_with_args(
        "memory_views",
        "reject_range",
        "unsupported executable node",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "memory_views",
        "reject_real",
        "invalid argument type",
        &["--edition", "sv2009"],
    );
}

#[test]
fn review_bundle_readmem_accepts_slices_and_runtime_rows() {
    sim_cli::run_case_with_files(
        "review_bundle",
        "r09_readmem_slice",
        "PASS r09_readmem_slice\n",
        "llg: $finish at time 0 at tb:11:5\n",
        &[],
        &["--edition", "sv2009"],
        &[(
            "words.hex",
            include_str!("fixtures/sim/review_bundle/words.hex"),
        )],
    );
    sim_cli::run_case_with_files(
        "review_bundle",
        "r09_readmem_runtime_row",
        "PASS r09_readmem_runtime_row\n",
        "llg: $finish at time 0 at tb:14:5\n",
        &[],
        &["--edition", "sv2009"],
        &[(
            "words.hex",
            include_str!("fixtures/sim/review_bundle/words.hex"),
        )],
    );
    sim_cli::run_case_with_files(
        "review_bundle",
        "r09_readmem_slice_address_bounds",
        "PASS r09_readmem_slice_address_bounds\n",
        concat!(
            "llg: memory file `words.hex`: selected range includes an address outside the destination memory\n",
            "llg: $finish at time 0 at tb:10:5\n",
        ),
        &[],
        &["--edition", "sv2009"],
        &[(
            "words.hex",
            include_str!("fixtures/sim/review_bundle/words.hex"),
        )],
    );
    sim_cli::run_case_with_files(
        "review_bundle",
        "r09_readmem_reversed_decl",
        "PASS r09_readmem_reversed_decl\n",
        "llg: $finish at time 0 at tb:10:5\n",
        &[],
        &["--edition", "sv2009"],
        &[(
            "words.hex",
            include_str!("fixtures/sim/review_bundle/words.hex"),
        )],
    );
}

#[test]
fn runtime_memory_view_selector_is_evaluated_once() {
    sim_cli::run_case_with_files(
        "review_bundle",
        "r09_readmem_runtime_selector_once",
        "PASS r09_readmem_runtime_selector_once calls=1\n",
        "llg: $finish at time 0 at tb:20:5\n",
        &[],
        &["--edition", "sv2009"],
        &[(
            "words.hex",
            include_str!("fixtures/sim/review_bundle/words.hex"),
        )],
    );
}

#[test]
fn native_memory_controls_preserve_edition_and_reversed_2d_order() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = sim_harness::TempDir::new("native-memory-controls")
        .expect("create native memory controls directory");
    let source = include_str!("fixtures/sim/review_bundle/native/native_controls.c");
    let executable =
        llg::sim::build::build_model_cmake(dir.path(), &[("native_controls.c", source)])
            .expect("native memory controls should compile");
    std::fs::write(
        dir.path().join("words.hex"),
        include_str!("fixtures/sim/review_bundle/words.hex"),
    )
    .expect("write native memory input");
    std::fs::write(
        dir.path().join("native_reversed_2d.hex"),
        include_str!("fixtures/sim/review_bundle/native_reversed_2d.hex"),
    )
    .expect("write native reversed 2D input");

    let mut command = Command::new(executable);
    command
        .current_dir(dir.path())
        .args(["words.hex", "native_reversed_2d.hex"]);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
        .expect("native memory controls should run");
    assert!(
        output.status.success(),
        "native memory controls failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        concat!(
            "memory_policy_0=PASS physical[11,22]\n",
            "memory_policy_1=PASS physical[22,11]\n",
            "memory_reversed_2d=PASS\n",
        ),
    );
    assert!(
        output.stderr.is_empty(),
        "native memory controls emitted stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
