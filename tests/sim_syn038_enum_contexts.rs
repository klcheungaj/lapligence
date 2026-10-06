//! Enum declaration, cast-stream assignment, and equality/inside RHS contexts
//! exercised through the public simulator CLI in both optimizer modes.

use crate::sim_cli;

#[test]
fn enum_initializer_cast_stream_and_inside_rhs_execute() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "enum_contexts",
        "runtime=00000101 calls=1\nstream=00000101 eq=1 inside=1 miss=0 direct=00000001\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
