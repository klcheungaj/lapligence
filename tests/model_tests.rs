//! Integration tests for the common processing layer: the compile pipeline
//! (`compile`), the owned semantic database (`db`) and the design model
//! (`model`).
//!
//! Tests which use relative source paths serialize changes to the process CWD.

use std::path::PathBuf;

use llg::core::{compile, db, elab, model};

#[path = "support/sim.rs"]
mod sim_harness;

/// Absolute path of the shared `param_top` test design.
const PARAMS_SV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/elaboration/params.sv");

/// Run `f` with the CWD set to a fresh temp dir, then restore it and clean up.
fn in_temp_dir<R>(f: impl FnOnce() -> R) -> R {
    sim_harness::with_frontend_temp_cwd("model-test", |_| Ok(f()))
        .expect("enter temporary model-test directory")
}

/// Build the owned design model for an `out.ok()` compile of `params.sv`.
fn compile_and_model(opts: compile::CompileOpts) -> (compile::CompileOut, model::DesignModel) {
    let out = compile::compile(&opts).expect("compile should start");
    assert!(
        out.ok(),
        "compile must succeed, diagnostics: {:?}",
        out.diagnostics
    );
    let database = db::Db::from_slang(&out.snapshot).expect("build semantic database");
    let model = model::DesignModel::from_db(&database);
    (out, model)
}

/// Build the owned database for a clean frontend compile.  The returned
/// database contains no native handles; its range projection is the data that
/// later consumers are allowed to read.
fn compile_and_db(opts: compile::CompileOpts) -> (compile::CompileOut, db::Db) {
    let out = compile::compile(&opts).expect("compile should start");
    assert!(
        out.ok(),
        "compile must succeed, diagnostics: {:?}",
        out.diagnostics
    );
    let database = db::Db::from_slang(&out.snapshot).expect("build owned database");
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
        // Port types are resolved per elaborated instance.
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
    });
}

