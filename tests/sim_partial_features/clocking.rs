use super::run_case_with_stderr;

#[test]
fn clocking_input_skews_sample_preponed_observed_and_history_values() {
    run_case_with_stderr(
        "clocking_h13",
        "t=6 raw=1 step=0 zero=1 two=0\nt=16 raw=1 step=1 zero=1 two=1\n",
        "llg: $finish at time 18 at tb:23:9\n",
    );
}

#[test]
fn clocking_inputs_resolve_defaults_and_aliases_through_interfaces() {
    run_case_with_stderr(
        "clocking_h13_interface",
        "zero t=6000 raw=0 sample=0\ndefault t=6000 raw=1 sample=0\ndefault t=16000 raw=1 sample=1\nzero t=16000 raw=1 sample=1\n",
        "llg: $finish at time 18000 at tb:28:8\n",
    );
}

#[test]
fn clocking_inputs_resolve_through_static_virtual_interfaces() {
    run_case_with_stderr(
        "clocking_h13_virtual",
        "t=6000 data=1 sampled=0\n",
        "llg: $finish at time 15000 at tb:18:9\n",
    );
}
