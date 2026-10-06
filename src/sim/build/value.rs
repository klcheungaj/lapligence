use super::{BuildError, CmakeBuildOpts};
use crate::sim::value_backend::{CompactKernel, ValueBackend};
use std::path::PathBuf;

/// Where compact GMP kernels get GMP.
enum GmpInput {
    /// The `vendor/gmp` subset llg writes under `gmp/` and compiles into the
    /// runtime (see [`crate::sim::rt::gmp`]).
    Bundled,
    /// An installation named by `gmp_root` or `GMP_ROOT`.
    External(ExternalGmp),
}

struct ExternalGmp {
    include: PathBuf,
    library: PathBuf,
    identity: String,
}

impl GmpInput {
    fn identity(&self) -> String {
        match self {
            Self::Bundled => format!(
                "gmp-bundled-{:016x}",
                crate::sim::rt::gmp::bundled_gmp_identity()
            ),
            Self::External(input) => input.identity.clone(),
        }
    }
}

fn gmp_input(opts: &CmakeBuildOpts) -> Result<Option<GmpInput>, BuildError> {
    // An empty `GMP_ROOT` selects the bundled sources, like an unset one.
    let env_root = std::env::var_os("GMP_ROOT").filter(|root| !root.is_empty());
    gmp_input_with(opts, env_root.map(PathBuf::from))
}

/// [`gmp_input`] with the `GMP_ROOT` environment value supplied by the caller.
fn gmp_input_with(
    opts: &CmakeBuildOpts,
    env_root: Option<PathBuf>,
) -> Result<Option<GmpInput>, BuildError> {
    opts.value_config
        .validate()
        .map_err(BuildError::InvalidValueConfig)?;
    if opts.value_config.kernel != CompactKernel::Gmp {
        return Ok(None);
    }
    let Some(root) = opts.gmp_root.clone().or(env_root) else {
        return Ok(Some(GmpInput::Bundled));
    };
    external_gmp(root).map(|input| Some(GmpInput::External(input)))
}

/// Whether the build compiles the bundled GMP sources, which the caller must
/// then write with [`crate::sim::rt::write_bundled_gmp_sources`].
pub(super) fn uses_bundled_gmp(opts: &CmakeBuildOpts) -> Result<bool, BuildError> {
    Ok(matches!(gmp_input(opts)?, Some(GmpInput::Bundled)))
}

