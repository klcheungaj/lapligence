use crate::sim_cli;

/// `wide[(wide = 0) +: 1] = wide` runs the nested assignment before reading
/// the right-hand side (decision EO-D1, IEEE 1800-2009 11.4.2 L14500), so the
/// selected bit takes the new zero.
#[test]
fn emitted_value_traffic_arithmetic_aliases_and_boundaries() {
    sim_cli::run_case(
        "emit_value_traffic", "arithmetic",
        &format!("nba=22\noutput=8,9\nselector=0\nxz={},zzzz\nunsigned65=08000000000000000\nsigned65=18000000000000000\nback64=8000000000000000\n", "x".repeat(128)),
        "", &[],
    );
}

#[test]
fn emitted_value_traffic_snapshots_survive_suspension() {
    sim_cli::run_case(
        "emit_value_traffic",
        "suspension",
        "snapshot=7,source=9\ntask=29\n",
        "",
        &[],
    );
}
