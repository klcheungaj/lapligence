use super::run_case;

#[test]
fn scalar_selects_translate_ascending_and_offset_declared_ranges() {
    run_case(
        "declared_select_ranges",
        "read 1 0 a 9\nindexed 9 2\npartial xx10 xx10\nwrite 38 d7\nreverse 110 101\n",
    );
}

#[test]
fn array_element_selects_translate_ranges_and_preserve_other_bits() {
    run_case("array_select_ranges", "read 1 0 a 9\nwrite 34 a7\n");
}

#[test]
fn array_indexed_part_selects_preserve_ranges_and_assignment_conversions() {
    run_case("array_indexed_select", "read 9 001\nwrite 8d d6 14 03 20\n");
}

#[test]
fn array_indexed_nbas_capture_indices_once_and_merge_at_commit() {
    run_case(
        "array_indexed_nba",
        "issued 0 00 00 2\nearly 1 50 a0 2\nlate 2 d9 a0 2\n",
    );
}

#[test]
fn array_indexed_part_selects_guard_invalid_indices_and_cross_limb_boundaries() {
    run_case("array_indexed_bounds", "bounds e7 xx11 11xx\nwide 66 1 0\n");
}
