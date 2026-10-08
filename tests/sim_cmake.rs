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

#[path = "support/c_compiler.rs"]
mod c_compiler;
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
const STUB_MODEL_C: &str = "#define LLG_MODEL_VALUE_ABI 5\n#define LLG_MODEL_VALUE_BACKEND 1\n#define LLG_MODEL_COMPACT_KERNELS 1\nint main(void) { return 0; }\n";

fn fresh_dir(tag: &str) -> sim_harness::TempDir {
    sim_harness::TempDir::new(&format!("sim-cmake-{tag}")).expect("create temp dir")
}

#[test]
fn generated_release_has_one_explicit_optimization_level() {
    use sim::build::{CmakeBuildOpts, ModelOptLevel};
    let dir = fresh_dir("optimization");
    for level in [
        ModelOptLevel::O0,
        ModelOptLevel::O1,
        ModelOptLevel::O2,
        ModelOptLevel::O3,
        ModelOptLevel::Os,
    ] {
        let opts = CmakeBuildOpts {
            model_opt_level: level,
            ..Default::default()
        };
        sim::build::generate_model_sources_with_opts(
            dir.path(),
            &[("model.c", STUB_MODEL_C)],
            &opts,
        )
        .unwrap();
        let cmake = std::fs::read_to_string(dir.path().join("CMakeLists.txt")).unwrap();
        assert!(cmake.contains("set(CMAKE_C_FLAGS_RELEASE \"-DNDEBUG\")"));
        assert!(cmake.contains("set(CMAKE_C_FLAGS_RELEASE \"/DNDEBUG\")"));
        assert_eq!(cmake.matches(level.gnu_flag()).count(), 1);
        assert_eq!(cmake.matches(level.msvc_flag()).count(), 1);
        if level != ModelOptLevel::O3 {
            assert!(!cmake.contains("-O3"));
        }
    }
}

