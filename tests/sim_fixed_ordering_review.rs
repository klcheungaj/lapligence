//! Fixed array ordering regressions from the synthesizable review.
use crate::sim_cli;

#[test]
fn unpacked_record_reverse_preserves_fields() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r07_unpacked_record_reverse",
        "PASS r07_unpacked_record_reverse\n",
        "llg: $finish at time 0 at tb:13:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn unpacked_record_sort_uses_integral_map_key() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r07_unpacked_record_sort",
        "PASS r07_unpacked_record_sort\n",
        "llg: $finish at time 0 at tb:13:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_rows_are_reversed_and_sorted_as_owned_elements() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r07_fixed_rows",
        "PASS r07_fixed_rows\n",
        "llg: $finish at time 0 at tb:35:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn unpacked_record_sort_requires_an_integral_comparison_key() {
    sim_cli::reject_case_with_args(
        "review_bundle",
        "r07_unmapped_record_sort",
        "can only be called on unpacked arrays of comparable values",
        &["--edition", "2009"],
    );
}
