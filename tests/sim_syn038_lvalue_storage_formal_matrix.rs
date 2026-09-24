//! Selected SYN-038 lvalue shapes across storage, formal, call, and function contexts.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn lvalue_storage_and_formal_paths_preserve_selected_values() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "lvalue_storage_formal_matrix",
        "union=1234,5678 port=10 task=10 function=10:01 dynamic=10/1,1 direct=10 concat=10 inout=11 concat_inout=11 concat_fn=4142 field=61,62 row=31,32 formal=11,12 static=21,22 automatic=23,24 return=10 task_local=92 ref=a0 ref_field=7a\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
