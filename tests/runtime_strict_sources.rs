//! Linux/macOS stand-in for MSVC's unreachable-code warning on the runtime C
//! that the standalone probes build with `/W4 /WX`.
//!
//! MSVC at `/O2` inlines a `static` helper that ends in `abort()` and then
//! reports a statement after an unconditional call to it as unreachable
//! (C4702), which `/WX` turns into a failed probe build (CI round 6:
//! `llg_constraint.c(744)` and `(7643)`). GCC and Clang accept that code at
//! `-Wall -Wextra`. Clang's `-Wmissing-noreturn` names a helper that never
//! returns but is not declared `_Noreturn`, and `-Wunreachable-code-aggressive`
//! then names a statement after a call to it, which together approximate
//! C4702 (Clang does not report dead code that comes from a macro expansion,
//! such as `return NULL;`). Skipped without a Clang driver and on Windows,
//! where the probes themselves build with MSVC.

use std::path::Path;
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

/// Integer types narrower than or as wide as `unsigned int`: a cast to one
/// of them from the complement of an unsigned constant drops set high bits.
const NARROW_CAST_TYPES: &[&str] = &[
    "uint8_t",
    "int8_t",
    "uint16_t",
    "int16_t",
    "uint32_t",
    "int32_t",
    "char",
    "signed char",
    "unsigned char",
    "short",
    "unsigned short",
    "int",
    "unsigned",
    "unsigned int",
];

/// The 1-based columns of each `(<narrow type>)~<constant>` on `line`, where
/// the constant is a literal or an all-caps macro name, with optional
/// parentheses and spaces between the parts. MSVC folds such a cast as a
/// constant and reports C4310 "cast truncates constant value" (`~0x80u` is
/// `0xFFFFFF7F`), which `/WX` makes an error; GCC and Clang never warn on an
/// explicit cast. `(uint8_t)(x & ~CONST)` casts a non-constant and is fine.
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
        if !NARROW_CAST_TYPES.contains(&ty.as_str()) {
            continue;
        }
        let mut at = close + 1;
        let skip = |at: &mut usize, also_paren: bool| {
            while *at < bytes.len()
                && (bytes[*at].is_ascii_whitespace() || (also_paren && bytes[*at] == b'('))
            {
                *at += 1;
            }
        };
        skip(&mut at, true);
        if bytes.get(at) != Some(&b'~') {
            continue;
        }
        at += 1;
        skip(&mut at, true);
        let start = at;
        while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
            at += 1;
        }
        let operand = &code[start..at];
        let constant = operand.starts_with(|c: char| c.is_ascii_digit())
            || (operand.starts_with(|c: char| c.is_ascii_uppercase())
                && operand
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'));
        // Only the first operand is inspected, so `(uint8_t)~(FLAG | x)` is
        // flagged too; spell it as a cast of the masked value instead.
        if constant {
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
}

/// Every runtime C source and header, including those only generated models
/// compile, and the standalone probes: none may cast the complement of a
/// constant to a narrow type (MSVC C4310 under `/WX`; CI: `scheduler/sdf.c(309)`).
#[test]
fn runtime_sources_have_no_truncating_constant_casts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut pending = vec![
        root.join("src/sim/rt"),
        root.join("tests/runtime_value_storage"),
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
         `x = (uint8_t)(x & ~FLAG)` (MSVC C4310 under /WX):\n{}",
        findings.join("\n")
    );
}
