//! SYN-038 written locals flow through function returns and declaration initializers.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{
        Db, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind, VariableLifetime,
        VariableLifetimeQualifier,
    },
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/written_source_return_initializers.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/written_source_return_initializers.sv");
const EXPECTED_STDOUT: &str = "local_initializer=5a\ninitializer_source=5a\nstatic_initializer=5a\nlocal_return=5a\nmodule_initializer=5a\nconstant_initializer=5a\n";

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("the SV2009 written-source return/initializer fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn source_line(anchor: &str) -> u32 {
    FIXTURE_SOURCE
        .lines()
        .position(|line| line.trim() == anchor)
        .unwrap_or_else(|| panic!("fixture is missing source anchor `{anchor}`")) as u32
        + 1
}

fn node_at_line(
    db: &Db,
    line: u32,
    matches: impl Fn(&NodeKind) -> bool,
    description: &str,
) -> NodeId {
    let found = db
        .node_ids()
        .filter(|id| db.node(*id).line() == line && matches(db.node_kind(*id)))
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "one {description} at fixture line {line}");
    found[0]
}

fn declaration(db: &Db, name: &str, anchor: &str) -> NodeId {
    let line = source_line(anchor);
    let found = db
        .node_ids()
        .filter(|id| {
            db.node(*id).name() == name
                && db.node(*id).line() == line
                && matches!(db.node_kind(*id), NodeKind::Var { .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        found.len(),
        1,
        "one variable `{name}` at fixture line {line}"
    );
    found[0]
}

fn ref_target(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => *target,
        other => panic!("expected a bound direct source reference, got {other:?}"),
    }
}

fn process_ancestor(db: &Db, mut node: NodeId) -> NodeId {
    loop {
        if matches!(db.node_kind(node), NodeKind::Process { .. }) {
            return node;
        }
        node = db.node(node).parent().unwrap_or_else(|| {
            panic!(
                "node {node:?} (`{}`, line {}) has no process ancestor",
                db.node(node).name(),
                db.node(node).line()
            )
        });
    }
}

fn initial_process(db: &Db) -> NodeId {
    node_at_line(
        db,
        source_line("initial begin : check"),
        |kind| {
            matches!(
                kind,
                NodeKind::Process {
                    kind: ProcessKind::Initial
                }
            )
        },
        "initial process",
    )
}

fn blocking_writer(db: &Db, source: NodeId, anchor: &str) -> (NodeId, NodeId) {
    let writer = node_at_line(
        db,
        source_line(anchor),
        |kind| {
            matches!(
                kind,
                NodeKind::Stmt(StmtKind::Assign {
                    blocking: true,
                    op: Operation::Assignment,
                    delay: None,
                })
            )
        },
        "whole-object blocking writer",
    );
    let children = db.node(writer).children();
    assert_eq!(children.len(), 2, "writer retains its LHS and RHS");
    let target = ref_target(db, children[0]);
    assert_eq!(
        db.source_identity(target),
        db.source_identity(source),
        "blocking LHS targets the named focal declaration"
    );
    (writer, target)
}

fn return_read(db: &Db, anchor: &str, source: NodeId) -> (NodeId, NodeId) {
    let statement = node_at_line(
        db,
        source_line(anchor),
        |kind| matches!(kind, NodeKind::Stmt(StmtKind::Return { value: Some(_) })),
        "value-return statement",
    );
    let NodeKind::Stmt(StmtKind::Return { value: Some(value) }) = db.node_kind(statement) else {
        unreachable!("filtered to a value-return statement")
    };
    let target = ref_target(db, *value);
    assert_eq!(
        db.source_identity(target),
        db.source_identity(source),
        "return RHS reads the focal declaration"
    );
    (statement, target)
}

fn assert_call_initializer(db: &Db, receiver: NodeId, callee_name: &str) -> NodeId {
    let initializer = db
        .var_initializer(receiver)
        .expect("receiver retains its declaration initializer");
    let NodeKind::FuncCall {
        name,
        is_task: false,
        callee,
        ..
    } = db.node_kind(initializer)
    else {
        panic!("receiver initializer is a function call");
    };
    assert_eq!(name, callee_name);
    let callee = callee.expect("initializer call resolves to its helper");
    assert_eq!(db.node(callee).name(), callee_name);
    initializer
}

fn subroutine(db: &Db, name: &str) -> NodeId {
    let found = db
        .node_ids()
        .filter(|id| {
            db.node(*id).name() == name
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: false, .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "one function named `{name}`");
    found[0]
}

fn top_module(db: &Db) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == "tb"
                && matches!(db.node_kind(*id), NodeKind::ModuleInst { is_top: true, .. })
        })
        .expect("top module instance is captured")
}