#[test]
#[cfg(unix)]
fn configured_release_preserves_flag_order_without_accumulating_levels() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("optimization-configure");
    let opts = sim::build::CmakeBuildOpts {
        model_opt_level: sim::build::ModelOptLevel::O1,
        ..Default::default()
    };
    sim::build::generate_model_sources_with_opts(dir.path(), &[("model.c", STUB_MODEL_C)], &opts)
        .unwrap();
    for extra in ["", "", "-O0"] {
        let output = Command::new("cmake")
            .arg("-S")
            .arg(dir.path())
            .arg("-B")
            .arg(dir.path().join("build"))
            .arg("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON")
            .arg(format!("-DCMAKE_C_FLAGS:STRING={extra}"))
            .output()
            .expect("configure generated project");
        assert!(output.status.success(), "{output:?}");
        let commands: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("build/compile_commands.json")).unwrap(),
        )
        .unwrap();
        for entry in commands.as_array().unwrap() {
            let command = entry["command"].as_str().unwrap();
            let flags: Vec<_> = command.split_whitespace().collect();
            assert_eq!(
                flags.iter().filter(|flag| **flag == "-O1").count(),
                1,
                "{command}"
            );
            assert!(!flags.contains(&"-O3"), "{command}");
            assert!(flags.contains(&"-DNDEBUG"), "{command}");
            if !extra.is_empty() {
                let base = flags.iter().position(|flag| *flag == "-O1").unwrap();
                let user = flags.iter().position(|flag| *flag == "-O0").unwrap();
                assert!(base < user, "{command}");
            } else {
                assert_eq!(
                    flags.iter().filter(|flag| flag.starts_with("-O")).count(),
                    1,
                    "{command}"
                );
            }
        }
    }
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

    let (header, source) =
        sim::rt::value_sources_for(sim::value_backend::ValueConfig::default().backend);
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
        cmake.contains("add_library(llg_runtime STATIC llg_value.c llg_rng.c llg_co.c llg_rt.c llg_random.c llg_vpi.c")
            && cmake.contains("add_executable(sim model.c)")
    );
    assert!(cmake.contains("project(llg_sim_model C)"));
    assert!(!cmake.contains("project(llg_sim_model C ASM)"));
    // The CMake estimate must match the runtime header's 640 KiB default.
    assert!(cmake.contains("set(LLG_HOST_STACK_ESTIMATE_BYTES 655360 CACHE STRING"));
    let (runtime_header, _) = sim::rt::runtime_sources();
    assert!(
        runtime_header.contains("#define LLG_HOST_STACK_MEASURED_BYTES (367u * 1024u)")
            && runtime_header
                .contains("#define LLG_HOST_STACK_FOREIGN_HEADROOM_BYTES (256u * 1024u)")
    );
    assert!(cmake.contains("set(LLG_HOST_STACK_RESERVE_BYTES 1048576)"));
    assert!(
        cmake.contains("target_link_options(sim PRIVATE /STACK:${LLG_HOST_STACK_RESERVE_BYTES})")
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
    let stale_model = r#"#define LLG_MODEL_VALUE_ABI 5
#define LLG_MODEL_VALUE_BACKEND 1
#define LLG_MODEL_COMPACT_KERNELS 1
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

    let compiler = c_compiler::host_c_compiler();
    let msvc = c_compiler::is_msvc(&compiler);
    if !c_compiler::c_compiler_available(&compiler) {
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

    let object = if msvc { "obj" } else { "o" };
    let stale_object = link_dir.join(format!("stale.{object}"));
    let model_object = link_dir.join(format!("model.{object}"));
    for (directory, source, output) in [
        (&stale_dir, "llg_co.c", &stale_object),
        (&link_dir, "model.c", &model_object),
    ] {
        let mut command = Command::new(&compiler);
        command.current_dir(directory);
        if msvc {
            command
                .args(["/nologo", "/std:c11", "/I.", "/c", source])
                .arg(format!("/Fo{}", output.display()));
        } else {
            command
                .args(["-std=c11", "-I.", "-c", source, "-o"])
                .arg(output);
        }
        let result = command
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
    let mut link = Command::new(&compiler);
    link.args([model_object.as_os_str(), stale_object.as_os_str()]);
    if msvc {
        // link.exe reports the unresolved symbol on stdout (LNK2019).
        link.arg("/nologo")
            .arg(format!("/Fe{}", executable.display()));
    } else {
        link.arg("-o").arg(&executable);
    }
    let link = link.output().expect("link mixed coroutine ABI probe");
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
    sim::build::generate_model_sources(&project, &gen.sources())
        .expect("generate self-contained model sources");
    assert_eq!(
        std::fs::read_to_string(project.join("model.symbols.tsv")).unwrap(),
        gen.symbols_tsv
    );

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

#[test]
fn generated_symbol_maps_are_replaced_and_pruned_with_the_source_set() {
    let dir = fresh_dir("symbol-map");
    let sources = [
        ("model.c", STUB_MODEL_C),
        ("model.symbols.tsv", "short\toriginal_long_name\n"),
    ];
    sim::build::generate_model_sources(dir.path(), &sources).unwrap();
    let map = dir.path().join("model.symbols.tsv");
    assert_eq!(std::fs::read_to_string(&map).unwrap(), sources[1].1);
    let cmake = std::fs::read_to_string(dir.path().join("CMakeLists.txt")).unwrap();
    assert!(!cmake.contains("model.symbols.tsv"));
    sim::build::generate_model_sources(
        dir.path(),
        &[("model.c", STUB_MODEL_C), ("model.symbols.tsv", "")],
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&map).unwrap(), "");
    sim::build::generate_model_sources(dir.path(), &[("model.c", STUB_MODEL_C)]).unwrap();
    assert!(!map.exists());
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

/// `$LLG_C_LAUNCHER` selects the compiler launcher when the option is unset,
/// reaching the runtime archive and the model compile; an explicit option wins
/// and an explicit empty option suppresses the variable. Recording shell
/// scripts stand in for `ccache` and forward to the real compiler.
#[cfg(unix)]
#[test]
fn launcher_environment_variable_and_option_precedence() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("launcher-env");
    let recorder = |name: &str| {
        let log = dir.path().join(format!("{name}.log"));
        let script = dir.path().join(format!("{name}.sh"));
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec \"$@\"\n",
                log.display()
            ),
        )
        .expect("write launcher");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("mark launcher executable");
        (script.to_string_lossy().into_owned(), log)
    };
    let (from_env, env_log) = recorder("from-env");
    let (from_option, option_log) = recorder("from-option");
    let build = |name: &str, launcher: Option<String>| {
        let model_dir = dir.path().join(name);
        std::fs::create_dir(&model_dir).expect("create model directory");
        sim_harness::with_cwd(dir.path(), || {
            let generated = compile_counter(&model_dir)?;
            let opts = sim::build::CmakeBuildOpts {
                launcher,
                runtime_cache_dir: Some(dir.path().join(format!("{name}-runtime-cache"))),
                ..Default::default()
            };
            sim::build::build_model_cmake_with_opts(
                &model_dir,
                &[("model.c", generated.model_c.as_str())],
                &opts,
            )
            .map_err(|error| format!("cmake build: {error}"))
        })
        .expect("launcher build should succeed")
    };
    let logged = |log: &std::path::Path| std::fs::read_to_string(log).unwrap_or_default();

    let _env = EnvVarGuard::set("LLG_C_LAUNCHER", &from_env);
    let executable = build("env-only", None);
    assert_eq!(run_sim(&executable).unwrap(), EXPECTED_STDOUT);
    let env_calls = logged(&env_log);
    assert!(env_calls.contains("model.c"), "model compile: {env_calls}");
    assert!(
        env_calls.contains("llg_runtime") || env_calls.contains("llg_rt"),
        "runtime archive compile: {env_calls}"
    );

    let calls_before = env_calls.lines().count();
    build("option-wins", Some(from_option));
    assert!(logged(&option_log).contains("model.c"));
    assert_eq!(logged(&env_log).lines().count(), calls_before);

    build("empty-option", Some(String::new()));
    assert_eq!(logged(&env_log).lines().count(), calls_before);
}

