//! SYN-030 public qualification of fixed multidimensional memory-file views.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const DONE: &str = "llg: simulation ended without $finish (no processes remain) at time 0\n";

#[test]
fn mixed_direction_3d_addresses_walk_each_dimension_low_to_high() {
    sim_cli::run_case_with_files(
        "syn030_memory_views",
        "mixed_3d",
        "outer1=11,12,13,14 outer2=21,22,23,24\n",
        DONE,
        &[],
        &["--edition", "2009"],
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
        &["--edition", "2009"],
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
        &["--edition", "2009"],
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
        &["--edition", "2009"],
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
        &["--edition", "2009"],
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
        &["--edition", "2009"],
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
        &["--edition", "2009"],
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
        &["--edition", "2009"],
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
        &["--edition", "2001"],
    );
    sim_cli::reject_case_with_args(
        "syn030_memory_views",
        "legacy_slice",
        "memory slices require SystemVerilog-2009",
        &["--edition", "2001"],
    );
}
