//! Linux/macOS stand-ins for MSVC `/W4 /WX` diagnostics that GCC and Clang
//! accept at `-Wall -Wextra`, on the runtime C, the standalone probes and the
//! C fixtures that Windows CI builds with `/W4 /WX`. Each check names the CI
//! failure that motivated it; the platform rules are in
//! `docs/cross_platform.md` (Windows, MSVC compiler).
//!
//! - Unreachable code (C4702): MSVC at `/O2` inlines a `static` helper that
//!   ends in `abort()` and then reports a statement after an unconditional
//!   call to it as unreachable (CI round 6: `llg_constraint.c(744)` and
//!   `(7643)`). Clang's `-Wmissing-noreturn` names a helper that never
//!   returns but is not declared `_Noreturn`, and
//!   `-Wunreachable-code-aggressive` then names a statement after a call to
//!   it, which together approximate C4702 (Clang does not report dead code
//!   that comes from a macro expansion, such as `return NULL;`).
//! - Implicit narrowing (C4244, C4267) and possibly uninitialized locals
//!   (C4701, C4703): Clang's conversion and uninitialized-use warnings, see
//!   [`MSVC_W4_FLOW_ERRORS`].
//! - Truncating constant casts (C4310): a source scan, since no GCC or Clang
//!   warning covers explicit casts.
//! - Platform-width integer types: a source scan for `int`, `unsigned`,
//!   `short`, `long` and `long long` in the runtime, since `long` is 32-bit
//!   under MSVC and every other host compiler makes it 64-bit, and the runtime
//!   states every width (`int32_t`, `uint32_t`, ...).
//!
//! The compiler checks are skipped without a Clang driver and on Windows,
//! where the same sources build with MSVC itself.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::{c_compiler, sim_harness};

/// A Clang driver: the host compiler when it is Clang, else `clang` on PATH.
fn clang_driver() -> Option<String> {
    if cfg!(windows) {
        return None;
    }
    let host = c_compiler::host_c_compiler();
    if matches!(
        c_compiler::cmake_compiler_id(&host),
        Some("Clang" | "AppleClang")
    ) {
        return Some(host);
    }
    (c_compiler::c_compiler_available("clang")
        && c_compiler::cmake_compiler_id("clang").is_some_and(|id| id != "GNU"))
    .then(|| "clang".to_owned())
}

fn check_unreachable_code(clang: &str, directory: &Path, source: &str) {
    let mut command = Command::new(clang);
    command.current_dir(directory).args([
        "-std=c11",
        "-fsyntax-only",
        "-I.",
        "-Werror=missing-noreturn",
        "-Werror=unreachable-code-aggressive",
        source,
    ]);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
        .unwrap_or_else(|error| panic!("run `{clang}` on {source}: {error}"));
    assert!(
        output.status.success(),
        "{source}: a helper that never returns must be declared _Noreturn and no statement may \
         follow an unconditional call to it (MSVC /W4 C4702):\n{}",
        c_compiler::compiler_diagnostics(&output)
    );
}

/// Every runtime source a standalone `/W4 /WX` probe compiles: the values
/// (legacy backend, as the probes use), RNG, strings, containers, random
/// service and constraint solver.
#[test]
fn probe_built_runtime_sources_have_no_code_after_fatal_helpers() {
    let Some(clang) = clang_driver() else {
        eprintln!("SKIP: no Clang driver to approximate MSVC C4702");
        return;
    };
    let dir = sim_harness::TempDir::new("runtime-strict-sources").expect("create temp directory");
    sim_harness::write_value_backend_sources(dir.path());
    sim_harness::write_compiler_header(dir.path());
    let (value_header, value_source) = llg::sim::rt::value_sources();
    let (rng_header, rng_source) = llg::sim::rt::rng_sources();
    let (string_header, string_source) = llg::sim::rt::string_sources();
    let (container_header, container_source) = llg::sim::rt::container_sources();
    let (random_header, random_source) = llg::sim::rt::random_sources();
    let (constraint_header, constraint_source) = llg::sim::rt::constraint_sources();
    let files = [
        ("llg_value.h", value_header),
        ("llg_value.c", value_source),
        ("llg_rng.h", rng_header),
        ("llg_rng.c", rng_source),
        ("llg_string.h", string_header),
        ("llg_string.c", string_source),
        ("llg_container.h", container_header),
        ("llg_container.c", container_source),
        ("llg_random.h", random_header),
        ("llg_random.c", random_source),
        ("llg_constraint.h", constraint_header),
        ("llg_constraint.c", constraint_source),
    ];
    for (name, contents) in files {
        std::fs::write(dir.path().join(name), contents).expect("write runtime source");
    }
    for source in files.iter().map(|(name, _)| *name) {
        if source.ends_with(".c") {
            check_unreachable_code(&clang, dir.path(), source);
        }
    }
}

/// Clang errors that approximate MSVC's implicit-conversion and
/// uninitialized-use warnings at `/W4`: C4244 "conversion from 'int' to
/// 'int8_t', possible loss of data" (`implicit-int-conversion`,
/// `implicit-float-conversion` for double to float, `float-conversion` for
/// floating to integer), C4267 for `size_t` to a 32-bit type
/// (`shorten-64-to-32`), and C4701/C4703 "potentially uninitialized local
/// variable used" (`uninitialized`, `sometimes-uninitialized`, and
/// `conditional-uninitialized` for a local set only on some paths; CI:
/// `llg_vpi.c(4051)`). GCC and Clang accept all of them at `-Wall -Wextra`
/// without optimization. Everything else is off (`-Wno-everything` first), so
/// the check names only these classes. The uninitialized-use set reported
/// nothing else on the runtime, probes and fixtures when it was added.
const MSVC_W4_FLOW_ERRORS: &[&str] = &[
    "-Wno-everything",
    "-Werror=implicit-int-conversion",
    "-Werror=shorten-64-to-32",
    "-Werror=implicit-float-conversion",
    "-Werror=float-conversion",
    "-Werror=uninitialized",
    "-Werror=sometimes-uninitialized",
    "-Werror=conditional-uninitialized",
];

/// One `clang -fsyntax-only` run of the narrowing and uninitialized-use check.
struct NarrowingUnit {
    source: PathBuf,
    includes: Vec<PathBuf>,
    defines: &'static [&'static str],
    /// Whether `includes` names `vendor/zlib`, which then needs the
    /// definitions in [`BUNDLED_ZLIB_DEFINES`].
    bundled_zlib: bool,
}

