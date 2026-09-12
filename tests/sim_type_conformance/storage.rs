use super::run_case;

fn wide_net_and_state_conversion(width: usize) {
    run_case(
        &format!("wide-net-conversion-{width}"),
        "PASS wide net conversion\n",
    );
}

#[test]
fn net_and_two_state_conversion_65536_bits() {
    wide_net_and_state_conversion(65_536);
}

#[test]
fn net_and_two_state_conversion_at_maximum_width() {
    wide_net_and_state_conversion((1 << 20) - 1);
}

#[test]
fn arrays_containers_and_aggregates_preserve_state_domains() {
    super::run_case_with_stderr("state-storage", "PASS state storage\n", "");
}

#[test]
fn two_state_port_function_task_and_nba_boundaries() {
    run_case("state-boundaries", "PASS state boundaries\n");
}

#[test]
fn string_byte_and_chandle_copy_boundaries() {
    run_case("object-types", "PASS object types\n");
}

#[test]
fn real_and_realtime_rounding_and_shortreal_precision() {
    run_case("real-type-boundaries", "PASS real boundaries\n");
}
