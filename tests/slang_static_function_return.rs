//! Slang semantic coverage for implicit function results and hierarchical drivers.

use llg::core::compile::{self, CompileError, CompileOpts, OwnedSource};
use llg::ffi::slang::{
    self, CompileOptions, CompileRequest, DiagnosticSeverity, LanguageEdition, SemanticEdgeRole,
    SemanticKind, SemanticOperation, Source,
};

fn compile_frontend(name: &str, source: &str) -> slang::Snapshot {
    let sources = [Source::compilation_unit(name, source)];
    let options = CompileOptions {
        edition: LanguageEdition::SystemVerilog2009,
        top_modules: vec!["tb".into()],
        ..CompileOptions::default()
    };
    slang::compile(&CompileRequest {
        sources: &sources,
        library_sources: &[],
        options: &options,
    })
    .expect("Slang should return an owned snapshot")
}

fn compile_checked(name: &str, source: &str) -> Result<compile::CompileOut, CompileError> {
    compile::compile_sources_checked(
        &[OwnedSource::compilation_unit(name, source)],
        &CompileOpts {
            top: Some("tb".into()),
            edition: LanguageEdition::SystemVerilog2009,
            ..CompileOpts::default()
        },
    )
}

fn edges_for<'a>(
    snapshot: &'a slang::Snapshot,
    node: &slang::SemanticNode,
) -> &'a [slang::SemanticEdge] {
    let start = usize::try_from(node.edge_start).expect("semantic edge offset fits usize");
    let count = usize::try_from(node.edge_count).expect("semantic edge count fits usize");
    let end = start
        .checked_add(count)
        .expect("semantic edge range fits usize");
    &snapshot.semantic_edges[start..end]
}

fn assert_frontend_rejects(name: &str, source: &str, expected_diagnostic: &str) {
    let snapshot = compile_frontend(name, source);
    assert!(
        snapshot.has_errors(),
        "{name} unexpectedly compiled: {:?}",
        snapshot.diagnostics
    );
    assert!(
        snapshot.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.severity,
                DiagnosticSeverity::Error | DiagnosticSeverity::Fatal
            ) && diagnostic.name == expected_diagnostic
        }),
        "{name} did not report {expected_diagnostic}: {:?}",
        snapshot.diagnostics
    );

    assert!(
        matches!(
            compile_checked(name, source),
            Err(CompileError::FrontendDiagnostics(_))
        ),
        "checked compilation exposed an invalid snapshot for {name}"
    );
}

fn assert_frontend_warns(name: &str, source: &str, expected_diagnostic: &str) {
    let snapshot = compile_frontend(name, source);
    assert!(!snapshot.has_errors(), "{name}: {:?}", snapshot.diagnostics);
    assert!(
        snapshot.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Warning
                && diagnostic.name == expected_diagnostic
        }),
        "{name} did not warn with {expected_diagnostic}: {:?}",
        snapshot.diagnostics
    );
    assert!(
        compile_checked(name, source).is_ok(),
        "checked compilation should retain Slang's warning-only contract for {name}"
    );
}

#[test]
fn static_implicit_function_result_is_a_hierarchically_driven_static_variable() {
    let snapshot = compile_frontend(
        "static-function-result.sv",
        r#"
module tb;
    logic source;
    function static logic f;
    endfunction
    assign tb.f.f = source;
    initial begin
        source = 1'b1;
        #1 $display("result=%b", f());
    end
endmodule
"#,
    );
    assert!(!snapshot.has_errors(), "{:?}", snapshot.diagnostics);
    assert_eq!(snapshot.edition(), LanguageEdition::SystemVerilog2009);
    assert!(compile_checked(
        "static-function-result.sv",
        r#"
module tb;
    logic source;
    function static logic f;
    endfunction
    assign tb.f.f = source;
    initial begin
        source = 1'b1;
        #1 $display("result=%b", f());
    end
endmodule
"#
    )
    .is_ok());

    let function = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == SemanticKind::Subroutine && node.name == "f")
        .expect("function declaration is captured");
    let result = snapshot
        .semantic_nodes
        .iter()
        .find(|node| {
            node.kind == SemanticKind::Variable
                && node.name == "f"
                && node.parent_id == Some(function.id)
                && node.is_implicit
        })
        .expect("implicit function result variable is captured under its function");
    assert!(!result.is_automatic);
    assert_eq!(result.auxiliary, 1, "repository lifetime tag: static");
    assert!(edges_for(&snapshot, result).iter().any(|edge| {
        edge.role == SemanticEdgeRole::ReturnOwner && edge.target_id == function.id
    }));

    let assignment = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == SemanticKind::ContinuousAssign)
        .expect("hierarchical continuous assignment is captured");
    let body_id = edges_for(&snapshot, assignment)
        .iter()
        .find(|edge| edge.role == SemanticEdgeRole::Body)
        .expect("continuous assignment body edge")
        .target_id;
    let body = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.id == body_id)
        .expect("assignment body expression");
    assert_eq!(body.kind, SemanticKind::Expression);
    assert_eq!(body.operation, SemanticOperation::Assign);
    let lhs_id = edges_for(&snapshot, body)
        .iter()
        .find(|edge| edge.role == SemanticEdgeRole::Lhs)
        .expect("assignment LHS edge")
        .target_id;
    let lhs = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.id == lhs_id)
        .expect("hierarchical LHS expression");
    assert_eq!(lhs.target_id, Some(result.id));
}

#[test]
fn automatic_mixed_driver_and_function_scope_targets_are_rejected() {
    assert_frontend_rejects(
        "automatic-function-result.sv",
        r#"
module tb;
    logic source;
    function automatic logic f;
    endfunction
    assign tb.f.f = source;
endmodule
"#,
        "AutoVariableHierarchical",
    );

    assert_frontend_warns(
        "mixed-function-result-driver.sv",
        r#"
module tb;
    logic source;
    function static logic f;
        f = 1'b0;
    endfunction
    assign tb.f.f = source;
endmodule
"#,
        "MixedVarAssigns",
    );

    assert_frontend_rejects(
        "function-scope-is-not-result.sv",
        r#"
module tb;
    logic source;
    function static logic f;
    endfunction
    assign tb.f = source;
endmodule
"#,
        "MissingInvocationParens",
    );
}