/// The definitions model builds compile the bundled zlib with
/// (`zlib_cmake!` in `src/sim/build.rs`): prefixed symbols, and the POSIX
/// I/O its configure step would enable (this check never runs on Windows).
const BUNDLED_ZLIB_DEFINES: &[&str] = &["-DZ_PREFIX", "-DZ_HAVE_UNISTD_H"];

/// The include directories of the runtime and probe units: the runtime, libfst
/// and the bundled zlib that `fstapi.h` includes.
fn runtime_include_dirs(root: &Path) -> Vec<PathBuf> {
    ["src/sim/rt", "vendor/libfst", "vendor/zlib"]
        .iter()
        .map(|directory| root.join(directory))
        .collect()
}

/// Runtime translation units (relative to `src/sim/rt`) in every value
/// backend; the compact backend replaces `llg_value.c` with its
/// `value_gmp/*.c` units. `llg_vpi_unit.c` is written beside the check: the
/// VPI bridge, its dynamic-object layer and the DPI-C bridge form one unit,
/// in [`llg::sim::rt::vpi_sources`] order.
const RUNTIME_UNITS: &[&str] = &[
    "llg_rt.c",
    "llg_string.c",
    "llg_container.c",
    "llg_constraint.c",
    "llg_co.c",
    "llg_random.c",
    "llg_rng.c",
    "llg_wave.c",
    "llg_rt_selftest.c",
    "llg_wave_selftest.c",
];

/// Value backend and feature selections the runtime units are checked under.
/// GMP kernels (`LLG_SV4_GMP_KERNELS=1`) are left out: they need `<gmp.h>`,
/// which a test host need not have.
const RUNTIME_CONFIGURATIONS: &[&[&str]] = &[
    &[],
    &["-DLLG_WAVEFORM=1", "-DLLG_CO_DEBUG=1"],
    &[COMPACT_VALUES],
];

/// Selects compact values, the shipped value backend.
const COMPACT_VALUES: &str = "-DLLG_SV4_USE_GMP=1";

fn c_sources_under(directory: &Path) -> Vec<PathBuf> {
    let mut pending = vec![directory.to_path_buf()];
    let mut sources = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("read directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "c") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    sources
}

/// Every C file a Windows `/W4 /WX` build compiles, under Clang errors for
/// MSVC's implicit narrowing and possibly uninitialized locals (CI:
/// `llg_vpi.c(1271)` C4244 and `llg_vpi.c(4051)` C4701 each broke runtime
/// probe builds): the runtime units in each backend, the standalone probes
/// and every C fixture (DPI-C and VPI companions). The compact probes
/// (`compact_*.c`) build only inside their CMake oracle projects and are left
/// to those builds. Skipped without a Clang driver and on Windows.
///
/// Sources that include `fstapi.h` get the bundled zlib (`vendor/zlib`,
/// prefixed as model builds use it): the Linux CI test containers have no
/// system zlib headers.
#[test]
fn runtime_and_companion_c_has_no_narrowing_or_uninitialized_use() {
    let Some(clang) = clang_driver() else {
        eprintln!("SKIP: no Clang driver to approximate MSVC C4244/C4267/C4701");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = root.join("src/sim/rt");
    let runtime_includes = runtime_include_dirs(root);
    let dir = sim_harness::TempDir::new("runtime-narrowing").expect("create temp directory");
    let mut units = runtime_units(root, dir.path());
    let probes = root.join("tests/runtime_value_storage");
    for source in c_sources_under(&probes) {
        if source
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("compact_"))
        {
            continue;
        }
        units.push(NarrowingUnit {
            source,
            includes: std::iter::once(probes.clone())
                .chain(runtime_includes.iter().cloned())
                .collect(),
            defines: &[],
            bundled_zlib: true,
        });
    }
    // The facade client is built against every backend under `/W4 /WX`.
    units.push(NarrowingUnit {
        source: probes.join("facade_compare_bridge.c"),
        includes: std::iter::once(probes.clone())
            .chain(runtime_includes.iter().cloned())
            .collect(),
        defines: &["-DLLG_SV4_USE_GMP=1"],
        bundled_zlib: false,
    });
    for source in c_sources_under(&root.join("tests/fixtures")) {
        let own = source.parent().expect("fixture directory").to_path_buf();
        units.push(NarrowingUnit {
            source,
            includes: vec![own, rt.clone()],
            defines: &[],
            bundled_zlib: false,
        });
    }

    let mut arguments = vec!["-std=c11", "-fsyntax-only"];
    arguments.extend_from_slice(MSVC_W4_FLOW_ERRORS);
    let findings = check_units(&clang, root, &units, &arguments);
    assert!(
        findings.is_empty(),
        "make each narrowing conversion explicit, after checking the value fits, \
         and initialize each local on every path before its use \
         (MSVC C4244/C4267/C4701 under /W4 /WX; see docs/cross_platform.md):\n{}",
        findings.join("\n")
    );
}

/// The runtime translation units in each of [`RUNTIME_CONFIGURATIONS`], with
/// the VPI unit written to `scratch`.
fn runtime_units(root: &Path, scratch: &Path) -> Vec<NarrowingUnit> {
    let rt = root.join("src/sim/rt");
    let runtime_includes = runtime_include_dirs(root);
    let vpi_unit = scratch.join("llg_vpi_unit.c");
    std::fs::write(
        &vpi_unit,
        "#include \"llg_vpi.c\"\n#include \"llg_vpi_dynamic.c\"\n#include \"llg_vpi_runtime.c\"\n#include \"llg_vpi_services.c\"\n#include \"llg_dpi.c\"\n",
    )
    .expect("write the VPI unit");
    let mut units = Vec::new();
    for &defines in RUNTIME_CONFIGURATIONS {
        let compact = defines.contains(&COMPACT_VALUES);
        let mut sources: Vec<PathBuf> = RUNTIME_UNITS.iter().map(|unit| rt.join(unit)).collect();
        sources.push(vpi_unit.clone());
        if compact {
            sources.extend(c_sources_under(&rt.join("value_gmp")));
        } else {
            sources.push(rt.join("llg_value.c"));
        }
        units.extend(sources.into_iter().map(|source| NarrowingUnit {
            source,
            includes: runtime_includes.clone(),
            defines,
            bundled_zlib: true,
        }));
    }
    units
}

/// Compiles each unit with `compiler` and `arguments` (plus the unit's
/// definitions and include directories) on up to eight threads, and returns
/// one finding per failing unit.
fn check_units(
    compiler: &str,
    root: &Path,
    units: &[NarrowingUnit],
    arguments: &[&str],
) -> Vec<String> {
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let findings = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(unit) = units.get(index) else {
                    break;
                };
                let mut command = Command::new(compiler);
                command.args(arguments).args(unit.defines);
                if unit.bundled_zlib {
                    command.args(BUNDLED_ZLIB_DEFINES);
                }
                for include in &unit.includes {
                    command.arg("-I").arg(include);
                }
                command.arg(&unit.source);
                let output = sim_harness::run_command(&mut command, Duration::from_secs(300))
                    .unwrap_or_else(|error| panic!("run `{compiler}`: {error}"));
                if !output.status.success() {
                    findings.lock().expect("findings lock").push(format!(
                        "{} {}:\n{}",
                        unit.source
                            .strip_prefix(root)
                            .unwrap_or(&unit.source)
                            .display(),
                        unit.defines.join(" "),
                        c_compiler::compiler_diagnostics(&output)
                    ));
                }
            });
        }
    });
    findings.into_inner().expect("findings lock")
}

