//! Continuous fixed arrays keep RHS dependencies and per-site contribution slots.
use super::*;

#[test]
fn continuous_array_graph_keeps_static_topology_and_rhs_only_dependencies() {
    let database = {
        let result = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "continuous_identity.sv",
                include_str!("../../../../../../tests/fixtures/sim/continuation_20_23/continuous_identity.sv"),
            )],
            &crate::core::compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
        ).unwrap();
        Db::from_slang(&result.snapshot).unwrap()
    };
    database.validate().unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    let tops = cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    cg.collect_timescales();
    cg.build_net_groups().unwrap();
    cg.validate_process_semantics().unwrap();
    for top in tops {
        cg.emit_pass(top, Pass::Comb).unwrap();
    }
    let array = |name: &str| {
        cg.array_globals
            .iter()
            .find_map(|(node, array)| {
                (database.node(*node).name == name).then_some(cg.reference_array(array.ir))
            })
            .expect("named source array")
    };
    let left = IrDependency::ArrayContents(array("left"));
    let right = IrDependency::ArrayContents(array("right"));
    let choice = database
        .nodes()
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            if node.name == "choice" {
                cg.signal_of(NodeId::from_index(index))
                    .map(|signal| cg.signal_dependency(signal))
            } else {
                None
            }
        })
        .expect("selector signal");
    let expected: HashSet<_> = [left.clone(), right, choice].into_iter().collect();
    let mut constant = 0;
    let mut conditional = 0;
    for process in &cg.model.processes {
        match &process.shape {
            IrShape::RunOnce => constant += 1,
            IrShape::SensLoop { reads } if reads.len() == 3 => {
                assert_eq!(reads.iter().cloned().collect::<HashSet<_>>(), expected);
                conditional += 1;
            }
            IrShape::SensLoop { reads } => {
                assert_eq!(reads.len(), 1);
                assert!(expected.contains(&reads[0]), "no LHS/self-wake dependency");
            }
            _ => panic!("continuous driver is neither run-once nor source-sensitive"),
        }
    }
    assert_eq!(constant, 1);
    assert_eq!(conditional, 1);
    assert_eq!(cg.model.processes.len(), 5);
    let mut counts = HashMap::new();
    let mut slots = HashSet::new();
    for ((_, source, group), id) in &cg.structural_driver_sites {
        if matches!(database.node_kind(*source), NodeKind::ContAssign { .. }) {
            let driver = &cg.structural_drivers[id.0 as usize];
            let (actual_group, slot) = cg.model.signals[driver.signal].net_driver.unwrap();
            assert_eq!(*group, actual_group);
            assert!(
                slots.insert((actual_group, slot)),
                "independent continuous sources share storage"
            );
            *counts.entry(actual_group).or_insert(0usize) += 1;
        }
    }
    // Each bit of a fixed net-array cell is its own electrical group, and
    // two whole-array sources and the positional-pattern source
    // each own one contribution slot on every bit of both elements.
    assert_eq!(counts.len(), 2 * 65, "one group per fixed net-array bit");
    assert!(counts.values().all(|count| *count == 3));
    for group in counts.keys() {
        assert_eq!(cg.model.net_groups[*group].width, 1);
    }
}

#[test]
fn continuous_variable_conflicts_are_owned_errors_but_overrides_are_not() {
    for (name, source, conflict) in [
        ("mixed.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_20_23/continuous_mixed_writer_error.sv"), true),
        ("initialized.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_20_23/continuous_initialized_writer_error.sv"), true),
        ("force.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_20_23/continuous_force_control.sv"), false),
    ] {
        let database = {
            // The pinned frontend warns about these conflicts; the owned
            // simulator boundary must still refuse to execute the invalid mix.
            let result = crate::core::compile::compile_sources_checked(
                &[crate::core::compile::OwnedSource::compilation_unit(name, source)],
                &crate::core::compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
            ).unwrap();
            Db::from_slang(&result.snapshot).unwrap()
        };
        database.validate().unwrap();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        cg.collect_design().unwrap();
        cg.bind_reference_ports().unwrap();
        cg.collect_timescales();
        cg.build_net_groups().unwrap();
        let result = cg.validate_process_semantics();
        if conflict {
            assert!(result.unwrap_err().contains("has both a continuous assignment"));
        } else {
            result.expect("force/release and disjoint bit assignments are not mixed drivers");
        }
    }
}

