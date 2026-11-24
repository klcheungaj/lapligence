//! CLI integration tests for the lint pass every `llg` run performs, its
//! `--config` `[lint]` rule settings, `--lint-only`, `--lint-json` and
//! `-Werror`.
//!
//! Each test drives the real `llg` binary (via `CARGO_BIN_EXE_llg`)
//! in a fresh temp dir, so the generated `build/sim/` tree stays isolated
//! per test.

use std::path::Path;
use std::process::Command;

use llg::core::report::{self, Severity};

/// Whether `err` reports a lint finding of `severity` for `rule` (any rule
/// when `None`) whose text starts with `text`; lines are parsed with the
/// message layout (`llg::core::report`).
fn lint_finding(err: &str, severity: Severity, rule: Option<&str>, text: &str) -> bool {
    err.lines().any(|line| {
        report::parse(line).is_some_and(|message| {
            message.tool() == "LINT"
                && message.severity == severity
                && message.id != report::id("LINT", 0)
                && rule.is_none_or(|rule| message.id == report::catalog::lint_id(rule))
                && message.text.starts_with(text)
        })
    })
}

fn has_lint(err: &str, severity: Severity, rule: Option<&str>) -> bool {
    lint_finding(err, severity, rule, "")
}

/// The text of the lint summary (`LINT-000`), if one was printed.
fn lint_summary(err: &str) -> Option<String> {
    err.lines()
        .filter_map(report::parse)
        .find(|message| message.id == report::id("LINT", 0))
        .map(|message| message.text)
}

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

