//! Integration tests for the CMake-based model builder (`sim::build`) — the
//! only supported model-build path — and the `llg` driver's build-time
//! generator/launcher, output-directory and toolchain options.
//!
//! The
//! library-level cases run with the CWD pointed at a fresh harness temp dir;
//! the driver-level cases spawn `llg` with its own temp CWD instead.
//! Every test holds one shared mutex to serialize process-CWD changes (like the
//! other simulator suites), it also keeps the process-global `$LLG_CMAKE`
//! mutation in `missing_cmake_error` from racing another test's
//! `build_model_cmake` call (and its `cmake_available()` probe).
//!
//! Skip policy: cmake is probed once (`sim::build::cmake_available`,
//! `OnceLock`); when unavailable those cases print
//! `SKIP: cmake not available` and return early instead of failing.
//! Contributor hosts may lack cmake.  The explicit-generator case additionally
//! skips when the host cmake does not list the requested generator backend.
//! Exception: `missing_cmake_error` needs no working cmake — pointing
//! `$LLG_CMAKE` at a nonexistent program fails identically either way — so it
//! runs unconditionally to pin the actionable-error contract on cmake-less
//! hosts too.

use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

static TEST_LOCK: Mutex<()> = Mutex::new(());

/// The counter design + hand-simulated trace from `tests/sim_counter.rs`
/// (`sim_counter_end_to_end`): reset at t=1, then two posedges increment
/// count to 8 by t=34 and 9 by t=44.
const COUNTER_SV: &str = r#"module counter #(parameter WIDTH = 8, parameter [3:0] INIT = 4'h5) (
    input  logic clk, input logic rst_n,
    output logic [WIDTH-1:0] count, output logic done
);
    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) count <= INIT;
        else count <= count + 1;
    end
    assign done = (count == 8'hff);
endmodule

module tb;
    reg clk; reg rst_n; wire [7:0] count; wire done;
    counter #(.WIDTH(8), .INIT(4'h5)) u(.clk(clk), .rst_n(rst_n), .count(count), .done(done));
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        rst_n = 1;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #30 $display("count=%0d done=%b", count, done);
        #10 $display("count=%0d done=%b", count, done);
        $finish;
    end
endmodule
"#;

const EXPECTED_STDOUT: &str = "count=8 done=0\ncount=9 done=0\n";

/// Portable generator backend for the explicit-`-G` case.
const GENERATOR: &str = "Unix Makefiles";

/// Minimal stand-in model source; only used where cmake must fail *before*
/// compiling anything.
const STUB_MODEL_C: &str = "#define LLG_MODEL_VALUE_ABI 4\nint main(void) { return 0; }\n";

fn fresh_dir(tag: &str) -> sim_harness::TempDir {
    sim_harness::TempDir::new(&format!("sim-cmake-{tag}")).expect("create temp dir")
}

#[test]
fn generated_sources_keep_value_runtime_as_a_separate_translation_unit() {
    let dir = fresh_dir("value-sources");
    let extra = [("model.c", STUB_MODEL_C)];
    sim::build::generate_model_sources(dir.path(), &extra).expect("generate model sources");
    std::fs::write(dir.path().join("stale.c"), "stale").expect("write stale source");
    for stale in ["aco.h", "aco.c", "acosw.S", "aco_assert_override.h"] {
        std::fs::write(dir.path().join(stale), "stale").expect("write stale coroutine source");
    }
    sim::build::generate_model_sources(dir.path(), &extra).expect("regenerate model sources");

    let (header, source) = sim::rt::value_sources();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("llg_value.h")).unwrap(),
        header
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("llg_value.c")).unwrap(),
        source
    );
    let (rng_header, rng_source) = sim::rt::rng_sources();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("llg_rng.h")).unwrap(),
        rng_header
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("llg_rng.c")).unwrap(),
        rng_source
    );
    let (coroutine_header, coroutine_source) = sim::rt::coroutine_sources();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("llg_co.h")).unwrap(),
        coroutine_header
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("llg_co.c")).unwrap(),
        coroutine_source
    );
    assert!(!dir.path().join("stale.c").exists());
    for stale in ["aco.h", "aco.c", "acosw.S", "aco_assert_override.h"] {
        assert!(
            !dir.path().join(stale).exists(),
            "regeneration retained stale {stale}"
        );
    }
    let cmake = std::fs::read_to_string(dir.path().join("CMakeLists.txt")).unwrap();
    assert!(
        cmake.contains("model.c llg_value.c llg_rng.c llg_co.c llg_rt.c llg_random.c llg_vpi.c")
    );
    assert!(cmake.contains("project(llg_sim_model C)"));
    assert!(!cmake.contains("project(llg_sim_model C ASM)"));
    assert!(cmake.contains("set(LLG_HOST_STACK_ESTIMATE_BYTES 8388608 CACHE STRING"));
    assert!(
        cmake.contains("target_link_options(sim PRIVATE /STACK:${LLG_HOST_STACK_ESTIMATE_BYTES})")
    );
    assert!(cmake.contains(
        "set_source_files_properties(llg_co.c PROPERTIES COMPILE_DEFINITIONS LLG_CO_HOST_ALLOC=1)"
    ));
    assert!(cmake.contains(
        "set_source_files_properties(model.c PROPERTIES COMPILE_OPTIONS -Wno-misleading-indentation)"
    ));
    assert!(cmake.contains("if(LLG_RUNTIME_LIBRARY)"));
    assert!(!cmake.contains("target_compile_definitions(sim PRIVATE LLG_MODEL_STACK_VALUES"));
    let (runtime_header, runtime_source) = sim::rt::runtime_sources();
    assert!(runtime_header.contains("#include \"llg_value.h\""));
    assert!(!runtime_source.contains("#include \"llg_value.c\""));
}