#[test]
fn model_exposes_source_declared_packages() {
    let source = compile::OwnedSource::compilation_unit(
        "package_model.sv",
        "// llg-test-fixture: tests/model_tests.rs/package_model.sv\n\
         package cfg_pkg;\n\
         parameter int WIDTH = 8;\n\
         endpackage\n\
         module package_top;\n\
         import cfg_pkg::*;\n\
         logic [WIDTH-1:0] value;\n\
         endmodule\n",
    );
    let (_out, model) = compile_and_model(compile::CompileOpts {
        sources: vec![source],
        top: Some("package_top".to_owned()),
        ..Default::default()
    });

    let package = model
        .packages
        .iter()
        .find(|package| package.name == "cfg_pkg")
        .unwrap_or_else(|| panic!("source package not found: {:?}", model.packages));
    assert_eq!(package.file.as_deref(), Some("package_model.sv"));
    assert_eq!(package.line, 2);
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
            param_overrides: vec!["WIDTH=8".to_owned()],
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
fn packed_ranges_distinguish_same_named_locals_in_unnamed_blocks() {
    let compiled = compile::compile_checked(&compile::CompileOpts {
        sources: vec![compile::OwnedSource::compilation_unit(
            "local_ranges.sv",
            "module tb; initial begin
                begin logic [7:0] value; value = 8'h81; $display(value); end
                begin logic [64:0] value; value = '1; $display(value); end
                begin logic [7:0] value; value = 8'h02; $display(value); end
             end endmodule",
        )],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("valid same-named locals");
    let database = db::Db::from_slang(&compiled.snapshot).expect("distinct declaration identities");
    let ranges: Vec<_> = database
        .elaborated_type_ranges()
        .iter()
        .filter(|entry| entry.name == "value")
        .collect();
    assert_eq!(ranges.len(), 3);
    let mut bounds = Vec::new();
    let mut identities = std::collections::HashSet::new();
    for entry in ranges {
        assert!(identities.insert(entry.declaration));
        assert_eq!(database.node(entry.declaration).name, "value");
        assert_eq!(entry.packed_ranges.len(), 1);
        bounds.push(entry.packed_ranges[0].expect("exact range").left);
    }
    bounds.sort_unstable();
    assert_eq!(bounds, [7, 7, 64]);
}

#[test]
fn instance_containers_normalize_gate_arrays_and_reject_cycles() {
    let compiled = compile::compile_checked(&compile::CompileOpts {
        sources: vec![compile::OwnedSource::compilation_unit(
            "gate_arrays.sv",
            include_str!("fixtures/sim/gates/array_scopes.sv"),
        )],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("valid primitive instance arrays");
    let database = db::Db::from_slang(&compiled.snapshot).expect("normalized instance arrays");
    let scope_gates = database
        .node_ids()
        .filter(|id| {
            matches!(
                database.node_kind(*id),
                db::NodeKind::ModuleInst { .. } | db::NodeKind::GenScope
            )
        })
        .map(|id| {
            database
                .node(id)
                .children
                .iter()
                .filter(|child| matches!(database.node_kind(**child), db::NodeKind::Gate { .. }))
                .count()
        })
        .filter(|count| *count != 0)
        .collect::<Vec<_>>();
    assert_eq!(scope_gates, vec![4, 4]);

    let mut snapshot = compiled.snapshot;
    let array = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == llg::ffi::slang::SemanticKind::Instance && node.subkind == 193)
        .expect("primitive array container");
    let edges = array.edge_start as usize..(array.edge_start + array.edge_count) as usize;
    let child = snapshot.semantic_edges[edges]
        .iter_mut()
        .find(|edge| edge.role == llg::ffi::slang::SemanticEdgeRole::Child)
        .expect("array child edge");
    child.target_id = array.id;
    assert!(matches!(
        db::Db::from_slang(&snapshot),
        Err(db::DbError::InvalidSnapshot(message)) if message.contains("instance container")
    ));
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
        // `assign w = ;` is a real syntax error.
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
fn db_retains_every_continuous_and_gate_delay_expression() {
    in_temp_dir(|| {
        for (fixture, expected) in [
            ("inertial_continuous_two_delays", &[2, 3][..]),
            ("inertial_continuous_three_delays", &[2, 3, 4][..]),
            ("inertial_gate_two_delays", &[2, 3][..]),
            ("inertial_gate_three_delays", &[2, 3, 4][..]),
        ] {
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/sim/partial_features")
                .join(format!("{fixture}.sv"));
            let out = compile::compile_checked(&compile::CompileOpts {
                files: vec![source.to_string_lossy().into_owned()],
                top: Some("tb".to_owned()),
                ..Default::default()
            })
            .expect("compile driver delay fixture");
            let database = db::Db::from_slang(&out.snapshot).expect("build owned database");
            drop(out);
            let delays = database
                .node_ids()
                .filter_map(|id| match database.node_kind(id) {
                    db::NodeKind::ContAssign { delay, .. } | db::NodeKind::Gate { delay, .. } => {
                        *delay
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(delays.len(), 1, "{fixture}");
            let expressions = match delays[0] {
                db::DriverDelay::Single(delay) => vec![delay],
                db::DriverDelay::RiseFall(rise, fall) => vec![rise, fall],
                db::DriverDelay::RiseFallTurnOff(rise, fall, turn_off) => {
                    vec![rise, fall, turn_off]
                }
            };
            let actual = expressions
                .into_iter()
                .map(|mut id| {
                    while let db::NodeKind::Expr(db::ExprKind::Cast { operand, .. }) =
                        database.node_kind(id)
                    {
                        id = *operand;
                    }
                    let db::NodeKind::Expr(db::ExprKind::Constant { value, size, .. }) =
                        database.node_kind(id)
                    else {
                        panic!("{fixture}: delay is not a constant");
                    };
                    let elab::Val::Bits(value) =
                        elab::decode_value_data(value, *size).expect("decode delay")
                    else {
                        panic!("{fixture}: delay is not integral");
                    };
                    value.to_u64().expect("known delay")
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{fixture}: preserve transition order");
        }
    });
}

#[test]
fn owned_snapshot_survives_native_compile_teardown() {
    in_temp_dir(|| {
        let out = compile::compile_checked(&compile::CompileOpts {
            files: vec![PARAMS_SV.to_owned()],
            top: Some("param_top".to_owned()),
            ..compile::CompileOpts::default()
        })
        .expect("compile parameters design");
        let snapshot = out.snapshot.clone();
        drop(out);

        assert!(snapshot
            .instances
            .iter()
            .any(|instance| instance.name == "param_top"));
        assert!(snapshot
            .parameters
            .iter()
            .any(|parameter| parameter.name == "W"));
    });
}
