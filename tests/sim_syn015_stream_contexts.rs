//! SYN-015 public-pipeline evidence for fixed bit-stream casts and streams.

use crate::sim_cli;

#[test]
fn stream_width_matrix_keeps_order_alignment_and_state_in_both_modes() {
    // Oracles inside the fixture use only concatenation and selects; the
    // three instances cover 7-, 65- and 129-bit lanes.
    sim_cli::run_case(
        "data_types_next",
        "syn_015_stream_width_matrix",
        "SYN015_STREAM_WIDTH_MATRIX_PASS\n",
        "",
        &[],
    );
}

#[test]
fn runtime_sized_streams_align_and_keep_storage_order() {
    sim_cli::run_case(
        "data_types_next",
        "syn_015_runtime_streams",
        "SYN015_RUNTIME_STREAMS_PASS\n",
        "",
        &[],
    );
    sim_cli::reject_case(
        "data_types_next",
        "syn_015_runtime_oversize",
        "streaming concatenation is larger than its fixed-size target",
    );
}

#[test]
fn illegal_stream_and_cast_shapes_are_rejected() {
    sim_cli::reject_case(
        "data_types_next",
        "syn_015_bad_cast_size",
        "cannot be converted",
    );
    sim_cli::reject_case(
        "data_types_next",
        "syn_015_bad_stream_target_size",
        "does not fit source size",
    );
    sim_cli::reject_case(
        "data_types_next",
        "syn_015_bad_unpack_size",
        "does not fit source size",
    );
    sim_cli::reject_case(
        "data_types_next",
        "syn_015_bad_real_stream",
        "is not a bit-stream type",
    );
    sim_cli::reject_case(
        "data_types_next",
        "syn_015_bad_associative_stream",
        "is not a bit-stream type",
    );
    sim_cli::reject_case(
        "data_types_next",
        "syn_015_bad_with_source",
        "requires a one-dimensional unpacked array",
    );
    sim_cli::reject_case(
        "data_types_next",
        "syn_015_bad_with_target",
        "requires a one-dimensional unpacked array",
    );
}
