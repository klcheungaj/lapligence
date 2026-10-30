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
    &["-DLLG_SV4_USE_GMP=1"],
];

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
    let vpi_unit = dir.path().join("llg_vpi_unit.c");
    std::fs::write(
        &vpi_unit,
        "#include \"llg_vpi.c\"\n#include \"llg_vpi_dynamic.c\"\n#include \"llg_dpi.c\"\n",
    )
    .expect("write the VPI unit");

    let mut units = Vec::new();
    let runtime = |source: PathBuf, defines| NarrowingUnit {
        source,
        includes: runtime_includes.clone(),
        defines,
        bundled_zlib: true,
    };
    for &defines in RUNTIME_CONFIGURATIONS {
        let compact = defines.contains(&"-DLLG_SV4_USE_GMP=1");
        let mut sources: Vec<PathBuf> = RUNTIME_UNITS.iter().map(|unit| rt.join(unit)).collect();
        sources.push(vpi_unit.clone());
        if compact {
            sources.extend(c_sources_under(&rt.join("value_gmp")));
        } else {
            sources.push(rt.join("llg_value.c"));
        }
        units.extend(sources.into_iter().map(|source| runtime(source, defines)));
    }
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
    for source in c_sources_under(&root.join("tests/fixtures")) {
        let own = source.parent().expect("fixture directory").to_path_buf();
        units.push(NarrowingUnit {
            source,
            includes: vec![own, rt.clone()],
            defines: &[],
            bundled_zlib: false,
        });
    }

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
                let mut command = Command::new(&clang);
                command
                    .args(["-std=c11", "-fsyntax-only"])
                    .args(MSVC_W4_FLOW_ERRORS)
                    .args(unit.defines);
                if unit.bundled_zlib {
                    command.args(BUNDLED_ZLIB_DEFINES);
                }
                for include in &unit.includes {
                    command.arg("-I").arg(include);
                }
                command.arg(&unit.source);
                let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
                    .unwrap_or_else(|error| panic!("run `{clang}`: {error}"));
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
    let findings = findings.into_inner().expect("findings lock");
    assert!(
        findings.is_empty(),
        "make each narrowing conversion explicit, after checking the value fits, \
         and initialize each local on every path before its use \
         (MSVC C4244/C4267/C4701 under /W4 /WX; see docs/cross_platform.md):\n{}",
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
