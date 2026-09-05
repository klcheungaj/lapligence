//! Integration tests for the common processing layer: the compile pipeline
//! (`compile`), the owned design model (`model`) and the session builder
//! (`surelog::SessionBuilder`).
//!
//! Surelog writes `slpp_all/` into the process working directory, so every
//! test runs with the CWD pointing at a fresh temp dir and restores it
//! afterwards.  All tests in this file share one process, so they are
//! serialized through a mutex.

use std::path::PathBuf;
use std::sync::Mutex;

use llg::core::{compile, db, elab, model};
use llg::ffi::surelog;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Absolute path of the shared `param_top` test design.
const PARAMS_SV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/elaboration/params.sv");

/// Run `f` with the CWD set to a fresh temp dir, then restore it and clean up.
fn in_temp_dir<R>(f: impl FnOnce() -> R) -> R {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_model_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = f();
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Build the owned design model for an `out.ok()` compile of `params.sv`.
fn compile_and_model(opts: compile::CompileOpts) -> (compile::CompileOut, model::DesignModel) {
    let out = compile::compile(&opts).expect("compile should start");
    assert!(
        out.ok(),
        "compile must succeed, diagnostics: {:?}",
        out.diagnostics
    );
    let design = out.uhdm_design().expect("no UHDM design handle");
    let model = model::DesignModel::build(design).expect("build design model");
    (out, model)
}

/// Build the owned database for a clean frontend compile.  The returned
/// database contains no VPI handles; its range projection is the data that
/// later LSP consumers are allowed to read.
fn compile_and_db(opts: compile::CompileOpts) -> (compile::CompileOut, db::Db) {
    let out = compile::compile(&opts).expect("compile should start");
    assert!(
        out.ok(),
        "compile must succeed, diagnostics: {:?}",
        out.diagnostics
    );
    let design = out.uhdm_design().expect("no UHDM design handle");
    let database = db::Db::build(design).expect("build owned database");
    (out, database)
}

/// Extract the integer value of a resolved parameter.
fn param_u64(p: &model::ParamModel) -> Option<u64> {
    match &p.value {
        Some(elab::Val::Bits(v)) => v.to_u64(),
        _ => None,
    }
}

#[test]
fn compile_ok_params() {
    in_temp_dir(|| {
        let opts = compile::CompileOpts {
            files: vec![PARAMS_SV.to_string()],
            top: Some("param_top".to_string()),
            ..Default::default()
        };
        let (_out, model) = compile_and_model(opts);

        assert!(
            model.design_name.contains("param_top"),
            "design_name = {}",
            model.design_name
        );
        assert_eq!(model.top_instances.len(), 1, "one top instance");

        let u0 = model
            .instance("param_top.u0")
            .expect("instance param_top.u0");
        let w = u0.params.iter().find(|p| p.name == "W").expect("W param");
        assert_eq!(param_u64(w), Some(8), "W of u0");
        let init = u0
            .params
            .iter()
            .find(|p| p.name == "INIT")
            .expect("INIT param");
        assert_eq!(param_u64(init), Some(5), "INIT of u0");
        assert!(u0.children.is_empty(), "u0 has no children");
        // Port types resolve through the ref_typespec → vpiActual chain.
        let o = u0.ports.iter().find(|p| p.name == "o").expect("port o");
        assert_eq!(o.direction, model::Direction::Output);
        assert_eq!(o.ty.kind, "logic");
        assert_eq!(o.ty.width, Some(8), "o is [7:0] after W=8");

        let u1 = model
            .instance("param_top.u1")
            .expect("instance param_top.u1");
        let w = u1.params.iter().find(|p| p.name == "W").expect("W param");
        assert_eq!(param_u64(w), Some(16), "W of u1");
        let init = u1
            .params
            .iter()
            .find(|p| p.name == "INIT")
            .expect("INIT param");
        assert_eq!(param_u64(init), Some(10), "INIT of u1");

        let child = model
            .modules
            .iter()
            .find(|m| m.name == "param_child" || m.name.ends_with("param_child"))
            .unwrap_or_else(|| panic!("module param_child not found: {:?}", model.modules));
        assert!(child.file.is_some(), "param_child has a file");
        assert!(child.line > 0, "param_child has a declaration line");

        assert!(!model.packages.is_empty(), "builtin packages exist");
    });
}

#[test]
fn model_exposes_internal_signals_without_port_backing_duplicates() {
    in_temp_dir(|| {
        let source = PathBuf::from("signal_shapes.sv");
        std::fs::write(
            &source,
            "module signal_shapes(input logic clk, output logic out);\n\
             wire [3:0] internal_wire;\n\
             tri [3:0] internal_tri;\n\
             logic [1:0] internal_var;\n\
             assign out = internal_wire[0] ^ internal_tri[0] ^ internal_var[0] ^ clk;\n\
             endmodule\n",
        )
        .expect("write signal-shapes source");
        let opts = compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("signal_shapes".to_owned()),
            ..Default::default()
        };
        let (_out, model) = compile_and_model(opts);
        let top = model
            .top_instances
            .first()
            .expect("signal_shapes top instance");

        assert!(top.ports.iter().any(|port| port.name == "clk"));
        assert!(top.ports.iter().any(|port| port.name == "out"));
        assert!(
            top.signals
                .iter()
                .all(|signal| signal.name != "clk" && signal.name != "out"),
            "formal ports must not be repeated as signals: {:?}",
            top.signals
        );
        let internal_wire = top
            .signals
            .iter()
            .find(|signal| signal.name == "internal_wire")
            .expect("internal wire");
        assert_eq!(internal_wire.kind, "wire");
        assert_eq!(internal_wire.ty.kind, "logic");
        assert_eq!(internal_wire.ty.width, Some(4));
        let internal_tri = top
            .signals
            .iter()
            .find(|signal| signal.name == "internal_tri")
            .expect("internal tri net");
        assert_eq!(internal_tri.kind, "tri");
        assert_eq!(internal_tri.ty.kind, "logic");
        assert_eq!(internal_tri.ty.width, Some(4));
        let internal_var = top
            .signals
            .iter()
            .find(|signal| signal.name == "internal_var")
            .expect("internal variable");
        assert_eq!(internal_var.kind, "var");
        assert_eq!(internal_var.ty.kind, "logic");
        assert_eq!(internal_var.ty.width, Some(2));
    });
}

