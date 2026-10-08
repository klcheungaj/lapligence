//! Link closure of the bundled GMP subset (`src/sim/rt/gmp/llg_gmp.cmake`).
//!
//! GCC and Clang drop calls behind constant conditions even at `-O0`
//! (invertappr.c's `! MAYBE_dcpi1_divappr || ...`), but MSVC `/Od` keeps them,
//! so a Linux `-O0` link alone cannot show that the subset is closed for MSVC.
//! This test takes every `__gmpn_*`/`__gmp_*` name the preprocessed subset
//! sources contain in their own lines, whether or not a compiler would keep the
//! call, and links an unoptimized executable that references all of them.

use crate::c_compiler;
use crate::sim_harness;

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const STAGE_TIMEOUT: Duration = Duration::from_secs(300);

/// Names the preprocessed sources contain that are not external references:
/// toom22_mul.c and its siblings declare a local `const int
/// __gmpn_cpuvec_initialized` that stands in for the fat-build variable.
const NOT_EXTERNAL: [&str; 1] = ["__gmpn_cpuvec_initialized"];

const MAIN_C: &str = "#include <gmp.h>
int main(void) {
  mp_limb_t a[4] = {1, 2, 3, 4}, b[4] = {5, 6, 7, 8}, r[8] = {0}, q[4], rem[4];
  char text[64];
  mpn_mul_n(r, a, b, 4);
  mpn_mul_1(r, a, 4, 3);
  mpn_addmul_1(r, a, 4, 3);
  mpn_tdiv_qr(q, rem, 0, r, 8, b, 4);
  return mpn_get_str((unsigned char *)text, 10, r, 4) == 0;
}
";

/// Subset source names listed by the recipe.
fn recipe_sources(recipe: &str) -> Vec<&str> {
    let start = recipe
        .find("set(LLG_GMP_MPN_SOURCES")
        .expect("recipe lists mpn sources");
    let end = start + recipe[start..].find(')').expect("closed list");
    recipe[start..end].split_whitespace().skip(1).collect()
}

/// The `__gmpn_*` and `__gmp_*` identifiers in the lines of `preprocessed`
/// that come from `source` itself: macro expansions land there, while the
/// prototypes of gmp.h and gmp-impl.h do not.
fn own_references(preprocessed: &str, source: &Path, into: &mut Vec<String>) {
    let mut in_source = false;
    for line in preprocessed.lines() {
        if let Some(marker) = line.strip_prefix("# ") {
            if let Some(file) = marker.split('"').nth(1) {
                in_source = Path::new(file) == source;
                continue;
            }
        }
        if !in_source {
            continue;
        }
        let bytes = line.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_') {
                index += 1;
                continue;
            }
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let name = &line[start..index];
            if name.starts_with("__gmpn_") || name.starts_with("__gmp_") {
                into.push(name.to_owned());
            }
        }
    }
}

#[test]
fn bundled_subset_links_every_reference_at_o0() {
    let cmake = std::env::var("LLG_CMAKE").unwrap_or_else(|_| "cmake".to_owned());
    if Command::new(&cmake).arg("--version").output().is_err() {
        eprintln!("SKIP: CMake `{cmake}` not available");
        return;
    }
    let compiler = c_compiler::host_c_compiler();
    if c_compiler::is_msvc(&compiler) || !c_compiler::c_compiler_available(&compiler) {
        eprintln!("SKIP: no GCC/Clang-style compiler `{compiler}` to preprocess with");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gmp = root.join("vendor/gmp");
    let tables = root.join("src/sim/rt/gmp/generated");
    let recipe_path = root.join("src/sim/rt/gmp/llg_gmp.cmake");
    let dir = sim_harness::TempDir::new("gmp-closure").expect("create scratch directory");
    let project = dir.path();
    let cmake_path = |path: &Path| path.to_string_lossy().replace('\\', "/");
    std::fs::write(
        project.join("CMakeLists.txt"),
        format!(
            "cmake_minimum_required(VERSION 3.16)
project(llg_gmp_closure C)
set(LLG_GMP_SOURCE_DIR \"{}\")
set(LLG_GMP_TABLE_DIR \"{}\")
include(\"{}\")
add_executable(closure main.c refs.c $<TARGET_OBJECTS:llg_gmp>)
target_include_directories(closure PRIVATE ${{LLG_GMP_INCLUDE_DIRS}})
",
            cmake_path(&gmp),
            cmake_path(&tables),
            cmake_path(&recipe_path)
        ),
    )
    .expect("write project");
    std::fs::write(project.join("main.c"), MAIN_C).expect("write main.c");
    std::fs::write(project.join("refs.c"), "int llg_gmp_closure_refs_unused;\n")
        .expect("write placeholder refs.c");

    let mut configure = Command::new(&cmake);
    configure
        .arg("-S")
        .arg(project)
        .arg("-B")
        .arg(project.join("build"))
        .arg(format!("-DCMAKE_C_COMPILER={compiler}"))
        .arg("-DCMAKE_BUILD_TYPE=Debug")
        .arg("-DCMAKE_C_FLAGS:STRING=-O0");
    run_stage("configure", &mut configure);

    let recipe = std::fs::read_to_string(&recipe_path).expect("read recipe");
    let generated = project.join("build/llg_gmp");
    let mut references = Vec::new();
    for name in recipe_sources(&recipe) {
        let source = gmp.join("mpn/generic").join(format!("{name}.c"));
        let output = sim_harness::run_command(
            Command::new(&compiler)
                .args(["-E", "-w", "-D__GMP_WITHIN_GMP"])
                .arg(format!("-DOPERATION_{name}"))
                .arg(format!("-I{}", generated.display()))
                .arg(format!("-I{}", tables.display()))
                .arg(format!("-I{}", gmp.display()))
                .arg(format!("-I{}", gmp.join("mpn").display()))
                .arg(&source),
            STAGE_TIMEOUT,
        )
        .expect("preprocess a subset source");
        assert!(
            output.status.success(),
            "preprocessing {name}.c failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        own_references(
            &String::from_utf8_lossy(&output.stdout),
            &source,
            &mut references,
        );
    }
    references.retain(|name| !NOT_EXTERNAL.contains(&name.as_str()));
    references.sort();
    references.dedup();
    assert!(
        references.len() > 40,
        "too few references found ({}); is the preprocessed text attributed to the source?",
        references.len()
    );

    let mut refs = String::new();
    for name in &references {
        writeln!(refs, "extern char {name}[];").unwrap();
    }
    refs.push_str("void *const llg_gmp_closure_refs[] = {\n");
    for name in &references {
        writeln!(refs, "  {name},").unwrap();
    }
    refs.push_str("};\n");
    std::fs::write(project.join("refs.c"), refs).expect("write refs.c");

    let mut build = Command::new(&cmake);
    build.arg("--build").arg(project.join("build"));
    run_stage("link", &mut build);
}

fn run_stage(stage: &str, command: &mut Command) {
    let output = sim_harness::run_command(command, STAGE_TIMEOUT)
        .unwrap_or_else(|error| panic!("GMP closure {stage}: {error}"));
    assert!(
        output.status.success(),
        "GMP closure {stage} failed; a link error names a symbol whose mpn source is missing from \
         LLG_GMP_MPN_SOURCES (and bundled_gmp_sources):\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
