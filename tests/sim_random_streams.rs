//! H07 random-stream contracts in both optimizer configurations.

use std::path::Path;

use llg::core::compile;
use llg::sim;
use llg::sim::opt::OptConfig;

#[path = "support/sim.rs"]
mod sim_harness;

const RANDOM_STREAMS: &str = include_str!("fixtures/sim/random_streams/random_streams.sv");

fn build_and_run(dir: &Path, db: &llg::core::db::Db, opts: &OptConfig, name: &str) -> String {
    let model = sim::codegen::generate_from_db_with_opts(db, opts)
        .unwrap_or_else(|error| panic!("codegen {name}: {error}"));
    let output_dir = dir.join(name);
    let executable = sim::build::build_model_cmake(&output_dir, &[("model.c", &model.model_c)])
        .unwrap_or_else(|error| panic!("cmake {name}: {error}"));
    sim_harness::run_executable(&executable).unwrap_or_else(|error| panic!("run {name}: {error}"))
}

#[test]
fn random_streams_replay_ranges_and_fork_children() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("random-streams", |dir| {
        let source = dir.join("random_streams.sv");
        std::fs::write(&source, RANDOM_STREAMS)
            .map_err(|error| format!("write fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile fixture: {error}"))?;
        let db = llg::core::db::Db::from_slang(&compiled.snapshot)
            .map_err(|error| format!("capture database: {error}"))?;
        let optimized = build_and_run(dir, &db, &OptConfig::default(), "optimized");
        let unoptimized = build_and_run(dir, &db, &OptConfig::none(), "unoptimized");
        if optimized != "random streams ok\n" || unoptimized != optimized {
            return Err(format!(
                "random-stream output mismatch: optimized={optimized:?}, unoptimized={unoptimized:?}"
            ));
        }
        Ok(())
    })
    .expect("random-stream fixture should execute identically");
}