/// The driver ranks the launcher as `--launcher` > `$LLG_C_LAUNCHER` >
/// `build.launcher` (the shared command line > environment > config rule).
/// Each case uses a fresh out directory and runtime cache so its compiles run.
#[cfg(unix)]
#[test]
fn driver_launcher_precedence_is_cli_then_environment_then_config() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("driver-launcher");
    std::fs::write(dir.path().join("counter.sv"), COUNTER_SV).expect("write source");
    let recorder = |name: &str| {
        let log = dir.path().join(format!("{name}.log"));
        let script = dir.path().join(format!("{name}.sh"));
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec \"$@\"\n",
                log.display()
            ),
        )
        .expect("write launcher");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("mark launcher executable");
        (script.to_string_lossy().into_owned(), log)
    };
    let (from_config, config_log) = recorder("from-config");
    let (from_env, env_log) = recorder("from-env");
    let (from_cli, cli_log) = recorder("from-cli");
    std::fs::write(
        dir.path().join("llg.toml"),
        format!("schema_version = 1\n[build]\nlauncher = \"{from_config}\"\n"),
    )
    .expect("write config");
    let run = |name: &str, environment: Option<&str>, cli_launcher: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .args(["--config", "llg.toml", "--top", "tb", "--out-dir", name])
            .arg("--runtime-cache")
            .arg(dir.path().join(format!("{name}-runtime-cache")));
        if let Some(launcher) = cli_launcher {
            command.args(["--launcher", launcher]);
        }
        command
            .arg("counter.sv")
            .env_remove(sim::build::C_LAUNCHER_ENV)
            .current_dir(dir.path());
        if let Some(launcher) = environment {
            command.env(sim::build::C_LAUNCHER_ENV, launcher);
        }
        let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
            .expect("llg should start");
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), EXPECTED_STDOUT);
    };
    let used = |log: &std::path::Path| log.is_file();

    run("config-only", None, None);
    assert!(used(&config_log) && !used(&env_log) && !used(&cli_log));
    std::fs::remove_file(&config_log).unwrap();

    run("env-over-config", Some(&from_env), None);
    assert!(used(&env_log) && !used(&config_log) && !used(&cli_log));
    std::fs::remove_file(&env_log).unwrap();

    run("cli-over-env", Some(&from_env), Some(&from_cli));
    assert!(used(&cli_log) && !used(&env_log) && !used(&config_log));
    std::fs::remove_file(&cli_log).unwrap();

    run("empty-env-is-unset", Some(""), None);
    assert!(used(&config_log) && !used(&env_log) && !used(&cli_log));
}

