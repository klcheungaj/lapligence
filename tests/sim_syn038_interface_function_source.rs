//! SYN-038 mutable interface-member source read through a function result.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, ExprKind, NodeKind, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/interface_function_source.sv");
const EXPECTED_STDOUT: &str = "member_through_function=31,6b sibling=92\n";
const EXPECTED_STDERR: &str = "llg: $finish at time 0 at tb:35:9\n";

#[test]
fn mutable_interface_member_source_flows_through_function_result_in_both_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_function_source.sv\n"
    ));
    for anchor in [
        "data_if bus();",
        "data_if sibling();",
        "function automatic logic [7:0] read_member();",
        "return bus.member;",
        "bus.member = 8'h31;",
        "sibling.member = 8'h92;",
        "first_sample = read_member();",
        "bus.member = 8'h6b;",
        "second_sample = read_member();",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "missing source anchor: {anchor}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "interface_function_source",
        EXPECTED_STDOUT,
        EXPECTED_STDERR,
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn slang_binds_the_return_source_to_the_exact_mutable_interface_member_node() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn038_pairwise/interface_function_source.sv");
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal function return from mutable interface storage compiles");
    let db = Db::from_slang(&compiled.snapshot).expect("capture owned semantic database");

    let bus = db
        .node_ids()
        .find(|id| {
            db.node(*id).name() == "bus"
                && matches!(
                    db.node_kind(*id),
                    NodeKind::ModuleInst {
                        def_name,
                        is_interface: true,
                        ..
                    } if def_name == "data_if"
                )
        })
        .expect("primary interface instance is captured");
    let sibling = db
        .node_ids()
        .find(|id| {
            db.node(*id).name() == "sibling"
                && matches!(
                    db.node_kind(*id),
                    NodeKind::ModuleInst {
                        def_name,
                        is_interface: true,
                        ..
                    } if def_name == "data_if"
                )
        })
        .expect("independent interface instance is captured");
    let member_of = |instance| {
        db.node(instance)
            .children()
            .iter()
            .copied()
            .find(|child| {
                db.node(*child).name() == "member"
                    && matches!(db.node_kind(*child), NodeKind::Var { .. })
            })
            .expect("mutable interface data member is captured")
    };
    let bus_member = member_of(bus);
    let sibling_member = member_of(sibling);
    assert_ne!(
        bus_member, sibling_member,
        "instance members have distinct NodeIds"
    );

    let focal_line = FIXTURE_SOURCE
        .lines()
        .position(|line| line.trim() == "return bus.member;")
        .expect("fixture contains the focal return source") as u32
        + 1;
    let focal_source = db
        .node_ids()
        .find(|id| {
            let node = db.node(*id);
            node.file()
                .is_some_and(|file| file.ends_with("interface_function_source.sv"))
                && node.line() == focal_line
                && node.name() == "member"
                && matches!(db.node_kind(*id), NodeKind::Expr(ExprKind::Ref { .. }))
        })
        .expect("the return member expression is retained as an exact source node");
    let NodeKind::Expr(ExprKind::Ref { target }) = db.node_kind(focal_source) else {
        unreachable!("filtered to the focal reference")
    };
    assert_eq!(*target, Some(bus_member));
    assert_ne!(*target, Some(sibling_member));
    let focal_source_line = FIXTURE_SOURCE
        .lines()
        .nth(focal_line as usize - 1)
        .expect("focal return line exists");
    assert_eq!(
        db.node(focal_source).column() as usize,
        focal_source_line
            .find("bus.member")
            .expect("member route spelling")
            + 1,
        "Slang binds the exact `bus.member` source occurrence"
    );

    let function = db
        .node_ids()
        .find(|id| {
            db.node(*id).name() == "read_member"
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: false, .. })
        })
        .expect("user function declaration is captured");
    let function_body = match db.node_kind(function) {
        NodeKind::FuncTask {
            body: Some(body), ..
        } => *body,
        _ => unreachable!("filtered to a function with a body"),
    };
    let mut pending = vec![function_body];
    let mut return_reads_focal_source = false;
    while let Some(node) = pending.pop() {
        if matches!(
            db.node_kind(node),
            NodeKind::Stmt(StmtKind::Return { value: Some(value) }) if *value == focal_source
        ) {
            return_reads_focal_source = true;
            break;
        }
        pending.extend(db.node(node).children().iter().copied());
    }
    assert!(
        return_reads_focal_source,
        "function return statement points to the exact focal source NodeId"
    );

    let caller_call = db
        .node_ids()
        .find(|id| {
            let node = db.node(*id);
            node.file()
                .is_some_and(|file| file.ends_with("interface_function_source.sv"))
                && matches!(
                    db.node_kind(*id),
                    NodeKind::FuncCall { name, callee: Some(callee), is_task: false, .. }
                        if name == "read_member" && *callee == function
                )
        })
        .expect("caller call binds to the function whose return reads the interface member");
    assert!(db.node(caller_call).line() > focal_line);

    let mut caller_assignments = db
        .node_ids()
        .filter(|id| {
            matches!(
                db.node_kind(*id),
                NodeKind::Stmt(StmtKind::Assign { blocking: true, .. })
            ) && db.node(*id).children().get(1) == Some(&caller_call)
        })
        .collect::<Vec<_>>();
    assert_eq!(caller_assignments.len(), 1);
    let caller_assignment = caller_assignments.pop().expect("one caller assignment");
    let lhs = db.node(caller_assignment).children()[0];
    let NodeKind::Expr(ExprKind::Ref {
        target: Some(target),
    }) = db.node_kind(lhs)
    else {
        panic!("function result is assigned to a distinct local variable");
    };
    assert!(matches!(db.node_kind(*target), NodeKind::Var { .. }));
    assert_ne!(
        *target, bus_member,
        "caller destination is not the focal source slot"
    );
    assert!(
        db.node(*target).name() == "first_sample" || db.node(*target).name() == "second_sample",
        "function result destination is one of the independent sample variables"
    );

    let mut parent = db.node(caller_assignment).parent();
    let mut in_initial = false;
    while let Some(ancestor) = parent {
        if matches!(
            db.node_kind(ancestor),
            NodeKind::Process {
                kind: ProcessKind::Initial
            }
        ) {
            in_initial = true;
            break;
        }
        parent = db.node(ancestor).parent();
    }
    assert!(
        in_initial,
        "outer function-result use executes in initial process"
    );
}
