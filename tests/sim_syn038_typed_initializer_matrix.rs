//! SYN-038 packed aggregate and predicate declaration initializer sites.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn typed_initializers_keep_their_constant_and_runtime_values() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "typed_initializer_matrix",
        "const=1234,b5c6,1 runtime=2143,c5d6,1 static=3153,d5e6,1 auto=4163,e5f6,1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
