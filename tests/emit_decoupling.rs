//! Architectural decoupling test for the simulator pipeline.
//!
//! The C11 backend (`sim::emit_c`) must consume only IR types — never the
//! lowering frontend's database or the Surelog binding layer — and the
//! lowering (`sim::codegen`) must build IR instead of emitting runtime C
//! calls directly.  This pins the
//! `core::db (lowering) → IrModel → optimization passes → C11 emit backend`
//! pipeline shape, in the spirit of the repo's other grep-based invariants.

use std::path::PathBuf;

fn sim_src(file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/sim")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// `emit_c.rs` is a pure IR consumer: no database access, no Surelog/VPI
/// bindings, no `unsafe`.
#[test]
fn emit_c_consumes_only_ir() {
    let src = sim_src("emit_c.rs");
    for banned in ["core::db", "crate::ffi", "vpi", "unsafe", "VpiHandle"] {
        assert!(
            !src.contains(banned),
            "src/sim/emit_c.rs must not reference `{banned}`: the C11 \
             backend consumes only `crate::sim::ir` types"
        );
    }
}

/// The lowering builds an `IrModel`; raw runtime-call emission moved to the
/// backend.
#[test]
fn codegen_builds_ir_not_c_text() {
    let src = sim_src("codegen.rs");
    for banned in [
        "llg_spawn(",
        "llg_ba(",
        "llg_ba_d(",
        "llg_nba(",
        "llg_nba_d(",
        "llg_net_write(",
        "llg_wait_time(",
        "llg_wait_any(",
        "llg_wait_any_events(",
        "llg_fork(",
        "llg_join(",
        "llg_force(",
        "llg_release(",
        "llg_display(",
        "llg_monitor(",
        "llg_strobe(",
        "llg_rt_run(",
        "static void p_",
    ] {
        assert!(
            !src.contains(banned),
            "src/sim/codegen.rs must not emit runtime C text containing \
             `{banned}`: emission belongs to `crate::sim::emit_c`"
        );
    }
    // The pipeline seam exists on both sides.
    assert!(
        src.contains("generate_with_opts"),
        "generate_with_opts must expose the optimizer configuration"
    );
    let emit = sim_src("emit_c.rs");
    assert!(
        emit.contains("pub fn render(model: &IrModel)"),
        "the backend entry point renders a complete IrModel"
    );
}
