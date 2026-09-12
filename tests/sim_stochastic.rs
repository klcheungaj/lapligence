//! IEEE stochastic-analysis queue system tasks, separate from SV queue values.
//!
//! The checked-in fixture is executed with and without optimization so queue
//! state, status codes, and simulation-time statistics cannot depend on IR
//! rewriting.

#[path = "support/sim.rs"]
mod sim_harness;

use std::path::Path;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

#[test]
fn stochastic_queue_order_status_and_statistics() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/stochastic/queue_statistics.sv");
    sim_harness::with_frontend_temp_cwd("stochastic-queue", |dir| {
        let source = dir.join("queue_statistics.sv");
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
        let expected = concat!(
            "init=0\n",
            "bad_type=4\n",
            "bad_length=5\n",
            "duplicate=6\n",
            "add0=0\n",
            "add5=0 full=1 full_status=0\n",
            "add_full=1\n",
            "remove_fifo=0 job=1 info=101\n",
            "stat1=1 status=0\n",
            "stat2=3 status=0\n",
            "stat3=2 status=0\n",
            "stat4=10 status=0\n",
            "stat5=5 status=0\n",
            "stat6=5 status=0\n",
            "bad_stat=321 status=4\n",
            "active1=2 status=0\n",
            "active2=7 status=0\n",
            "active3=2 status=0\n",
            "active4=10 status=0\n",
            "active5=15 status=0\n",
            "active6=8 status=0\n",
            "empty=3\n",
            "unknown_add=2\n",
            "unknown_remove=2\n",
            "unknown_full=0 status=2\n",
            "unknown_exam=2\n",
            "remove_lifo1=0 job=12 info=120\n",
            "remove_lifo2=0 job=11 info=110\n",
        );
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{variant} lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} C model build: {error}"))?;
            let output = sim_harness::run_executable(&executable)
                .map_err(|error| format!("{variant} simulation: {error}"))?;
            if output != expected {
                return Err(format!("{variant}: expected {expected:?}, got {output:?}"));
            }
        }
        Ok(())
    })
    .expect("stochastic queue simulation");
}
