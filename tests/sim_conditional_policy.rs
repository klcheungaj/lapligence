//! Public CLI probes for the packed conditional tables in both supplied editions.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn packed_conditional_policy_matches_both_lrm_tables() {
    let expected = concat!(
        "known0=zx10zx10zx10zx10\n",
        "known1=zzzzxxxx11110000\n",
        "x=xxxxxxxxxx1xxxx0\n",
        "z=xxxxxxxxxx1xxxx0\n",
        "constant=x frontend=x mixed=10xx01x0 wide=",
        "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n",
    );
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            "conditional_policy",
            "packed_mux_policy",
            expected,
            "",
            &[],
            &["--edition", edition],
        );
    }
}
