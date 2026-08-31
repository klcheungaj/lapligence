//! Integration tests proving that configuration-driven compile inputs reach
//! elaboration: preprocessor defines (`-D`) select `` `ifdef `` branches, and
//! top-level parameter overrides (`-P`, from `[compile.param_overrides]`)
//! drive parameter-conditioned generate selection.
//!
//! These mirror what the LSP does per root: `config::compile_opts` turns
//! `llg.toml` entries into verbatim Surelog arguments on `CompileOpts`
//! (unit-tested in `src/bin/llg/config.rs`); here the compiled DESIGN must
//! observably change, so the oracle is the owned `DesignModel` (resolved
//! parameter values + the single kept conditional-generate branch).
//!
//! Surelog writes `slpp_all/` into the process working directory, so every
//! test runs with the CWD in a fresh temp dir (which also hosts the generated
//! designs) and restores it afterwards.  Tests share one process and are
//! serialized through a mutex.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use llg::core::{compile, elab, model};

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Run `f` with the CWD set to a fresh temp dir, then restore it and clean up.
/// Returns the temp dir path so tests can stage design files into it BEFORE
/// compiling; cleanup happens via the returned guard's drop.
fn in_temp_dir<R>(f: impl FnOnce(&PathBuf) -> R) -> R {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_config_effect_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = f(&dir);
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn write_design(dir: &Path, name: &str, text: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write design");
    path.to_string_lossy().into_owned()
}

/// Compile and build the owned model, requiring a clean frontend.
fn compile_and_model(opts: &compile::CompileOpts) -> model::DesignModel {
    let out = compile::compile(opts).expect("compile should start");
    assert!(
        out.ok(),
        "compile must succeed, diagnostics: {:?}",
        out.diagnostics
    );
    let design = out.uhdm_design().expect("no UHDM design handle");
    model::DesignModel::build(design).expect("build design model")
}

/// The integer value of a named parameter on an instance.
fn param_u64(inst: &model::InstanceModel, name: &str) -> u64 {
    inst.params
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| match &p.value {
            Some(elab::Val::Bits(v)) => v.to_u64(),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "parameter {name} must resolve to an integer; available: {:?}",
                inst.params
            )
        })
}

const IFDEF_SV: &str = r#"module ifdef_top;
`ifdef ENABLE_FOO
  localparam int MODE = 1;
`elsif ENABLE_BAR
  localparam int MODE = 2;
`else
  localparam int MODE = 3;
`endif
endmodule
"#;

#[test]
fn define_selects_ifdef_branch() {
    in_temp_dir(|dir| {
        let file = write_design(dir, "ifdef_top.sv", IFDEF_SV);
        let base = compile::CompileOpts {
            files: vec![file],
            top: Some("ifdef_top".to_owned()),
            ..Default::default()
        };

        // Without any define the `else` branch applies.
        let model = compile_and_model(&base);
        assert_eq!(model.top_instances.len(), 1);
        assert_eq!(
            param_u64(&model.top_instances[0], "MODE"),
            3,
            "no define: `else branch"
        );

        // Defining ENABLE_FOO selects the first branch.
        let foo = compile::CompileOpts {
            defines: vec!["-DENABLE_FOO".to_owned()],
            ..base.clone()
        };
        let model = compile_and_model(&foo);
        assert_eq!(
            param_u64(&model.top_instances[0], "MODE"),
            1,
            "-DENABLE_FOO: `ifdef branch"
        );

        // Defining only ENABLE_BAR selects the `elsif branch.
        let bar = compile::CompileOpts {
            defines: vec!["-DENABLE_BAR".to_owned()],
            ..base.clone()
        };
        let model = compile_and_model(&bar);
        assert_eq!(
            param_u64(&model.top_instances[0], "MODE"),
            2,
            "-DENABLE_BAR: `elsif branch"
        );
    });
}

const GEN_SV: &str = r#"module gen_top #(parameter int WIDTH = 4)();
  generate
    if (WIDTH >= 8) begin : g_wide
      localparam int BRANCH = 8;
    end else begin : g_narrow
      localparam int BRANCH = 4;
    end
  endgenerate
endmodule
"#;

#[test]
fn param_override_flips_generate_branch() {
    in_temp_dir(|dir| {
        let file = write_design(dir, "gen_top.sv", GEN_SV);

        // Default elaboration: WIDTH stays 4 and the narrow branch survives
        // (conditional generate keeps exactly the taken branch).
        let base = compile::CompileOpts {
            files: vec![file.clone()],
            top: Some("gen_top".to_owned()),
            ..Default::default()
        };
        let model = compile_and_model(&base);
        let top = &model.top_instances[0];
        assert_eq!(param_u64(top, "WIDTH"), 4, "default parameter value");
        assert_eq!(
            top.gen_scopes
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>(),
            vec!["g_narrow"],
            "only the taken branch is elaborated"
        );
        let narrow = &top.gen_scopes[0];
        assert_eq!(
            narrow
                .params
                .iter()
                .find(|p| p.name == "BRANCH")
                .and_then(|p| match &p.value {
                    Some(elab::Val::Bits(v)) => v.to_u64(),
                    _ => None,
                }),
            Some(4),
            "branch-local parameter resolves inside g_narrow"
        );

        // With the top-level override the wide branch is taken instead.
        let overridden = compile::CompileOpts {
            param_overrides: vec!["-PWIDTH=8".to_owned()],
            ..base
        };
        let model = compile_and_model(&overridden);
        let top = &model.top_instances[0];
        assert_eq!(param_u64(top, "WIDTH"), 8, "override reaches elaboration");
        assert_eq!(
            top.gen_scopes
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>(),
            vec!["g_wide"],
            "the override selects the other generate branch"
        );
        assert_eq!(
            top.gen_scopes[0]
                .params
                .iter()
                .find(|p| p.name == "BRANCH")
                .and_then(|p| match &p.value {
                    Some(elab::Val::Bits(v)) => v.to_u64(),
                    _ => None,
                }),
            Some(8),
            "branch-local parameter resolves inside g_wide"
        );
    });
}

#[test]
fn unknown_param_override_is_reported_as_error() {
    in_temp_dir(|dir| {
        let file = write_design(dir, "gen_top.sv", GEN_SV);
        let opts = compile::CompileOpts {
            files: vec![file],
            top: Some("gen_top".to_owned()),
            param_overrides: vec!["-PNO_SUCH_PARAM=1".to_owned()],
            ..Default::default()
        };
        let out = compile::compile(&opts).expect("compile should start");
        assert!(
            !out.ok(),
            "Surelog reports ELAB_UNKNOWN_PARAMETER_COMMAND as an error"
        );
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.message.contains("NO_SUCH_PARAM")),
            "diagnostic names the bogus override: {:?}",
            out.diagnostics
        );
    });
}
