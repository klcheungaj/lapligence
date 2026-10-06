use llg::core::compile::{compile_checked, CompileOpts};
use llg::core::db::Db;
use llg::sim::{codegen::generate_from_db_with_opts, opt::OptConfig};

use crate::sim_cli;

fn render(stem: &str, options: &OptConfig) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/frame_cells")
        .join(format!("{stem}.sv"));
    let compiled = compile_checked(&CompileOpts {
        files: vec![path.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("frame cells fixture compiles");
    let db = Db::from_slang(&compiled.snapshot).expect("owned frame cells database");
    generate_from_db_with_opts(&db, options)
        .expect("frame cells emit")
        .model_c
}

#[test]
fn nonescaping_cells_survive_resumes_returns_disable_and_frame_reuse() {
    sim_cli::run_case(
        "frame_cells",
        "reuse",
        "cell 3 0.5\ncell 4 0.5\nkept 3\ncell 3 0.5\n",
        "",
        &[],
    );
    for options in [OptConfig::default(), OptConfig::none()] {
        let source = render("reuse", &options);
        assert!(source.contains("llg_value_scope_register("));
        assert!(source.contains("llg_value_scope_register_object("));
        assert!(source.contains("llg_value_scope_t _llg_cell_scope_"));
    }
}

#[test]
fn zero_resume_cells_survive_finish_until_runtime_unwind() {
    sim_cli::run_case("frame_cells", "finish", "finish 7 0.5\n", "", &[]);
    for options in [OptConfig::default(), OptConfig::none()] {
        let source = render("finish", &options);
        assert!(source.contains("llg_value_scope_t _llg_cell_scope_"));
        assert!(source
            .lines()
            .any(|line| line.contains("llg_value_scope_register(") && line.contains("&F->")));
    }
}

#[test]
fn ref_timing_task_local_event_and_detached_capture_keep_heap_cells() {
    sim_cli::run_case(
        "frame_cells",
        "ref_wait_fork",
        "capture 11\njoined 11\ndetached 11\ncapture 12\njoined 12\ndetached 12\n",
        "",
        &[],
    );
    for options in [OptConfig::default(), OptConfig::none()] {
        let source = render("ref_wait_fork", &options);
        assert!(source.contains("llg_value_scope_values(llg_value_scope_begin(1))"));
        assert!(source.contains("llg_fork_with_frame("));
    }
}

#[test]
fn automatic_nba_followed_by_return_preserves_frontend_rejection() {
    sim_cli::reject_case_with_args(
        "frame_cells",
        "nba_return_rejected",
        "nonblocking assignment to automatic variable",
        &[],
    );
}

#[test]
fn local_event_subscription_keeps_heap_identity_until_cancel() {
    sim_cli::run_case(
        "frame_cells",
        "local_event",
        "waiting 7\nfinished\n",
        "",
        &[],
    );
    for options in [OptConfig::default(), OptConfig::none()] {
        let source = render("local_event", &options);
        assert!(source.contains("llg_value_scope_values(llg_value_scope_begin(1))"));
    }
}

#[test]
fn automatic_deferred_readers_preserve_frontend_rejection() {
    sim_cli::reject_case_with_args(
        "frame_cells",
        "deferred_rejected",
        "automatic variable",
        &[],
    );
}
