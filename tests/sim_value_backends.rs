//! Build integration components for experimental compile-time value selection.
use llg::sim::{
    build,
    value_backend::{CompactKernel, ValueBackend, ValueConfig},
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::sim_cli;
use crate::sim_harness;

const PROBE: &str = r#"
#include "llg_value.h"
#include <stdio.h>
int main(void) {
    llg_value_require_abi();
    sv4_t a = sv4_from_u64(17, 129, 0), b = sv4_from_u64(19, 129, 0);
    sv4_t product = sv4_mul(a, b);
    sv4_t count = sv4_from_u64(1, 129, 0);
    sv4_t shifted = sv4_shl(product, count);
    sv4_t ones = sv4_countones(product);
    sv4_t inside = sv4_inside_range(product, a, product);
    if (sv4_to_u64(shifted) != 646 || sv4_to_u64(ones) != 4 ||
        !sv4_to_bool(inside)) return 1;
    printf("%llu\n", (unsigned long long)sv4_to_u64(product));
    sv4_destroy(&inside); sv4_destroy(&ones); sv4_destroy(&shifted); sv4_destroy(&count);
    sv4_destroy(&product); sv4_destroy(&a); sv4_destroy(&b);
    return 0;
}
"#;

fn configs() -> Vec<ValueConfig> {
    vec![
        ValueConfig::default(),
        ValueConfig {
            backend: ValueBackend::Compact,
            kernel: CompactKernel::Portable,
        },
        ValueConfig {
            backend: ValueBackend::Compact,
            kernel: CompactKernel::Gmp,
        },
    ]
}

/// The external GMP override under test, if any; otherwise GMP kernels use
/// the bundled sources.
fn test_gmp_override() -> Option<PathBuf> {
    Some(sim_harness::test_gmp_root())
        .filter(|root| !root.is_empty())
        .map(PathBuf::from)
}

fn options(config: ValueConfig, cache: &Path) -> build::CmakeBuildOpts {
    build::CmakeBuildOpts {
        value_config: config,
        gmp_root: test_gmp_override(),
        runtime_cache_dir: Some(cache.to_owned()),
        build_jobs: Some(6),
        ..Default::default()
    }
}

fn find_archive(root: &Path) -> PathBuf {
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .is_some_and(|name| name == "libllg_runtime.a" || name == "llg_runtime.lib")
        {
            return path;
        }
        if path.is_dir() {
            if let Some(archive) = find_archive_optional(&path) {
                return archive;
            }
        }
    }
    panic!("runtime archive missing in {}", root.display());
}

fn find_archive_optional(root: &Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .is_some_and(|name| name == "libllg_runtime.a" || name == "llg_runtime.lib")
        {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(archive) = find_archive_optional(&path) {
                return Some(archive);
            }
        }
    }
    None
}

