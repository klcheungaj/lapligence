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

/// A call made from an outlined run of a task records its call as the
/// callee frame's `vpiOrigin`, and the task's frame lists its automatics
/// with their values (decisions A19-D75, A19-D76): the helper stores the
/// origin before the call, and the run never holds the task's locals.
#[test]
fn vpi_frames_see_calls_made_from_outlined_runs() {
    let directory =
        crate::sim_harness::TempDir::new("outline-frames").expect("temporary directory");
    let mut source = String::from(
        "module tb;\n  int g, k;\n  function automatic int h(int v);\n    $d75(1);\n    return v + 1;\n  endfunction\n  task automatic t(input int a);\n    int acc = a + 1;\n    g = acc;\n",
    );
    for index in 0..3000 {
        source.push_str(&format!("    g = (g ^ {index}) + {};\n", index % 7));
        if index == 1500 {
            source.push_str("    k = h(g);\n");
        }
    }
    source.push_str("    g = g + k + acc;\n  endtask\n  initial begin\n    t(5);\n    $display(\"g=%0d k=%0d\", g, k);\n    $finish;\n  end\nendmodule\n");
    let path = directory.path().join("outline_frames.sv");
    std::fs::write(&path, source).expect("write source");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let companion = root.join("tests/fixtures/sim/lrm_decisions/A19-D75_frame_origins.c");
    let archive = crate::c_compiler::static_archive(&companion, directory.path(), "frames", &[])
        .expect("VPI application archive");
    let expected = "step 1\n  tb.h origin vpiFuncCall@1511 calls tb.h (error 0) (error 0) stmt vpiSysTaskCall@4 (error 0)\n  tb.t origin vpiTaskCall@3014 calls tb.t (error 0) (error 0) stmt vpiAssignment@1511 (error 0)\ng=1766 k=810\n";
    for optimized in [true, false] {
        let work = directory
            .path()
            .join(if optimized { "opt" } else { "noopt" });
        std::fs::create_dir_all(&work).expect("work directory");
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .current_dir(&work)
            .args(["--top", "tb", "-Wno-error"]);
        if !optimized {
            command.arg("--no-opt");
        }
        let output = command
            .args(["--define-system-task", "task $d75(input int)", "--dpi-lib"])
            .arg(&archive)
            .arg(&path)
            .output()
            .expect("run llg");
        let stdout = String::from_utf8_lossy(&crate::sim_harness::host_text_to_lf(output.stdout))
            .into_owned();
        assert!(
            output.status.success() && stdout == expected,
            "optimized={optimized} status={:?}\n{stdout}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        let model_c = std::fs::read_to_string(work.join("build/sim/tb/model.c")).expect("model C");
        assert!(
            model_c.contains("static int fn_tb_t_llg_run_"),
            "optimized={optimized}: the task body has no outlined run"
        );
    }
}