#[test]
fn writes_inside_continuously_called_functions_are_procedural() {
    // SV 6.5: only the left-hand side is a continuous driver. A write made by
    // a function the right-hand side calls is a procedural assignment, so it
    // may share storage with other procedural writers but not with a
    // continuous driver.
    for (source, conflict) in [
        (
            "module tb; logic [7:0] x, y; int cnt = 0;\n\
             function automatic logic [7:0] f(input logic [7:0] v); cnt++; return v; endfunction\n\
             assign y = f(x); initial cnt = 5; endmodule",
            false,
        ),
        (
            "module tb; logic [7:0] x, y, z; int cnt;\n\
             function automatic logic [7:0] f(input logic [7:0] v); cnt++; return v; endfunction\n\
             assign y = f(x); assign z = f(x); endmodule",
            false,
        ),
        (
            "module tb; logic [7:0] x, y; int cnt;\n\
             function automatic logic [7:0] f(input logic [7:0] v); cnt++; return v; endfunction\n\
             assign cnt = 0; assign y = f(x); endmodule",
            true,
        ),
    ] {
        let database = {
            let result = crate::core::compile::compile_sources_checked(
                &[crate::core::compile::OwnedSource::compilation_unit(
                    "called_writes.sv",
                    source,
                )],
                &crate::core::compile::CompileOpts {
                    top: Some("tb".to_owned()),
                    ..Default::default()
                },
            )
            .unwrap();
            Db::from_slang(&result.snapshot).unwrap()
        };
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        cg.collect_design().unwrap();
        cg.bind_reference_ports().unwrap();
        cg.collect_timescales();
        cg.build_net_groups().unwrap();
        let result = cg.validate_process_semantics();
        if conflict {
            let error = result.unwrap_err();
            assert!(
                error.contains("has both a continuous assignment"),
                "{error}"
            );
            assert!(error.contains("(called function)"), "{error}");
        } else {
            result.unwrap();
        }
    }
}

fn diagnostic_database(source: &str) -> Db {
    let compiled = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            "storage_diagnostics.sv",
            source,
        )],
        &crate::core::compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    Db::from_slang(&compiled.snapshot).unwrap()
}

