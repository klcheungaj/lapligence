//! CLI integration tests for the lint pass every `llg` run performs, its
//! `--config` `[lint]` rule settings, `--lint-only`, `--lint-json` and
//! `-Werror`.
//!
//! Each test drives the real `llg` binary (via `CARGO_BIN_EXE_llg`)
//! in a fresh temp dir, so the generated `build/sim/` tree stays isolated
//! per test.

use std::path::Path;
use std::process::Command;

const SIM_BIN: &str = env!("CARGO_BIN_EXE_llg");

/// Design with one `unused-signal` finding (`b` is never used; `a` is
/// cont-assign driven and therefore exempt).  `$finish` lets the simulator
/// terminate when the lint pass is clean and codegen proceeds.
const UNUSED_SV: &str = r#"module unused;
    logic a;
    assign a = 1'b0;
    logic b;
    initial $finish;
endmodule
"#;

/// Delays are intentionally present so human lint and codegen exercise the
/// same owned semantic database.
const DELAYED_SV: &str = r#"module delayed;
    logic value = 1'b0;
    initial begin
        #3 value = 1'b1;
        $display("value=%0d time=%0t", value, $time);
        #2 $finish;
    end
endmodule
"#;

const CARELESS_MORE_SV: &str = r#"module careless_more (
    input logic a,
    input logic b,
    input logic [7:0] bus,
    input logic [1:0] sel,
    output logic sensitivity_result,
    output logic case_result,
    output logic floating_result,
    output logic range_result,
    output logic xz_result
);
    logic floating;
    assign floating_result = floating;
    always @(a) sensitivity_result = a & b;
    assign range_result = bus[8];
    assign xz_result = (a == 1'bx);
    always_comb begin
        case (sel)
            2'd0: case_result = 1'b0;
            2'd1: case_result = 1'b1;
            2'd1: case_result = a;
            default: case_result = b;
        endcase
    end
endmodule
"#;

const CARELESS_CONTROL_SV: &str = r#"module careless_control (
    input logic a,
    input logic b,
    input logic [1:0] sel,
    output logic empty_result,
    output logic condition_result,
    output logic casex_result
);
    logic condition_lhs;
    always @(*) empty_result = 1'b0;
    always_comb begin
        if ((condition_lhs = b))
            condition_result = 1'b1;
        else
            condition_result = 1'b0;
    end
    always_comb begin
        casex (sel)
            2'b1x: casex_result = 1'b1;
            default: casex_result = 1'b0;
        endcase
    end
endmodule
"#;

struct TempDir {
    path: std::path::PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("llg_lint_cli_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        // Diagnostics name the resolved source path (macOS /private/var,
        // Windows long names instead of 8.3 short names).
        let path = llg::ffi::platform::canonicalize(&path).expect("resolve temp dir");
        TempDir { path }
    }

    /// Write an `llg.toml` whose `[lint]` table holds `rules`, given as
    /// `[lint.rules.<id>]` tables.
    fn lint_config(&self, rules: &str) -> std::path::PathBuf {
        self.write("llg.toml", &format!("schema_version = 1\n{rules}"))
    }

