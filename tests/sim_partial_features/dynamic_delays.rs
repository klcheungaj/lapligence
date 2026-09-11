use super::{reject_case, run_case};

#[test]
fn runtime_delays_capture_values_and_keep_function_and_task_dependencies() {
    run_case(
        "dynamic_delay_capture",
        "first 3\nsecond 6\ncall 8 1\ntask 11\n",
    );
}

#[test]
fn real_delays_round_at_local_precision_before_global_scaling() {
    run_case(
        "dynamic_delay_rounding",
        "first 0.200\nchild 0.030\nsecond 0.500\nthird 0.600\nzero 0.600\n",
    );
}

#[test]
fn blocking_real_intra_assignment_captures_before_suspending() {
    run_case("blocking_real_delay", "real 2 1.25\nshort 3 16777216\n");
}

#[test]
fn runtime_nba_delays_capture_time_value_and_target_without_suspending() {
    run_case(
        "dynamic_delay_nba",
        "issued 0 0000\nearly 2 0010 2.5\nlate 4 0011\n",
    );
}

#[test]
fn unknown_delays_are_zero_and_remain_before_the_nba_region() {
    run_case(
        "unknown_delay_zero",
        "x 0 0\nz 0 0\nliteral 0 0\nlater 1 1\n",
    );
}

#[test]
fn negative_packed_delays_convert_to_unsigned_time_width() {
    run_case("negative_delay_time_cast", "18446744073709551614\n");
}

#[test]
fn runtime_delay_overflow_and_nonfinite_values_fail_explicitly() {
    for fixture in [
        "dynamic_delay_wide_overflow",
        "dynamic_delay_scale_overflow",
    ] {
        reject_case(fixture, "delay exceeds the 64-bit tick range");
    }
    reject_case(
        "dynamic_delay_nonfinite",
        "real delay must be finite and nonnegative",
    );
}