#[test]
fn component_selected_exports_build_and_wrong_backend_archives_fail() {
    assert!(build::cmake_available());
    let dir = sim_harness::TempDir::new("value-backends").unwrap();
    let cache = dir.path().join("cache");
    let mut archives = Vec::new();
    for (index, config) in configs().into_iter().enumerate() {
        let project = dir.path().join(format!("project-{index}"));
        let opts = options(config, &cache);
        let executable =
            build::build_model_cmake_with_opts(&project, &[("probe.c", PROBE)], &opts).unwrap();
        let output = sim_harness::run_executable_output(&executable).unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(output.stdout, b"323\n");
        let exported = dir.path().join(format!("export-{index}"));
        build::generate_model_sources_with_opts(&exported, &[("probe.c", PROBE)], &opts).unwrap();
        let mut configure = Command::new("cmake");
        configure
            .arg("-S")
            .arg(&exported)
            .arg("-B")
            .arg(exported.join("build"));
        let output = sim_harness::run_command(&mut configure, Duration::from_secs(60)).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut compile = Command::new("cmake");
        compile
            .arg("--build")
            .arg(exported.join("build"))
            .args(["--parallel", "6"]);
        let output = sim_harness::run_command(&mut compile, Duration::from_secs(180)).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let executable = ["sim", "sim.exe", "Debug/sim", "Debug/sim.exe"]
            .into_iter()
            .map(|name| exported.join("build/bin").join(name))
            .find(|path| path.is_file())
            .expect("clean exported executable");
        let output = sim_harness::run_executable_output(&executable).unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"323\n");
        let entries = std::fs::read_dir(&cache)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|entry| entry.join("ready").is_file())
            .collect::<Vec<_>>();
        let entry = entries
            .iter()
            .find(|entry| {
                !archives.iter().any(|(_, old): &(PathBuf, PathBuf)| {
                    find_archive_optional(entry).as_ref() == Some(old)
                })
            })
            .unwrap();
        archives.push((project, find_archive(entry)));
    }
    if cfg!(unix) {
        let mixed = [
            (0, 1, 0, 0, "llg_value_v4"),
            (1, 0, 1, 0, "llg_value_v5"),
            (1, 2, 1, 0, "llg_value_v5_b1_k0"),
            (2, 1, 1, 1, "llg_value_v5_b1_k1"),
        ];
        for (source, library, backend, kernel, guard) in mixed {
            let mut compiler = Command::new("cc");
            compiler
                .args(["-std=c11", "-DLLG_VALUE_BUILD_CONFIG=1"])
                .arg(format!("-DLLG_SV4_USE_GMP={backend}"))
                .arg(format!("-DLLG_SV4_GMP_KERNELS={kernel}"))
                .arg("-I")
                .arg(&archives[source].0)
                .arg(archives[source].0.join("probe.c"))
                .arg(&archives[library].1)
                .args(["-lm", "-o"])
                .arg(dir.path().join(format!("wrong-{source}")));
            // The bundled GMP is inside the runtime archive; an override is not.
            if let Some(root) = test_gmp_override().filter(|_| library == 2) {
                compiler.arg(root.join("lib/libgmp.a"));
            }
            let output = sim_harness::run_command(&mut compiler, Duration::from_secs(60)).unwrap();
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(guard),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    let project = &archives[0].0;
    build::generate_model_sources_with_opts(
        project,
        &[("probe.c", PROBE)],
        &options(
            ValueConfig {
                backend: ValueBackend::Compact,
                kernel: CompactKernel::Portable,
            },
            &cache,
        ),
    )
    .unwrap();
    assert!(!project.join("value/backend.h").exists());
    build::generate_model_sources(project, &[("probe.c", PROBE)]).unwrap();
    assert!(!project.join("value_gmp").exists());
}

#[test]
fn component_invalid_c_selectors_and_missing_gmp_are_rejected() {
    let dir = sim_harness::TempDir::new("value-invalid").unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sim/rt");
    std::fs::write(dir.path().join("probe.c"), "#include \"llg_value.h\"\n").unwrap();
    for name in ["LLG_SV4_USE_GMP", "LLG_SV4_GMP_KERNELS"] {
        for selector in ["2", "true", "00", "1u", "-1", "(1)"] {
            let output = Command::new("cc")
                .args(["-std=c11", "-fsyntax-only", "-I"])
                .arg(&root)
                .arg(format!("-D{name}={selector}"))
                .arg(dir.path().join("probe.c"))
                .output()
                .unwrap();
            assert!(!output.status.success(), "accepted {name}={selector}");
        }
    }
    let opts = build::CmakeBuildOpts {
        value_config: ValueConfig {
            backend: ValueBackend::Compact,
            kernel: CompactKernel::Gmp,
        },
        gmp_root: Some(dir.path().join("absent")),
        ..Default::default()
    };
    let error = build::generate_model_sources_with_opts(&dir.path().join("missing"), &[], &opts)
        .unwrap_err();
    assert!(error.to_string().contains("GMP_ROOT"));
    assert!(!dir.path().join("missing").exists());
    let legacy = dir.path().join("legacy");
    build::generate_model_sources_with_opts(
        &legacy,
        &[],
        &build::CmakeBuildOpts {
            gmp_root: Some(dir.path().join("absent")),
            ..Default::default()
        },
    )
    .unwrap();
    let cmake = std::fs::read_to_string(legacy.join("CMakeLists.txt")).unwrap();
    assert!(!cmake.contains("gmp.h") && !cmake.contains("CheckCSourceRuns"));
}

#[test]
fn legacy_hdl_subset_matches_independent_outputs() {
    sim_cli::run_case("value_backends", "implemented", "product=323\n", "", &[]);
    sim_cli::run_case("value_backends", "missing_shift", "shift=34\n", "", &[]);
    sim_cli::run_case("value_backends", "missing_select", "part=19\n", "", &[]);
}

#[test]
fn compact_hdl_subset_matches_legacy_and_independent_outputs() {
    sim_cli::run_case_backend_parity("value_backends", "implemented", "product=323\n", &[], &[]);
    sim_cli::run_case_backend_parity("value_backends", "missing_shift", "shift=34\n", &[], &[]);
    sim_cli::run_case_backend_parity("value_backends", "missing_select", "part=19\n", &[], &[]);
}

#[test]
fn component_invalid_driver_selector_is_rejected() {
    let output = sim_cli::invoke_with_env(
        "value_backends",
        "implemented",
        true,
        &["--gen-only"],
        &[("LLG_VALUE_BACKEND", "invalid")],
        &[],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid LLG_VALUE_BACKEND"));
}

#[test]
fn component_gmp_header_library_and_limb_mismatches_fail_configure() {
    let Some(root) = sim_harness::test_gmp_installation("GMP dependency witnesses") else {
        return;
    };
    let dir = sim_harness::TempDir::new("gmp-mismatch").unwrap();
    let header = std::fs::read_to_string(root.join("include/gmp.h")).unwrap();
    for (index, macro_name, replacement) in [
        (0, "__GNU_MP_VERSION", "99"),
        (1, "GMP_NAIL_BITS", "1"),
        (2, "GMP_LIMB_BITS", "32"),
    ] {
        let installation = dir.path().join(format!("gmp-{index}"));
        std::fs::create_dir_all(installation.join("include")).unwrap();
        std::fs::create_dir_all(installation.join("lib")).unwrap();
        let mut changed = false;
        let header = header
            .lines()
            .map(|line| {
                let words = line.split_whitespace().collect::<Vec<_>>();
                if words.len() >= 3 && words[0] == "#define" && words[1] == macro_name {
                    changed = true;
                    format!("#define {macro_name} {replacement}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(changed, "missing header macro {macro_name}");
        std::fs::write(installation.join("include/gmp.h"), header).unwrap();
        let library = qualification::GMP_LIBRARIES
            .iter()
            .find(|name| root.join(name).is_file())
            .expect("static GMP library");
        std::fs::copy(root.join(library), installation.join(library)).unwrap();
        let project = dir.path().join(format!("project-{index}"));
        let opts = build::CmakeBuildOpts {
            value_config: ValueConfig {
                backend: ValueBackend::Compact,
                kernel: CompactKernel::Gmp,
            },
            gmp_root: Some(installation),
            ..Default::default()
        };
        build::generate_model_sources_with_opts(&project, &[("probe.c", PROBE)], &opts).unwrap();
        let output = Command::new("cmake")
            .arg("-S")
            .arg(&project)
            .arg("-B")
            .arg(project.join("build"))
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("GMP_ROOT headers/library mismatch"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[path = "sim_value_backends/parity.rs"]
mod parity;
#[path = "sim_value_backends/qualification.rs"]
mod qualification;