#[test]
fn model_preserves_elaborated_width_for_multiline_and_unpacked_signals() {
    in_temp_dir(|| {
        let source = PathBuf::from("elaborated_shapes.sv");
        std::fs::write(
            &source,
            "module elaborated_shapes #(parameter int WIDTH = 4)(input logic clk);\n\
             logic\n\
               [WIDTH-1:0]\n\
               payload;\n\
             logic [WIDTH-1:0] memory [0:1];\n\
             assign payload = memory[0] ^ {WIDTH{clk}};\n\
             endmodule\n",
        )
        .expect("write elaborated-shapes source");
        let opts = compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("elaborated_shapes".to_owned()),
            param_overrides: vec!["-PWIDTH=8".to_owned()],
            ..Default::default()
        };
        let (_out, model) = compile_and_model(opts);
        let top = model
            .top_instances
            .first()
            .expect("elaborated_shapes top instance");

        let payload = top
            .signals
            .iter()
            .find(|signal| signal.name == "payload")
            .expect("multiline payload signal");
        assert_eq!(payload.ty.kind, "logic");
        assert_eq!(payload.ty.width, Some(8));

        let memory = top
            .signals
            .iter()
            .find(|signal| signal.name == "memory")
            .expect("packed-plus-unpacked memory signal");
        assert_eq!(memory.kind, "array");
        assert_eq!(memory.ty.kind, "logic");
        assert_eq!(memory.ty.width, Some(8));
    });
}

#[test]
fn db_owns_ordered_packed_ranges_per_elaborated_instance() {
    in_temp_dir(|| {
        let source = PathBuf::from("owned_ranges.sv");
        std::fs::write(
            &source,
            "module shaped #(parameter int ROWS = 2, parameter int COLS = 3) (\n\
             input logic [ROWS-1:0][COLS-1:0] data\n\
             );\n\
             logic [ROWS-1:0][COLS-1:0] payload;\n\
             logic [ROWS-1:0][COLS-1:0] memory [0:1][2:0];\n\
             assign payload = data;\n\
             endmodule\n\
             module owned_ranges_top;\n\
             shaped #(.ROWS(8), .COLS(2)) u0();\n\
             shaped #(.ROWS(4), .COLS(3)) u1();\n\
             endmodule\n",
        )
        .expect("write owned-ranges source");
        let opts = compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("owned_ranges_top".to_owned()),
            ..Default::default()
        };
        let (_out, database) = compile_and_db(opts);
        let ranges = database.elaborated_type_ranges();

        let find = |instance: &str, name: &str| {
            ranges
                .iter()
                .find(|entry| entry.instance == instance && entry.name == name)
                .unwrap_or_else(|| panic!("missing owned ranges for {instance}.{name}: {ranges:?}"))
        };
        let expected_u0 = vec![
            Some(db::PackedRange { left: 7, right: 0 }),
            Some(db::PackedRange { left: 1, right: 0 }),
        ];
        let expected_u1 = vec![
            Some(db::PackedRange { left: 3, right: 0 }),
            Some(db::PackedRange { left: 2, right: 0 }),
        ];

        assert_eq!(
            find("owned_ranges_top.u0", "data").packed_ranges,
            expected_u0
        );
        assert_eq!(
            find("owned_ranges_top.u0", "payload").packed_ranges,
            expected_u0
        );
        assert_eq!(
            find("owned_ranges_top.u0", "memory").packed_ranges,
            expected_u0,
            "unpacked dimensions must not be mixed into packed ranges"
        );
        assert_eq!(
            find("owned_ranges_top.u1", "data").packed_ranges,
            expected_u1
        );
        assert_eq!(
            find("owned_ranges_top.u1", "payload").packed_ranges,
            expected_u1
        );
        assert_eq!(
            find("owned_ranges_top.u1", "memory").packed_ranges,
            expected_u1
        );
        assert!(
            ranges.iter().all(|entry| entry.instance != "shaped"),
            "only elaborated per-instance objects should be projected: {ranges:?}"
        );
    });
}

