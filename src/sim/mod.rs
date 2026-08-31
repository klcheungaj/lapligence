//! sim — Verilog/SystemVerilog → C11 code generation and simulation runtime.
//!
//! [`codegen::generate`] lowers an elaborated UHDM design (from
//! `core::compile`) into a C11 model; [`rt`] provides the embedded C runtime
//! (4-state `sv4_t` values and a coroutine-based event scheduler) and the
//! vendored libaco sources.  The single model builder is
//! [`build::build_model_cmake`] (CMake-only; invoked automatically right
//! after C emission — see the `build` module docs for env vars and generator
//! selection).
//!
//! v1 scope: 4-state semantics (X and Z stored and displayed distinctly),
//! vectors up to 1024 bits, processes (`initial`/`always`, including
//! generate-block processes), event control, timescale-aware delays, NBA,
//! continuous assignments, parameter propagation, port + interface links,
//! functions/tasks (recursion, defaults, inlining), fork/join, arrays/
//! memories, casez/casex, hierarchical reads, `$display`/`$monitor`/`$strobe`/
//! `$finish`/`$time`.

pub mod build;
pub mod codegen;
pub mod emit_c;
pub mod ir;
pub mod opt;
pub mod rt;

use std::path::Path;

/// Create `out_dir` and write the runtime + libaco sources plus `extra`
/// (e.g. the generated `model.c`) into it.  Shared by
/// [`build::generate_model_sources`] / [`build::build_model_cmake_with_opts`].
pub(crate) fn write_sim_sources(out_dir: &Path, extra: &[(&str, &str)]) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    let (rt_h, rt_c) = rt::runtime_sources();
    let (aco_h, aco_c, aco_s) = rt::libaco_sources();
    let mut files = vec![
        ("llg_rt.h", rt_h),
        ("llg_rt.c", rt_c),
        ("aco.h", aco_h),
        ("aco.c", aco_c),
        ("acosw.S", aco_s),
        (
            "aco_assert_override.h",
            include_str!("../../vendor/libaco/aco_assert_override.h"),
        ),
    ];
    files.extend_from_slice(extra);

    for (name, content) in &files {
        let path = out_dir.join(name);
        std::fs::write(&path, content).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}
