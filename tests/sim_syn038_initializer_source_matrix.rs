//! SYN-038 source-storage values flow into core declaration initializers.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn declaration_initializers_read_each_source_storage_kind() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "initializer_source_matrix",
        "automatic=11,21,31,41 formal=52,62,72 return=84,94,a4,b4 static=01,01,02 interface=00,00,d5\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
