//! CLI acceptance tests for fixed multidimensional memory views.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn multidimensional_read_walks_rows_and_honors_outer_address_jumps() {
    sim_cli::run_case_with_files(
        "memory_views",
        "multidim_read",
        "m0_2=12 m0_3=13 m1_2=22 m1_3=23\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
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
        &["--edition", "2009"],
        &[("row.mem", "aa\nbb\ncc\n")],
    );
    sim_cli::run_case_with_files(
        "memory_views",
        "selected_slice",
        "s0=41 s1=42 other=ee\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
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
        &["--edition", "2009"],
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
        &["--edition", "2009"],
        &[("negative.mem", "11\n12\n21\n22\n31\n32\n")],
    );
    sim_cli::run_case_with_files(
        "memory_views",
        "negative_outer_jump",
        "m-2-0=21 m-2-1=22 m-1-0=11 m-1-1=12\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("negative_jump.mem", "@-1\n11\n12\n@-2\n21\n22\n")],
    );
    sim_cli::run_case_with_files(
        "memory_views",
        "writer_round_trip",
        "w0=11 w1=12 w2=21 w3=22\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[],
    );
    sim_cli::run_case_with_files(
        "memory_views",
        "packed_struct",
        "s0=12 s1=34\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("struct.mem", "12\n34\n")],
    );
}

#[test]
fn unsupported_memory_view_shapes_fail_at_lowering() {
    sim_cli::reject_case_with_args(
        "memory_views",
        "reject_range",
        "unsupported executable node",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "memory_views",
        "reject_real",
        "invalid argument type",
        &["--edition", "2009"],
    );
}