#[test]
fn mixed_process_and_coroutine_abis_fail_to_build_or_link() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("mixed-abi");
    let stale_model = r#"#define LLG_MODEL_VALUE_ABI 4
#define LLG_MODEL_PROCESS_ABI 1
#include "llg_rt.h"
int main(void) { return 0; }
"#;
    let error = sim::build::build_model_cmake(dir.path(), &[("model.c", stale_model)])
        .expect_err("a stale generated-model process ABI must not compile");
    assert!(matches!(&error, sim::build::BuildError::Compile { .. }));
    assert!(
        error.contains("generated model process ABI does not match llg_rt.h"),
        "unexpected stale-model failure: {error}"
    );

    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    let available = Command::new(&compiler)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !available {
        eprintln!("SKIP: C compiler `{compiler}` not available for mixed-link ABI probe");
        return;
    }

    let link_dir = dir.path().join("link");
    let stale_dir = link_dir.join("stale");
    std::fs::create_dir_all(&stale_dir).expect("create stale runtime directory");
    let (header, implementation) = sim::rt::coroutine_sources();
    std::fs::write(link_dir.join("llg_co.h"), header).expect("write current coroutine header");
    std::fs::write(
        stale_dir.join("llg_co.h"),
        header.replace(
            "#define LLG_CO_ABI_VERSION 1",
            "#define LLG_CO_ABI_VERSION 0",
        ),
    )
    .expect("write stale coroutine header");
    std::fs::write(stale_dir.join("llg_co.c"), implementation)
        .expect("write stale coroutine runtime");
    std::fs::write(
        link_dir.join("model.c"),
        "#include \"llg_co.h\"\nint main(void) { llg_co_arena_t arena = {0}; llg_co_arena_release(&arena); return 0; }\n",
    )
    .expect("write current coroutine model");

    let stale_object = link_dir.join("stale.o");
    let model_object = link_dir.join("model.o");
    for (directory, source, output) in [
        (&stale_dir, "llg_co.c", &stale_object),
        (&link_dir, "model.c", &model_object),
    ] {
        let result = Command::new(&compiler)
            .current_dir(directory)
            .args(["-std=c11", "-I.", "-c", source, "-o"])
            .arg(output)
            .output()
            .expect("run C compiler for mixed-link ABI probe");
        assert!(
            result.status.success(),
            "compile {source} failed:\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let executable = link_dir.join(format!("mixed-abi{}", std::env::consts::EXE_SUFFIX));
    let link = Command::new(&compiler)
        .args([model_object.as_os_str(), stale_object.as_os_str()])
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("link mixed coroutine ABI probe");
    assert!(
        !link.status.success(),
        "a model requesting _abi1 unexpectedly linked to an _abi0 runtime"
    );
    let diagnostic = format!(
        "{}\n{}",
        String::from_utf8_lossy(&link.stdout),
        String::from_utf8_lossy(&link.stderr)
    );
    assert!(
        diagnostic.contains("llg_co_arena_release_abi1"),
        "mixed-link failure did not name the versioned symbol: {diagnostic}"
    );
}

/// Whether the host cmake lists `generator` as an available `-G` backend
/// (`cmake --help` prints the generator table on stdout).
fn generator_supported(generator: &str) -> bool {
    let mut command = Command::new(std::env::var("LLG_CMAKE").unwrap_or_else(|_| "cmake".into()));
    command.arg("--help");
    sim_harness::run_command(&mut command, Duration::from_secs(30))
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains(generator))
        .unwrap_or(false)
}

