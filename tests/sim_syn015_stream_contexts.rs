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

/// `<<` re-ordering at every consumed width 1..64 under slice sizes around
/// each block boundary, through the runtime's unpack-source path (a runtime
/// `with` target) and its fixed-array stream copy (whole descriptor-backed
/// arrays), both of which reverse small values in the compact backend's
/// inline stream loop. CI: MSVC 19.52 /O2 built that loop, inlined into those
/// two runtime callers, with every block one slice too high (11 Windows
/// failures, master a8fe1963); the same loop inlined elsewhere stayed right,
/// so the fixture must reach these callers, which the generated source shows.
#[test]
fn reverse_slices_keep_block_boundaries_in_runtime_unpack_and_array_copy() {
    use crate::sim_harness;
    use std::process::Command;
    use std::time::Duration;
    let directory = sim_harness::TempDir::new("syn015-reverse-slices").expect("scratch");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_next/syn_015_reverse_slice_boundaries.sv");
    let output = sim_harness::run_command(
        Command::new(env!("CARGO_BIN_EXE_llg"))
            .arg(source)
            .arg("--gen-only")
            .arg("--out-dir")
            .arg(directory.path()),
        Duration::from_secs(120),
    )
    .expect("generate the model");
    assert!(output.status.success(), "{output:?}");
    let model =
        std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model source");
    for entry in ["llg_stream_unpack_source_to", "llg_fixed_array_stream_copy"] {
        assert!(
            model.contains(entry),
            "the fixture no longer reaches {entry}"
        );
    }
    sim_cli::run_case(
        "data_types_next",
        "syn_015_reverse_slice_boundaries",
        "SYN015_REVERSE_SLICE_BOUNDARIES_PASS\n",
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
