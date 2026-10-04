//! SYN-012 fixed integral aggregate layout and value-context evidence.
#![allow(clippy::needless_borrows_for_generic_args)]

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const WIDTHS: &[&str] = &["1", "7", "8", "31", "32", "33", "64", "65", "129"];

#[test]
fn fixed_layout_matrix_runs_at_required_widths_in_both_optimizer_modes() {
    for width in WIDTHS {
        let define = format!("SYN012_W={width}");
        let expected = format!("PASS syn012_fixed_layout W={width}\n");
        sim_cli::run_case_with_args(
            "syn012_fixed_layout",
            "matrix",
            &expected,
            "",
            &[],
            &["--define", &define],
        );
    }
}

#[test]
fn unequal_packed_union_members_remain_a_frontend_error() {
    sim_cli::reject_case(
        "syn012_fixed_layout",
        "packed_union_width_rejected",
        "same width",
    );
}

#[test]
fn distinct_unpacked_record_identity_remains_a_frontend_error() {
    sim_cli::reject_case(
        "syn012_fixed_layout",
        "nominal_record_mismatch_rejected",
        "no implicit conversion",
    );
}

#[test]
fn native_record_conditionals_remain_outside_the_fixed_integral_path() {
    sim_cli::run_case(
        "syn012_fixed_layout",
        "native_record_rejected",
        "xx 0.0\n",
        "",
        &[],
    );
}

#[test]
fn unequal_unpacked_union_bitstream_cast_remains_illegal() {
    sim_cli::reject_case(
        "syn012_fixed_layout",
        "unpacked_union_bitstream_rejected",
        "invalid casting type",
    );
}
