//! Integration coverage for raw and checked Slang compile contracts.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use llg::core::compile::{self, CompileError, CompileOpts, Severity};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("llg-compile-errors-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).expect("create compile-errors temp directory");
        Self(path)
    }

    fn write(&self, name: &str, source: &str) -> String {
        let path = self.0.join(name);
        fs::write(&path, source).expect("write compile fixture");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn is_blocking(severity: Severity) -> bool {
    matches!(
        severity,
        Severity::Fatal | Severity::Syntax | Severity::Error
    )
}

fn assert_checked_diagnostics(opts: &CompileOpts, raw_diagnostics: Vec<compile::Diag>) {
    let error = compile::compile_checked(opts).expect_err("checked compile must reject errors");
    assert!(error.startup_message().is_none());
    assert!(error.to_string().contains("blocking frontend diagnostic"));
    let diagnostics = error
        .diagnostics()
        .expect("frontend failure must expose diagnostics");
    assert_eq!(diagnostics, raw_diagnostics.as_slice());
    assert!(diagnostics
        .iter()
        .any(|diagnostic| is_blocking(diagnostic.severity)));
    assert!(matches!(&error, CompileError::FrontendDiagnostics(_)));
    assert_eq!(
        error
            .into_diagnostics()
            .expect("frontend diagnostics remain owned after native teardown"),
        raw_diagnostics
    );
}

#[test]
fn syntax_errors_remain_inspectable_raw_but_checked_compile_rejects_them() {
    let temp = TempDir::new();
    let file = temp.write(
        "syntax_error.sv",
        concat!(
            "// llg-test-fixture: tests/compile_errors.rs/syntax_error.sv\n",
            "module syntax_error;\n",
            "  wire value;\n",
            "  assign value = ;\n",
            "endmodule\n",
        ),
    );
    let opts = CompileOpts {
        files: vec![file.clone()],
        ..CompileOpts::default()
    };

    let raw = compile::compile(&opts).expect("raw compile should start");

    assert!(!raw.ok());
    assert!(raw.diagnostics.iter().any(|diagnostic| {
        is_blocking(diagnostic.severity)
            && diagnostic.file.as_deref() == Some(file.as_str())
            && diagnostic.line > 0
    }));
    assert!(raw.snapshot.diagnostics.iter().any(|diagnostic| {
        !diagnostic.name.is_empty()
            && diagnostic.primary.is_some()
            && matches!(
                diagnostic.severity,
                compile::DiagnosticSeverity::Error | compile::DiagnosticSeverity::Fatal
            )
    }));
    let diagnostics = raw.diagnostics.clone();
    drop(raw);
    assert_checked_diagnostics(&opts, diagnostics);
}

#[test]
fn syntax_diagnostics_are_readable_in_core_and_cli() {
    let temp = TempDir::new();
    let file = temp.write(
        "broken.sv",
        concat!(
            "// llg-test-fixture: tests/compile_errors.rs/broken.sv\n",
            "module broken;\n",
            "  assign value = ;\n",
            "endmodule\n",
        ),
    );
    let out = compile::compile(&CompileOpts {
        files: vec![file.clone()],
        ..CompileOpts::default()
    })
    .expect("compile broken source");
    assert!(!out.ok());
    assert!(out.snapshot.diagnostics.iter().any(|diagnostic| {
        !diagnostic.name.is_empty()
            && diagnostic
                .primary
                .is_some_and(|range| range.end >= range.start)
    }));

    for binary in [env!("CARGO_BIN_EXE_llg"), env!("CARGO_BIN_EXE_elab_check")] {
        let output = std::process::Command::new(binary)
            .arg(&file)
            .output()
            .expect("run CLI on broken source");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&file), "{stderr}");
        assert!(!stderr.trim().is_empty(), "{stderr}");
    }
}

#[test]
fn unknown_parameter_override_is_withheld_by_checked_compile() {
    let temp = TempDir::new();
    let file = temp.write(
        "parameter_error.sv",
        concat!(
            "// llg-test-fixture: tests/compile_errors.rs/parameter_error.sv\n",
            "module parameter_error #(parameter int WIDTH = 1);\n",
            "  logic [WIDTH-1:0] value;\n",
            "endmodule\n",
        ),
    );
    let opts = CompileOpts {
        files: vec![file],
        top: Some("parameter_error".to_owned()),
        param_overrides: vec!["NO_SUCH_PARAM=1".to_owned()],
        ..CompileOpts::default()
    };

    let raw = compile::compile(&opts).expect("raw compile should start");

    assert!(!raw.ok());
    assert!(raw
        .snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("NO_SUCH_PARAM")));
    let diagnostics = raw.diagnostics.clone();
    assert_checked_diagnostics(&opts, diagnostics);
}

#[test]
fn checked_compile_allows_a_clean_owned_snapshot() {
    let temp = TempDir::new();
    let file = temp.write(
        "clean.sv",
        concat!(
            "// llg-test-fixture: tests/compile_errors.rs/clean.sv\n",
            "module clean;\n",
            "  logic value;\n",
            "endmodule\n",
        ),
    );
    let opts = CompileOpts {
        files: vec![file],
        top: Some("clean".to_owned()),
        ..CompileOpts::default()
    };

    let out = compile::compile_checked(&opts).expect("clean checked compile must succeed");

    assert!(out.ok());
    assert!(out
        .snapshot
        .instances
        .iter()
        .any(|instance| { instance.name == "clean" && instance.definition_name == "clean" }));
    assert!(out
        .diagnostics
        .iter()
        .all(|diagnostic| !is_blocking(diagnostic.severity)));
}
