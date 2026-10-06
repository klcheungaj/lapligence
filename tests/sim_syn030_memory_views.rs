//! SYN-030 public qualification of fixed multidimensional memory-file views.

use crate::sim_cli;

const DONE: &str = "llg: simulation ended without $finish (no processes remain) at time 0\n";

#[test]
fn signed_jumps_in_selected_multidimensional_view_preserve_other_cells() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "signed_selected",
        "selected=31,32,41,51 retained=ee,ee\n",
        DONE,
        &[],
        &["--edition", "sv2009"],
        &[(
            "selected.mem",
            "@-9 11 12 @-09 21 22 @-0009 31 32 @-8 41 42 @-7 51 52\n",
        )],
    );
    sim_cli::run_case_with_files(
        "syn030_memory_views", "signed_selected",
        "selected=aa,ee,ee,ee retained=ee,ee\n",
        concat!(
            "llg: memory file `selected.mem`: address jump is outside the destination memory or selected range; load terminated\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[], &["--edition", "sv2009"],
        &[("selected.mem", "@-9 aa @-b bb\n")],
    );
}

#[test]
fn mixed_direction_3d_addresses_walk_each_dimension_low_to_high() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "mixed_3d",
        "outer1=11,12,13,14 outer2=21,22,23,24\n",
        DONE,
        &[],
        &["--edition", "sv2009"],
        &[("mixed.mem", "@2\n21\n22\n23\n24\n@1\n11\n12\n13\n14\n")],
    );
}

#[test]
fn runtime_selected_slice_honors_descending_start_finish_and_retains_other_cells() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "selected_slice_bounds",
        "selected=21,22,11,12 retained=ee,ee\n",
        DONE,
        &[],
        &["--edition", "sv2009"],
        &[("slice.mem", "21\n22\n11\n12\n")],
    );
}

#[test]
fn invalid_selected_slice_range_leaves_all_cells_unchanged() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "selected_bad_bounds",
        "selected=ee,ee retained=ee\n",
        concat!(
            "llg: memory file `bounds.mem`: selected range includes an address outside the destination memory\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "sv2009"],
        &[("bounds.mem", "11\n22\n")],
    );
}

#[test]
fn invalid_jump_in_selected_row_retains_the_loaded_prefix() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "selected_bad_jump",
        "selected=ee,11,ee other=ee\n",
        concat!(
            "llg: memory file `jump.mem`: address jump is outside the destination memory or selected range; load terminated\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "sv2009"],
        &[("jump.mem", "@1\n11\n@3\n22\n")],
    );
}

#[test]
fn selected_load_notifies_changed_leaf_readers_after_settling() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "leaf_notification",
        "values=55,22,33 wakes=1,0,0\n",
        "llg: $finish at time 3000 at tb:19:5\n",
        &[],
        &["--edition", "sv2009"],
        &[("notify.mem", "55\n22\n")],
    );
}

#[test]
fn selected_enum_load_stops_before_non_fitting_word() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "enum_selected_stop",
        "selected=1,0 other=0\n",
        concat!(
            "llg: memory file `enum.mem`: numeric memory data does not fit the enum base type; load terminated\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "sv2009"],
        &[("enum.mem", "1\n4\n0\n")],
    );
}

#[test]
fn selected_packed_struct_and_two_state_binary_elements_convert_per_leaf() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "selected_element_types",
        "struct=12,34,ee bits=1000,1010,1111\n",
        concat!(
            "llg: memory file `bits.mem`: X/Z memory data converted to a two-state element\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "sv2009"],
        &[("struct.mem", "12\n34\n"), ("bits.mem", "1x0z\n1010\n")],
    );
}

#[test]
fn selected_wide_packed_words_keep_the_129th_bit() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "wide_selected",
        "PASS wide selected memory\n",
        DONE,
        &[],
        &["--edition", "sv2009"],
        &[(
            "wide.mem",
            "1_0000_0000_0000_0000_0000_0000_0000_0000\nff\n",
        )],
    );
}

#[test]
fn multidimensional_and_selected_slice_sources_reject_in_verilog_2001() {
    sim_cli::reject_case_with_args(
        "memory_editions",
        "multidim_2009",
        "multidimensional memory views require SystemVerilog-2009",
        &["--edition", "v2001"],
    );
    sim_cli::reject_case_with_args(
        "syn030_memory_views",
        "legacy_slice",
        "memory slices require SystemVerilog-2009",
        &["--edition", "v2001"],
    );
}