fn assert_has_ancestor(db: &Db, mut node: NodeId, ancestor: NodeId, message: &str) {
    loop {
        if node == ancestor {
            return;
        }
        node = db
            .node(node)
            .parent()
            .unwrap_or_else(|| panic!("{message}: no matching lexical ancestor"));
    }
}

#[test]
fn written_return_and_initializer_sources_match_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_source_return_initializers.sv\n"
    ));
    for vector in [
        "CO=function_return_statement in HC=subroutine;\n    // its caller runs in PC=initial.",
        "CO=function_return_statement, HC=subroutine, PC=none.",
        "CO=declaration_initializer, IN=runtime_declaration,\n    // HC=module, PC=none.",
        "CO=declaration_initializer, IN=constant_declaration, HC=module, PC=none.",
        "CO=declaration_initializer, IN=automatic_local,\n            // HC=module, PC=initial, WK=procedural_blocking, LV=whole_object.",
        "CO=declaration_initializer, IN=static_local, HC=module, PC=none;",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(vector),
            "missing focal vector: {vector}"
        );
    }
    for anchor in [
        "return_source = 8'h5a;",
        "return return_source;",
        "helper_source = seed;",
        "return helper_source;",
        "logic [7:0] module_initializer = written_helper(8'h5a);",
        "localparam logic [7:0] constant_initializer = written_helper(8'h5a);",
        "module_source = 8'h5a;",
        "initializer_source = 8'h5a;",
        "automatic logic [7:0] local_initializer = module_source;",
        "automatic logic [7:0] initializer_copy = initializer_source;",
        "static logic [7:0] static_initializer = written_helper(8'h5a);",
        "$finish(0);",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "missing focal source anchor: {anchor}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "written_source_return_initializers",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_blocking_writers_share_identity_with_return_and_initializer_reads() {
    let db = compile_fixture();
    let top = top_module(&db);

    let return_source = declaration(&db, "return_source", "logic [7:0] return_source;");
    assert_eq!(
        db.variable_lifetime(return_source),
        VariableLifetime::Automatic,
        "function-local return source has automatic storage"
    );
    let (return_writer, return_target) =
        blocking_writer(&db, return_source, "return_source = 8'h5a;");
    let (_, return_read_target) = return_read(&db, "return return_source;", return_source);
    assert_eq!(
        db.source_identity(return_target),
        db.source_identity(return_read_target),
        "writer LHS and function-return RHS share the same focal source identity"
    );
    let return_call = node_at_line(
        &db,
        source_line("local_return_value = return_written_local();"),
        |kind| matches!(kind, NodeKind::FuncCall { name, is_task: false, .. } if name == "return_written_local"),
        "function call at the initial-process consumer site",
    );
    assert!(matches!(
        db.node_kind(process_ancestor(&db, return_call)),
        NodeKind::Process {
            kind: ProcessKind::Initial
        }
    ));
    let return_function = subroutine(&db, "return_written_local");
    let mut parent = db.node(return_writer).parent();
    let mut in_return_function = false;
    while let Some(ancestor) = parent {
        if ancestor == return_function {
            in_return_function = true;
            break;
        }
        parent = db.node(ancestor).parent();
    }
    assert!(
        in_return_function,
        "return writer is lexical to its function"
    );

    let module_source = declaration(&db, "module_source", "logic [7:0] module_source;");
    let initializer_source =
        declaration(&db, "initializer_source", "logic [7:0] initializer_source;");
    let initial = initial_process(&db);
    for (source, write_anchor, receiver_name, receiver_anchor) in [
        (
            module_source,
            "module_source = 8'h5a;",
            "local_initializer",
            "automatic logic [7:0] local_initializer = module_source;",
        ),
        (
            initializer_source,
            "initializer_source = 8'h5a;",
            "initializer_copy",
            "automatic logic [7:0] initializer_copy = initializer_source;",
        ),
    ] {
        let receiver = declaration(&db, receiver_name, receiver_anchor);
        assert_has_ancestor(
            &db,
            receiver,
            top,
            "automatic declaration initializer is lexically in the module",
        );
        assert_eq!(
            db.variable_lifetime(receiver),
            VariableLifetime::Automatic,
            "automatic initializer receiver has automatic storage"
        );
        let (writer, writer_target) = blocking_writer(&db, source, write_anchor);
        let initializer = db
            .var_initializer(receiver)
            .expect("automatic receiver retains its initializer RHS");
        let initializer_target = ref_target(&db, initializer);
        assert_eq!(
            db.source_identity(writer_target),
            db.source_identity(initializer_target),
            "writer LHS and initializer RHS share the outer Slang source identity"
        );
        assert_eq!(
            db.source_identity(initializer_target),
            db.source_identity(source)
        );
        assert!(db.node(writer).line() < db.node(initializer).line());
        assert_eq!(process_ancestor(&db, writer), initial);
    }

    let helper_source = declaration(&db, "helper_source", "logic [7:0] helper_source;");
    assert_eq!(
        db.variable_lifetime(helper_source),
        VariableLifetime::Automatic,
        "automatic helper's source local has automatic storage"
    );
    let (_, helper_writer_target) = blocking_writer(&db, helper_source, "helper_source = seed;");
    let (_, helper_return_target) = return_read(&db, "return helper_source;", helper_source);
    assert_eq!(
        db.source_identity(helper_writer_target),
        db.source_identity(helper_return_target),
        "helper writer LHS and return RHS share the same focal source identity"
    );
    let helper_function = subroutine(&db, "written_helper");

    let module_initializer = declaration(
        &db,
        "module_initializer",
        "logic [7:0] module_initializer = written_helper(8'h5a);",
    );
    assert_eq!(
        db.variable_lifetime(module_initializer),
        VariableLifetime::Static,
        "module runtime initializer receiver has module storage"
    );
    let module_call = assert_call_initializer(&db, module_initializer, "written_helper");
    let NodeKind::FuncCall {
        callee: Some(module_callee),
        ..
    } = db.node_kind(module_call)
    else {
        unreachable!("initializer call has a resolved callee")
    };
    assert_eq!(
        db.source_identity(*module_callee),
        db.source_identity(helper_function),
        "module initializer resolves to the helper whose local return was written"
    );
    assert!(matches!(
        db.node_kind(
            db.node(module_initializer)
                .parent()
                .expect("module receiver has owner")
        ),
        NodeKind::ModuleInst { is_top: true, .. }
    ));
    assert_eq!(
        db.node(module_initializer).parent(),
        Some(top),
        "runtime declaration initializer is a module-level consumer"
    );

    let constant_initializer = db
        .node_ids()
        .find(|id| {
            db.node(*id).name() == "constant_initializer"
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Param {
                        value: Some(_),
                        local: true,
                        ..
                    }
                )
        })
        .expect("constant declaration has a captured elaborated value");
    assert!(matches!(
        db.node_kind(
            db.node(constant_initializer)
                .parent()
                .expect("constant has owner")
        ),
        NodeKind::ModuleInst { is_top: true, .. }
    ));

    let static_initializer = declaration(
        &db,
        "static_initializer",
        "static logic [7:0] static_initializer = written_helper(8'h5a);",
    );
    assert_has_ancestor(
        &db,
        static_initializer,
        top,
        "static local initializer is lexically in the module",
    );
    assert_eq!(
        db.variable_lifetime(static_initializer),
        VariableLifetime::Static,
        "static local initializer receiver has static storage"
    );
    assert_eq!(
        db.variable_lifetime_qualifier(static_initializer),
        VariableLifetimeQualifier::Static,
        "static local is explicitly qualified static"
    );
    let static_call = assert_call_initializer(&db, static_initializer, "written_helper");
    let NodeKind::FuncCall {
        callee: Some(static_callee),
        ..
    } = db.node_kind(static_call)
    else {
        unreachable!("static initializer call has a resolved callee")
    };
    assert_eq!(
        db.source_identity(*static_callee),
        db.source_identity(helper_function),
        "static initializer resolves to the helper whose local return was written"
    );
}
