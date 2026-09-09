//! End-to-end coverage for wide constants flowing from SystemVerilog source
//! through Slang elaboration into the fully owned design model.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use llg::core::compile::{self, CompileError, CompileOpts, Severity};
use llg::core::{elab, model};

static CWD_LOCK: Mutex<()> = Mutex::new(());

struct TempCwd {
    path: PathBuf,
    previous: PathBuf,
}

impl TempCwd {
    fn enter(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("llg-wide-elab-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create wide-elaboration temp directory");
        let previous = std::env::current_dir().expect("read current directory");
        std::env::set_current_dir(&path).expect("enter wide-elaboration temp directory");
        Self { path, previous }
    }

    fn write(&self, name: &str, source: &str) -> String {
        let path = self.path.join(name);
        fs::write(&path, source).expect("write wide-elaboration fixture");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for TempCwd {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).expect("restore current directory");
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn in_temp_cwd(tag: &str, f: impl FnOnce(&TempCwd)) {
    let _lock = CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cwd = TempCwd::enter(tag);
    f(&cwd);
}

fn compile_model(cwd: &TempCwd, source: &str) -> model::DesignModel {
    let file = cwd.write("wide_elab.sv", source);
    let out = compile::compile_checked(&CompileOpts {
        files: vec![file],
        top: Some("wide_top".to_owned()),
        ..Default::default()
    })
    .expect("wide design must compile and elaborate cleanly");
    let database =
        llg::core::db::Db::from_slang(&out.snapshot).expect("build owned wide semantic database");
    let owned = model::DesignModel::from_db(&database);
    drop(out);
    owned
}

fn param_bits<'a>(instance: &'a model::InstanceModel, name: &str) -> &'a elab::Value {
    let parameter = instance
        .params
        .iter()
        .find(|parameter| parameter.name == name)
        .unwrap_or_else(|| panic!("missing parameter {name}: {:?}", instance.params));
    match parameter.value.as_ref() {
        Some(elab::Val::Bits(value)) => value,
        other => panic!("parameter {name} must resolve to bits, got {other:?}"),
    }
}

#[test]
fn wide_constants_and_arithmetic_retain_bits_above_u64() {
    in_temp_cwd("expressions", |cwd| {
        // Arrange and act. The override exercises the per-instance param-assign
        // path; every dependent localparam must use that elaborated wide value.
        let design = compile_model(
            cwd,
            r#"module wide_child #(
    parameter logic [127:0] BASE = 128'd1
) ();
    localparam logic [127:0] SUM = BASE + 128'd7;
    localparam logic [127:0] PRODUCT = BASE * 128'd3;
    localparam logic [127:0] ROUND_TRIP = (PRODUCT - 128'd15) / 128'd3;
endmodule

module wide_top;
    wide_child #(
        .BASE(128'h00000010000000000000000000000005)
    ) u_wide ();
endmodule
"#,
        );

        // Assert against the owned model after the native compile has returned.
        let child = design
            .instance("wide_top.u_wide")
            .expect("elaborated wide child instance");
        let base = (1u128 << 100) | 5;
        let expected = [
            ("BASE", base),
            ("SUM", base + 7),
            ("PRODUCT", base * 3),
            ("ROUND_TRIP", 1u128 << 100),
        ];

        for (name, expected_value) in expected {
            let value = param_bits(child, name);
            assert_eq!(value.width(), 128, "width of {name}");
            assert_eq!(value.to_u128(), Some(expected_value), "value of {name}");
            assert_eq!(value.to_u64(), None, "{name} must not fit in u64");
        }

        assert_eq!(
            param_bits(child, "BASE").bit_lsb(100),
            elab::Bit::One,
            "the override's upper limb must survive elaboration"
        );
    });
}

#[test]
fn checked_compile_rejects_a_malformed_wide_literal() {
    in_temp_cwd("malformed", |cwd| {
        // Arrange
        let file = cwd.write(
            "malformed_wide.sv",
            "module wide_top; localparam logic [127:0] BAD = 128'h12q; endmodule\n",
        );
        let opts = CompileOpts {
            files: vec![file],
            top: Some("wide_top".to_owned()),
            ..Default::default()
        };

        // Act
        let error = match compile::compile_checked(&opts) {
            Ok(_) => panic!("checked compile must reject a malformed wide literal"),
            Err(error) => error,
        };

        // Assert
        let CompileError::FrontendDiagnostics(diagnostics) = error else {
            panic!("malformed source must be a frontend diagnostic failure");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| matches!(
                diagnostic.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "malformed literal must produce a blocking diagnostic: {diagnostics:?}"
        );
    });
}
