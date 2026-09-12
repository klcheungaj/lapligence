//! sim — Verilog/SystemVerilog → C11 code generation and simulation runtime.
//!
//! [`codegen::generate`] lowers a frontend-neutral [`semantic`] model into
//! typed [`execution`] blocks, applies [`opt`], and renders them through
//! [`emit_c`]. [`rt`] provides
//! the embedded C runtime and vendored libaco sources. The single model builder is
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

use std::path::Path;

/// Create `out_dir` and write the runtime + libaco sources plus `extra`
/// (e.g. the generated `model.c`) into it.  Shared by
/// [`build::generate_model_sources`] / [`build::build_model_cmake_with_opts`].
pub(crate) fn write_sim_sources(
    out_dir: &Path,
    extra: &[(&str, &str)],
) -> Result<(), build::BuildError> {
    std::fs::create_dir_all(out_dir).map_err(|source| build::BuildError::Io {
        action: "create",
        path: out_dir.to_path_buf(),
        source,
    })?;

    let (rt_h, rt_c) = rt::runtime_sources();
    let (value_h, value_c) = rt::value_sources();
    let (random_h, random_c) = rt::random_sources();
    let (rng_h, rng_c) = rt::rng_sources();
    let (container_h, container_c) = rt::container_sources();
    let (string_h, string_c) = rt::string_sources();
    let (aco_h, aco_c, aco_s) = rt::libaco_sources();
    let mut files = vec![
        ("llg_rt.h", rt_h),
        ("llg_rt.c", rt_c),
        ("llg_value.h", value_h),
        ("llg_value.c", value_c),
        ("llg_random.h", random_h),
        ("llg_random.c", random_c),
        ("llg_rng.h", rng_h),
        ("llg_rng.c", rng_c),
        ("llg_container.h", container_h),
        ("llg_container.c", container_c),
        ("llg_string.h", string_h),
        ("llg_string.c", string_c),
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
        std::fs::write(&path, content).map_err(|source| build::BuildError::Io {
            action: "write",
            path,
            source,
        })?;
    }
    Ok(())
}