const WIDE_INDEX_SV: &str = r#"module wide_index;
    logic [7:0] mem [0:15];
    logic [63:0] addr;
    initial begin
        mem[3] = 8'h5a;
        addr = 64'd3;
        $display("%h", mem[addr]);
        $finish;
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
    // The design has no time scale, so `missing-timescale` remains.
    assert_eq!(
        lint_summary(&err).as_deref(),
        Some("0 error(s), 1 warning(s)"),
        "only the missing-timescale warning remains: {err}"
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
    assert!(has_lint(&err, Severity::Error, None), "stderr: {err}");
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
    // unused-signal and missing-timescale (the design has no time scale).
    assert!(
        stdout.contains("\"rule\": \"missing-timescale\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"errors\": 0"), "stdout: {stdout}");
    assert!(stdout.contains("\"warnings\": 2"), "stdout: {stdout}");
    assert!(stdout.contains("\"total\": 2"), "stdout: {stdout}");
    assert!(
        !has_lint(&err, Severity::Warning, None),
        "human lint lines suppressed: {err}"
    );
    assert!(
        lint_summary(&err).is_none(),
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
    // missing-timescale stays a warning.
    assert!(stdout.contains("\"errors\": 1"), "stdout: {stdout}");
    assert!(stdout.contains("\"warnings\": 1"), "stdout: {stdout}");
    assert!(stdout.contains("\"total\": 2"), "stdout: {stdout}");
    assert!(
        !has_lint(&err, Severity::Error, None),
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
        has_lint(&err, Severity::Warning, Some("unused-signal")),
        "lint ran without an option: {err}"
    );
    // unused-signal and missing-timescale (the design has no time scale).
    assert!(
        lint_summary(&err).as_deref() == Some("0 error(s), 2 warning(s)"),
        "{err}"
    );
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
    assert!(
        has_lint(&err, Severity::Error, Some("unused-signal")),
        "{err}"
    );
    // unused-signal and missing-timescale (the design has no time scale).
    assert!(
        lint_summary(&err).as_deref() == Some("2 error(s), 0 warning(s)"),
        "{err}"
    );
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
    assert!(stdout.contains("\"errors\": 2"), "stdout: {stdout}");

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
/// warnings remain, 1 on errors, and a zero lint summary for a clean design.
#[test]
fn lint_only_stops_before_code_generation() {
    let dir = TempDir::new("lint_only");
    dir.write("design.sv", UNUSED_SV);
    let out = run_llg(&dir.path, &["--lint-only", "--top", "unused", "design.sv"]);
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(
        has_lint(&err, Severity::Warning, Some("unused-signal")),
        "{err}"
    );
    assert!(out.stdout.is_empty(), "nothing ran: {err}");
    assert!(!dir.path.join("build").exists(), "no model was generated");

    let out = run_llg(
        &dir.path,
        &["--lint-only", "-Werror", "--top", "unused", "design.sv"],
    );
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));

    dir.write("clean.sv", DELAYED_SV);
    // An explicit default time scale keeps the design free of findings.
    let out = run_llg(
        &dir.path,
        &[
            "--lint-only",
            "--timescale",
            "1ns/1ps",
            "--top",
            "delayed",
            "clean.sv",
        ],
    );
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert_eq!(
        err,
        format!(
            "{}\n",
            report::summary("LINT", &report::Counts::default()).render()
        )
    );

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

/// One source construct, elaborated once per instance and generate iteration,
/// is one finding unless its message tells the copies apart: by the elaborated
/// width (`leaf`) or by the scope (`dead`). The summary, the JSON counts and
/// `-Werror` follow what is reported.
const REPEATED_SV: &str = r#"module leaf #(parameter int W = 2) (input logic [W-1:0] a);
    logic [7:0] t;
    initial begin
        t = a;
        $display("%0d", t);
    end
endmodule
module repeated;
    logic [3:0] d = 4'd1;
    leaf #(.W(2)) u0(.a(d[1:0]));
    leaf #(.W(3)) u1(.a(d[2:0]));
    leaf #(.W(3)) u2(.a(d[2:0]));
    for (genvar g = 0; g < 4; g++) begin : gen
        logic BadName;
        logic [1:0] narrow;
        logic dead;
        initial begin
            narrow = 8'd3;
            BadName = narrow[0];
            $display("%b", BadName);
        end
    end
endmodule
"#;

#[test]
fn equal_findings_of_elaborated_copies_are_reported_once() {
    let dir = TempDir::new("repeated");
    dir.write("design.sv", REPEATED_SV);
    let args = [
        "--lint-only",
        "--timescale",
        "1ns/1ps",
        "--top",
        "repeated",
        "design.sv",
    ];
    let out = run_llg(&dir.path, &args);
    let err = stderr(&out);
    assert!(out.status.success(), "{err}");
    let findings: Vec<_> = err
        .lines()
        .filter_map(report::parse)
        .filter(|message| message.id.starts_with("LINT-") && message.id != report::id("LINT", 0))
        .collect();
    let count = |text: &str| {
        findings
            .iter()
            .filter(|message| message.text.contains(text))
            .count()
    };
    assert_eq!(count("`BadName`"), 1, "{err}");
    assert_eq!(count("LHS `narrow` is 2 bits"), 1, "{err}");
    assert_eq!(count("LHS `t` is 8 bits, RHS is 2 bits"), 1, "{err}");
    assert_eq!(count("LHS `t` is 8 bits, RHS is 3 bits"), 1, "{err}");
    assert_eq!(count("signal `dead` in"), 4, "{err}");
    let of = |severity| {
        findings
            .iter()
            .filter(|message| message.severity == severity)
            .count()
    };
    let (warnings, infos) = (of(Severity::Warning), of(Severity::Info));
    assert_eq!(
        lint_summary(&err),
        Some(format!(
            "0 error(s), {warnings} warning(s), {infos} info(s)"
        )),
        "{err}"
    );

    let json = run_llg(&dir.path, &[&args[1..], &["--lint-json"]].concat());
    let text = String::from_utf8_lossy(&json.stdout).into_owned();
    assert!(
        text.contains(&format!("\"total\": {}", findings.len())),
        "{text}"
    );

    let out = run_llg(&dir.path, &[&args[..], &["-Werror"]].concat());
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert_eq!(
        lint_summary(&err),
        Some(format!(
            "{warnings} error(s), 0 warning(s), {infos} info(s)"
        )),
        "{err}"
    );
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
    assert!(
        has_lint(&err, Severity::Error, Some("mixed-assignments")),
        "{err}"
    );
    assert!(out.stdout.is_empty(), "the model must not run");
}

/// The frontend's own unused-parameter warning skips the parameters an FSM
/// pragma names as states (IEEE 1800-2009 40.4.1) and keeps the others.
#[test]
fn fsm_state_parameters_have_no_frontend_unused_warning() {
    let dir = TempDir::new("fsm_states");
    dir.write("design.sv", FSM_STATES_SV);
    let out = run_llg(
        &dir.path,
        &["--lint-only", "--top", "fsm_states", "design.sv"],
    );
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(err.contains("unused parameter 'UNREAD'"), "{err}");
    for state in ["IDLE", "RUN", "DONE"] {
        assert!(
            !err.contains(&format!("unused parameter '{state}'")),
            "{state}: {err}"
        );
    }
}

const FSM_STATES_SV: &str = "module fsm_states(input logic clk);
  /* tool state_vector cs enum E */
  logic [1:0] cs = 0;
  parameter [1:0] /* tool enum E */ IDLE = 0, RUN = 1, DONE = 2;
  parameter UNREAD = 5;
  always_ff @(posedge clk) cs <= cs + 2'd1;
endmodule
";

/// `wide-index` warns on a 64-bit index into a memory without stopping the
/// run, `-Werror` stops it, and the rule can be disabled in `llg.toml`.
#[test]
fn wide_index_warns_and_honors_config_and_werror() {
    let dir = TempDir::new("wide_index");
    dir.write("design.sv", WIDE_INDEX_SV);

    let out = run_llg(&dir.path, &["--top", "wide_index", "design.sv"]);
    let err = stderr(&out);
    assert!(
        out.status.success(),
        "a warning must not stop the run: {err}"
    );
    assert!(
        lint_finding(
            &err,
            Severity::Warning,
            Some("wide-index"),
            "index of an unpacked array is 64 bits wide"
        ),
        "{err}"
    );

    let out = run_llg(&dir.path, &["-Werror", "--top", "wide_index", "design.sv"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(has_lint(&err, Severity::Error, Some("wide-index")), "{err}");

    let cfg = dir.lint_config("[lint.rules.wide-index]\nenabled = false\n");
    let out = run_llg(
        &dir.path,
        &[
            "-Werror",
            "--timescale",
            "1ns/1ps",
            "--config",
            cfg.to_str().unwrap(),
            "--top",
            "wide_index",
            "design.sv",
        ],
    );
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(!err.contains("wide-index"), "{err}");
}

/// An override that makes a public virtual method protected (decision
/// S08-D1): the frontend warns at the override and the model runs;
/// `-Werror`, or `warnings_as_errors = true` in `llg.toml`, reports the same
/// diagnostic as an error, which stops the run before codegen.
#[test]
fn werror_restores_override_visibility_error() {
    const OVERRIDE_SV: &str = r#"class B;
  virtual function int f(); return 1; endfunction
endclass
class D extends B;
  protected virtual function int f(); return 2; endfunction
endclass
module tb;
  D d;
  B b;
  initial begin
    d = new;
    b = d;
    $display("b.f=%0d", b.f());
    $finish;
  end
endmodule
"#;
    fn visibility(err: &str, severity: Severity) -> Vec<String> {
        err.lines()
            .filter_map(report::parse)
            .filter(|message| {
                message.id == "SLANG-VirtualVisibilityMismatch" && message.severity == severity
            })
            .map(|message| {
                let location = message.location.unwrap_or_default();
                let file = location.rsplit(['/', '\\']).next().unwrap_or_default();
                format!("{file}: {}", message.text)
            })
            .collect()
    }
    let expected =
        ["design.sv:5:34: virtual method 'f' is 'protected' but declared 'public' in superclass method"];
    let dir = TempDir::new("override_visibility");
    dir.write("design.sv", OVERRIDE_SV);
    let out = run_llg(&dir.path, &["--top", "tb", "design.sv"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "stderr: {err}");
    assert_eq!(visibility(&err, Severity::Warning), expected, "{err}");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "b.f=2\n", "{err}");

    let cfg = dir.write(
        "llg.toml",
        "schema_version = 1\n[lint]\nwarnings_as_errors = true\n",
    );
    let cfg = cfg.to_str().unwrap();
    for args in [
        &["-Werror", "--top", "tb", "design.sv"][..],
        &["--config", cfg, "--top", "tb", "design.sv"][..],
    ] {
        let _ = std::fs::remove_dir_all(dir.path.join("build"));
        let out = run_llg(&dir.path, args);
        let err = stderr(&out);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {err}");
        assert_eq!(
            visibility(&err, Severity::Error),
            expected,
            "{args:?}: {err}"
        );
        assert!(visibility(&err, Severity::Warning).is_empty(), "{err}");
        assert!(out.stdout.is_empty(), "{args:?}: no model ran: {err}");
        assert!(
            !dir.path.join("build/sim").exists(),
            "{args:?}: no model was generated"
        );
    }
    let out = run_llg(
        &dir.path,
        &["--config", cfg, "-Wno-error", "--top", "tb", "design.sv"],
    );
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
}

/// An unpacked array of derived-class handles assigned to an array of
/// base-class handles (decision UVB-D1): the frontend warns at the
/// assignment, citing 7.6, and the model copies each handle; `-Werror`, or
/// `warnings_as_errors = true` in `llg.toml`, reports the same diagnostic as
/// an error, which stops the run before codegen.
#[test]
fn werror_restores_derived_class_array_error() {
    const ARRAY_SV: &str = r#"class B;
  int id;
endclass
class D extends B;
endclass
module tb;
  D d[2];
  B b[];
  initial begin
    foreach (d[i]) begin d[i] = new; d[i].id = i + 1; end
    b = d;
    $display("%0d %0d", b.size(), b[1].id);
    $finish;
  end
endmodule
"#;
    fn array_assignment(err: &str, severity: Severity) -> Vec<String> {
        err.lines()
            .filter_map(report::parse)
            .filter(|message| {
                message.id == "SLANG-DerivedClassArrayAssignment" && message.severity == severity
            })
            .map(|message| {
                let location = message.location.unwrap_or_default();
                let file = location.rsplit(['/', '\\']).next().unwrap_or_default();
                format!("{file}: {}", message.text)
            })
            .collect()
    }
    let expected = [
        "design.sv:11:7: unpacked array of type 'D$[2]' is assigned to unpacked array of type \
         'B$[]', whose element class is a base class of the source's; IEEE 1800-2009 7.6 \
         requires equivalent element types",
    ];
    let dir = TempDir::new("derived_class_array");
    dir.write("design.sv", ARRAY_SV);
    let out = run_llg(&dir.path, &["--top", "tb", "design.sv"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "stderr: {err}");
    assert_eq!(array_assignment(&err, Severity::Warning), expected, "{err}");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "2 2\n", "{err}");

    let cfg = dir.write(
        "llg.toml",
        "schema_version = 1\n[lint]\nwarnings_as_errors = true\n",
    );
    let cfg = cfg.to_str().unwrap();
    for args in [
        &["-Werror", "--top", "tb", "design.sv"][..],
        &["--config", cfg, "--top", "tb", "design.sv"][..],
    ] {
        let _ = std::fs::remove_dir_all(dir.path.join("build"));
        let out = run_llg(&dir.path, args);
        let err = stderr(&out);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {err}");
        assert_eq!(
            array_assignment(&err, Severity::Error),
            expected,
            "{args:?}: {err}"
        );
        assert!(
            array_assignment(&err, Severity::Warning).is_empty(),
            "{err}"
        );
        assert!(out.stdout.is_empty(), "{args:?}: no model ran: {err}");
        assert!(
            !dir.path.join("build/sim").exists(),
            "{args:?}: no model was generated"
        );
    }
}

/// The forms decision UVB-D1 does not accept stay located frontend errors:
/// base to derived for each container kind, unrelated classes, an inout
/// formal (its copy-out is base to derived), a `ref` formal and associative
/// arrays of another index type. None is reported as the UVB-D1 warning.
#[test]
fn derived_class_array_reverse_forms_stay_errors() {
    const REVERSE_SV: &str = r#"class B;
  int id;
endclass
class D extends B;
endclass
class O extends B;
endclass
module tb;
  B bd[];
  B bq[$];
  B bf[3];
  B ba[int];
  D dd[];
  D dq[$];
  D df[3];
  D da[int];
  D ds[string];
  O od[];
  task automatic t_in(input D i[]); endtask
  task automatic t_io(inout B io[]); endtask
  task automatic t_ref(ref B r[]); endtask
  initial begin
    dd = bd;
    dq = bq;
    df = bf;
    da = ba;
    dd = bq;
    od = dd;
    t_in(bd);
    t_io(dd);
    t_ref(dd);
    ba = ds;
  end
endmodule
"#;
    let dir = TempDir::new("derived_class_array_reverse");
    dir.write("design.sv", REVERSE_SV);
    let out = run_llg(&dir.path, &["--top", "tb", "design.sv"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    let errors: Vec<(String, String)> = err
        .lines()
        .filter_map(report::parse)
        .filter(|message| message.severity == Severity::Error)
        .map(|message| {
            let location = message.location.unwrap_or_default();
            let line = location
                .split(':')
                .rev()
                .nth(1)
                .unwrap_or_default()
                .to_owned();
            (line, message.id)
        })
        .collect();
    let bad = "SLANG-BadAssignment".to_owned();
    let expected: Vec<(String, String)> = (23..=30)
        .map(|line| (line.to_string(), bad.clone()))
        .chain([
            ("31".to_owned(), "SLANG-RefTypeMismatch".to_owned()),
            ("32".to_owned(), bad.clone()),
        ])
        .collect();
    assert_eq!(errors, expected, "{err}");
    assert!(!err.contains("SLANG-DerivedClassArrayAssignment"), "{err}");
}
