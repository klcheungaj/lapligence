//! SYN-038 process, call-actual, and port-actual lvalue paths.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn selected_lvalue_contexts_keep_actual_storage_and_process_paths_distinct() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "lvalue_context_matrix",
        "proc=11,22,3,45,01 comb=89,80 latch=cd,81 ff=5,01 if=31,4,56,01 call=c1,d2,6,7,e3,f4 child=9a init=23,34,45,56,12 extra=45,12,78,67,12\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