/// Compile + codegen the counter design in an isolated CWD.
fn compile_counter(dir: &std::path::Path) -> Result<sim::codegen::GeneratedModel, String> {
    let src = dir.join("counter.sv");
    std::fs::write(&src, COUNTER_SV).expect("write source");
    let out = compile::compile(&compile::CompileOpts {
        files: vec![src.to_string_lossy().into_owned()],
        top: Some("tb".to_string()),
        ..Default::default()
    })
    .map_err(|e| format!("compile: {e}"))?;
    if !out.ok() {
        return Err(format!("compile diagnostics: {:?}", out.diagnostics));
    }
    let db =
        llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
    sim::codegen::generate(&db).map_err(|e| format!("codegen: {e}"))
}

/// Run a built simulator executable and return its captured stdout.
fn run_sim(exe: &std::path::Path) -> Result<String, String> {
    sim_harness::run_executable(exe)
}

/// Library level: compile → codegen → `build_model_cmake` → run, asserting the
/// exact counter trace.
#[test]
fn end_to_end_cmake() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("end-to-end");
    let result = sim_harness::with_cwd(dir.path(), || {
        let gen = compile_counter(dir.path())?;
        let exe = sim::build::build_model_cmake(dir.path(), &[("model.c", gen.model_c.as_str())])
            .map_err(|e| format!("cmake build: {e}"))?;
        run_sim(&exe)
    });

    let stdout = result.expect("cmake-built simulation should run");
    assert_eq!(stdout, EXPECTED_STDOUT);
}

/// A source-only export must compile without injecting the cached runtime
/// archive, so every packaged runtime translation unit is exercised.
#[test]
fn generated_sources_build_as_a_self_contained_cmake_project() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("self-contained");
    let gen = compile_counter(dir.path()).expect("compile counter");
    let project = dir.path().join("project");
    sim::build::generate_model_sources(&project, &[("model.c", gen.model_c.as_str())])
        .expect("generate self-contained model sources");

    let cmake = std::env::var("LLG_CMAKE").unwrap_or_else(|_| "cmake".to_owned());
    let build = project.join("build");
    let mut configure = Command::new(&cmake);
    configure
        .args(["-S"])
        .arg(&project)
        .args(["-B"])
        .arg(&build)
        .arg("-DCMAKE_BUILD_TYPE=Release");
    let output = sim_harness::run_command(&mut configure, Duration::from_secs(60))
        .expect("configure self-contained model");
    assert!(
        output.status.success(),
        "self-contained configure failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut compile = Command::new(&cmake);
    compile
        .arg("--build")
        .arg(&build)
        .args(["--config", "Release"]);
    let output = sim_harness::run_command(&mut compile, Duration::from_secs(120))
        .expect("build self-contained model");
    assert!(
        output.status.success(),
        "self-contained build failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let executable_name = format!("sim{}", std::env::consts::EXE_SUFFIX);
    let candidates = [
        build.join("bin").join(&executable_name),
        build.join("bin/Release").join(&executable_name),
    ];
    let executable = candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| {
            panic!(
                "self-contained simulator not found under {}",
                build.display()
            )
        });
    assert_eq!(run_sim(executable).unwrap(), EXPECTED_STDOUT);
}

