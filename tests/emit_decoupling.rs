//! Architectural decoupling test for the simulator pipeline.
//!
//! The C11 backend (`sim::emit_c`) must consume only IR types — never the
//! lowering frontend's semantic database or the Slang binding layer — and the
//! lowering (`sim::codegen`) must build IR instead of emitting runtime C
//! calls directly.  This pins the
//! `SemanticModel → ExecutionModel → optimization passes → C11 emit backend`
//! pipeline shape, in the spirit of the repo's other grep-based invariants.

use std::path::{Path, PathBuf};

fn sim_sources(entry: &str) -> Vec<(PathBuf, String)> {
    let sim_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/sim");
    let mut sources = Vec::new();
    collect_rust_sources(&sim_dir.join(format!("{entry}.rs")), &mut sources);
    collect_rust_sources(&sim_dir.join(entry), &mut sources);
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

fn collect_rust_sources(path: &Path, sources: &mut Vec<(PathBuf, String)>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
        {
            let entry = entry.expect("read simulator source entry");
            collect_rust_sources(&entry.path(), sources);
        }
    } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        sources.push((path.to_path_buf(), source));
    }
}

/// `emit_c.rs` is a pure execution-IR consumer: no semantic database access or FFI
/// bindings, no `unsafe`.
#[test]
fn emit_c_consumes_only_ir() {
    for (path, source) in sim_sources("emit_c") {
        for banned in ["core::db", "crate::ffi", "unsafe", "VpiHandle"] {
            assert!(
                !source.contains(banned),
                "{} must not reference `{banned}`: the C11 backend consumes only IR types",
                path.display()
            );
        }
    }
}

/// The lowering builds an `ExecutionModel`; raw runtime-call emission moved to the
/// backend.
#[test]
fn codegen_builds_ir_not_c_text() {
    let codegen = sim_sources("codegen");
    for (path, source) in &codegen {
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
                !source.contains(banned),
                "{} must not emit runtime C text containing `{banned}`: emission belongs to emit_c",
                path.display()
            );
        }
    }
    // The pipeline seam exists on both sides.
    assert!(
        codegen
            .iter()
            .any(|(_, source)| source.contains("generate_with_opts")),
        "generate_with_opts must expose the optimizer configuration"
    );
    let emit = sim_sources("emit_c");
    assert!(
        emit.iter().any(|(_, source)| source
            .contains("pub fn render(execution: &ExecutionModel) -> Result<String, EmitError>")),
        "the backend entry point renders a complete ExecutionModel"
    );
}

/// The simulator core must not link the LSP's transport/async stack: the LSP
/// is a consumer of `sim`, never the reverse. `cargo check --lib
/// --no-default-features` is the executable gate; this pins the source shape.
#[test]
fn simulator_sources_do_not_reference_lsp_dependencies() {
    let mut sources = Vec::new();
    collect_rust_sources(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/sim"),
        &mut sources,
    );
    assert!(
        !sources.is_empty(),
        "simulator sources should be discovered"
    );
    for (path, source) in &sources {
        for banned in ["tower_lsp", "tokio", "dashmap"] {
            assert!(
                !source.contains(banned),
                "{} must not reference LSP dependency `{banned}`",
                path.display()
            );
        }
    }
}
