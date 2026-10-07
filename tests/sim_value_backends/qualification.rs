//! V09 dependency qualification: GMP limb-type adaptation, relocated source
//! exports and runtime-cache isolation between external GMP installations.
use super::{configs, find_archive_optional, options, sim_harness, test_gmp_override, PROBE};
use llg::sim::{
    build,
    value_backend::{CompactKernel, ValueBackend, ValueConfig},
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Wide multiply (low-half path below `LLG_SV4_MUL_FULL_THRESHOLD`, full
/// product at 129 words), divide, modulo and multi-limb decimal formatting.
/// Every GMP kernel entry point is reached; `q*c + r == a` is checked in C and
/// the printed digits are compared across backends and kernels by the caller.
const WIDE_PROBE: &str = r#"
#include "llg_value.h"
#include <stdio.h>
#include <string.h>
static sv4_t pattern(uint32_t width, uint64_t seed, uint32_t words) {
    sv4_t acc = sv4_from_u64(0, width, 0);
    for (uint32_t k = 0; k < words; ++k) {
        sv4_t word = sv4_from_u64(seed * (k + 1u) ^ (UINT64_C(0x9e3779b97f4a7c15) * k), width, 0);
        sv4_t count = sv4_from_u64(64u * k, 32, 0);
        sv4_t shifted = sv4_shl(word, count);
        sv4_t next = sv4_xor(acc, shifted);
        sv4_destroy(&word); sv4_destroy(&count); sv4_destroy(&shifted); sv4_destroy(&acc);
        acc = next;
    }
    return acc;
}
static char text[3][4096];
static void show(const char* name, sv4_t v, int slot) {
    sv4_to_dec_string(v, text[slot], sizeof text[slot]);
    printf("%s=%s\n", name, text[slot]);
}
int main(void) {
    llg_value_require_abi();
    sv4_t a = pattern(8256, UINT64_C(0xd1b54a32d192ed03), 129);
    sv4_t b = pattern(8256, UINT64_C(0x8cb92ba72f3d8dd7), 129);
    sv4_t c = pattern(8256, UINT64_C(0xa0761d6478bd642f), 70);
    sv4_t full = sv4_mul(a, b);
    sv4_t q = sv4_div(a, c), r = sv4_mod(a, c);
    sv4_t qc = sv4_mul(q, c);
    sv4_t back = sv4_add(qc, r);
    sv4_t n1 = pattern(1000, UINT64_C(0xe7037ed1a0b428db), 16);
    sv4_t n2 = pattern(1000, UINT64_C(0x589965cc75374cc3), 16);
    sv4_t low = sv4_mul(n1, n2);
    show("full", full, 0);
    show("quotient", q, 0);
    show("remainder", r, 0);
    show("low", low, 0);
    show("a", a, 1);
    show("back", back, 2);
    if (strcmp(text[1], text[2]) != 0)
        return 3;
    sv4_destroy(&low); sv4_destroy(&n2); sv4_destroy(&n1); sv4_destroy(&back);
    sv4_destroy(&qc); sv4_destroy(&r); sv4_destroy(&q); sv4_destroy(&full);
    sv4_destroy(&c); sv4_destroy(&b); sv4_destroy(&a);
    return 0;
}
"#;

pub(super) const GMP_LIBRARIES: [&str; 3] = ["lib/libgmp.a", "lib/gmp.lib", "lib/libgmp.lib"];

/// Copy an installation's header and static library into `destination`,
/// optionally rewriting the header. Library and header stay inside the new
/// root, as `GMP_ROOT` validation requires.
fn copy_installation(
    root: &Path,
    destination: &Path,
    header: impl FnOnce(String) -> Option<String>,
) -> Option<PathBuf> {
    let library = GMP_LIBRARIES
        .iter()
        .find(|name| root.join(name).is_file())
        .expect("LLG_TEST_GMP_ROOT has a static GMP library");
    let text = std::fs::read_to_string(root.join("include/gmp.h")).unwrap();
    let text = header(text)?;
    std::fs::create_dir_all(destination.join("include")).unwrap();
    std::fs::create_dir_all(destination.join("lib")).unwrap();
    std::fs::write(destination.join("include/gmp.h"), text).unwrap();
    std::fs::copy(root.join(library), destination.join(library)).unwrap();
    Some(destination.to_owned())
}

fn gmp_options(root: &Path, cache: &Path) -> build::CmakeBuildOpts {
    build::CmakeBuildOpts {
        gmp_root: Some(root.to_owned()),
        ..options(
            ValueConfig {
                backend: ValueBackend::Compact,
                kernel: CompactKernel::Gmp,
            },
            cache,
        )
    }
}

fn run_probe(project: &Path, probe: &str, opts: &build::CmakeBuildOpts) -> Vec<u8> {
    let executable = build::build_model_cmake_with_opts(project, &[("probe.c", probe)], opts)
        .unwrap_or_else(|error| panic!("{}: {error}", project.display()));
    let output = sim_harness::run_executable_output(&executable).unwrap();
    assert!(output.status.success(), "{}: {output:?}", project.display());
    output.stdout
}

/// GMP declares 64-bit limbs as `unsigned long` on LP64 Unix and as
/// `unsigned long long` with `_LONG_LONG_LIMB` (64-bit Windows). macOS declares
/// `uint64_t` as `unsigned long long`, so its GMP limbs are a distinct C type of
/// the same size. The kernels then copy through native limb scratch instead of
/// aliasing. The bundled GMP takes the host's spelling (and, at 8256 bits, its
/// Toom and FFT paths). Given an external installation whose header has
/// undefined `_LONG_LONG_LIMB` and `unsigned long` is 64 bits (Linux, macOS),
/// defining it yields the other spelling with an identical calling convention,
/// so both the direct and the copying path run on one host. Results must equal
/// legacy and portable.
#[test]
fn component_gmp_limb_type_adapter_matches_portable_and_legacy() {
    assert!(build::cmake_available());
    let dir = sim_harness::TempDir::new("gmp-limb-types").unwrap();
    let cache = dir.path().join("cache");
    let reference = run_probe(
        &dir.path().join("legacy"),
        WIDE_PROBE,
        &options(ValueConfig::default(), &cache),
    );
    let text = String::from_utf8(reference.clone()).unwrap();
    assert_eq!(text.lines().count(), 6, "{text}");
    assert!(text.lines().all(|line| line.len() > 200), "{text}");
    let portable = run_probe(
        &dir.path().join("portable"),
        WIDE_PROBE,
        &options(
            ValueConfig {
                backend: ValueBackend::Compact,
                kernel: CompactKernel::Portable,
            },
            &cache,
        ),
    );
    assert_eq!(portable, reference, "compact/portable differs from legacy");
    let bundled = run_probe(
        &dir.path().join("gmp-bundled"),
        WIDE_PROBE,
        &build::CmakeBuildOpts {
            gmp_root: None,
            ..options(
                ValueConfig {
                    backend: ValueBackend::Compact,
                    kernel: CompactKernel::Gmp,
                },
                &cache,
            )
        },
    );
    assert_eq!(
        bundled, reference,
        "bundled compact/GMP differs from legacy"
    );
    let Some(root) = sim_harness::test_gmp_installation("external GMP limb-type adapter") else {
        return;
    };
    let native = run_probe(
        &dir.path().join("gmp"),
        WIDE_PROBE,
        &gmp_options(&root, &cache),
    );
    assert_eq!(native, reference, "compact/GMP differs from legacy");
    let alternate = copy_installation(&root, &dir.path().join("gmp-long-long"), |header| {
        let undefined = "/* #undef _LONG_LONG_LIMB */";
        (header.contains(undefined)
            && header.contains("#define GMP_LIMB_BITS                      64")
            && cfg!(not(windows)))
        .then(|| header.replacen(undefined, "#define _LONG_LONG_LIMB 1", 1))
    });
    let Some(alternate) = alternate else {
        eprintln!("host GMP header already uses long long limbs; one limb spelling exercised");
        return;
    };
    let adapted = run_probe(
        &dir.path().join("gmp-long-long-model"),
        WIDE_PROBE,
        &gmp_options(&alternate, &cache),
    );
    assert_eq!(
        adapted, reference,
        "alternate limb spelling differs from legacy"
    );
}

/// A `--gen-only` export is moved away from where it was generated before it
/// is configured and built, so the project cannot depend on its original
/// location. The bundled GMP travels with the export; an external GMP stays
/// a dependency named by absolute path.
#[test]
fn component_relocated_source_exports_build_outside_their_tree() {
    assert!(build::cmake_available());
    let dir = sim_harness::TempDir::new("value-relocated").unwrap();
    let cache = dir.path().join("cache");
    for (index, config) in configs().into_iter().enumerate() {
        let original = dir.path().join(format!("generated-{index}"));
        build::generate_model_sources_with_opts(
            &original,
            &[("probe.c", PROBE)],
            &options(config, &cache),
        )
        .unwrap();
        let moved = dir.path().join(format!("elsewhere/copy-{index}"));
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::rename(&original, &moved).unwrap();
        assert!(!original.exists());
        let spelling = original.to_string_lossy().replace('\\', "/");
        for entry in std::fs::read_dir(&moved).unwrap().flatten() {
            let path = entry.path();
            if let Ok(text) = std::fs::read_to_string(&path) {
                assert!(
                    !text.replace('\\', "/").contains(&spelling),
                    "{} names its generation directory",
                    path.display()
                );
            }
        }
        let build_dir = moved.join("build");
        let mut configure = Command::new("cmake");
        configure.arg("-S").arg(&moved).arg("-B").arg(&build_dir);
        let output = sim_harness::run_command(&mut configure, Duration::from_secs(120)).unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // MSVC compiles gmp.h's plain `__inline` functions into an external
        // definition in every object that uses one, which the program link
        // rejects as duplicates; the bundled recipe makes them static copies.
        if config.kernel == CompactKernel::Gmp && test_gmp_override().is_none() {
            let header = std::fs::read_to_string(build_dir.join("llg_gmp/gmp.h")).unwrap();
            assert!(
                header.contains("#ifdef _MSC_VER\n#define __GMP_EXTERN_INLINE  static __inline\n"),
                "bundled gmp.h keeps external MSVC inline definitions"
            );
        }
        let mut compile = Command::new("cmake");
        compile
            .arg("--build")
            .arg(&build_dir)
            .args(["--config", "Release", "--parallel", "6"]);
        let output = sim_harness::run_command(&mut compile, Duration::from_secs(300)).unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let executable = ["sim", "sim.exe", "Release/sim", "Release/sim.exe"]
            .into_iter()
            .map(|name| build_dir.join("bin").join(name))
            .find(|path| path.is_file())
            .expect("relocated export executable");
        let output = sim_harness::run_executable_output(&executable).unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"323\n");
    }
}

fn ready_entries(cache: &Path) -> Vec<PathBuf> {
    let mut entries = std::fs::read_dir(cache)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|entry| entry.join("ready").is_file())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

/// Runtime archives link GMP, so two installations whose header or library
/// bytes differ must never share an archive, while a byte-identical copy at
/// another path reuses it.
#[test]
fn component_distinct_gmp_installations_use_distinct_runtime_archives() {
    assert!(build::cmake_available());
    let Some(root) = sim_harness::test_gmp_installation("GMP runtime cache isolation") else {
        return;
    };
    let dir = sim_harness::TempDir::new("gmp-cache-isolation").unwrap();
    let cache = dir.path().join("cache");
    let same = copy_installation(&root, &dir.path().join("gmp-same"), Some).unwrap();
    let edited = copy_installation(&root, &dir.path().join("gmp-edited"), |header| {
        Some(header + "\n/* distinct installation */\n")
    })
    .unwrap();
    assert_eq!(
        run_probe(&dir.path().join("a"), PROBE, &gmp_options(&root, &cache)),
        b"323\n"
    );
    let first = ready_entries(&cache);
    assert_eq!(first.len(), 1);
    assert_eq!(
        run_probe(&dir.path().join("b"), PROBE, &gmp_options(&same, &cache)),
        b"323\n"
    );
    assert_eq!(
        ready_entries(&cache),
        first,
        "identical bytes reuse the archive"
    );
    assert_eq!(
        run_probe(&dir.path().join("c"), PROBE, &gmp_options(&edited, &cache)),
        b"323\n"
    );
    let entries = ready_entries(&cache);
    assert_eq!(entries.len(), 2, "edited GMP must not reuse {first:?}");
    let archives = entries
        .iter()
        .map(|entry| find_archive_optional(entry).expect("runtime archive"))
        .collect::<Vec<_>>();
    assert_ne!(archives[0], archives[1]);
}
