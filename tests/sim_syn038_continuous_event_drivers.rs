//! SYN-038 event observers track net and variable continuous drivers.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{
        AlwaysKind, Db, EventSpec, ExprKind, NetType, NodeId, NodeKind, Operation, ProcessKind,
        StmtKind,
    },
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/continuous_event_drivers.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/continuous_event_drivers.sv");
const EXPECTED_STDOUT: &str = "wire=1,1,1,1,0,1 logic=1,1,1,1,0,1\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventContext {
    Initial,
    Always,
    AlwaysComb,
    AlwaysLatch,
    AlwaysFf,
    Subroutine,
}

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 continuous event drivers compile");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn top_module(db: &Db) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == "tb"
                && matches!(db.node_kind(*id), NodeKind::ModuleInst { is_top: true, .. })
        })
        .expect("top module instance is captured")
}

fn outer_declaration(db: &Db, module: NodeId, name: &str) -> NodeId {
    let declaration = db
        .node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Net { .. } | NodeKind::Var { .. }
                )
        })
        .unwrap_or_else(|| panic!("outer declaration `{name}` is captured"));
    assert_eq!(
        db.node(declaration).parent(),
        Some(module),
        "`{name}` is declared directly in tb"
    );
    declaration
}

fn direct_reference_target(db: &Db, expression: NodeId) -> Option<NodeId> {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => Some(*target),
        _ => None,
    }
}

fn required_direct_reference_target(db: &Db, expression: NodeId) -> NodeId {
    direct_reference_target(db, expression).unwrap_or_else(|| {
        panic!(
            "expected a bound direct reference, got {:?}",
            db.node_kind(expression)
        )
    })
}

fn source_line(anchor: &str) -> u32 {
    FIXTURE_SOURCE
        .lines()
        .position(|line| line.trim() == anchor)
        .unwrap_or_else(|| panic!("fixture source is missing `{anchor}`")) as u32
        + 1
}

fn continuous_driver(db: &Db, source: NodeId, line: u32) -> NodeId {
    let matches = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::ContAssign {
                        net_decl: false,
                        ..
                    }
                )
                && db
                    .node(*id)
                    .children()
                    .first()
                    .and_then(|lhs| direct_reference_target(db, *lhs))
                    .is_some_and(|target| db.source_identity(target) == db.source_identity(source))
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one whole-object continuous driver");
    let driver = matches[0];
    let children = db.node(driver).children();
    assert_eq!(children.len(), 2, "driver retains LHS and RHS");
    assert_eq!(
        db.source_identity(required_direct_reference_target(db, children[0])),
        db.source_identity(source)
    );
    driver
}

fn event_context(db: &Db, mut node: NodeId) -> Option<EventContext> {
    loop {
        match db.node_kind(node) {
            NodeKind::Process {
                kind: ProcessKind::Initial,
            } => return Some(EventContext::Initial),
            NodeKind::Process {
                kind:
                    ProcessKind::Always {
                        always_type: AlwaysKind::Always,
                    },
            } => return Some(EventContext::Always),
            NodeKind::Process {
                kind:
                    ProcessKind::Always {
                        always_type: AlwaysKind::Comb,
                    },
            } => return Some(EventContext::AlwaysComb),
            NodeKind::Process {
                kind:
                    ProcessKind::Always {
                        always_type: AlwaysKind::Latch,
                    },
            } => return Some(EventContext::AlwaysLatch),
            NodeKind::Process {
                kind:
                    ProcessKind::Always {
                        always_type: AlwaysKind::FlipFlop,
                    },
            } => return Some(EventContext::AlwaysFf),
            NodeKind::FuncTask { is_task: true, .. } => return Some(EventContext::Subroutine),
            _ => {}
        }
        node = db.node(node).parent()?;
    }
}

