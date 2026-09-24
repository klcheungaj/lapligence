//! Provisional SYN-038 witnesses replacing function-result rows that attached
//! CP=function to a separate receiving target.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{AlwaysKind, Db, ExprKind, NodeId, NodeKind, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE_SOURCE: &str = include_str!("fixtures/sim/syn038_pairwise/legacy_cp_replacements.sv");
const EXPECTED_STDOUT: &str = "initial=12/34 array=12,34,56 conditional=21,43 net=5a function=5a direct=5a var=7d function=7d direct=7d comb=6b/6b\nupdated=net=a5 function=a5 direct=a5 var=c6 function=c6 direct=c6 comb=7c/7c\nff=11->22 sample=11\nff=22->33 sample=22\n";

#[derive(Clone, Copy, Debug)]
enum ProcessLane {
    AlwaysComb,
    AlwaysFF,
}

fn is_process_kind(kind: &ProcessKind, expected: ProcessLane) -> bool {
    matches!(
        (kind, expected),
        (
            ProcessKind::Always {
                always_type: AlwaysKind::Comb
            },
            ProcessLane::AlwaysComb
        ) | (
            ProcessKind::Always {
                always_type: AlwaysKind::FlipFlop
            },
            ProcessLane::AlwaysFF
        )
    )
}

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn038_pairwise/legacy_cp_replacements.sv");
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("the provisional SV2009 witness fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn ref_target(db: &Db, expression: NodeId) -> Option<NodeId> {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref { target }) => *target,
        _ => None,
    }
}

fn declaration(db: &Db, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Var { .. } | NodeKind::Net { .. }
                )
        })
        .unwrap_or_else(|| panic!("declaration `{name}` is captured"))
}

fn function(db: &Db, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: false, .. })
        })
        .unwrap_or_else(|| panic!("function `{name}` is captured"))
}

fn return_source_ref(db: &Db, function: NodeId, source_name: &str) -> NodeId {
    let NodeKind::FuncTask {
        body: Some(body), ..
    } = db.node_kind(function)
    else {
        panic!("function has a body")
    };
    let mut pending = vec![*body];
    while let Some(node) = pending.pop() {
        if let NodeKind::Stmt(StmtKind::Return { value: Some(value) }) = db.node_kind(node) {
            if db.node(*value).name() == source_name && ref_target(db, *value).is_some() {
                return *value;
            }
        }
        pending.extend(db.node(node).children().iter().copied());
    }
    panic!("function returns the bound source `{source_name}`");
}

fn has_explicit_write_to(db: &Db, root: NodeId, target: NodeId, blocking: bool) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if matches!(
            db.node_kind(node),
            NodeKind::Stmt(StmtKind::Assign { blocking: assignment_blocking, .. })
                if *assignment_blocking == blocking
        ) && db
            .node(node)
            .children()
            .first()
            .is_some_and(|lhs| ref_target(db, *lhs) == Some(target))
        {
            return true;
        }
        pending.extend(db.node(node).children().iter().copied());
    }
    false
}

fn ancestor_process(db: &Db, node: NodeId, expected: ProcessLane) -> bool {
    let mut parent = db.node(node).parent();
    while let Some(ancestor) = parent {
        if matches!(db.node_kind(ancestor), NodeKind::Process { kind } if is_process_kind(kind, expected))
        {
            return true;
        }
        parent = db.node(ancestor).parent();
    }
    false
}

