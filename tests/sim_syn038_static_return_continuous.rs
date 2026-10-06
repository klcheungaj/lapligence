//! SYN-038 hierarchical continuous writes to a static function result.

use crate::sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, ExprKind, NodeId, NodeKind, VariableLifetime},
};
use std::collections::HashSet;
use std::{
    process::{Command, Output},
    time::Duration,
};

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn038_pairwise/static_return_continuous.sv")
}

fn invoke(optimized: bool, define: Option<&str>) -> Output {
    let source = fixture_path();
    let directory = sim_harness::TempDir::new("syn038-static-return")
        .expect("create isolated CLI working directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command.current_dir(directory.path()).args(["--top", "tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    command.args(["--edition", "sv2009"]);
    if let Some(define) = define {
        command.args(["--define", define]);
    }
    command.arg(source);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
        .expect("public llg CLI invocation");
    drop(directory);
    output
}

fn assert_exact_cli(
    define: Option<&str>,
    expected_status: i32,
    expected_stdout: &str,
    expected_stderr: &str,
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke(optimized, define);
        let label = format!("static_return_continuous, optimized={optimized}");
        assert_eq!(output.status.code(), Some(expected_status), "{label}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected_stdout,
            "{label}"
        );
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr),
            expected_stderr,
            "{label}"
        );
    }
}

fn invoke_call_route(optimized: bool) -> Output {
    let source = fixture_path();
    let directory = sim_harness::TempDir::new("syn038-static-return-call")
        .expect("create isolated CLI working directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .current_dir(directory.path())
        .args(["--top", "call_tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    command
        .args(["--edition", "sv2009", "--define", "SYN038_CALL_ROUTE"])
        .arg(source);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
        .expect("public llg CLI invocation for continuous function-call source");
    drop(directory);
    output
}

fn subtree_calls_function(db: &Db, root: NodeId, function: NodeId) -> bool {
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(node) = pending.pop() {
        if !visited.insert(node) {
            continue;
        }
        if matches!(
            db.node_kind(node),
            NodeKind::FuncCall {
                name,
                is_task: false,
                callee: Some(callee),
                ..
            } if name == "f" && *callee == function
        ) {
            return true;
        }
        pending.extend(db.node(node).children().iter().copied());
    }
    false
}

#[test]
fn static_function_result_accepts_hierarchical_continuous_variable_assignment() {
    let source = fixture_path();
    let expected_stderr = format!(
        "Warning: {}:10:27 non-void function 'f' does not return a value\n",
        sim_harness::source_display(&source)
    );
    assert_exact_cli(None, 0, "result=1\n", &expected_stderr);
}

#[test]
fn continuous_assignment_call_reads_the_same_driven_static_result() {
    let source = fixture_path();
    let expected_stderr = format!(
        "Warning: {}:4:8 module definition is unused\n\
Warning: {}:43:27 non-void function 'f' does not return a value\n",
        sim_harness::source_display(&source),
        sim_harness::source_display(&source)
    );
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_call_route(optimized);
        let label = format!("static_return_continuous call route, optimized={optimized}");
        assert_eq!(output.status.code(), Some(0), "{label}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "result=1 call=1\n",
            "{label}"
        );
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr),
            expected_stderr,
            "{label}"
        );
    }
}

#[test]
fn owned_hierarchical_result_target_preserves_implicit_variable_identity() {
    let source = fixture_path();
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal static function return hierarchy compiles");
    let db = Db::from_slang(&compiled.snapshot).expect("capture owned semantic database");

    let function = db
        .node_ids()
        .find(|id| {
            db.node(*id).name == "f"
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: false, .. })
        })
        .expect("function declaration is captured");
    let result = db
        .node(function)
        .children()
        .iter()
        .copied()
        .find(|child| {
            db.node(*child).name == "f" && matches!(db.node_kind(*child), NodeKind::Var { .. })
        })
        .expect("implicit result variable is a function child");
    assert_eq!(db.variable_lifetime(result), VariableLifetime::Static);
    assert_eq!(db.node(result).parent(), Some(function));
    let assignment = db
        .node_ids()
        .find(|id| matches!(db.node_kind(*id), NodeKind::ContAssign { .. }))
        .expect("continuous assignment is captured");
    let lhs = db
        .node(assignment)
        .children()
        .first()
        .copied()
        .expect("continuous assignment has an LHS");
    assert_eq!(db.semantic_detail(lhs), Some("HierarchicalValue"));
    let NodeKind::Expr(ExprKind::Ref { target }) = db.node_kind(lhs) else {
        panic!(
            "continuous-assignment LHS should remain a reference: {:?}",
            db.node_kind(lhs)
        );
    };
    assert_eq!(*target, Some(result));
}

