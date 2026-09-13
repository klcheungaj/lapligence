use super::run_case;

#[test]
fn real_waits_events_combination_and_ports_use_typed_change_dependencies() {
    run_case(
        "real_sensitivity",
        "port=16777217.5 short=16777216\nwait=1 events=3 expr=3 comb=4 signed_zero=1 nan=1 doubled_nan=1\ncase=2\n",
    );
}