/// Library level with an explicit generator backend:
/// `CmakeBuildOpts { generator: Some(...) }` must reach cmake as `-G` and
/// still produce a working executable.
#[test]
fn explicit_generator_build_and_run() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    if !generator_supported(GENERATOR) {
        eprintln!("SKIP: generator {GENERATOR:?} not supported by host cmake");
        return;
    }
    let dir = fresh_dir("explicit-generator");
    let result = sim_harness::with_cwd(dir.path(), || {
        let gen = compile_counter(dir.path())?;
        let opts = sim::build::CmakeBuildOpts {
            generator: Some(GENERATOR.to_string()),
            ..Default::default()
        };
        let exe = sim::build::build_model_cmake_with_opts(
            dir.path(),
            &[("model.c", gen.model_c.as_str())],
            &opts,
        )
        .map_err(|e| format!("cmake build: {e}"))?;
        run_sim(&exe)
    });

    let stdout = result.expect("explicit-generator simulation should run");
    assert_eq!(stdout, EXPECTED_STDOUT);
}

/// A caller-provided compiler launcher reaches both the cached runtime build
/// and the per-model compile. POSIX `env` is a transparent executable launcher
/// and avoids requiring an optional compiler-cache package on test hosts.
#[cfg(unix)]
#[test]
fn explicit_launcher_build_and_run() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("explicit-launcher");
    let result = sim_harness::with_cwd(dir.path(), || {
        let generated = compile_counter(dir.path())?;
        let opts = sim::build::CmakeBuildOpts {
            launcher: Some("env".to_owned()),
            ..Default::default()
        };
        let executable = sim::build::build_model_cmake_with_opts(
            dir.path(),
            &[("model.c", generated.model_c.as_str())],
            &opts,
        )
        .map_err(|error| format!("cmake build: {error}"))?;
        run_sim(&executable)
    });

    assert_eq!(
        result.expect("launcher-built simulation should run"),
        EXPECTED_STDOUT
    );
}

/// An unsupported generator name must surface as a clear error from the
/// cmake configure step.
#[test]
fn invalid_generator_error() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    if generator_supported("No Such Generator") {
        eprintln!("SKIP: host cmake unexpectedly accepts the probe generator name");
        return;
    }
    let dir = fresh_dir("invalid_gen");

    let opts = sim::build::CmakeBuildOpts {
        generator: Some("No Such Generator".to_string()),
        ..Default::default()
    };
    let result =
        sim::build::build_model_cmake_with_opts(dir.path(), &[("model.c", STUB_MODEL_C)], &opts);

    let err = result.expect_err("unsupported generator must fail the configure step");
    assert!(matches!(&err, sim::build::BuildError::Configure { .. }));
    assert!(err.contains("cmake configure failed"), "error: {err}");
}

/// Driver default path: `llg` without flags must build through CMake and
/// produce the exact simulation output with exit 0.
#[test]
fn driver_default_uses_cmake() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("driver_default");
    std::fs::write(dir.path().join("counter.sv"), COUNTER_SV).expect("write source");

    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .args(["--top", "tb", "counter.sv"])
        .current_dir(dir.path());
    let output =
        sim_harness::run_command(&mut command, Duration::from_secs(60)).expect("llg should start");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    assert!(
        output.status.success(),
        "exit {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout, EXPECTED_STDOUT);
    assert!(
        dir.path().join("build/sim/tb/model.c").is_file(),
        "default model output is build/sim/<design>"
    );
    assert!(
        !dir.path().join("target").exists(),
        "the driver must not write the legacy target/ tree"
    );
}

