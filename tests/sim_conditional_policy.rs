//! Public CLI probes for the packed conditional tables in both supplied editions.

use crate::sim_cli;

#[test]
fn packed_conditional_policy_uses_published_z_z_cell_in_both_editions() {
    let expected = format!(
        concat!(
            "known0=zx10zx10zx10zx10\n",
            "known1=zzzzxxxx11110000\n",
            "x=xxxxxxxxxx1xxxx0\n",
            "z=xxxxxxxxxx1xxxx0\n",
            "constant=x frontend=x generated_case=xxxx mixed=10xx01x0 wide={}\n",
        ),
        "x".repeat(65),
    );
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            "conditional_policy",
            "packed_mux_policy",
            &expected,
            "",
            &[],
            &["--edition", edition],
        );
        sim_cli::run_case_with_args(
            "review_bundle",
            "r10_packed_mux_policy",
            "MUX_POLICY result=x\n",
            "llg: $finish at time 0 at tb:10:5\n",
            &[],
            &["--edition", edition],
        );
    }
}