/// Both `cmake --build` invocations (runtime archive and model) must carry
/// `--parallel <N>`. A recording wrapper stands in for the cmake program and
/// forwards to the real one.
#[cfg(unix)]
#[test]
fn model_and_runtime_builds_pass_parallel_jobs() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("parallel-jobs");
    let log = dir.path().join("cmake-args.log");
    let wrapper = dir.path().join("cmake-wrapper.sh");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec cmake \"$@\"\n",
            log.display()
        ),
    )
    .expect("write wrapper");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))
        .expect("mark wrapper executable");
    let model_dir = dir.path().join("model");
    std::fs::create_dir(&model_dir).expect("create model directory");
    let result = sim_harness::with_cwd(dir.path(), || {
        let generated = compile_counter(&model_dir)?;
        let opts = sim::build::CmakeBuildOpts {
            cmake: Some(wrapper.to_string_lossy().into_owned()),
            runtime_cache_dir: Some(dir.path().join("runtime-cache")),
            build_jobs: Some(3),
            ..Default::default()
        };
        sim::build::build_model_cmake_with_opts(
            &model_dir,
            &[("model.c", generated.model_c.as_str())],
            &opts,
        )
        .map_err(|error| format!("cmake build: {error}"))
    });
    result.expect("wrapped build should succeed");

    let log = std::fs::read_to_string(&log).expect("wrapper log");
    let builds: Vec<&str> = log
        .lines()
        .filter(|line| line.starts_with("--build "))
        .collect();
    assert_eq!(builds.len(), 2, "runtime and model builds: {log}");
    assert!(
        builds
            .iter()
            .all(|line| line.contains("--config Release --parallel 3")),
        "{log}"
    );
    assert!(
        builds
            .iter()
            .any(|line| line.ends_with("--target llg_runtime")),
        "{log}"
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

/// The `CMakeFiles/<version>` platform directory of a configured tree.
fn platform_dir(build: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(build.join("CMakeFiles"))
        .expect("configured tree has CMakeFiles")
        .map(|entry| entry.expect("read CMakeFiles").path())
        .find(|path| path.join("CMakeSystem.cmake").is_file())
        .expect("configured tree has a platform directory")
}

/// Whether CMake identified the compiler in this tree instead of reusing a
/// seed: detection leaves its `CompilerIdC` scratch project behind.
fn ran_compiler_detection(build: &std::path::Path) -> bool {
    platform_dir(build).join("CompilerIdC").exists()
}

/// Published toolchain seeds (entries with a `ready` marker) under a runtime
/// cache root.
fn published_seeds(cache: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut seeds: Vec<_> = std::fs::read_dir(cache.join(sim::build::TOOLCHAIN_SEED_DIR))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir() && path.join("ready").is_file())
                .collect()
        })
        .unwrap_or_default();
    seeds.sort();
    seeds
}