/// Driver output and tool flags win over their environment fallbacks: every
/// fallback below is unusable, so the run succeeds only through the flags.
/// `--runtime-cache` names the suite's shared cache to avoid a runtime rebuild.
#[test]
fn driver_output_and_tool_flags_override_environment() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("driver_flags");
    std::fs::write(dir.path().join("counter.sv"), COUNTER_SV).expect("write source");
    let blocked = dir.path().join("not-a-directory");
    std::fs::write(&blocked, "").expect("write blocking file");
    let cache = sim::build::runtime_cache_dir_from_env()
        .unwrap_or_else(|| dir.path().join("runtime-cache"));

    let mut generate = Command::new(env!("CARGO_BIN_EXE_llg"));
    generate
        .args([
            "--top",
            "tb",
            "--gen-only",
            "--out-dir",
            "gen",
            "counter.sv",
        ])
        .current_dir(dir.path());
    let output =
        sim_harness::run_command(&mut generate, Duration::from_secs(60)).expect("llg should start");
    assert!(output.status.success(), "{output:?}");
    let printed = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        std::path::Path::new(printed.trim()),
        std::path::Path::new("gen/sim/tb")
    );
    assert!(dir.path().join("gen/sim/tb/CMakeLists.txt").is_file());

    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .args(["--top", "tb", "--out-dir", "out/run1", "--cmake", "cmake"])
        .args(["--cc", "cc", "--cflags", ""])
        .arg("--runtime-cache")
        .arg(&cache)
        .arg("counter.sv")
        .env(sim::build::RUNTIME_CACHE_DIR_ENV, &blocked)
        .env("LLG_CMAKE", dir.path().join("missing-cmake"))
        .env("LLG_CC", dir.path().join("missing-cc"))
        .env("LLG_CFLAGS", "-DX=\"quoted\"")
        .current_dir(dir.path());
    let output =
        sim_harness::run_command(&mut command, Duration::from_secs(120)).expect("llg should start");
    assert!(
        output.status.success(),
        "exit {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), EXPECTED_STDOUT);
    assert!(dir.path().join("out/run1/sim/tb/model.c").is_file());
    assert!(
        !dir.path().join("out/run1/llg-runtime-cache").exists(),
        "--runtime-cache must replace the <out-dir> default"
    );
}

/// Panic-safe `$LLG_CMAKE` scope: remembers the previous value and restores
/// (or removes) it on drop, so a panic in the guarded region cannot leak the
/// override to another test in this process.
struct EnvVarGuard {
    name: &'static str,
    saved: Option<String>,
}

impl EnvVarGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let saved = std::env::var(name).ok();
        std::env::set_var(name, value);
        Self { name, saved }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.saved {
            Some(v) => std::env::set_var(self.name, v),
            None => std::env::remove_var(self.name),
        }
    }
}

/// An unresolvable `$LLG_CMAKE` override must surface as an actionable error
/// naming the program and telling the user to install cmake.  The failure is
/// identical with or without a usable host cmake, so the case runs
/// unconditionally.  The env var mutation is process-global, hence serialized
/// under TEST_LOCK, and the guard restores it even if the build call panics.
#[test]
fn missing_cmake_error() {
    let _guard = TEST_LOCK.lock().unwrap();
    let dir = fresh_dir("missing_cmake");

    let _env = EnvVarGuard::set("LLG_CMAKE", "/nonexistent/llg-no-such-cmake");
    let result = sim::build::build_model_cmake(dir.path(), &[("model.c", STUB_MODEL_C)]);
    drop(_env);

    let err = result.expect_err("unresolvable LLG_CMAKE must fail the build");
    assert!(matches!(&err, sim::build::BuildError::CmakeLaunch { .. }));
    assert!(err.contains("not runnable"), "error: {err}");
    assert!(err.contains("install cmake"), "error: {err}");
    assert!(!err.contains("--direct-cc"), "error: {err}");
    assert!(
        err.contains("/nonexistent/llg-no-such-cmake"),
        "error: {err}"
    );
}
