//! Integration tests for the CMake-based model builder (`sim::build`) — the
//! only supported model-build path — and the `llg` driver's build-time
//! `--generator` flag.
//!
//! Surelog writes `slpp_all/` into the process working directory, so the
//! library-level cases run with the CWD pointed at a fresh harness temp dir;
//! the driver-level cases spawn `llg` with its own temp CWD instead.
//! Every test holds one shared mutex: besides serializing Surelog (like the
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
const STUB_MODEL_C: &str = "int main(void) { return 0; }\n";

fn fresh_dir(tag: &str) -> sim_harness::TempDir {
    sim_harness::TempDir::new(&format!("sim-cmake-{tag}")).expect("create temp dir")
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

/// Compile + codegen the counter design (Surelog needs the CWD juggling).
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
    let design = out.uhdm_design().ok_or("no UHDM design")?;
    sim::codegen::generate(design).map_err(|e| format!("codegen: {e}"))
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