/// Build the counter in `model_dir` with `opts` and return its output.
fn build_counter(
    dir: &std::path::Path,
    model_dir: &std::path::Path,
    opts: &sim::build::CmakeBuildOpts,
) -> Result<String, String> {
    std::fs::create_dir_all(model_dir).map_err(|error| error.to_string())?;
    sim_harness::with_cwd(dir, || {
        let generated = compile_counter(model_dir)?;
        let exe = sim::build::build_model_cmake_with_opts(
            model_dir,
            &[("model.c", generated.model_c.as_str())],
            opts,
        )
        .map_err(|error| format!("cmake build: {error}"))?;
        run_sim(&exe)
    })
}

/// Fresh model trees reuse one published toolchain detection: the runtime
/// archive and both models build and run correctly, and none of their trees
/// identifies the compiler again (the probe that published the seed did).
#[test]
fn fresh_model_trees_reuse_the_toolchain_detection_seed() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("toolchain-seed");
    let cache = dir.path().join("runtime-cache");
    let opts = sim::build::CmakeBuildOpts {
        runtime_cache_dir: Some(cache.clone()),
        ..Default::default()
    };
    for model in ["first", "second"] {
        let model_dir = dir.path().join(model);
        let stdout = build_counter(dir.path(), &model_dir, &opts).expect("seeded model runs");
        assert_eq!(stdout, EXPECTED_STDOUT, "{model}");
        assert!(
            !ran_compiler_detection(&model_dir.join("build")),
            "{model} model tree identified the compiler again"
        );
    }
    let seeds = published_seeds(&cache);
    assert_eq!(seeds.len(), 1, "one toolchain, one seed: {seeds:?}");
    assert!(!seeds[0].join("rejected").exists());
    assert!(!seeds[0].join("probe").exists(), "probe trees are removed");
    // Consumers enumerate runtime archives as the root's children with a
    // `ready` marker; seeds must stay out of that set (they live under one
    // `cmake-toolchain/` child without a marker).
    let ready: Vec<_> = std::fs::read_dir(&cache)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|entry| entry.join("ready").is_file())
        .collect();
    assert_eq!(
        ready.len(),
        1,
        "only the runtime archive is ready: {ready:?}"
    );
    assert!(
        ready[0].join("build").join("CMakeCache.txt").is_file(),
        "{ready:?}"
    );
    let directories: Vec<_> = std::fs::read_dir(&cache)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    assert!(
        directories
            .iter()
            .all(|path| path == &ready[0] || path.ends_with(sim::build::TOOLCHAIN_SEED_DIR)),
        "unexpected cache root children: {directories:?}"
    );
    assert!(!ran_compiler_detection(&ready[0].join("build")));

    // An existing compatible tree is reconfigured incrementally as before.
    let stdout = build_counter(dir.path(), &dir.path().join("first"), &opts)
        .expect("incremental rebuild runs");
    assert_eq!(stdout, EXPECTED_STDOUT);
}

/// A different compiler is a different key: its fresh tree never reuses the
/// first compiler's detection, and records its own compiler.
#[cfg(unix)]
#[test]
fn a_changed_compiler_does_not_reuse_another_toolchain_seed() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let (Some(gcc), Some(clang)) = (find_on_path("gcc"), find_on_path("clang")) else {
        eprintln!("SKIP: gcc and clang are both required");
        return;
    };
    let dir = fresh_dir("toolchain-seed-compilers");
    let cache = dir.path().join("runtime-cache");
    for (name, compiler, id) in [("gcc", &gcc, "GNU"), ("clang", &clang, "Clang")] {
        let opts = sim::build::CmakeBuildOpts {
            runtime_cache_dir: Some(cache.clone()),
            cc: Some(compiler.to_string_lossy().into_owned()),
            ..Default::default()
        };
        let model_dir = dir.path().join(name);
        let stdout = build_counter(dir.path(), &model_dir, &opts).expect("model runs");
        assert_eq!(stdout, EXPECTED_STDOUT, "{name}");
        let build = model_dir.join("build");
        assert!(!ran_compiler_detection(&build), "{name}");
        let compiler_file =
            std::fs::read_to_string(platform_dir(&build).join("CMakeCCompiler.cmake")).unwrap();
        assert!(
            compiler_file.contains(&format!("set(CMAKE_C_COMPILER_ID \"{id}\")")),
            "{name} tree must record its own compiler: {compiler_file}"
        );
    }
    assert_eq!(published_seeds(&cache).len(), 2, "one seed per compiler");
}