#[test]
fn continuous_call_and_hierarchical_driver_share_the_same_result_identity() {
    let source = fixture_path();
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("call_tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        defines: vec!["SYN038_CALL_ROUTE".into()],
        ..Default::default()
    })
    .expect("legal continuous function call compiles");
    let db = Db::from_slang(&compiled.snapshot).expect("capture owned semantic database");

    let function = db
        .node_ids()
        .find(|id| {
            db.node(*id).name == "f"
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: false, .. })
        })
        .expect("function declaration is captured");
    let result = db
        .node(function)
        .children()
        .iter()
        .copied()
        .find(|child| {
            db.node(*child).name == "f" && matches!(db.node_kind(*child), NodeKind::Var { .. })
        })
        .expect("implicit result variable is a function child");
    assert_eq!(db.variable_lifetime(result), VariableLifetime::Static);

    let driver = db
        .node_ids()
        .find(|id| {
            matches!(db.node_kind(*id), NodeKind::ContAssign { .. })
                && db.node(*id).children().first().is_some_and(|lhs| {
                    matches!(
                        db.node_kind(*lhs),
                        NodeKind::Expr(ExprKind::Ref { target: Some(target) })
                            if *target == result
                    )
                })
        })
        .expect("continuous driver targets the function's result slot");
    let driver_lhs = db.node(driver).children()[0];
    assert_eq!(db.semantic_detail(driver_lhs), Some("HierarchicalValue"));

    let call_result = db
        .node_ids()
        .find(|id| {
            db.node(*id).name == "call_result" && matches!(db.node_kind(*id), NodeKind::Var { .. })
        })
        .expect("continuous invocation destination is captured");
    let call_assignment = db
        .node_ids()
        .find(|id| {
            matches!(db.node_kind(*id), NodeKind::ContAssign { .. })
                && db.node(*id).children().first().is_some_and(|lhs| {
                    matches!(
                        db.node_kind(*lhs),
                        NodeKind::Expr(ExprKind::Ref { target: Some(target) })
                            if *target == call_result
                    )
                })
        })
        .expect("a continuous assignment consumes the function call");
    let rhs = *db
        .node(call_assignment)
        .children()
        .get(1)
        .expect("continuous invocation assignment has an RHS");
    assert!(
        subtree_calls_function(&db, rhs, function),
        "the continuous RHS call is bound to the function owning the driven result slot"
    );
}

#[test]
fn package_task_static_local_keeps_its_owned_variable_identity() {
    let package_source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/function/package_runtime_state.sv");
    let package_compiled = compile::compile_checked(&CompileOpts {
        files: vec![package_source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("package fixture compiles");
    let package_db = Db::from_slang(&package_compiled.snapshot).expect("package db");
    let task = package_db
        .node_ids()
        .find(|id| {
            package_db.node(*id).name == "bump"
                && matches!(
                    package_db.node_kind(*id),
                    NodeKind::FuncTask { is_task: true, .. }
                )
        })
        .expect("package task is captured");
    let local = package_db
        .node_ids()
        .find(|id| {
            package_db.node(*id).name == "local_count"
                && matches!(package_db.node_kind(*id), NodeKind::Var { .. })
        })
        .expect("package task local is captured");
    assert_eq!(
        package_db.variable_lifetime(local),
        VariableLifetime::Static
    );
    assert_ne!(package_db.node(local).parent(), Some(task));
    let references = package_db
        .node_ids()
        .filter(|id| {
            package_db.node(*id).name == "local_count"
                && matches!(
                    package_db.node_kind(*id),
                    NodeKind::Expr(ExprKind::Ref { .. })
                )
        })
        .collect::<Vec<_>>();
    assert!(
        !references.is_empty(),
        "task local has read/write references"
    );
    for reference in references {
        let NodeKind::Expr(ExprKind::Ref { target }) = package_db.node_kind(reference) else {
            unreachable!("filtered to reference expressions");
        };
        assert_eq!(*target, Some(local));
    }
}

#[test]
fn static_function_result_rejects_duplicate_continuous_drivers() {
    let source = fixture_path();
    let source = sim_harness::source_display(&source);
    let expected_stderr = format!(
        "Warning: {source}:10:27 non-void function 'f' does not return a value\n\
Warning: {source}:18:12 cannot have multiple continuous assignments to variable 'f'\n\
llg: codegen error: semantic error: multiple continuous assignments to variable storage `tb.f.f` at {source}:16:12 (also written by `tb.continuous` at {source}:18:12)\n"
    );
    assert_exact_cli(Some("SYN038_DUPLICATE_DRIVER"), 1, "", &expected_stderr);
}

#[test]
fn static_function_result_rejects_warning_only_mixed_driver() {
    let source = fixture_path();
    let source = sim_harness::source_display(&source);
    let expected_stderr = format!(
        "Warning: {source}:16:12 cannot mix continuous and procedural assignments to variable 'f'\n\
Warning: {source}:27:13 cannot mix continuous and procedural assignments to variable 'f'\n\
llg: codegen error: semantic error: static function result `f` has both a continuous driver at {source}:16:12 and a procedural write in its body at {source}:10:27\n"
    );
    assert_exact_cli(Some("SYN038_MIXED_DRIVER"), 1, "", &expected_stderr);
}
