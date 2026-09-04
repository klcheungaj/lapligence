//! Integration coverage for raw and checked Surelog compile contracts.
//!
//! Surelog and the process CWD both carry process-global state, so these tests
//! serialize setup, compilation, and cleanup while using a fresh temporary
//! working tree for every test.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use llg::core::compile::{self, CompileError, CompileOpts, Severity};

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

struct TempCwd {
    path: PathBuf,
    previous: PathBuf,
}

impl TempCwd {
    fn enter() -> Self {
        let path = std::env::temp_dir().join(format!("llg-compile-errors-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create checked-compile temp directory");
        let previous = std::env::current_dir().expect("read current directory");
        std::env::set_current_dir(&path).expect("enter checked-compile temp directory");
        Self { path, previous }
    }

    fn write(&self, name: &str, source: &str) -> String {
        let path = self.path.join(name);
        fs::write(&path, source).expect("write compile fixture");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for TempCwd {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).expect("restore current directory");
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn in_temp_cwd(f: impl FnOnce(&TempCwd)) {
    let _lock = SURELOG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cwd = TempCwd::enter();
    f(&cwd);
}

fn is_blocking(severity: Severity) -> bool {
    matches!(
        severity,
        Severity::Fatal | Severity::Syntax | Severity::Error
    )
}

fn assert_checked_diagnostics(opts: &CompileOpts, raw_diagnostics: Vec<compile::Diag>) {
    let error = match compile::compile_checked(opts) {
        Ok(_) => panic!("checked compile must reject errors"),
        Err(error) => error,
    };
    assert!(error.session_start_message().is_none());
    assert!(error.to_string().contains("blocking frontend diagnostic"));
    let diagnostics = error
        .diagnostics()
        .expect("frontend failure must expose diagnostics");
    assert_eq!(diagnostics, raw_diagnostics.as_slice());
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| is_blocking(diagnostic.severity)),
        "checked error must contain a blocking diagnostic: {diagnostics:?}"
    );
    assert!(matches!(&error, CompileError::FrontendDiagnostics(_)));
    assert_eq!(
        error
            .into_diagnostics()
            .expect("frontend diagnostics remain owned after session teardown"),
        raw_diagnostics
    );
}

#[test]
fn syntax_errors_remain_inspectable_raw_but_checked_compile_rejects_them() {
    in_temp_cwd(|cwd| {
        // Arrange
        let file = cwd.write(
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
            ..Default::default()
        };

        // Act
        let raw = compile::compile(&opts).expect("raw compile should start");

        // Assert
        assert!(!raw.ok());
        assert!(raw.diagnostics.iter().any(|diagnostic| {
            is_blocking(diagnostic.severity)
                && diagnostic.file.as_deref() == Some(file.as_str())
                && diagnostic.line > 0
        }));
        let diagnostics = raw.diagnostics.clone();
        drop(raw);
        assert_checked_diagnostics(&opts, diagnostics);
    });
}

#[test]
fn elaboration_errors_cannot_escape_checked_compile_with_partial_uhdm() {
    in_temp_cwd(|cwd| {
        // Arrange
        let file = cwd.write(
            "elaboration_error.sv",
            concat!(
                "// llg-test-fixture: tests/compile_errors.rs/elaboration_error.sv\n",
                "module elaboration_error #(parameter int WIDTH = 1);\n",
                "  logic [WIDTH-1:0] value;\n",
                "endmodule\n",
            ),
        );
        let opts = CompileOpts {
            files: vec![file],
            top: Some("elaboration_error".to_owned()),
            param_overrides: vec!["-PNO_SUCH_PARAM=1".to_owned()],
            ..Default::default()
        };

        // Act
        let raw = compile::compile(&opts).expect("raw compile should start");

        // Assert
        assert!(!raw.ok());
        assert!(
            raw.uhdm_design().is_some(),
            "Surelog should expose the partial UHDM that checked compile must withhold"
        );
        assert!(raw.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Error && diagnostic.message.contains("NO_SUCH_PARAM")
        }));
        let diagnostics = raw.diagnostics.clone();
        drop(raw);
        assert_checked_diagnostics(&opts, diagnostics);
    });
}

#[test]
fn checked_compile_allows_a_clean_elaborated_design() {
    in_temp_cwd(|cwd| {
        // Arrange
        let file = cwd.write(
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
            ..Default::default()
        };

        // Act
        let out = compile::compile_checked(&opts).expect("clean checked compile must succeed");

        // Assert
        assert!(out.ok());
        assert!(out.uhdm_design().is_some());
        assert!(
            out.diagnostics
                .iter()
                .all(|diagnostic| !is_blocking(diagnostic.severity)),
            "successful checked compile may contain warnings but no errors: {:?}",
            out.diagnostics
        );
    });
}
