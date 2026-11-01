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

/// Lines of the longest C function of process `owner` or its helpers.
fn longest_function(model_c: &str, owner: &str) -> usize {
    let mut longest = 0;
    let mut start = None;
    for (index, line) in model_c.lines().enumerate() {
        if line.starts_with("static ") && line.ends_with('{') && line.contains(owner) {
            start = Some(index);
        } else if line == "}" {
            if let Some(first) = start.take() {
                longest = longest.max(index - first + 1);
            }
        }
    }
    longest
}

/// The optimizer folds every `$display` argument of the `rtl_006` constant
/// rows to a literal. Each such argument renders several C lines for one IR
/// node, so the run weight counts them, and their helpers stay as short as
/// those of the same rows evaluated at run time (`--no-opt`). Weighed by IR
/// nodes alone, the optimized rows became one 4,600-line helper that MSVC
/// `/O2` did not compile within the 180 s run limit.
#[test]
fn folded_constant_rows_render_helpers_of_bounded_size() {
    const MAX_FUNCTION_LINES: usize = 2_000;
    for width in [32, 65] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_006")
            .join(format!("arith_constants_w{width}.sv"));
        let compiled = compile::compile(&CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .expect("compile fixture");
        assert!(compiled.ok());
        let database = Db::from_slang(&compiled.snapshot).expect("owned database");
        for options in [sim::opt::OptConfig::none(), sim::opt::OptConfig::default()] {
            let model = sim::codegen::generate_from_db_with_opts(&database, &options)
                .expect("generate model");
            assert!(model
                .model_c
                .contains("static int p_tb_proc_0_llg_run_2(sv4_t* _llg_t"));
            let longest = longest_function(&model.model_c, "p_tb_proc_0");
            assert!(
                longest <= MAX_FUNCTION_LINES,
                "width {width}: {longest} lines in one function"
            );
        }
    }
}