fn event_controls_for_source(
    db: &Db,
    source: NodeId,
) -> Vec<(NodeId, NodeId, EventContext, EventSpec)> {
    db.node_ids()
        .filter_map(|id| {
            let NodeKind::Stmt(StmtKind::EventControl {
                specs,
                implicit: false,
                ..
            }) = db.node_kind(id)
            else {
                return None;
            };
            let context = event_context(db, id)?;
            let spec = specs.first()?;
            let operand = match spec {
                EventSpec::AnyChange { sig } | EventSpec::Edge { sig, .. } => *sig,
                EventSpec::Qualified { .. } | EventSpec::Named(_) => return None,
            };
            (direct_reference_target(db, operand)
                .is_some_and(|target| db.source_identity(target) == db.source_identity(source)))
            .then(|| (id, operand, context, spec.clone()))
        })
        .collect()
}

fn assignment_rhs_consumers_for_source(
    db: &Db,
    source: NodeId,
) -> Vec<(NodeId, NodeId, EventContext)> {
    db.node_ids()
        .filter_map(|id| {
            if !matches!(
                db.node_kind(id),
                NodeKind::Stmt(StmtKind::Assign {
                    blocking: true,
                    op: Operation::Assignment,
                    delay: None,
                })
            ) {
                return None;
            }
            let rhs = *db.node(id).children().get(1)?;
            let target = direct_reference_target(db, rhs)?;
            if db.source_identity(target) != db.source_identity(source) {
                return None;
            }
            let context = event_context(db, id)?;
            matches!(
                context,
                EventContext::AlwaysComb | EventContext::AlwaysLatch
            )
            .then_some((id, rhs, context))
        })
        .collect()
}

fn named_task(db: &Db, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: true, .. })
        })
        .unwrap_or_else(|| panic!("task `{name}` is captured"))
}

fn task_has_initial_caller(db: &Db, task: NodeId) -> bool {
    db.node_ids().any(|id| {
        matches!(
            db.node_kind(id),
            NodeKind::FuncCall {
                is_task: true,
                callee: Some(callee),
                ..
            } if db.source_identity(*callee) == db.source_identity(task)
        ) && runs_in_initial(db, id)
    })
}

fn runs_in_initial(db: &Db, mut node: NodeId) -> bool {
    loop {
        if matches!(
            db.node_kind(node),
            NodeKind::Process {
                kind: ProcessKind::Initial
            }
        ) {
            return true;
        }
        let Some(parent) = db.node(node).parent() else {
            return false;
        };
        node = parent;
    }
}

