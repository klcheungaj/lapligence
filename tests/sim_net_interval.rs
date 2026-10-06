//! Undriven net-array cells: hand-computed values on every value backend and
//! generated-code bounds that do not grow with the cell count.
use crate::sim_cli;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};
use std::time::{Duration, Instant};

const SUITE: &str = "net_interval";
/// Generated C for the large fixtures; the per-cell lowering produced
/// 140.5 MB (65,537 cells) and over 300 MB (200,000 cells).
const MODEL_C_CAP_BYTES: usize = 1 << 20;
/// Codegen wall time per optimizer mode, generous for loaded debug hosts; the
/// per-cell lowering took 28 s at 65,537 cells and did not finish 200,000.
const CODEGEN_CAP: Duration = Duration::from_secs(20);

fn assert_bounded_codegen(fixture: &str) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("fixture compiles");
    let db = Db::from_slang(&compiled.snapshot).expect("owned database");
    for (variant, opts) in [("opt", OptConfig::default()), ("no-opt", OptConfig::none())] {
        let started = Instant::now();
        let model = sim::codegen::generate_from_db_with_opts(&db, &opts).expect("codegen");
        let elapsed = started.elapsed();
        assert!(
            model.model_c.len() <= MODEL_C_CAP_BYTES,
            "{fixture}/{variant}: model.c has {} bytes",
            model.model_c.len()
        );
        assert!(
            elapsed <= CODEGEN_CAP,
            "{fixture}/{variant}: codegen took {elapsed:?}"
        );
    }
}

#[test]
fn undriven_cells_keep_net_type_values_on_every_backend() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "undriven_cells",
        "w=f 5 f f u=f\n\
         t=00 10 00\n\
         s=111 111 free=zzzz zzzz\n\
         v=Pu0 HiZ Pu1 Su1\n\
         w2=f\n\
         forced=1010 zzzz\n\
         released=zzzz\n",
        &[],
        &[],
    );
}

#[test]
fn whole_array_inout_of_65537_cells_is_bounded_and_exact() {
    assert_bounded_codegen("inout_65537");
    sim_cli::run_case_backend_parity(
        SUITE,
        "inout_65537",
        "3c zz zz zz 3c\nzz 5a 5a zz\n00111100 01011010\n",
        &[],
        &[],
    );
}

#[test]
fn udp_cell_in_200000_cell_array_is_bounded_and_exact() {
    assert_bounded_codegen("udp_200000");
    sim_cli::run_case_backend_parity(
        SUITE,
        "udp_200000",
        "zzzz1zzz zzzzzzzz zzzzzzzz\nzzzz0zzz zzzzzzzz\nzzzzxzzz\n",
        &[],
        &[],
    );
}

#[test]
fn undriven_cell_rejects_procedural_assignment() {
    sim_cli::reject_case(
        SUITE,
        "procedural_cell",
        "cannot assign to a net within a procedural context",
    );
}
