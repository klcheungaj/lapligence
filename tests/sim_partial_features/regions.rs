use super::run_case_with_stderr;

#[test]
fn verilog_active_inactive_nba_and_postponed_subset_remains_ordered() {
    run_case_with_stderr(
        "region_boundaries",
        "active 0 0\nactive 1 0\ninactive 1 0\npostponed 1 1\n",
        "INFO: region_boundaries.sv:14:12: [SIM-001] (tb @ 1 ns) $finish called\n",
    );
}