/// A seed that breaks configuration is not trusted again: the build retries
/// from scratch without it, succeeds, and the key is marked rejected so later
/// fresh trees configure unseeded.
#[test]
fn a_failing_seed_falls_back_to_a_clean_configure_and_is_rejected() {
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = fresh_dir("toolchain-seed-stale");
    let cache = dir.path().join("runtime-cache");
    let opts = sim::build::CmakeBuildOpts {
        runtime_cache_dir: Some(cache.clone()),
        ..Default::default()
    };
    let stdout = build_counter(dir.path(), &dir.path().join("first"), &opts).expect("first model");
    assert_eq!(stdout, EXPECTED_STDOUT);
    let seeds = published_seeds(&cache);
    assert_eq!(seeds.len(), 1, "{seeds:?}");
    let platform = std::fs::read_dir(seeds[0].join("platform"))
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .next()
        .expect("published platform directory");
    std::fs::write(
        platform.join("CMakeCCompiler.cmake"),
        "message(FATAL_ERROR \"corrupt llg toolchain seed\")\n",
    )
    .unwrap();

    let second = dir.path().join("second");
    let stdout = build_counter(dir.path(), &second, &opts).expect("clean retry builds");
    assert_eq!(stdout, EXPECTED_STDOUT);
    assert!(
        ran_compiler_detection(&second.join("build")),
        "the retry configures from scratch"
    );
    let rejected = std::fs::read_to_string(seeds[0].join("rejected")).expect("seed rejected");
    assert!(
        rejected.starts_with("a seeded configure failed"),
        "{rejected}"
    );

    let third = dir.path().join("third");
    let stdout = build_counter(dir.path(), &third, &opts).expect("unseeded model");
    assert_eq!(stdout, EXPECTED_STDOUT);
    assert!(ran_compiler_detection(&third.join("build")));
}

