//! SYN-038 interface-member storage through selected lvalue assignment forms.

use crate::sim_cli;

#[test]
fn interface_member_lvalues_and_initializer_keep_separate_readbacks() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "interface_lvalues",
        "interface=01,a0,b2,01 seeded=12 lanes=34,56\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