#[test]
fn dependency_diagnostics_use_owned_source_paths_and_declared_indices() {
    let database = diagnostic_database(
        r#"
module child;
    logic [12:5] \bus+index ;
    logic [0:7] ascending;
    logic [1:0][5:2] matrix;
    real level;
    logic [7:0] grid [2:1][-1:1];
    int queue_value[$];
    int map_value[string];
    string message;
    typedef struct { logic [7:0] data; logic [12:5] shifted; string text; } inner_t;
    typedef struct { inner_t inner; logic [7:0] lanes [2:1]; } record_t;
    record_t value;
    wire [7:0] linked;
    assign linked = \bus+index ;
endmodule
module tb;
    child \a.b ();
endmodule
"#,
    );
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    let signal = |name: &str| {
        let node = database
            .node_ids()
            .find(|node| database.node(*node).name == name)
            .unwrap();
        cg.signal_of(node).unwrap()
    };
    let bus = cg.signal_dependency(signal("bus+index"));
    assert_eq!(cg.dependency_label(&bus), "tb.a.b.bus+index");
    assert_eq!(
        cg.dependency_label(&cg.signal_dependency(signal("level"))),
        "tb.a.b.level"
    );
    assert_eq!(
        cg.dependency_label(&IrDependency::PackedRange {
            storage: Box::new(bus),
            lsb: 1,
            width: 3,
        }),
        "tb.a.b.bus+index[6 +: 3]"
    );
    assert_eq!(
        cg.dependency_label(&IrDependency::PackedRange {
            storage: Box::new(cg.signal_dependency(signal("ascending"))),
            lsb: 1,
            width: 3,
        }),
        "tb.a.b.ascending[6 -: 3]"
    );
    assert_eq!(
        cg.dependency_label(&IrDependency::PackedRange {
            storage: Box::new(cg.signal_dependency(signal("matrix"))),
            lsb: 1,
            width: 3,
        }),
        "tb.a.b.matrix (packed bits 1 +: 3)"
    );
    let grid = cg
        .array_globals
        .iter()
        .find(|(node, _)| database.node(**node).name == "grid")
        .unwrap()
        .1
        .ir;
    assert_eq!(
        cg.dependency_label(&IrDependency::ArrayElement {
            array: grid,
            index: 0
        }),
        "tb.a.b.grid[2][-1]"
    );
    assert_eq!(
        cg.dependency_label(&IrDependency::ArrayElement {
            array: grid,
            index: 5
        }),
        "tb.a.b.grid[1][1]"
    );
    assert_eq!(
        cg.dependency_label(&IrDependency::ArrayContents(grid)),
        "tb.a.b.grid"
    );
    let queue = cg
        .container_globals
        .iter()
        .find(|(node, _)| database.node(**node).name == "queue_value")
        .unwrap()
        .1
        .ir;
    assert_eq!(
        cg.dependency_label(&IrDependency::ContainerContents(queue)),
        "tb.a.b.queue_value"
    );
    assert_eq!(
        cg.dependency_label(&IrDependency::ContainerShape(queue)),
        "tb.a.b.queue_value.size()"
    );
    let map = cg
        .container_globals
        .iter()
        .find(|(node, _)| database.node(**node).name == "map_value")
        .unwrap()
        .1
        .ir;
    assert_eq!(
        cg.dependency_label(&IrDependency::ContainerShape(map)),
        "tb.a.b.map_value.num()"
    );
    let message = *cg
        .object_globals
        .iter()
        .find(|(node, _)| database.node(**node).name == "message")
        .unwrap()
        .1;
    assert_eq!(
        cg.dependency_label(&IrDependency::Object(message)),
        "tb.a.b.message"
    );
    let labels: Vec<_> = (0..cg.model.signals.len())
        .map(|signal| cg.signal_label(signal))
        .collect();
    assert!(
        labels.contains(&"tb.a.b.value.inner.data".to_owned()),
        "{labels:?}"
    );
    assert!(
        labels.contains(&"tb.a.b.value.lanes[2]".to_owned()),
        "{labels:?}"
    );
    let value = database
        .node_ids()
        .find(|node| database.node(*node).name == "value")
        .unwrap();
    let shifted = cg.unpacked_aggregates[&value]
        .leaves
        .iter()
        .find(|leaf| {
            leaf.path
                == [
                    AggregatePathPart::Member("inner".to_owned()),
                    AggregatePathPart::Member("shifted".to_owned()),
                ]
        })
        .unwrap()
        .signal
        .as_ref()
        .unwrap();
    assert_eq!(
        cg.dependency_label(&IrDependency::PackedRange {
            storage: Box::new(cg.signal_dependency(shifted)),
            lsb: 1,
            width: 3,
        }),
        "tb.a.b.value.inner.shifted[6 +: 3]"
    );
    let text = cg.unpacked_aggregates[&value]
        .leaves
        .iter()
        .find_map(|leaf| leaf.object)
        .unwrap();
    assert_eq!(
        cg.dependency_label(&IrDependency::Object(text)),
        "tb.a.b.value.inner.text"
    );

    cg.collect_timescales();
    cg.build_net_groups().unwrap();
    let group = cg
        .model
        .signals
        .iter()
        .find(|signal| signal.hdl_name.as_deref() == Some("tb\u{1f}a.b\u{1f}linked"))
        .unwrap()
        .net_driver
        .unwrap()
        .0;
    cg.model.net_groups[group].n_drivers = LLG_MAX_NET_DRIVERS;
    let source = database
        .node_ids()
        .find(|node| matches!(database.node_kind(*node), NodeKind::ContAssign { .. }))
        .unwrap();
    let error = cg
        .add_structural_driver_for_terminal(group, source, (6, 6), 1)
        .unwrap_err();
    assert!(error.contains("resolved net `tb.a.b.linked`"), "{error}");
    assert!(!error.contains("G_"), "{error}");
}