/// The arguments that make GCC report uninitialized uses of one unit as
/// errors under `flags`. Only the diagnostics count (these checks never run on
/// Windows).
fn uninitialized_use_arguments(flags: &[&'static str]) -> Vec<&'static str> {
    let mut arguments = vec![
        "-std=c11",
        "-Wuninitialized",
        "-Wmaybe-uninitialized",
        "-Werror=uninitialized",
        "-Werror=maybe-uninitialized",
    ];
    arguments.extend_from_slice(flags);
    arguments.extend(["-c", "-o", "/dev/null"]);
    arguments
}

/// The arguments of the GCC 11 check: the CMake Release level without the
/// jump threading of newer GCC.
fn gcc11_arguments() -> Vec<&'static str> {
    let mut flags = vec!["-O3"];
    flags.extend_from_slice(c_compiler::GCC11_MAYBE_UNINITIALIZED_FLAGS);
    uninitialized_use_arguments(&flags)
}

/// The runtime units in each backend under GCC's flow-sensitive
/// uninitialized-use errors at `-O3` (CMake Release), with the jump threading
/// that hides GCC 11's findings from GCC 13+ turned off
/// ([`c_compiler::GCC11_MAYBE_UNINITIALIZED_FLAGS`]). CI: GCC 11.5 on Rocky 9
/// and GCC 11 on Ubuntu 22.04 rejected `llg_vpi_dynamic.c:2061: 'ai' may be
/// used uninitialized` (a local filled through `decode_handle`'s inlined
/// out-parameter) while GCC 13 and 14 accepted it. Initialize such locals at
/// their declaration. Compiling to an object is needed: `-fsyntax-only` runs
/// no flow analysis. Skipped without GCC and on Windows.
#[test]
fn runtime_units_have_no_gcc11_maybe_uninitialized_use() {
    let Some(gcc) = c_compiler::gcc_driver() else {
        eprintln!("SKIP: no GNU GCC driver for the GCC 11 maybe-uninitialized check");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = sim_harness::TempDir::new("runtime-gcc11-uninit").expect("create temp directory");
    let units = runtime_units(root, dir.path());
    let findings = check_units(&gcc, root, &units, &gcc11_arguments());
    assert!(
        findings.is_empty(),
        "initialize each local at its declaration when an out-parameter call \
         fills it only on success (GCC 11 -Wmaybe-uninitialized; see \
         docs/cross_platform.md, Compiler versions):\n{}",
        findings.join("\n")
    );
}

/// GCC options of the heavier-inlining pass. More inlining exposes a local
/// filled through an out-parameter that only another compiler or target would
/// inline (CI: GCC 13 on AArch64 rejected `coverage/database.c:999 'types'`),
/// and without jump threading, dominator optimization and VRP the failure path
/// on which it looks unset stays. `-O1` with the inliners finds as much as
/// `-O3` (15 of the 15 sites of the sweep that added this, against 14) in less
/// than half the time. The last two flags undo hardening that a distribution's
/// GCC may enable by default: together, `_FORTIFY_SOURCE` and zeroed automatic
/// variables hid a site the pass otherwise reports (measured with GCC 14.2).
const HEAVY_INLINING_FLAGS: &[&str] = &[
    "-O1",
    "-finline-functions",
    "-finline-small-functions",
    "-findirect-inlining",
    "-fpartial-inlining",
    "--param=max-inline-insns-single=2000",
    "--param=max-inline-insns-auto=500",
    "--param=inline-unit-growth=200",
    "-fno-thread-jumps",
    "-fno-tree-dominator-opts",
    "-fno-tree-vrp",
    "-U_FORTIFY_SOURCE",
    "-ftrivial-auto-var-init=uninitialized",
];

/// [`c_compiler::heavy_inlining_gcc`], failing when the GCC that
/// `$LLG_TEST_HEAVY_INLINING_GCC` names cannot run the pass.
fn heavy_inlining_gcc() -> Option<String> {
    c_compiler::heavy_inlining_gcc().unwrap_or_else(|reason| panic!("{reason}"))
}

/// A second maybe-uninitialized pass over the runtime units with the inlining
/// that other compilers and targets choose ([`HEAVY_INLINING_FLAGS`]), so a
/// GCC 14 host finds sites that only their inlining shows. Only the shipped
/// configuration (compact values) is compiled: it holds every site the other
/// configurations show, and the pass is the costliest of this file (about
/// 20 s; one `llg_rt.c` compile is most of it). Skipped without GCC 14, unless
/// `$LLG_TEST_HEAVY_INLINING_GCC` asks for it, as one CI job does
/// (`tests/ci.md`).
#[test]
fn runtime_units_have_no_maybe_uninitialized_use_under_heavy_inlining() {
    let Some(gcc) = heavy_inlining_gcc() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = sim_harness::TempDir::new("runtime-inline-uninit").expect("create temp directory");
    let mut units = runtime_units(root, dir.path());
    units.retain(|unit| unit.defines.contains(&COMPACT_VALUES));
    assert!(!units.is_empty(), "no unit of the compact configuration");
    let findings = check_units(
        &gcc,
        root,
        &units,
        &uninitialized_use_arguments(HEAVY_INLINING_FLAGS),
    );
    assert!(
        findings.is_empty(),
        "initialize each local at its declaration when an out-parameter call \
         fills it only on success, and let helpers write their out-parameter \
         on every path (see docs/cross_platform.md, Compiler versions):\n{}",
        findings.join("\n")
    );
}

/// The flags name the shape they were chosen for: a local filled, on success
/// only, by a helper that GCC inlines only with the larger limits. The flags
/// of the GCC 11 check accept it.
#[test]
fn heavy_inlining_flags_report_a_local_a_large_helper_leaves_unset() {
    let Some(gcc) = heavy_inlining_gcc() else {
        return;
    };
    let dir = sim_harness::TempDir::new("runtime-inline-probe").expect("create temp directory");
    let source = dir.path().join("maybe_unset.c");
    let body: String = (0..80)
        .map(|step| format!("    total += (cursor[{step}] ^ total) * {}u;\n", step + 3))
        .collect();
    let caller = |name: &str| {
        format!(
            "uint32_t {name}(const uint8_t *cursor) {{\n    uint32_t field;\n\
             \x20   read_field(cursor, &field);\n    return sink(field);\n}}\n"
        )
    };
    std::fs::write(
        &source,
        format!(
            "#include <stdint.h>\nuint32_t sink(uint32_t value);\n\
             static int32_t read_field(const uint8_t *cursor, uint32_t *out) {{\n\
             \x20   uint32_t total = 0;\n{body}\
             \x20   if (total & 1u) return 0;\n    *out = total;\n    return 1;\n}}\n\
             {}{}",
            caller("first"),
            caller("second")
        ),
    )
    .expect("write the probe");
    let compile = |arguments: &[&str]| {
        let mut command = Command::new(&gcc);
        command.args(arguments).arg(&source);
        sim_harness::run_command(&mut command, Duration::from_secs(120))
            .unwrap_or_else(|error| panic!("run `{gcc}`: {error}"))
    };
    let accepted = compile(&gcc11_arguments());
    assert!(
        accepted.status.success(),
        "{}",
        c_compiler::compiler_diagnostics(&accepted)
    );
    let rejected = compile(&uninitialized_use_arguments(HEAVY_INLINING_FLAGS));
    let diagnostics = c_compiler::compiler_diagnostics(&rejected);
    assert!(
        !rejected.status.success() && diagnostics.contains("maybe-uninitialized"),
        "{diagnostics}"
    );
}

/// A host without a new enough GCC skips the pass; an explicit request that
/// cannot be met is an error, and never falls back to the host's GCC.
#[test]
fn heavy_inlining_gcc_selection_fails_only_an_explicit_request() {
    let major = |compiler: &str| match compiler {
        "gcc-15" => Some(15),
        "gcc-14" => Some(14),
        "gcc" | "gcc-13" => Some(13),
        _ => None,
    };
    let select = |requested: Option<&str>, host: Option<&str>| {
        c_compiler::select_heavy_inlining_gcc(
            requested.map(str::to_owned),
            || host.map(str::to_owned),
            major,
        )
    };
    assert_eq!(select(None, Some("gcc-14")).as_deref(), Ok("gcc-14"));
    assert_eq!(select(None, Some("gcc-15")).as_deref(), Ok("gcc-15"));
    assert_eq!(
        select(Some("gcc-14"), Some("gcc")).as_deref(),
        Ok("gcc-14"),
        "the request replaces an older host GCC"
    );
    assert_eq!(select(Some("gcc-14"), None).as_deref(), Ok("gcc-14"));
    for (host, found) in [(Some("gcc"), "GCC 13"), (None, "no GNU GCC")] {
        let Err(c_compiler::Unusable::Host(reason)) = select(None, host) else {
            panic!("host {host:?} must be a skip");
        };
        assert!(reason.contains(&format!("(found {found})")), "{reason}");
    }
    for (requested, found) in [("gcc-13", "GCC 13"), ("gcc-absent", "no GNU GCC")] {
        let Err(c_compiler::Unusable::Requested(reason)) = select(Some(requested), Some("gcc-14"))
        else {
            panic!("request {requested} must be an error");
        };
        assert!(
            reason.contains(&format!("names `{requested}` (found {found})")),
            "{reason}"
        );
    }
}

/// CI runs the pass in one job (`tests/ci.md`): a variable name that differs
/// between the workflow and [`c_compiler::HEAVY_INLINING_GCC_VARIABLE`] would
/// turn it into a skip.
#[test]
fn one_ci_job_names_the_heavy_inlining_gcc() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let mut passed = Vec::new();
    for entry in std::fs::read_dir(&workflows).expect("read the workflow directory") {
        let path = entry.expect("workflow entry").path();
        let text = std::fs::read_to_string(&path).expect("read a workflow");
        let option = format!("--env {}=", c_compiler::HEAVY_INLINING_GCC_VARIABLE);
        passed.extend(
            text.lines()
                .filter(|line| line.contains(&option))
                .map(|_| path.file_name().expect("file name").to_owned()),
        );
    }
    assert_eq!(passed, ["linux-platform.yml"]);
}

/// Every C fixture (DPI-C and VPI companions) under Clang at the warning
/// level `tests/support/c_compiler.rs` builds them with on GNU-like hosts,
/// `-Wall -Wextra -Werror`. Apple Clang is the host compiler on macOS, and
/// Clang's `-Wall` holds warnings GCC's lacks (CI: macOS arm64 rejected
/// `adv_019/threads.c:52: variable 'depth' set but not used
/// [-Wunused-but-set-variable]`, a loop counter only incremented, which GCC
/// accepts). Skipped without a Clang driver and on Windows.
#[test]
fn companion_c_builds_under_clang_wall_wextra() {
    let Some(clang) = clang_driver() else {
        eprintln!("SKIP: no Clang driver to approximate Apple Clang -Wall -Wextra");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = root.join("src/sim/rt");
    let units: Vec<NarrowingUnit> = c_sources_under(&root.join("tests/fixtures"))
        .into_iter()
        .map(|source| NarrowingUnit {
            includes: vec![
                source.parent().expect("fixture directory").to_path_buf(),
                rt.clone(),
            ],
            source,
            defines: &[],
            bundled_zlib: false,
        })
        .collect();
    assert!(!units.is_empty(), "no C fixtures found");
    let findings = check_units(
        &clang,
        root,
        &units,
        &["-std=c11", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"],
    );
    assert!(
        findings.is_empty(),
        "C fixtures must build under Clang -Wall -Wextra -Werror (Apple Clang on macOS; \
         see docs/cross_platform.md, macOS):\n{}",
        findings.join("\n")
    );
}

/// Integer types as wide as `unsigned int` or narrower, with their MSVC width
/// and signedness (plain `char` is signed there), including the fixed-width
/// typedefs of `vpi_user.h` and `svdpi.h` that companions cast to.
const NARROW_CAST_TYPES: &[(&str, u32, bool)] = &[
    ("uint8_t", 8, false),
    ("int8_t", 8, true),
    ("uint16_t", 16, false),
    ("int16_t", 16, true),
    ("uint32_t", 32, false),
    ("int32_t", 32, true),
    ("char", 8, true),
    ("signed char", 8, true),
    ("unsigned char", 8, false),
    ("short", 16, true),
    ("unsigned short", 16, false),
    ("int", 32, true),
    ("unsigned", 32, false),
    ("unsigned int", 32, false),
    ("PLI_BYTE8", 8, true),
    ("PLI_UBYTE8", 8, false),
    ("PLI_INT16", 16, true),
    ("PLI_UINT16", 16, false),
    ("PLI_INT32", 32, true),
    ("PLI_UINT32", 32, false),
    ("svScalar", 8, false),
    ("svBit", 8, false),
    ("svLogic", 8, false),
    ("svBitVecVal", 32, false),
];

/// The value of a C integer literal (`0x..`, octal `0..` or decimal, with
/// any `u`/`l` suffix), or `None` for anything else.
fn integer_literal(token: &str) -> Option<u128> {
    let digits = token.trim_end_matches(['u', 'U', 'l', 'L']);
    if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        return u128::from_str_radix(hex, 16).ok();
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return u128::from_str_radix(&digits[1..], 8).ok();
    }
    digits.parse().ok()
}

/// Whether MSVC reports a cast of the literal `value` (negated when
/// `negative`) to a `bits`-wide type as truncating. A 32-bit target takes
/// any 32-bit pattern, so only wider literals truncate there; a narrower
/// target must hold the value itself. A negated literal cast to an unsigned
/// type (`(uint8_t)-1`) is the usual all-ones idiom and is not flagged.
fn literal_truncates(value: u128, negative: bool, bits: u32, signed: bool) -> bool {
    if bits >= 32 {
        return !negative && value > u128::from(u32::MAX);
    }
    match (negative, signed) {
        (false, false) => value >= 1u128 << bits,
        (false, true) => value >= 1u128 << (bits - 1),
        (true, true) => value > 1u128 << (bits - 1),
        (true, false) => false,
    }
}

/// The 1-based columns of each cast on `line` that MSVC folds as a constant
/// and reports as C4310 "cast truncates constant value", which `/WX` makes
/// an error; GCC and Clang never warn on an explicit cast. Two shapes, with
/// optional parentheses and spaces between the parts:
///
/// - `(<narrow type>)~<constant>`, where the constant is a literal or an
///   all-caps macro name (`~0x80u` is `0xFFFFFF7F`); `(uint8_t)(x & ~CONST)`
///   casts a non-constant and is fine;
/// - `(<narrow type>)<literal>` whose value the type cannot hold
///   (`(PLI_BYTE8)0xff`, since `char` is signed under MSVC).
fn truncating_constant_casts(line: &str) -> Vec<usize> {
    let code = line.split("//").next().unwrap_or("");
    let bytes = code.as_bytes();
    let mut hits = Vec::new();
    for (open, _) in code.match_indices('(') {
        let Some(close) = code[open + 1..].find(')').map(|at| open + 1 + at) else {
            continue;
        };
        let ty = code[open + 1..close]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let Some(&(_, bits, signed)) = NARROW_CAST_TYPES.iter().find(|(name, ..)| *name == ty)
        else {
            continue;
        };
        let mut at = close + 1;
        let skip = |at: &mut usize| {
            while *at < bytes.len() && (bytes[*at].is_ascii_whitespace() || bytes[*at] == b'(') {
                *at += 1;
            }
        };
        let token = |at: &mut usize| {
            let start = *at;
            while *at < bytes.len() && (bytes[*at].is_ascii_alphanumeric() || bytes[*at] == b'_') {
                *at += 1;
            }
            &code[start..*at]
        };
        skip(&mut at);
        let complement = bytes.get(at) == Some(&b'~');
        let negative = bytes.get(at) == Some(&b'-');
        if complement || negative {
            at += 1;
            skip(&mut at);
        }
        let operand = token(&mut at);
        let truncates = if complement {
            // Only the first operand is inspected, so `(uint8_t)~(FLAG | x)`
            // is flagged too; spell it as a cast of the masked value instead.
            operand.starts_with(|c: char| c.is_ascii_digit())
                || (operand.starts_with(|c: char| c.is_ascii_uppercase())
                    && operand
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'))
        } else {
            // As above, only the first operand is inspected:
            // `(uint8_t)(300 / 2)` is flagged; cast the folded value.
            integer_literal(operand)
                .is_some_and(|value| literal_truncates(value, negative, bits, signed))
        };
        if truncates {
            hits.push(open + 1);
        }
    }
    hits
}

#[test]
fn truncating_constant_cast_scan_finds_the_msvc_c4310_shapes() {
    assert_eq!(
        truncating_constant_casts("x &= (uint8_t)~LLG_TIMING_DISABLED;"),
        vec![6]
    );
    assert_eq!(
        truncating_constant_casts("y = (unsigned char) ~ (0x80u);"),
        vec![5]
    );
    assert_eq!(
        truncating_constant_casts("z = (uint16_t)(~FLAG_A1);"),
        vec![5]
    );
    assert!(truncating_constant_casts("x = (uint8_t)(x & ~LLG_TIMING_DISABLED);").is_empty());
    assert!(truncating_constant_casts("s = (uint64_t)(uint32_t)~words[i].aval;").is_empty());
    assert!(truncating_constant_casts("v = (int64_t)~low;").is_empty());
    assert!(truncating_constant_casts("w = (uint64_t)~FLAG;").is_empty());
    assert!(truncating_constant_casts("// (uint8_t)~FLAG in a comment").is_empty());
    assert_eq!(
        truncating_constant_casts("raw[i] = (PLI_BYTE8)0xff;"),
        vec![10]
    );
    assert_eq!(truncating_constant_casts("c = (char) (128);"), vec![5]);
    assert_eq!(truncating_constant_casts("b = (uint8_t)0x100u;"), vec![5]);
    assert_eq!(truncating_constant_casts("s = (short)-32769;"), vec![5]);
    assert_eq!(
        truncating_constant_casts("w = (unsigned)0x100000000ull;"),
        vec![5]
    );
    assert!(truncating_constant_casts("c = (char)0x7f;").is_empty());
    assert!(truncating_constant_casts("c = (signed char)-128;").is_empty());
    assert!(truncating_constant_casts("b = (uint8_t)0377;").is_empty());
    assert!(truncating_constant_casts("b = (uint8_t)-1;").is_empty());
    assert!(truncating_constant_casts("i = (int)0xFFFFFFFFu;").is_empty());
    assert!(truncating_constant_casts("i = (int)-x;").is_empty());
    assert!(truncating_constant_casts("b = (svBit)(x & 1);").is_empty());
    assert!(truncating_constant_casts("q = (uint64_t)0x1ffffffffull;").is_empty());
}

/// Every runtime C source and header, including those only generated models
/// compile, the standalone probes and every C fixture (DPI-C and VPI
/// companions, built with `/W4 /WX` by `tests/support/c_compiler.rs`): none
/// may cast a constant to a type that cannot hold it (MSVC C4310 under `/WX`;
/// CI: `scheduler/sdf.c(309)`, `adv_020/value_array.c(241)`).
#[test]
fn runtime_sources_have_no_truncating_constant_casts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut pending = vec![
        root.join("src/sim/rt"),
        root.join("tests/runtime_value_storage"),
        root.join("tests/fixtures"),
    ];
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("read directory entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if !matches!(path.extension().and_then(|e| e.to_str()), Some("c" | "h")) {
                continue;
            }
            scanned += 1;
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            for (number, line) in text.lines().enumerate() {
                for column in truncating_constant_casts(line) {
                    findings.push(format!(
                        "{}:{}:{column}: {}",
                        path.strip_prefix(root).unwrap_or(&path).display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(scanned > 0, "no runtime C sources found");
    assert!(
        findings.is_empty(),
        "cast the masked value instead of the complemented constant, e.g. \
         `x = (uint8_t)(x & ~FLAG)`, and write constants the target type holds \
         (MSVC C4310 under /WX):\n{}",
        findings.join("\n")
    );
}

/// The narrowing check's runtime units resolve `<zlib.h>` to the bundled
/// header even where the host has a system copy, as the Linux CI test
/// containers do not (CI: `fstapi.h:36: 'zlib.h' file not found`).
#[test]
fn strict_source_units_include_the_bundled_zlib() {
    let Some(clang) = clang_driver() else {
        eprintln!("SKIP: no Clang driver");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(&clang);
    command.args(["-std=c11", "-M"]).args(BUNDLED_ZLIB_DEFINES);
    for include in runtime_include_dirs(root) {
        command.arg("-I").arg(include);
    }
    command.arg(root.join("src/sim/rt/llg_wave.c"));
    let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
        .unwrap_or_else(|error| panic!("run `{clang}`: {error}"));
    assert!(
        output.status.success(),
        "{}",
        c_compiler::compiler_diagnostics(&output)
    );
    let dependencies = String::from_utf8_lossy(&output.stdout);
    let bundled = root.join("vendor/zlib/zlib.h");
    assert!(
        dependencies.contains(&*bundled.to_string_lossy()),
        "llg_wave.c must include {}:\n{dependencies}",
        bundled.display()
    );
}

/// The flags name the `llg_vpi.c(4051)` C4701 shape: a local set only by an
/// out-parameter call that a short-circuit can skip, used under a flag.
#[test]
fn msvc_w4_flow_errors_reject_a_local_a_short_circuit_can_leave_unset() {
    let Some(clang) = clang_driver() else {
        eprintln!("SKIP: no Clang driver");
        return;
    };
    let dir = sim_harness::TempDir::new("runtime-uninitialized").expect("create temp directory");
    let source = dir.path().join("maybe_unset.c");
    std::fs::write(
        &source,
        "int get(int *out);\n\
         int sum(int n) {\n\
         \x20   int total = 0, ok = 1;\n\
         \x20   for (int i = 0; i < n; ++i) {\n\
         \x20       int value;\n\
         \x20       if (!ok || !get(&value)) ok = 0;\n\
         \x20       total += ok ? value : 0;\n\
         \x20   }\n\
         \x20   return total;\n\
         }\n",
    )
    .expect("write the probe");
    let mut command = Command::new(&clang);
    command
        .args(["-std=c11", "-fsyntax-only"])
        .args(MSVC_W4_FLOW_ERRORS)
        .arg(&source);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
        .unwrap_or_else(|error| panic!("run `{clang}`: {error}"));
    let diagnostics = c_compiler::compiler_diagnostics(&output);
    assert!(
        !output.status.success() && diagnostics.contains("conditional-uninitialized"),
        "{diagnostics}"
    );
}

/// C source text with comments, string literals and character literals
/// replaced by spaces; newlines are kept, so line numbers stay.
fn c_code_only(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let blank = |byte: u8| if byte == b'\n' { b'\n' } else { b' ' };
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                while at < bytes.len() && bytes[at] != b'\n' {
                    out.push(b' ');
                    at += 1;
                }
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                let end = text[at + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |offset| at + 2 + offset + 2);
                out.extend(bytes[at..end].iter().map(|&byte| blank(byte)));
                at = end;
            }
            quote @ (b'"' | b'\'') => {
                out.push(b' ');
                at += 1;
                while at < bytes.len() && bytes[at] != quote && bytes[at] != b'\n' {
                    let step = if bytes[at] == b'\\' { 2 } else { 1 };
                    let end = (at + step).min(bytes.len());
                    out.extend(bytes[at..end].iter().map(|&byte| blank(byte)));
                    at = end;
                }
                if at < bytes.len() && bytes[at] == quote {
                    out.push(b' ');
                    at += 1;
                }
            }
            byte => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The keywords of C11: never a routine name, so a file-scope
/// `void (*hook)(int)` is not taken for a standard routine's declaration.
const C_KEYWORDS: &[&str] = &[
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Bool",
    "_Complex",
    "_Generic",
    "_Imaginary",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
    "defined",
];

/// The standard foreign-interface headers (IEEE 1800-2009 Annex I `svdpi.h`,
/// Annex K `vpi_user.h`, Annex M `sv_vpi_user.h`), copied with the types the
/// standard gives them. The integer-type scan skips their `int`, `unsigned`
/// and `signed`, and allows those types in the runtime's own declarations and
/// definitions of the routines they declare.
const STANDARD_INTERFACE_HEADERS: &[&str] = &["svdpi.h", "vpi_user.h", "sv_vpi_user.h"];

/// Identifier and punctuation tokens of code (comments and literals removed
/// by [`c_code_only`]), each with its zero-based line.
fn c_tokens(code: &str) -> Vec<(&str, usize)> {
    let mut tokens = Vec::new();
    for (line_index, line) in code.lines().enumerate() {
        let mut chars = line.char_indices().peekable();
        while let Some((start, c)) = chars.next() {
            if c.is_ascii_alphanumeric() || c == '_' {
                let mut end = start + c.len_utf8();
                while let Some(&(at, next)) = chars.peek() {
                    if !(next.is_ascii_alphanumeric() || next == '_') {
                        break;
                    }
                    end = at + next.len_utf8();
                    chars.next();
                }
                tokens.push((&line[start..end], line_index));
            } else if !c.is_whitespace() {
                tokens.push((&line[start..start + c.len_utf8()], line_index));
            }
        }
    }
    tokens
}

/// The routines `code` (a standard header, comments and literals removed)
/// declares: every identifier followed by `(` that is not a keyword.
fn declared_routines(code: &str, routines: &mut std::collections::BTreeSet<String>) {
    let tokens = c_tokens(code);
    for pair in tokens.windows(2) {
        let name = pair[0].0;
        if pair[1].0 == "("
            && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && !C_KEYWORDS.contains(&name)
        {
            routines.insert(name.to_string());
        }
    }
}

/// Zero-based lines of `code` (comments and literals removed by
/// [`c_code_only`]) that name a platform-width integer type: `short`, `long`
/// or `long long`, signed or unsigned, and, when `scan_int` is set, `int`,
/// `unsigned` or `signed`. Two `long` forms stay allowed: `long double`, a
/// floating type (docs/cross_platform.md, arm64), and the casts
/// `(long long)` and `(unsigned long long)` that hand a 64-bit value to a
/// printf-family `%lld` or `%llu`. `int`, `unsigned` and `signed` are allowed
/// in a file-scope declaration of `main` or of a routine in `standard`, from
/// the start of the declaration through the `)` that closes its parameters:
/// the standard fixes those signatures.
fn platform_integer_type_lines(
    code: &str,
    scan_int: bool,
    standard: &std::collections::BTreeSet<String>,
) -> Vec<usize> {
    let tokens = c_tokens(code);
    let token = |index: usize| tokens.get(index).map_or("", |(text, _)| *text);
    // Preprocessor lines, with their backslash continuations.
    let mut directive = Vec::new();
    let mut continued = false;
    for line in code.lines() {
        let is_directive = continued || line.trim_start().starts_with('#');
        continued = is_directive && line.trim_end().ends_with('\\');
        directive.push(is_directive);
    }
    let mut standard_signature = vec![false; tokens.len()];
    let mut depth = 0i32;
    let mut start = 0usize;
    for (index, &(text, line)) in tokens.iter().enumerate() {
        if directive[line] {
            start = index + 1;
            continue;
        }
        match text {
            "{" => depth += 1,
            "}" => {
                depth -= 1;
                if depth == 0 {
                    start = index + 1;
                }
            }
            ";" if depth == 0 => start = index + 1,
            _ if depth == 0
                && token(index + 1) == "("
                && (text == "main" || standard.contains(text)) =>
            {
                let mut parens = 0i32;
                let mut close = index + 1;
                while close < tokens.len() {
                    match tokens[close].0 {
                        "(" => parens += 1,
                        ")" => {
                            parens -= 1;
                            if parens == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    close += 1;
                }
                let end = close.min(tokens.len() - 1);
                standard_signature[start..=end].fill(true);
            }
            _ => {}
        }
    }
    let mut lines = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let (text, line) = tokens[index];
        let names = match text {
            "(" if token(index + 1) == "long"
                && token(index + 2) == "long"
                && token(index + 3) == ")" =>
            {
                index += 4;
                continue;
            }
            "(" if token(index + 1) == "unsigned"
                && token(index + 2) == "long"
                && token(index + 3) == "long"
                && token(index + 4) == ")" =>
            {
                index += 5;
                continue;
            }
            "long" if token(index + 1) == "double" => {
                index += 2;
                continue;
            }
            "short" | "long" => true,
            "int" | "unsigned" | "signed" => scan_int && !standard_signature[index],
            _ => false,
        };
        if names && lines.last() != Some(&line) {
            lines.push(line);
        }
        index += 1;
    }
    lines
}

#[test]
fn platform_integer_type_scan_finds_platform_widths_and_plain_int() {
    let standard: std::collections::BTreeSet<String> =
        ["svSize".to_string(), "svGetBitselBit".to_string()].into();
    let names =
        |code: &str| !platform_integer_type_lines(&c_code_only(code), true, &standard).is_empty();
    assert!(names("long point = 0;"));
    assert!(names("static unsigned long id(void);"));
    assert!(names("x = (long)y;"));
    assert!(names("long long value = 0;"));
    assert!(names("unsigned long long total;"));
    assert!(names("static const unsigned short table[] = {0};"));
    assert!(names("short s;"));
    assert!(names("f((long long)a + b, (long)c);"));
    assert!(names("int count = 0;"));
    assert!(names("static unsigned flags;"));
    assert!(names("unsigned int width;"));
    assert!(names("signed char s;"));
    assert!(names("x = (int)y;"));
    assert!(names("static void (*hook)(int signal);"));
    assert!(names("#define LLG_COUNT(x) ((int)(x))"));
    assert!(names("int helper(int a) { return a; }"));
    assert!(names("void f(void) { int svSize = 0; (void)svSize; }"));
    assert!(names(
        "svBit svGetBitselBit(const svBitVecVal* s, int i);\nint other;"
    ));
    assert!(!names(
        "printf(\"%lld %llu\", (long long)a, (unsigned long long)b);"
    ));
    assert!(!names("x = (double)((long double)t * p);"));
    assert!(!names("// a long comment about short int values"));
    assert!(!names(
        "/* long */ int64_t longest = shortest; uint32_t interval;"
    ));
    assert!(!names("llg_msg(id, \"too long for an int\");"));
    assert!(!names("char c = 'l';"));
    assert!(!names("int32_t a; uint8_t b; int_fast16_t c; uintptr_t p;"));
    assert!(!names(
        "int main(int argc, char** argv) {\n    return argc;\n}"
    ));
    assert!(!names(
        "int svSize(const svOpenArrayHandle h,\n           int d) {\n    return llg_size(h, d);\n}"
    ));
    assert!(!names(
        "static int32_t before(void);\nsvBit svGetBitselBit(const svBitVecVal* s, int i) {\n    return 0;\n}"
    ));
    let lines = platform_integer_type_lines(
        &c_code_only(
            "int svSize(const svOpenArrayHandle h, int d) {\n    int i = d;\n    return i;\n}",
        ),
        true,
        &standard,
    );
    assert_eq!(lines, vec![1]);
    let platform_only =
        |code: &str| !platform_integer_type_lines(&c_code_only(code), false, &standard).is_empty();
    assert!(!platform_only(
        "XXTERN int svDimensions(const svOpenArrayHandle h);"
    ));
    assert!(platform_only("XXTERN long svBad(void);"));
}

#[test]
fn standard_header_routines_are_their_declared_names() {
    let mut routines = std::collections::BTreeSet::new();
    declared_routines(
        &c_code_only(
            "XXTERN int svSize(const svOpenArrayHandle h, int d);\n\
             typedef void (*svCallback)(void* data);\n\
             PLI_INT32 vpi_get(PLI_INT32 property, vpiHandle object);\n",
        ),
        &mut routines,
    );
    let expected: std::collections::BTreeSet<String> = ["svSize", "vpi_get"]
        .iter()
        .map(|name| name.to_string())
        .collect();
    assert_eq!(routines, expected);
}

/// Where a standard header or the platform fixes a platform-width integer
/// type: (file under `src/sim/rt`, code fragment of the line, reason). Each
/// entry must still match a line.
const PLATFORM_INTEGER_TYPE_EXCEPTIONS: &[(&str, &str, &str)] = &[
    (
        "llg_co.h",
        "long long ll;",
        "llg_co_max_align_t mirrors C11 max_align_t's widest standard types",
    ),
    (
        "llg_platform.h",
        "_Generic((int32_t)0, int: 1, default: 0)",
        "asserts that int32_t is int, which keeps the ABI and printf `*` widths",
    ),
    (
        "llg_platform.h",
        "_Generic((uint32_t)0, unsigned int: 1, default: 0)",
        "asserts that uint32_t is unsigned int, which keeps the ABI",
    ),
    (
        "llg_platform.h",
        "typedef volatile long llg_atomic_int_t;",
        "the Win32 _Interlocked* intrinsics operate on long",
    ),
    (
        "llg_platform.h",
        "_InterlockedExchange(p, (long)v)",
        "the Win32 _Interlocked* intrinsics operate on long",
    ),
    (
        "llg_platform.h",
        "_InterlockedCompareExchange(p, (long)desired, (long)expected) == (long)expected",
        "the Win32 _Interlocked* intrinsics operate on long",
    ),
    (
        "llg_platform_native.h",
        "(unsigned long)GetLastError()",
        "GetLastError returns a DWORD, printed with %lu",
    ),
    (
        "llg_vpi.c",
        "long long value = open ? strtoll(",
        "strtoll returns long long",
    ),
    (
        "llg_vpi_runtime.c",
        "long long value = strtoll(",
        "strtoll returns long long",
    ),
];

/// The runtime spells integers with fixed-width types (`src/sim/rt/AGENTS.md`):
/// `long` is 32-bit on Windows (LLP64) and 64-bit on Linux and macOS (LP64),
/// so code that compiles everywhere can still compute differently on one
/// platform, and `int` and `unsigned` leave the width to the compiler.
/// `short`, `long`, `long long`, `int`, `unsigned` and `signed` appear only in
/// [`PLATFORM_INTEGER_TYPE_EXCEPTIONS`] and the forms
/// [`platform_integer_type_lines`] allows; the standard foreign-interface
/// headers ([`STANDARD_INTERFACE_HEADERS`]) keep the standard's `int`.
#[test]
fn runtime_sources_use_fixed_width_integer_types() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = root.join("src/sim/rt");
    let mut standard = std::collections::BTreeSet::new();
    for header in STANDARD_INTERFACE_HEADERS {
        let path = rt.join(header);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        declared_routines(&c_code_only(&text), &mut standard);
    }
    assert!(
        standard.contains("svSize") && standard.contains("vpi_get"),
        "standard routines not found in {STANDARD_INTERFACE_HEADERS:?}"
    );
    let mut pending = vec![rt.clone()];
    let mut used = vec![false; PLATFORM_INTEGER_TYPE_EXCEPTIONS.len()];
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("read directory entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if !matches!(path.extension().and_then(|e| e.to_str()), Some("c" | "h")) {
                continue;
            }
            scanned += 1;
            let relative = path
                .strip_prefix(&rt)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let code = c_code_only(&text);
            let scan_int = !STANDARD_INTERFACE_HEADERS.contains(&relative.as_str());
            let lines: Vec<&str> = code.lines().collect();
            for number in platform_integer_type_lines(&code, scan_int, &standard) {
                let line = lines[number];
                let exception = PLATFORM_INTEGER_TYPE_EXCEPTIONS
                    .iter()
                    .position(|(file, fragment, _)| *file == relative && line.contains(fragment));
                match exception {
                    Some(index) => used[index] = true,
                    None => findings.push(format!(
                        "src/sim/rt/{relative}:{}: {}",
                        number + 1,
                        line.trim()
                    )),
                }
            }
        }
    }
    assert!(scanned > 0, "no runtime C sources found");
    assert!(
        findings.is_empty(),
        "use fixed-width integer types (int32_t, uint32_t, int64_t, uint8_t, ...; \
         size_t for sizes) instead of int, unsigned, signed, short, long or long \
         long; `long` is 32-bit on Windows. A C library, OS or standard-header \
         interface that fixes the type goes in PLATFORM_INTEGER_TYPE_EXCEPTIONS:\n{}",
        findings.join("\n")
    );
    let stale: Vec<String> = PLATFORM_INTEGER_TYPE_EXCEPTIONS
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|((file, fragment, reason), _)| format!("{file}: `{fragment}` ({reason})"))
        .collect();
    assert!(
        stale.is_empty(),
        "remove exceptions that no longer match a line:\n{}",
        stale.join("\n")
    );
}