#[test]
fn db_owns_dynamic_net_declaration_assignment_shape() {
    in_temp_dir(|| {
        let source = PathBuf::from("net_decl_shape.sv");
        std::fs::write(
            &source,
            "// llg-test-fixture: tests/model_tests.rs/net_decl_shape.sv\n\
             module net_decl_shape(input logic [7:0] a, b, output logic [7:0] out);\n\
             wire [7:0] y = a + b;\n\
             assign out = y;\n\
             endmodule\n",
        )
        .expect("write net-declaration source");
        let opts = compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("net_decl_shape".to_owned()),
            ..Default::default()
        };
        let (_out, database) = compile_and_db(opts);

        let declaration_assignments = database
            .node_ids()
            .filter(|id| {
                matches!(
                    database.node_kind(*id),
                    db::NodeKind::ContAssign { net_decl: true, .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            declaration_assignments.len(),
            1,
            "the dynamic declaration must remain distinguishable from plain assigns"
        );

        let assignment = database.node(declaration_assignments[0]);
        assert_eq!(assignment.children.len(), 2, "owned LHS and RHS required");
        let lhs_target = match database.node_kind(assignment.children[0]) {
            db::NodeKind::Expr(db::ExprKind::Ref {
                target: Some(target),
            }) => *target,
            other => panic!("unexpected declaration-assignment LHS: {other:?}"),
        };
        assert!(
            matches!(
                database.node_kind(lhs_target),
                db::NodeKind::Net {
                    net_type: db::NetType::Wire | db::NetType::Logic,
                    ..
                }
            ),
            "declaration LHS must resolve to the owned true-net declaration: {:?}",
            database.node_kind(lhs_target)
        );
        assert!(
            matches!(
                database.node_kind(assignment.children[1]),
                db::NodeKind::Expr(db::ExprKind::Operation { .. })
            ),
            "dynamic RHS must remain an owned expression tree"
        );
    });
}

#[test]
fn compile_diagnostics() {
    in_temp_dir(|| {
        // `assign w = ;` is a real syntax error in this Surelog version.
        let bad = std::env::temp_dir().join(format!("llg_bad_syntax_{}.sv", std::process::id()));
        std::fs::write(&bad, "module bad;\n  wire w;\n  assign w = ;\nendmodule\n")
            .expect("write bad file");
        let bad_path = bad.to_string_lossy().into_owned();

        let opts = compile::CompileOpts {
            files: vec![bad_path.clone()],
            top: None,
            ..Default::default()
        };
        let out = compile::compile(&opts).expect("compile should start");
        assert!(!out.ok(), "syntax error must fail ok()");

        let has_error_diag = out.diagnostics.iter().any(|d| {
            matches!(
                d.severity,
                compile::Severity::Error | compile::Severity::Syntax
            ) && d.file.as_deref() == Some(bad_path.as_str())
                && d.line > 0
        });
        assert!(
            has_error_diag,
            "expected an Error/Syntax diag for the file, got: {:?}",
            out.diagnostics
        );

        let _ = std::fs::remove_file(&bad);
    });
}

#[test]
fn session_builder_roundtrip() {
    in_temp_dir(|| {
        let mut builder = surelog::SessionBuilder::new().expect("builder");
        builder
            .set_parse()
            .set_write_pp_output()
            .set_compile()
            .set_elaborate()
            .set_elab_uhdm()
            .set_mute_stdout();
        assert!(builder.add_arg("-noinfo"));
        assert!(builder.add_arg("-nonote"));
        assert!(builder.add_arg(PARAMS_SV));
        assert!(builder.add_arg("-top"));
        assert!(builder.add_arg("param_top"));
        assert!(!builder.add_arg("interior\0null"), "interior NUL rejected");

        let session = builder.build().expect("session builds");
        let diags = session.diagnostics();
        assert!(
            !diags.iter().any(|d| {
                matches!(
                    d.severity,
                    surelog::Severity::Fatal | surelog::Severity::Syntax | surelog::Severity::Error
                )
            }),
            "no fatal/syntax/error diagnostics, got: {:?}",
            diags
        );
        assert!(session.uhdm_design().is_some(), "UHDM design present");
    });
}
