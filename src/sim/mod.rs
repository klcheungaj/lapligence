//! sim — Verilog/SystemVerilog → C11 code generation and simulation runtime.
//!
//! [`codegen::generate`] lowers a frontend-neutral [`semantic`] model into
//! typed [`execution`] blocks, applies [`opt`], and renders them through
//! [`emit_c`]. [`rt`] provides the embedded C runtime. The single model builder is
//! [`build::build_model_cmake`] (CMake-only; invoked automatically right
//! after C emission — see the `build` module docs for env vars and generator
//! selection).
//!
//! Supported language constructs and limits are maintained in
//! `docs/sim_features.md` rather than duplicated here.

pub mod build;
pub mod codegen;
pub mod emit_c;
pub mod execution;
pub mod ir;
pub mod opt;
pub mod rt;
pub mod semantic;
pub mod value_backend;

use std::path::Path;

/// Create `out_dir` and write the runtime sources plus `extra`
/// (e.g. the generated `model.c`) into it.  Shared by
/// [`build::generate_model_sources`] / [`build::build_model_cmake_with_opts`].
pub(crate) fn write_sim_sources(
    out_dir: &Path,
    extra: &[(&str, &str)],
    config: value_backend::ValueConfig,
) -> Result<(), build::BuildError> {
    std::fs::create_dir_all(out_dir).map_err(|source| build::BuildError::Io {
        action: "create",
        path: out_dir.to_path_buf(),
        source,
    })?;

    let (rt_h, rt_c) = rt::runtime_sources();
    let (value_h, value_c) = rt::value_sources_for(config.backend);
    let (random_h, random_c) = rt::random_sources();
    let (rng_h, rng_c) = rt::rng_sources();
    let (coroutine_h, coroutine_c) = rt::coroutine_sources();
    let (vpi_h, vpi_c) = rt::vpi_sources();
    let vpi_bridge_h = rt::vpi_bridge_header();
    let (container_h, container_c) = rt::container_sources();
    let (string_h, string_c) = rt::string_sources();
    let mut files = vec![
        ("llg_rt.h", rt_h),
        ("llg_rt.c", rt_c),
        ("llg_value.h", value_h),
        ("llg_value.c", value_c),
        ("llg_random.h", random_h),
        ("llg_random.c", random_c),
        ("llg_rng.h", rng_h),
        ("llg_rng.c", rng_c),
        ("llg_co.h", coroutine_h),
        ("llg_co.c", coroutine_c),
        ("vpi_user.h", vpi_h),
        ("llg_vpi.h", vpi_bridge_h),
        ("llg_vpi.c", vpi_c),
        ("llg_container.h", container_h),
        ("llg_container.c", container_c),
        ("llg_string.h", string_h),
        ("llg_string.c", string_c),
        (
            "svdpi.h",
            include_str!("../../vendor/slang/external/ieee1800/svdpi.h"),
        ),
    ];
    files.extend_from_slice(rt::value_backend_sources(config.backend));
    files.extend_from_slice(extra);

    for (name, content) in &files {
        let path = out_dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| build::BuildError::Io {
                action: "create",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&path, content).map_err(|source| build::BuildError::Io {
            action: "write",
            path,
            source,
        })?;
    }
    Ok(())
}
