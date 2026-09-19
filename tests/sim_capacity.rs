//! G1-32 capacity growth: legal models above the retired fixed registries must
//! execute (or fail with a specific resource diagnostic), never abort on a
//! silent fixed table. Each case runs in both optimizer modes.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

fn run_generated(tag: &str, source: &str, expected: &str) {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        for (variant, opts) in [("on", OptConfig::default()), ("off", OptConfig::none())] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &opts)
                .map_err(|error| error.to_string())?;
            let exe =
                sim::build::build_model_cmake(&dir.join(variant), &[("model.c", &model.model_c)])
                    .map_err(|error| error.to_string())?;
            assert_eq!(sim_harness::run_executable(&exe)?, expected, "{variant}");
        }
        Ok(())
    })
    .expect("capacity simulation");
}

#[test]
fn capacity_event_waiters_grow_past_old_limit() {
    // 100 concurrent waiters on one event previously aborted at 64.
    sim_cli::run_case(
        "feature_completion/g1_32",
        "event_waiter_growth",
        "CHECK: count=100\n",
        "",
        &[],
    );
}

#[test]
fn capacity_process_registry_grows_past_initial_table() {
    // 100 concurrent forked processes force the slot table past its initial
    // capacity; every child must still run and be counted.
    sim_cli::run_case(
        "feature_completion/g1_32",
        "process_registry_growth",
        "CHECK: count=100\n",
        "",
        &[],
    );
}

#[test]
fn capacity_force_entries_grow_past_old_limit() {
    // 100 distinct force targets previously aborted at 64. Releasing one
    // target lets a normal write reach it again while the others stay forced.
    sim_cli::run_case(
        "feature_completion/g1_32",
        "force_entry_growth",
        "CHECK: 1 1\nCHECK: 0 1\n",
        "",
        &[],
    );
}

#[test]
fn capacity_pca_bindings_grow_past_old_limit() {
    // 4100 simultaneous procedural continuous assignments exceed the retired
    // 4096-entry PCA table. Deassign frees the last target for a normal write.
    let n = 4100usize;
    let mut source = String::from(
        "// llg-test-fixture: tests/sim_capacity.rs/pca_growth.sv\nmodule tb;\n\
         logic a;\n",
    );
    for i in 0..n {
        source.push_str(&format!("logic v{i};\n"));
    }
    source.push_str("initial begin\n    a = 1'b1;\nend\n");
    // Keep each generated process small: one 4100-statement function makes the
    // generated C compiler's register allocation dominate the test.
    for chunk in (0..n).collect::<Vec<_>>().chunks(41) {
        source.push_str("initial begin\n");
        for i in chunk {
            source.push_str(&format!("    assign v{i} = a;\n"));
        }
        source.push_str("end\n");
    }
    source.push_str("initial begin\n    #1;\n");
    source.push_str(&format!(
        "    $display(\"CHECK: %b %b\", v0, v{});\n",
        n - 1
    ));
    source.push_str(&format!(
        "    deassign v{};\n    v{} = 1'b0;\n    #1;\n",
        n - 1,
        n - 1
    ));
    source.push_str(&format!("    $display(\"CHECK: %b\", v{});\n", n - 1));
    source.push_str("    $finish(0);\nend\nendmodule\n");
    let expected = "CHECK: 1 1\nCHECK: 0\n";
    run_generated("pca_growth", &source, expected);
}

#[test]
fn capacity_final_registrations_grow_past_old_limit() {
    // 1100 registered final blocks exceed the retired 1024-entry table. Every
    // final runs in source order and the last observes the accumulated count.
    let n = 1100usize;
    let mut source = String::from(
        "// llg-test-fixture: tests/sim_capacity.rs/final_growth.sv\nmodule tb;\n\
         int count;\n\
         initial begin\n    count = 0;\n    #1;\n    $finish(0);\nend\n",
    );
    for i in 0..n {
        if i + 1 == n {
            source.push_str(
                "final begin count = count + 1; $display(\"CHECK: count=%0d\", count); end\n",
            );
        } else {
            source.push_str("final begin count = count + 1; end\n");
        }
    }
    source.push_str("endmodule\n");
    run_generated("final_growth", &source, "CHECK: count=1100\n");
}