/// Compiler self-reports are probed once per toolchain and process: a second
/// model build spawns no `--version`/`-dumpmachine` probe, and a replaced
/// compiler file is probed again. The compiler is a counting wrapper script
/// around the host `cc`; CMake's own compiler calls never pass one lone
/// probe argument, so the log holds only llg's probes.
#[cfg(unix)]
#[test]
fn compiler_probes_run_once_per_toolchain_and_process() {
    use std::os::unix::fs::PermissionsExt;
    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let Some(host_cc) = find_on_path("cc") else {
        eprintln!("SKIP: cc is required");
        return;
    };
    let dir = fresh_dir("compiler-probe-memo");
    let log = dir.path().join("probes.log");
    let wrapper = dir.path().join("counting-cc");
    let write_wrapper = |revision: &str| {
        std::fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\n# revision {revision}\nif [ $# -eq 1 ]; then case \"$1\" in --version|-dumpmachine|/Bv) echo \"$1\" >> '{}';; esac; fi\nexec '{}' \"$@\"\n",
                log.display(),
                host_cc.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    let probes = || {
        std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    write_wrapper("1");
    let opts = sim::build::CmakeBuildOpts {
        runtime_cache_dir: Some(dir.path().join("runtime-cache")),
        cc: Some(wrapper.to_string_lossy().into_owned()),
        ..Default::default()
    };

    let stdout =
        build_counter(dir.path(), &dir.path().join("first"), &opts).expect("first model runs");
    assert_eq!(stdout, EXPECTED_STDOUT);
    assert_eq!(probes(), ["--version", "-dumpmachine"], "one probe set");

    let stdout =
        build_counter(dir.path(), &dir.path().join("second"), &opts).expect("second model runs");
    assert_eq!(stdout, EXPECTED_STDOUT);
    assert_eq!(probes().len(), 2, "the second build reuses the probe");

    // A replaced compiler file (other size and modification time) is probed
    // again; its self-report is unchanged, so the runtime archive is reused.
    std::thread::sleep(Duration::from_millis(20));
    write_wrapper("2, upgraded");
    let stdout =
        build_counter(dir.path(), &dir.path().join("third"), &opts).expect("third model runs");
    assert_eq!(stdout, EXPECTED_STDOUT);
    assert_eq!(
        probes(),
        ["--version", "-dumpmachine", "--version", "-dumpmachine"],
        "a changed compiler is re-probed"
    );
}

/// Program `name` on the current `PATH`, if any.
#[cfg(unix)]
fn find_on_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

/// The default generator is Ninja. Without a `ninja` program the driver must
/// fail at the first configure with an actionable message (install Ninja or
/// select another generator) instead of retrying and reporting a generic
/// configure failure. `PATH` holds only a recording cmake wrapper, so CMake
/// cannot find `ninja`.
#[cfg(unix)]
#[test]
fn missing_ninja_reports_an_actionable_error_without_a_retry() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = TEST_LOCK.lock().unwrap();
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let (Some(cmake), Some(cc)) = (find_on_path("cmake"), find_on_path("cc")) else {
        eprintln!("SKIP: cmake or cc is not on PATH");
        return;
    };
    let dir = fresh_dir("missing-ninja");
    std::fs::write(dir.path().join("counter.sv"), COUNTER_SV).expect("write source");
    let tools = dir.path().join("tools");
    std::fs::create_dir(&tools).expect("create tool directory");
    let log = dir.path().join("cmake-args.log");
    let wrapper = tools.join("cmake");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
            log.display(),
            cmake.display()
        ),
    )
    .expect("write wrapper");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))
        .expect("mark wrapper executable");

    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .args(["--top", "tb", "--cc"])
        .arg(&cc)
        .arg("--runtime-cache")
        .arg(dir.path().join("runtime-cache"))
        .arg("counter.sv")
        .env("PATH", &tools)
        .env_remove("CMAKE_GENERATOR")
        .env_remove("LLG_CMAKE")
        .current_dir(dir.path());
    let output =
        sim_harness::run_command(&mut command, Duration::from_secs(60)).expect("llg should start");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "build without ninja must fail");
    assert!(
        stderr.contains("no build program for generator `Ninja`"),
        "{stderr}"
    );
    assert!(stderr.contains("install Ninja"), "{stderr}");
    assert!(stderr.contains("CMAKE_GENERATOR"), "{stderr}");
    let log = std::fs::read_to_string(&log).expect("wrapper log");
    // The toolchain-seed probe may try first; the runtime project itself is
    // configured exactly once.
    let configures: Vec<&str> = log
        .lines()
        .filter(|line| line.starts_with("-S ") && !line.contains("/cmake-toolchain/"))
        .collect();
    assert_eq!(configures.len(), 1, "no from-scratch retry: {log}");
    assert!(configures[0].contains("-G Ninja"), "{log}");
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
            "--model-opt-level",
            "Os",
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
    let generated = std::fs::read_to_string(dir.path().join("gen/sim/tb/CMakeLists.txt")).unwrap();
    assert!(generated.contains("-Os -Wall"));
    assert!(!generated.contains("-O3"));

    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .args(["--top", "tb", "--out-dir", "out/run1", "--cmake", "cmake"])
        .args([
            "--cc",
            sim::build::DEFAULT_C_COMPILER,
            "--cflags",
            "",
            "--model-opt-level",
            "O1",
            "--build-jobs",
            "2",
        ])
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
