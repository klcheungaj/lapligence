//! CLI integration tests for `llg_sim --lint --lint-config <path>`.
//!
//! Each test drives the real `llg_sim` binary (via `CARGO_BIN_EXE_llg_sim`)
//! in a fresh temp dir, so Surelog's `slpp_all/` output and the generated
//! `target/sim/` tree stay isolated per test.

use std::path::Path;
use std::process::Command;

const SIM_BIN: &str = env!("CARGO_BIN_EXE_llg_sim");

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

fn run_llg_sim(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(SIM_BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("llg_sim should start")
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
    let out = run_llg_sim(
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
    let out = run_llg_sim(
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
    let out = run_llg_sim(
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
    let out = run_llg_sim(
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
    let out = run_llg_sim(&dir.path, &["--lint-json", "--top", "unused", "design.sv"]);
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
    let out = run_llg_sim(
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
    let out = run_llg_sim(
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
    let out = run_llg_sim(&dir.path, &["--lint-json", "--top", "unused", "design.sv"]);
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