    fn write(&self, name: &str, contents: &str) -> std::path::PathBuf {
        let p = self.path.join(name);
        std::fs::write(&p, contents).expect("write temp file");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Runs `llg`; the model's console output keeps the host's native newline,
/// so captured text is normalized to the LF the expectations use.
fn run_llg(dir: &Path, args: &[&str]) -> std::process::Output {
    let output = Command::new(SIM_BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("llg should start");
    std::process::Output {
        status: output.status,
        stdout: llg::ffi::platform::native_text_to_lf(output.stdout),
        stderr: llg::ffi::platform::native_text_to_lf(output.stderr),
    }
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Disabling a rule in the config suppresses its findings; with nothing left,
/// the lint pass prints nothing and the driver proceeds to codegen + run.
#[test]
fn cli_disabled_rule_suppresses_finding() {
    let dir = TempDir::new("disabled");
    dir.write("design.sv", UNUSED_SV);
    let cfg = dir.lint_config("[lint.rules.unused-signal]\nenabled = false\n");
    let out = run_llg(
        &dir.path,
        &[
            "--config",
            cfg.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(!err.contains("unused-signal"), "finding suppressed: {err}");
    assert!(
        !err.contains("lint:"),
        "a clean run prints no lint summary: {err}"
    );
}

#[test]
fn human_lint_reuses_delay_metadata_for_codegen() {
    let dir = TempDir::new("delayed_single_db");
    dir.write("design.sv", DELAYED_SV);
    let out = run_llg(&dir.path, &["--top", "delayed", "design.sv"]);
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "value=1 time=3000\n");
}

#[test]
fn simulation_and_lint_only_admit_time_literal_sources() {
    let source = r#"`timescale 1ns/100ps
module time_values;
    initial begin
        $display("literal=%.1f", 2.15ns);
        $finish;
    end
endmodule
"#;
    let dir = TempDir::new("time_lint");
    dir.write("design.sv", source);
    let output = run_llg(&dir.path, &["--top", "time_values", "design.sv"]);
    assert!(output.status.success(), "{}", stderr(&output));
    // IEEE 1800-2009 5.8 rounds 2.15ns to the local 100ps precision.
    assert_eq!(String::from_utf8_lossy(&output.stdout), "literal=2.2\n");
    let output = run_llg(
        &dir.path,
        &["--lint-only", "--top", "time_values", "design.sv"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stdout.is_empty(), "lint-only does not simulate");
}

/// Overriding a rule's severity to `error` promotes its findings; the lint
/// gate then aborts with exit code 1 before codegen.
#[test]
fn cli_severity_override_promotes_finding_to_error() {
    let dir = TempDir::new("severity");
    dir.write("design.sv", UNUSED_SV);
    let cfg = dir.lint_config("[lint.rules.unused-signal]\nseverity = \"error\"\n");
    let out = run_llg(
        &dir.path,
        &[
            "--config",
            cfg.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("[ERROR]"), "stderr: {err}");
    assert!(err.contains("unused-signal"), "stderr: {err}");
    assert!(err.contains("1 error(s)"), "stderr: {err}");
}

/// A missing config file aborts with a clear message before compiling.
#[test]
fn cli_missing_config_file_aborts() {
    let dir = TempDir::new("missing");
    dir.write("design.sv", UNUSED_SV);
    let missing = dir.path.join("does-not-exist.toml");
    let out = run_llg(
        &dir.path,
        &[
            "--config",
            missing.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("does not exist"), "stderr: {err}");
}

/// A malformed config file aborts with the parser's error messages.
#[test]
fn cli_malformed_config_aborts() {
    let dir = TempDir::new("malformed");
    dir.write("design.sv", UNUSED_SV);
    let cfg = dir.lint_config("[lint.rules.nope]\nenabled = true\n");
    let out = run_llg(
        &dir.path,
        &[
            "--config",
            cfg.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("unknown lint rule `nope`"), "stderr: {err}");
}

/// `--lint-json` emits one JSON object on stdout (the warning finding keeps
/// exit code 0) and suppresses the human-readable lint lines on stderr.
#[test]
fn cli_lint_json_stdout_emits_json() {
    let dir = TempDir::new("json_stdout");
    dir.write("design.sv", UNUSED_SV);
    let out = run_llg(&dir.path, &["--lint-json", "--top", "unused", "design.sv"]);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(
        stdout.contains("\"rule\": \"unused-signal\""),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("\"severity\": \"warning\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"errors\": 0"), "stdout: {stdout}");
    assert!(stdout.contains("\"warnings\": 1"), "stdout: {stdout}");
    assert!(stdout.contains("\"total\": 1"), "stdout: {stdout}");
    assert!(
        !err.contains("[WARNING]"),
        "human lint lines suppressed: {err}"
    );
    assert!(
        !err.contains("lint:"),
        "human lint summary suppressed: {err}"
    );
}

/// `--lint-json` + a severity override to `error` reports `"errors": 1` and
/// exits 1, with the JSON still emitted on stdout.
#[test]
fn cli_lint_json_severity_override_exits_1() {
    let dir = TempDir::new("json_severity");
    dir.write("design.sv", UNUSED_SV);
    let cfg = dir.lint_config("[lint.rules.unused-signal]\nseverity = \"error\"\n");
    let out = run_llg(
        &dir.path,
        &[
            "--lint-json",
            "--config",
            cfg.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(
        stdout.contains("\"severity\": \"error\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"errors\": 1"), "stdout: {stdout}");
    assert!(stdout.contains("\"warnings\": 0"), "stdout: {stdout}");
    assert!(stdout.contains("\"total\": 1"), "stdout: {stdout}");
    assert!(
        !err.contains("[ERROR]"),
        "human lint lines suppressed: {err}"
    );
}

/// `--lint-json <path>` writes the JSON object to the file and leaves stdout
/// empty.
#[test]
fn cli_lint_json_writes_file() {
    let dir = TempDir::new("json_file");
    dir.write("design.sv", UNUSED_SV);
    let report = dir.path.join("lint.json");
    let out = run_llg(
        &dir.path,
        &[
            "--lint-json",
            report.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(stdout.is_empty(), "stdout should be empty: {stdout}");
    let text = std::fs::read_to_string(&report).expect("lint JSON file exists");
    assert!(text.contains("\"rule\": \"unused-signal\""), "file: {text}");
    assert!(text.contains("\"errors\": 0"), "file: {text}");
}

/// `--lint-json` is report-only: it exits after emitting the JSON object and
/// never runs codegen/simulation, so stdout carries only the report even for a
/// design whose simulation would print.
#[test]
fn cli_lint_json_does_not_run_simulation() {
    let dir = TempDir::new("json_report_only");
    dir.write(
        "design.sv",
        r#"module unused;
    logic a;
    assign a = 1'b0;
    logic b;
    initial begin
        $display("SIM-OUTPUT");
        $finish;
    end
endmodule
"#,
    );
    let out = run_llg(&dir.path, &["--lint-json", "--top", "unused", "design.sv"]);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(
        stdout.contains("\"rule\": \"unused-signal\""),
        "stdout: {stdout}"
    );
    assert!(
        !stdout.contains("SIM-OUTPUT"),
        "simulation must not run: {stdout}"
    );
}

#[test]
fn cli_lint_json_exposes_expanded_shared_rules_and_severity_override() {
    let dir = TempDir::new("expanded_rules");
    dir.write("design.sv", CARELESS_MORE_SV);
    let cfg = dir.lint_config("[lint.rules.duplicate-case-item]\nseverity = \"error\"\n");
    let out = run_llg(
        &dir.path,
        &[
            "--lint-json",
            "--config",
            cfg.to_str().unwrap(),
            "--top",
            "careless_more",
            "design.sv",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = stderr(&out);
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("valid lint JSON");

    assert_eq!(
        out.status.code(),
        Some(1),
        "stderr: {err}\nstdout: {stdout}"
    );
    for rule in [
        "undriven-signal",
        "incomplete-sensitivity-list",
        "out-of-range-select",
        "xz-logical-equality",
        "duplicate-case-item",
    ] {
        assert!(
            stdout.contains(&format!("\"rule\": \"{rule}\"")),
            "missing {rule}: {stdout}"
        );
    }
    let duplicate = report["diagnostics"]
        .as_array()
        .and_then(|diags| {
            diags
                .iter()
                .find(|diag| diag["rule"] == "duplicate-case-item")
        })
        .expect("duplicate-case-item diagnostic");
    assert_eq!(duplicate["severity"], "error", "report: {report}");
    assert!(stdout.contains("\"errors\": 1"), "stdout: {stdout}");
}

#[test]
fn cli_lint_json_exposes_control_rules_and_honors_config() {
    let dir = TempDir::new("control_rules");
    dir.write("design.sv", CARELESS_CONTROL_SV);

    let baseline = run_llg(
        &dir.path,
        &["--lint-json", "--top", "careless_control", "design.sv"],
    );
    let baseline_stdout = String::from_utf8_lossy(&baseline.stdout).into_owned();
    let baseline_stderr = stderr(&baseline);
    assert!(baseline.status.success(), "stderr: {baseline_stderr}");
    let baseline_report: serde_json::Value =
        serde_json::from_str(&baseline_stdout).expect("valid baseline lint JSON");
    for rule in [
        "empty-implicit-sensitivity",
        "assignment-in-condition",
        "casex-statement",
    ] {
        let diag = baseline_report["diagnostics"]
            .as_array()
            .and_then(|diags| diags.iter().find(|diag| diag["rule"] == rule))
            .unwrap_or_else(|| panic!("missing {rule}: {baseline_report}"));
        assert_eq!(diag["severity"], "warning", "{rule}: {baseline_report}");
        assert!(
            diag["file"]
                .as_str()
                .is_some_and(|file| std::path::Path::new(file).ends_with("design.sv")),
            "{rule} should retain the real source path: {baseline_report}"
        );
    }

    let cfg = dir.lint_config(
        "[lint.rules.empty-implicit-sensitivity]\n\
         enabled = false\n\
         [lint.rules.casex-statement]\n\
         severity = \"error\"\n",
    );
    let configured = run_llg(
        &dir.path,
        &[
            "--lint-json",
            "--config",
            cfg.to_str().unwrap(),
            "--top",
            "careless_control",
            "design.sv",
        ],
    );
    let configured_stdout = String::from_utf8_lossy(&configured.stdout).into_owned();
    let configured_stderr = stderr(&configured);
    let configured_report: serde_json::Value =
        serde_json::from_str(&configured_stdout).expect("valid configured lint JSON");
    assert_eq!(
        configured.status.code(),
        Some(1),
        "stderr: {configured_stderr}\nstdout: {configured_stdout}"
    );
    assert!(
        configured_report["diagnostics"]
            .as_array()
            .is_some_and(|diags| diags
                .iter()
                .all(|diag| diag["rule"] != "empty-implicit-sensitivity")),
        "disabled rule remained: {configured_report}"
    );
    let casex = configured_report["diagnostics"]
        .as_array()
        .and_then(|diags| diags.iter().find(|diag| diag["rule"] == "casex-statement"))
        .expect("configured casex-statement diagnostic");
    assert_eq!(casex["severity"], "error", "{configured_report}");
}

/// Every run lints: warnings print with a count line and the model still
/// builds and runs.
#[test]
fn lint_runs_without_an_option_and_warnings_do_not_stop_the_run() {
    let dir = TempDir::new("always");
    dir.write("design.sv", UNUSED_SV);
    let out = run_llg(&dir.path, &["--top", "unused", "design.sv"]);
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(
        err.contains("[WARNING] unused-signal"),
        "lint ran without an option: {err}"
    );
    assert!(err.contains("lint: 0 error(s), 1 warning(s)"), "{err}");
    assert!(err.contains("$finish"), "the model ran: {err}");
}

/// `-Werror` reports warnings as errors, which stop the run before codegen;
/// `-Wno-error` restores a configured `warnings_as_errors = true`.
#[test]
fn werror_turns_lint_warnings_into_errors() {
    let dir = TempDir::new("werror");
    dir.write("design.sv", UNUSED_SV);
    let out = run_llg(&dir.path, &["-Werror", "--top", "unused", "design.sv"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("[ERROR] unused-signal"), "{err}");
    assert!(err.contains("lint: 1 error(s), 0 warning(s)"), "{err}");
    assert!(!err.contains("$finish"), "no model ran: {err}");
    assert!(
        !dir.path.join("build/sim").exists(),
        "no model was generated"
    );

    let out = run_llg(
        &dir.path,
        &["-Werror", "--lint-json", "--top", "unused", "design.sv"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(1), "stdout: {stdout}");
    assert!(stdout.contains("\"errors\": 1"), "stdout: {stdout}");

    let cfg = dir.write(
        "llg.toml",
        "schema_version = 1\n[lint]\nwarnings_as_errors = true\n",
    );
    let config = cfg.to_str().unwrap();
    let out = run_llg(
        &dir.path,
        &["--config", config, "--top", "unused", "design.sv"],
    );
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let out = run_llg(
        &dir.path,
        &[
            "--config",
            config,
            "-Wno-error",
            "--top",
            "unused",
            "design.sv",
        ],
    );
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

/// `--lint-only` reports findings and exits without emitting C: 0 when only
/// warnings remain, 1 on errors, and `lint: clean` for a clean design.
#[test]
fn lint_only_stops_before_code_generation() {
    let dir = TempDir::new("lint_only");
    dir.write("design.sv", UNUSED_SV);
    let out = run_llg(&dir.path, &["--lint-only", "--top", "unused", "design.sv"]);
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(err.contains("[WARNING] unused-signal"), "{err}");
    assert!(out.stdout.is_empty(), "nothing ran: {err}");
    assert!(!dir.path.join("build").exists(), "no model was generated");

    let out = run_llg(
        &dir.path,
        &["--lint-only", "-Werror", "--top", "unused", "design.sv"],
    );
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));

    dir.write("clean.sv", DELAYED_SV);
    let out = run_llg(&dir.path, &["--lint-only", "--top", "delayed", "clean.sv"]);
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert_eq!(err, "lint: clean\n");

    let cfg = dir.write("llg.toml", "schema_version = 1\n[lint]\nonly = true\n");
    let config = cfg.to_str().unwrap();
    let out = run_llg(
        &dir.path,
        &["--config", config, "--top", "delayed", "clean.sv"],
    );
    assert!(out.status.success() && out.stdout.is_empty(), "{out:?}");
    let out = run_llg(
        &dir.path,
        &[
            "--config",
            config,
            "--no-lint-only",
            "--top",
            "delayed",
            "clean.sv",
        ],
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "value=1 time=3000\n");
}

/// A lint error (default severity) stops the run before codegen.
#[test]
fn default_lint_errors_stop_the_run() {
    let dir = TempDir::new("lint_error");
    dir.write(
        "design.sv",
        r#"module mixed;
    logic clk = 1'b0, a, b;
    always @(posedge clk) begin
        a = 1'b0;
        b <= 1'b1;
    end
    initial begin
        $display("SIM-OUTPUT");
        $finish;
    end
endmodule
"#,
    );
    let out = run_llg(&dir.path, &["--top", "mixed", "design.sv"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("[ERROR] mixed-assignments"), "{err}");
    assert!(out.stdout.is_empty(), "the model must not run");
}