#[test]
fn real_edge_rejections_use_source_storage_names() {
    for (source, expected) in [
        (
            r#"module tb;
            real \level+offset ;
            initial @(\level+offset );
        endmodule"#,
            "tb.level+offset",
        ),
        (
            r#"module tb;
            task wait_for_edge;
                real \level+offset ;
                @(\level+offset );
            endtask
            initial wait_for_edge();
        endmodule"#,
            "tb.wait_for_edge.level+offset",
        ),
    ] {
        // Use a checked any-change source, then exercise the backend edge
        // guard directly: Slang replaces rejected real-edge expressions by Invalid.
        let database = diagnostic_database(source);
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        let top = cg.collect_design().unwrap()[0];
        cg.emit_func_prototypes(top).unwrap();
        cg.inst = top;
        let expression = database
            .node_ids()
            .find_map(|node| match database.node_kind(node) {
                NodeKind::Stmt(StmtKind::EventControl { specs, .. }) => {
                    specs.iter().find_map(|spec| match spec {
                        EventSpec::AnyChange { sig } => Some(*sig),
                        _ => None,
                    })
                }
                _ => None,
            })
            .unwrap();
        let mut context = EmitCtx::new(&mut cg, "tb".to_owned(), top, "0", None, None, false);
        let error = context
            .lower_event_specs(&[EventSpec::Edge {
                sig: expression,
                posedge: true,
            }])
            .unwrap_err();
        assert!(
            error.contains(&format!("real-valued signal `{expected}`")),
            "{error}"
        );
        for internal in ["G_", "D_", "cI_", "__llg_ident_"] {
            assert!(!error.contains(internal), "{error}");
        }
    }
}

fn lowered_continuous_model(source: &str) -> (Db, Vec<IrProcess>) {
    let database = diagnostic_database(source);
    database.validate().unwrap();
    let processes = {
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        let tops = cg.collect_design().unwrap();
        cg.bind_reference_ports().unwrap();
        cg.collect_timescales();
        cg.build_net_groups().unwrap();
        cg.validate_process_semantics().unwrap();
        for top in tops {
            cg.emit_pass(top, Pass::Comb).unwrap();
        }
        cg.model.processes.clone()
    };
    (database, processes)
}

fn wraps_feedback_loop(process: &IrProcess) -> bool {
    process
        .body
        .iter()
        .any(|statement| matches!(statement.unlocated(), IrStmt::While { .. }))
}

#[test]
fn component_self_feedback_wraps_only_self_dependent_drivers() {
    // SV 10.3.2: a continuous driver whose own write can change one of its
    // operands repeats in place. Drivers that read only other storage keep
    // the single-evaluation sensitivity loop.
    let (_database, processes) = lowered_continuous_model(
        "module tb;\n\
         wire [3:0] a, b, c; alias a = b; logic go;\n\
         logic [3:0] arr[0:1]; logic [3:0] v, w;\n\
         assign c = go ? 4'd1 : 4'd0;\n\
         assign a = go ? b + 4'd1 : 4'd0;\n\
         assign arr = '{4'd1, arr[0]};\n\
         assign v = w;\n\
         endmodule",
    );
    let continuous = processes
        .iter()
        .filter(|process| process.label.ends_with(".assign"))
        .collect::<Vec<_>>();
    assert_eq!(continuous.len(), 4);
    let wrapped = continuous
        .iter()
        .filter(|process| wraps_feedback_loop(process))
        .count();
    assert_eq!(wrapped, 2, "alias view and array cell feedback only");
}

#[test]
fn component_constant_rows_are_precise_writer_projections() {
    // SV 6.5: a constant row is its own longest static prefix, so a
    // continuous row and a procedural write of another row do not overlap.
    for (source, conflict) in [
        (
            "module tb; logic [3:0] m[0:1][0:2]; logic [3:0] x;\n\
             assign m[0] = '{x, x, x}; initial m[1][0] = 4'h1; endmodule",
            false,
        ),
        (
            "module tb; logic [3:0] m[0:1][0:2]; logic [3:0] x;\n\
             assign m[0] = '{x, x, x}; initial m[0][2] = 4'h1; endmodule",
            true,
        ),
        (
            "module tb; logic [3:0] m[0:1][0:2]; logic [3:0] x;\n\
             assign m[1] = '{x, x, x}; initial m[0] = '{x, x, x}; endmodule",
            false,
        ),
    ] {
        let database = diagnostic_database(source);
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        cg.collect_design().unwrap();
        cg.bind_reference_ports().unwrap();
        cg.collect_timescales();
        cg.build_net_groups().unwrap();
        let result = cg.validate_process_semantics();
        if conflict {
            let error = result.unwrap_err();
            assert!(error.contains("`tb.m[0][2]`"), "{error}");
        } else {
            result.unwrap();
        }
    }
}
