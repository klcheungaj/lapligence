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
pub mod legacy_unsupported;
pub mod opt;
pub mod rt;
pub mod sdf;
pub mod semantic;
pub mod value_backend;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Writes one generated source set. Each parent directory is created once
/// rather than once per file: a source set holds about 140 files in 9
/// directories (most of them the bundled GMP subset), and on Windows every
/// redundant `create_dir_all` is a failed `CreateDirectoryW` plus a metadata
/// query. The counts feed the `LLG_BUILD_TIMINGS` record.
#[derive(Debug, Default)]
pub(crate) struct SourceWriter {
    /// Directories this writer created or found to exist.
    directories: BTreeSet<PathBuf>,
    /// Files written.
    pub(crate) files: u64,
    /// Bytes written.
    pub(crate) bytes: u64,
    /// `create_dir_all` calls made.
    pub(crate) directory_calls: u64,
}

impl SourceWriter {
    /// Create `directory` and its parents unless this writer already did.
    pub(crate) fn create_dir(&mut self, directory: &Path) -> Result<(), build::BuildError> {
        if self.directories.contains(directory) {
            return Ok(());
        }
        self.directory_calls += 1;
        std::fs::create_dir_all(directory).map_err(|source| build::BuildError::Io {
            action: "create",
            path: directory.to_path_buf(),
            source,
        })?;
        self.directories.insert(directory.to_path_buf());
        Ok(())
    }

    /// Write `content` to `path`, creating its parent directory first.
    pub(crate) fn write(
        &mut self,
        path: &Path,
        content: impl AsRef<[u8]>,
    ) -> Result<(), build::BuildError> {
        if let Some(parent) = path.parent() {
            self.create_dir(parent)?;
        }
        let content = content.as_ref();
        std::fs::write(path, content).map_err(|source| build::BuildError::Io {
            action: "write",
            path: path.to_path_buf(),
            source,
        })?;
        self.files += 1;
        self.bytes += content.len() as u64;
        Ok(())
    }
}

/// Create `out_dir` and write the runtime sources plus `extra`
/// (e.g. the generated `model.c`) into it.  Shared by
/// [`build::generate_model_sources`] / [`build::build_model_cmake_with_opts`].
pub(crate) fn write_sim_sources(
    out_dir: &Path,
    extra: &[(&str, &str)],
    config: value_backend::ValueConfig,
    writer: &mut SourceWriter,
) -> Result<(), build::BuildError> {
    writer.create_dir(out_dir)?;

    let (rt_h, rt_c) = rt::runtime_sources();
    let (value_h, value_c) = rt::value_sources_for(config.backend);
    let (random_h, random_c) = rt::random_sources();
    let (rng_h, rng_c) = rt::rng_sources();
    let (constraint_h, constraint_c) = rt::constraint_sources();
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
        ("llg_constraint.h", constraint_h),
        ("llg_constraint.c", constraint_c),
        ("llg_co.h", coroutine_h),
        ("llg_co.c", coroutine_c),
        ("vpi_user.h", vpi_h),
        ("sv_vpi_user.h", rt::sv_vpi_header()),
        ("llg_vpi.h", vpi_bridge_h),
        ("llg_vpi.c", vpi_c),
        ("llg_dpi_context.c", rt::foreign_context_source()),
        ("llg_container.h", container_h),
        ("llg_container.c", container_c),
        ("llg_string.h", string_h),
        ("llg_string.c", string_c),
        ("svdpi.h", rt::svdpi_header()),
    ];
    files.extend_from_slice(rt::platform_headers());
    files.extend_from_slice(rt::value_backend_sources(config.backend));
    files.extend_from_slice(extra);

    for (name, content) in &files {
        writer.write(&out_dir.join(name), content)?;
    }
    Ok(())
}
