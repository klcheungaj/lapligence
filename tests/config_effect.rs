//! Integration tests proving that configuration-driven compile inputs reach
//! elaboration: preprocessor defines select `` `ifdef `` branches, and
//! top-level entries from `[compile.param_overrides]`
//! drive parameter-conditioned generate selection.
//!
//! These mirror what the LSP does per root: `config::compile_opts` turns
//! `llg.toml` entries into typed Slang options on `CompileOpts`
//! (unit-tested in `src/bin/llg_ls/config.rs`); here the compiled DESIGN must
//! observably change, so the oracle is the owned `DesignModel` (resolved
//! parameter values + the single kept conditional-generate branch).
//!
//! Every
//! test runs with the CWD in a fresh temp dir (which also hosts the generated
//! designs) and restores it afterwards.  Tests share one process and are
//! serialized through a mutex.

use std::path::{Path, PathBuf};

use llg::core::{compile, elab, model};

#[path = "support/sim.rs"]
mod sim_harness;

/// Run `f` with the CWD set to a fresh temp dir, then restore and clean up.
fn in_temp_dir<R>(f: impl FnOnce(&PathBuf) -> R) -> R {
    sim_harness::with_frontend_temp_cwd("config-effect", |dir| Ok(f(&dir.to_path_buf())))
        .expect("enter temporary config-effect directory")
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
    let database = llg::core::db::Db::from_slang(&out.snapshot).expect("build semantic database");
    model::DesignModel::from_db(&database)
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
            defines: vec!["ENABLE_FOO".to_owned()],
            ..base.clone()
        };
        let model = compile_and_model(&foo);
        assert_eq!(
            param_u64(&model.top_instances[0], "MODE"),
            1,
            "ENABLE_FOO: `ifdef branch"
        );

        // Defining only ENABLE_BAR selects the `elsif branch.
        let bar = compile::CompileOpts {
            defines: vec!["ENABLE_BAR".to_owned()],
            ..base.clone()
        };
        let model = compile_and_model(&bar);
        assert_eq!(
            param_u64(&model.top_instances[0], "MODE"),
            2,
            "ENABLE_BAR: `elsif branch"
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
            param_overrides: vec!["WIDTH=8".to_owned()],
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
            param_overrides: vec!["NO_SUCH_PARAM=1".to_owned()],
            ..Default::default()
        };
        let out = compile::compile(&opts).expect("compile should start");
        assert!(
            !out.ok(),
            "Slang reports an unknown parameter override as an error"
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
