//! CLI integration tests for `llg --lint --lint-config <path>`.
//!
//! Each test drives the real `llg` binary (via `CARGO_BIN_EXE_llg`)
//! in a fresh temp dir, so Surelog's `slpp_all/` output and the generated
//! `target/sim/` tree stay isolated per test.

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

/// Delays are intentionally present: Surelog v1.87 may consume their UHDM
/// relationships during a VPI walk, so human lint and codegen must share one
/// owned database rather than traversing the live design twice.
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
        TempDir { path }
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

fn run_llg(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(SIM_BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("llg should start")
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Disabling a rule in the config suppresses its findings; with nothing left,
/// the lint pass reports clean and the driver proceeds to codegen + run.
#[test]
fn cli_disabled_rule_suppresses_finding() {
    let dir = TempDir::new("disabled");
    dir.write("design.sv", UNUSED_SV);
    let cfg = dir.write("llg-lint.toml", "[rules.unused-signal]\nenabled = false\n");
    let out = run_llg(
        &dir.path,
        &[
            "--lint",
            "--lint-config",
            cfg.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert!(!err.contains("unused-signal"), "finding suppressed: {err}");
    assert!(err.contains("lint: clean"), "stderr: {err}");
}

#[test]
fn human_lint_reuses_delay_metadata_for_codegen() {
    let dir = TempDir::new("delayed_single_db");
    dir.write("design.sv", DELAYED_SV);
    let out = run_llg(&dir.path, &["--lint", "--top", "delayed", "design.sv"]);
    let err = stderr(&out);
    assert!(out.status.success(), "stderr: {err}");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "value=1 time=3\n");
}

#[test]
fn simulation_admits_time_literal_sources_with_and_without_lint() {
    let source = r#"`timescale 1ns/100ps
module time_values;
    initial begin
        $display("literal=%.1f", 2.15ns);
        $finish;
    end
endmodule
"#;
    for lint in [false, true] {
        let dir = TempDir::new(if lint { "time_lint" } else { "time_no_lint" });
        dir.write("design.sv", source);
        let args = if lint {
            vec!["--lint", "--top", "time_values", "design.sv"]
        } else {
            vec!["--top", "time_values", "design.sv"]
        };
        let output = run_llg(&dir.path, &args);
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(String::from_utf8_lossy(&output.stdout), "literal=2.2\n");
    }
}

/// Overriding a rule's severity to `error` promotes its findings; the lint
/// gate then aborts with exit code 1 before codegen.
#[test]
fn cli_severity_override_promotes_finding_to_error() {
    let dir = TempDir::new("severity");
    dir.write("design.sv", UNUSED_SV);
    let cfg = dir.write(
        "llg-lint.toml",
        "[rules.unused-signal]\nseverity = \"error\"\n",
    );
    let out = run_llg(
        &dir.path,
        &[
            "--lint",
            "--lint-config",
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
            "--lint",
            "--lint-config",
            missing.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("cannot read lint config"), "stderr: {err}");
}

/// A malformed config file aborts with the parser's error messages.
#[test]
fn cli_malformed_config_aborts() {
    let dir = TempDir::new("malformed");
    dir.write("design.sv", UNUSED_SV);
    let cfg = dir.write("llg-lint.toml", "[rules.nope]\nenabled = true\n");
    let out = run_llg(
        &dir.path,
        &[
            "--lint",
            "--lint-config",
            cfg.to_str().unwrap(),
            "--top",
            "unused",
            "design.sv",
        ],
    );
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("unknown rule `nope`"), "stderr: {err}");
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
    let cfg = dir.write(
        "llg-lint.toml",
        "[rules.unused-signal]\nseverity = \"error\"\n",
    );
    let out = run_llg(
        &dir.path,
        &[
            "--lint-json",
            "--lint-config",
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
    let cfg = dir.write(
        "llg-lint.toml",
        "[rules.duplicate-case-item]\nseverity = \"error\"\n",
    );
    let out = run_llg(
        &dir.path,
        &[
            "--lint-json",
            "--lint-config",
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
                .is_some_and(|file| file.ends_with("/design.sv")),
            "{rule} should retain the real source path: {baseline_report}"
        );
    }

    let cfg = dir.write(
        "llg-lint.toml",
        "[rules.empty-implicit-sensitivity]\n\
         enabled = false\n\
         [rules.casex-statement]\n\
         severity = \"error\"\n",
    );
    let configured = run_llg(
        &dir.path,
        &[
            "--lint-json",
            "--lint-config",
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