fn external_gmp(root: PathBuf) -> Result<ExternalGmp, BuildError> {
    let root = crate::ffi::platform::canonicalize(&root).map_err(|error| {
        BuildError::InvalidValueConfig(format!("invalid GMP_ROOT {}: {error}", root.display()))
    })?;
    let header = root.join("include/gmp.h");
    let library = [
        "lib/libgmp.a",
        "lib64/libgmp.a",
        "lib/libgmp.so",
        "lib64/libgmp.so",
        "lib/libgmp.dylib",
        "lib/gmp.lib",
        "lib/libgmp.lib",
    ]
    .iter()
    .map(|name| root.join(name))
    .find(|path| path.is_file())
    .ok_or_else(|| {
        BuildError::InvalidValueConfig(format!(
            "GMP_ROOT {} has no GMP library; system fallback is disabled",
            root.display()
        ))
    })?;
    let mut hash = 0xcbf29ce484222325u64;
    for path in [&header, &library] {
        let canonical = crate::ffi::platform::canonicalize(path).map_err(|error| {
            BuildError::InvalidValueConfig(format!("missing GMP input {}: {error}", path.display()))
        })?;
        if !canonical.starts_with(&root) {
            return Err(BuildError::InvalidValueConfig(format!(
                "GMP input {} escapes explicit GMP_ROOT",
                path.display()
            )));
        }
        let bytes = std::fs::read(&canonical).map_err(|source| BuildError::Io {
            action: "read GMP identity",
            path: canonical,
            source,
        })?;
        for byte in bytes {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    Ok(ExternalGmp {
        include: root.join("include"),
        library,
        identity: format!("gmp64-nail0-{hash:016x}"),
    })
}

pub(super) fn identity(opts: &CmakeBuildOpts) -> Result<String, BuildError> {
    let gmp = gmp_input(opts)?;
    Ok(format!(
        "v{}-b{}-k{}-{}",
        opts.value_config.backend.abi(),
        opts.value_config.backend.selector(),
        opts.value_config.kernel.selector(),
        gmp.map_or_else(|| "portable".into(), |input| input.identity())
    ))
}

pub(super) fn guard_header(opts: &CmakeBuildOpts) -> Result<String, BuildError> {
    let identity = identity(opts)?;
    Ok(format!("/* Selected value build identity: {identity}. Regenerate with the model. */\n#ifndef LLG_VALUE_BUILD_H\n#define LLG_VALUE_BUILD_H\n#if LLG_SV4_USE_GMP != {} || LLG_SV4_GMP_KERNELS != {}\n#error \"translation unit value selection differs from generated build\"\n#endif\n#define LLG_VALUE_LINK_GUARD llg_value_{}\n#endif\n", opts.value_config.backend.selector(), opts.value_config.kernel.selector(), identity.replace('-', "_")))
}

fn cmake_path(path: &std::path::Path) -> Result<String, BuildError> {
    let value = path.to_string_lossy().replace('\\', "/");
    if value.contains(['"', ';', '\n', '\r', '$']) {
        return Err(BuildError::InvalidValueConfig(format!(
            "GMP path cannot be represented in CMake: {}",
            path.display()
        )));
    }
    Ok(value)
}

pub(super) fn cmake_setup(opts: &CmakeBuildOpts, target: &str) -> Result<String, BuildError> {
    let gmp = gmp_input(opts)?;
    let mut out = format!("add_compile_definitions(LLG_SV4_USE_GMP={} LLG_SV4_GMP_KERNELS={} LLG_VALUE_BUILD_CONFIG=1)\n", opts.value_config.backend.selector(), opts.value_config.kernel.selector());
    if opts.value_config.backend == ValueBackend::Compact {
        out.push_str("# Compact operations use prefixed symbols; unavailable operations fail at link time.\n");
    }
    match gmp {
        None => {}
        Some(GmpInput::Bundled) => out.push_str(&bundled_cmake_setup(target)),
        Some(GmpInput::External(input)) => out.push_str(&external_cmake_setup(&input, target)?),
    }
    Ok(out)
}

/// Compile the bundled subset into the runtime library. A model project
/// builds it only when it builds the runtime itself (no cached
/// `LLG_RUNTIME_LIBRARY`); the cached archive already contains the objects.
fn bundled_cmake_setup(target: &str) -> String {
    let setup = r#"set(LLG_GMP_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/gmp")
set(LLG_GMP_TABLE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/gmp/generated")
include("${CMAKE_CURRENT_SOURCE_DIR}/gmp/llg_gmp.cmake")
target_sources(llg_runtime PRIVATE $<TARGET_OBJECTS:llg_gmp>)
target_include_directories(llg_runtime PRIVATE ${LLG_GMP_INCLUDE_DIRS})
"#;
    if target == "llg_runtime" {
        setup.to_owned()
    } else {
        format!("if(NOT LLG_RUNTIME_LIBRARY)\n{setup}endif()\n")
    }
}

fn external_cmake_setup(input: &ExternalGmp, target: &str) -> Result<String, BuildError> {
    let include = cmake_path(&input.include)?;
    let library = cmake_path(&input.library)?;
    Ok(format!(
        r#"set(CMAKE_REQUIRED_INCLUDES "{include}")
set(CMAKE_REQUIRED_LIBRARIES "{library}")
include(CheckCSourceRuns)
unset(LLG_GMP_COMPATIBLE CACHE)
check_c_source_runs([=[
#include <gmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
_Static_assert(GMP_LIMB_BITS == 64 && GMP_NAIL_BITS == 0, "64-bit nail-free GMP required");
_Static_assert(sizeof(mp_limb_t) == 8, "64-bit GMP required");
int main(void) {{
  char version[64];
  snprintf(version, sizeof(version), "%d.%d.%d", __GNU_MP_VERSION, __GNU_MP_VERSION_MINOR, __GNU_MP_VERSION_PATCHLEVEL);
  if (strcmp(version, gmp_version)) return 1;
  mp_limb_t a[2] = {{7, 0}}, b[2] = {{3, 0}}, p[4], q[2], r[2];
  mpn_mul_n(p, a, b, 2);
  mpn_mul_1(p, a, 2, 3);
  mpn_addmul_1(p, b, 2, 2);
  mpn_tdiv_qr(q, r, 0, a, 2, b, 1);
  unsigned char digits[128];
  return !mpn_get_str(digits, 10, a, 1) || q[0] != 2 || r[0] != 1;
}}
]=] LLG_GMP_COMPATIBLE)
if(NOT LLG_GMP_COMPATIBLE)
  message(FATAL_ERROR "GMP_ROOT headers/library mismatch, unavailable mpn APIs, or unsupported limbs: require 64-bit nail-free GMP (see CMakeConfigureLog.yaml)")
endif()
target_include_directories({target} PRIVATE "{include}")
target_link_libraries({target} {visibility} "{library}")
"#,
        visibility = if target == "llg_runtime" {
            "PUBLIC"
        } else {
            "PRIVATE"
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::value_backend::ValueConfig;

    fn installation(base: &std::path::Path, name: &str, header: &str) -> PathBuf {
        let root = base.join(name);
        std::fs::create_dir_all(root.join("include")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(root.join("include/gmp.h"), header).unwrap();
        std::fs::write(root.join("lib/libgmp.a"), "archive").unwrap();
        root
    }

    #[test]
    fn gmp_identity_follows_installation_bytes_not_location() {
        let base = std::env::temp_dir().join(format!("llg-gmp-identity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let options = |root: &PathBuf| CmakeBuildOpts {
            value_config: ValueConfig {
                backend: ValueBackend::Compact,
                kernel: CompactKernel::Gmp,
            },
            gmp_root: Some(root.clone()),
            ..Default::default()
        };
        let first = identity(&options(&installation(&base, "a", "header"))).unwrap();
        let moved = identity(&options(&installation(&base, "b", "header"))).unwrap();
        let edited = identity(&options(&installation(&base, "c", "header\n"))).unwrap();
        assert_eq!(first, moved);
        assert_ne!(first, edited);
        assert!(first.starts_with("v5-b1-k1-gmp64-nail0-"), "{first}");
        let portable = identity(&CmakeBuildOpts {
            value_config: ValueConfig {
                backend: ValueBackend::Compact,
                kernel: CompactKernel::Portable,
            },
            gmp_root: Some(base.join("a")),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(portable, "v5-b1-k0-portable");
        std::fs::remove_file(base.join("a/lib/libgmp.a")).unwrap();
        let missing = identity(&options(&base.join("a"))).unwrap_err();
        assert!(missing.to_string().contains("system fallback is disabled"));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn gmp_kernels_default_to_the_bundled_sources() {
        let options = CmakeBuildOpts {
            value_config: ValueConfig {
                backend: ValueBackend::Compact,
                kernel: CompactKernel::Gmp,
            },
            ..Default::default()
        };
        let input = gmp_input_with(&options, None).unwrap().unwrap();
        assert!(matches!(input, GmpInput::Bundled));
        let identity = input.identity();
        assert_eq!(
            identity,
            format!(
                "gmp-bundled-{:016x}",
                crate::sim::rt::gmp::bundled_gmp_identity()
            )
        );
        let setup = bundled_cmake_setup("llg_runtime");
        assert!(setup.contains("include(\"${CMAKE_CURRENT_SOURCE_DIR}/gmp/llg_gmp.cmake\")"));
        assert!(setup.contains("$<TARGET_OBJECTS:llg_gmp>"));
        // A model links the cached runtime archive, which already holds GMP.
        assert!(bundled_cmake_setup("sim").starts_with("if(NOT LLG_RUNTIME_LIBRARY)\n"));
        // An explicit root still selects an installation.
        let missing = gmp_input_with(&options, Some("/nonexistent-llg-gmp".into()));
        assert!(missing.is_err_and(|error| error.to_string().contains("invalid GMP_ROOT")));
    }
}
