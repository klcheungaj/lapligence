//! Long process and subprogram bodies whose statement runs the C emitter
//! moves into helper functions (`src/sim/emit_c/owned/outline.rs`). The
//! fixture's expected output comes from an independent 32-bit model of its
//! statements (fixture readme); the run must match it in both optimizer
//! modes, with `%m` scopes and the `$info` position of statements inside
//! outlined runs unchanged.

use crate::sim_cli;
use llg::core::compile::{self, CompileOpts};
use llg::core::db::Db;
use llg::sim;

const SUITE: &str = "outline_bodies";

#[test]
fn long_bodies_run_in_source_order_across_outlined_runs() {
    sim_cli::run_case(
        SUITE,
        "long_bodies",
        include_str!("fixtures/sim/outline_bodies/long_bodies.out"),
        "llg: severity info: tb:2399:5: acc ff6abae8\n",
        &[],
    );
}

/// Every body kind of the fixture is long enough to split, so the run above
/// exercises helpers in each: the `initial` and `final` procedures, the
/// function with an early `return`, the task with an automatic variable and
/// both fork branches.
#[test]
fn every_long_body_kind_outlines_in_both_optimizer_modes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("long_bodies.sv");
    let compiled = compile::compile(&CompileOpts {
        files: vec![path.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("compile fixture");
    assert!(compiled.ok());
    let database = Db::from_slang(&compiled.snapshot).expect("owned database");
    for options in [sim::opt::OptConfig::none(), sim::opt::OptConfig::default()] {
        let model =
            sim::codegen::generate_from_db_with_opts(&database, &options).expect("generate model");
        for owner in [
            "p_tb_proc_1",
            "p_tb_proc_4",
            "fn_tb_fold",
            "fn_tb_churn",
            "p_tb_fork_2_b0",
            "p_tb_fork_3_b1",
        ] {
            // Subprogram and fork-branch helpers also take the recursion depth.
            assert!(
                model
                    .model_c
                    .contains(&format!("static int {owner}_llg_run_0(sv4_t* _llg_t")),
                "{owner} has no outlined run"
            );
        }
    }
}