#[test]
fn continuous_net_and_variable_events_match_exact_cli_oracle_in_both_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/continuous_event_drivers.sv\n"
    ));
    for anchor in [
        "assign wire_source = wire_seed;",
        "assign logic_source = logic_seed;",
        "always_comb wire_comb_sample = wire_source;",
        "always_comb logic_comb_sample = logic_source;",
        "wire_latch_sample = wire_source;",
        "logic_latch_sample = logic_source;",
        "always @(wire_source) begin",
        "always @(logic_source) begin",
        "always_ff @(posedge wire_source) begin",
        "always_ff @(posedge logic_source) begin",
        "task automatic wait_wire_source();",
        "task automatic wait_logic_source();",
        "@(wire_source);",
        "@(logic_source);",
        "if (wire_source !== 1'b0 || logic_source !== 1'b0",
        "if (wire_source !== 1'b1)",
        "if (logic_source !== 1'b1)",
        "wire_comb_sample !== 1'b1 || logic_comb_sample !== 1'b1",
        "if (wire_latch_sample !== 1'b1 || logic_latch_sample !== 1'b1)",
        "if (!wire_initial_seen || !logic_initial_seen",
        "if (wire_always_sample !== 1'b1 || logic_always_sample !== 1'b1",
        "$display(\"wire=%0d,%0d,%0d,%0d,%0d,%0d logic=%0d,%0d,%0d,%0d,%0d,%0d\",",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost source oracle: {anchor}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "continuous_event_drivers",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_continuous_lhs_event_and_assignment_rhs_consumers_bind_to_outer_source() {
    let db = compile_fixture();
    let module = top_module(&db);

    for (source_name, seed_name, driver_anchor, task_name, comb_anchor, latch_anchor, variable) in [
        (
            "wire_source",
            "wire_seed",
            "assign wire_source = wire_seed;",
            "wait_wire_source",
            "always_comb wire_comb_sample = wire_source;",
            "wire_latch_sample = wire_source;",
            false,
        ),
        (
            "logic_source",
            "logic_seed",
            "assign logic_source = logic_seed;",
            "wait_logic_source",
            "always_comb logic_comb_sample = logic_source;",
            "logic_latch_sample = logic_source;",
            true,
        ),
    ] {
        let source = outer_declaration(&db, module, source_name);
        if variable {
            assert!(matches!(db.node_kind(source), NodeKind::Var { .. }));
        } else {
            assert!(matches!(
                db.node_kind(source),
                NodeKind::Net {
                    net_type: NetType::Wire,
                    ..
                }
            ));
        }

        let driver = continuous_driver(&db, source, source_line(driver_anchor));
        let seed = outer_declaration(&db, module, seed_name);
        let driver_rhs = db.node(driver).children()[1];
        assert_eq!(
            db.source_identity(required_direct_reference_target(&db, driver_rhs)),
            db.source_identity(seed)
        );

        let rhs_consumers = assignment_rhs_consumers_for_source(&db, source);
        assert_eq!(
            rhs_consumers.len(),
            2,
            "always_comb and always_latch read source"
        );
        for (context, anchor) in [
            (EventContext::AlwaysComb, comb_anchor),
            (EventContext::AlwaysLatch, latch_anchor),
        ] {
            let matching = rhs_consumers
                .iter()
                .filter(|(_, _, actual_context)| *actual_context == context)
                .collect::<Vec<_>>();
            assert_eq!(matching.len(), 1, "one {context:?} assignment RHS reader");
            let (statement, rhs, _) = matching[0];
            assert_eq!(db.node(*statement).line(), source_line(anchor));
            let lhs = *db
                .node(*statement)
                .children()
                .first()
                .expect("assignment retains its LHS");
            assert_ne!(
                direct_reference_target(&db, lhs).map(|target| db.source_identity(target)),
                Some(db.source_identity(source)),
                "the assignment-RHS focal source is not the receiving target"
            );
            assert_eq!(
                db.source_identity(required_direct_reference_target(&db, *rhs)),
                db.source_identity(source),
                "assignment RHS resolves to the continuously driven outer declaration"
            );
        }

        let uses = event_controls_for_source(&db, source);
        assert_eq!(uses.len(), 4, "each source has four event consumers");
        for context in [
            EventContext::Initial,
            EventContext::Always,
            EventContext::AlwaysFf,
            EventContext::Subroutine,
        ] {
            let matching = uses
                .iter()
                .filter(|(_, _, actual_context, _)| *actual_context == context)
                .collect::<Vec<_>>();
            assert_eq!(matching.len(), 1, "one {context:?} event consumer");
            let (control, operand, _, spec) = matching[0];
            assert_eq!(
                db.source_identity(required_direct_reference_target(&db, *operand)),
                db.source_identity(source),
                "event operand resolves to the continuously driven outer declaration"
            );
            if context == EventContext::AlwaysFf {
                assert!(matches!(spec, EventSpec::Edge { posedge: true, .. }));
            } else {
                assert!(matches!(spec, EventSpec::AnyChange { .. }));
            }
            if context == EventContext::Initial || context == EventContext::Always {
                assert!(runs_in_initial(&db, *control) == (context == EventContext::Initial));
            }
            if context == EventContext::Subroutine {
                assert_eq!(
                    db.source_identity(
                        db.node(*control)
                            .parent()
                            .and_then(|parent| {
                                let mut current = Some(parent);
                                while let Some(node) = current {
                                    if matches!(
                                        db.node_kind(node),
                                        NodeKind::FuncTask { is_task: true, .. }
                                    ) {
                                        return Some(node);
                                    }
                                    current = db.node(node).parent();
                                }
                                None
                            })
                            .expect("subroutine event has its containing task")
                    ),
                    db.source_identity(named_task(&db, task_name)),
                    "event expression remains in the corresponding task body"
                );
                assert!(task_has_initial_caller(&db, named_task(&db, task_name)));
            }
        }
    }
}
