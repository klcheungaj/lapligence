//! File-backed P39 expression-valued mutation regressions.
//!
//! The checked-in fixture is executed through the public simulator in both
//! optimizer modes so target capture and conversion behavior cannot diverge.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn expression_mutations_preserve_target_capture_and_value_conversions() {
    sim_cli::run_case(
        "expression_mutations",
        "expression_mutations",
        concat!(
            "packed=5a part=10 bit=1 indexed=1\n",
            "array=7 old_array=7 index=1 calls=1\n",
            "member=1/0 high=2 assign=2 two=1/0\n",
            "signed=-7/-8 overflow=0/15\n",
            "unknown=xxxx/x001 highz=xxxx/z001\n",
            "wide_top=1/0 wide_low=0000000000000000/ffffffffffffffff\n",
            "prefix=3/3 dec=2/2 selected=12/13 array=9\n",
            "assigned=90 real=1.5/2.5\n",
        ),
        "llg: $finish at time 0 at tb:115:9\n",
        &[],
    );
}