#[test]
fn function_source_replacement_witnesses_match_in_both_optimizer_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/legacy_cp_replacements.sv\n"
    ));
    for anchor in [
        "function automatic pair_t copy_pair(input pair_t value);",
        "pair_result = copy_pair(pair_source);",
        "make_array_result[1] = 8'h34;",
        "array_result = make_array_result();",
        "return select_right ? right_value : left_value;",
        "conditional_left_result = select_function_value(1'b0, 8'h21, 8'h43);",
        "assign continuous_net_source = net_seed;",
        "return continuous_net_source;",
        "assign net_function_sample = read_continuous_net();",
        "assign continuous_variable_source = variable_seed;",
        "return continuous_variable_source;",
        "assign variable_function_sample = read_continuous_variable();",
        "comb_state = comb_input;",
        "return comb_state;",
        "comb_function_sample = read_comb_state();",
        "ff_state <= ff_next_state;",
        "return ff_state;",
        "ff_function_sample <= read_ff_state();",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "missing source anchor: {anchor}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "legacy_cp_replacements",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn function_returns_bind_to_the_exact_net_and_process_written_variables() {
    let db = compile_fixture();
    let array_function = function(&db, "make_array_result");
    let return_array_slot = db
        .node(array_function)
        .children()
        .iter()
        .copied()
        .find(|child| {
            db.node(*child).name() == "make_array_result"
                && matches!(db.node_kind(*child), NodeKind::Array { .. })
        })
        .expect("function owns its declared fixed-array return slot");
    assert_eq!(
        ref_target(
            &db,
            return_source_ref(&db, array_function, "make_array_result")
        ),
        Some(array_function),
        "return statement reads the function's own result identity"
    );
    let own_element_writes = db
        .node_ids()
        .filter(|id| {
            matches!(
                db.node_kind(*id),
                NodeKind::Expr(ExprKind::ArraySelect { base, .. })
                    if *base == return_array_slot
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        own_element_writes.len(),
        3,
        "each element write targets the fixed-array return slot itself"
    );

    for (source_name, function_name) in [
        ("continuous_net_source", "read_continuous_net"),
        ("continuous_variable_source", "read_continuous_variable"),
        ("comb_state", "read_comb_state"),
        ("ff_state", "read_ff_state"),
    ] {
        let source_slot = declaration(&db, source_name);
        let source_ref = return_source_ref(&db, function(&db, function_name), source_name);
        assert_eq!(
            ref_target(&db, source_ref),
            Some(source_slot),
            "{function_name} returns the exact `{source_name}` source slot"
        );
    }

    for (source_name, function_name, source_is_net) in [
        ("continuous_net_source", "read_continuous_net", true),
        (
            "continuous_variable_source",
            "read_continuous_variable",
            false,
        ),
    ] {
        let source_slot = declaration(&db, source_name);
        let continuous_driver = db
            .node_ids()
            .find(|id| {
                matches!(db.node_kind(*id), NodeKind::ContAssign { .. })
                    && db
                        .node(*id)
                        .children()
                        .first()
                        .is_some_and(|lhs| ref_target(&db, *lhs) == Some(source_slot))
            })
            .unwrap_or_else(|| panic!("continuous driver writes `{source_name}`"));
        assert_eq!(
            matches!(db.node_kind(source_slot), NodeKind::Net { .. }),
            source_is_net,
            "`{source_name}` has the requested declared net or variable kind"
        );
        assert!(db.node(continuous_driver).children().len() >= 2);
        assert_eq!(
            ref_target(
                &db,
                return_source_ref(&db, function(&db, function_name), source_name)
            ),
            Some(source_slot),
            "continuous driver and function read share one source declaration"
        );
    }

    for (source_name, expected_process, blocking) in [
        ("comb_state", ProcessLane::AlwaysComb, true),
        ("ff_state", ProcessLane::AlwaysFF, false),
    ] {
        let source_slot = declaration(&db, source_name);
        let process = db
            .node_ids()
            .find(|id| {
                matches!(db.node_kind(*id), NodeKind::Process { kind } if is_process_kind(kind, expected_process))
                    && has_explicit_write_to(&db, *id, source_slot, blocking)
            })
            .unwrap_or_else(|| panic!("a {expected_process:?} process writes `{source_name}`"));
        assert!(has_explicit_write_to(&db, process, source_slot, blocking));
        let read_function = match source_name {
            "comb_state" => "read_comb_state",
            "ff_state" => "read_ff_state",
            _ => unreachable!(),
        };
        let call = db
            .node_ids()
            .find(|id| {
                matches!(
                    db.node_kind(*id),
                    NodeKind::FuncCall { name, callee: Some(callee), is_task: false, .. }
                        if name == read_function && *callee == function(&db, read_function)
                ) && ancestor_process(&db, *id, expected_process)
            })
            .unwrap_or_else(|| panic!("{read_function} is called in the source-writing process"));
        assert!(ancestor_process(&db, call, expected_process));
    }
}
