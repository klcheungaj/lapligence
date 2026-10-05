//! R02: owned foreach slots preserve expression-type bounds across import.

use llg::core::db::{NodeKind, StmtKind};
use llg::core::{compile, db};
use llg::ffi::slang::{SemanticEdgeRole, Snapshot};

fn capture(source: &str) -> Snapshot {
    compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(
            "foreach-owned.sv",
            source,
        )],
        &compile::CompileOpts {
            top: Some("tb".into()),
            ..Default::default()
        },
    )
    .expect("valid foreach source")
    .snapshot
}

const SOURCE: &str = r#"// llg-test-fixture: tests/slang_semantics/foreach.rs/owned-slots
module tb;
    logic [3:0] a [0:1];
    initial begin
        foreach (a[i,j]) begin end
        foreach (a[,k]) begin end
        foreach (a[l,]) begin end
        foreach (a[,]) begin end
        foreach (a[]) begin end
    end
endmodule
"#;

#[test]
fn foreach_mixed_owned_bounds_preserve_omitted_and_empty_slots() {
    let database = {
        let snapshot = capture(SOURCE);
        db::Db::from_slang(&snapshot).expect("owned foreach import")
    };
    database.validate().expect("owned database remains valid");
    let mut loops = database
        .nodes()
        .iter()
        .filter_map(|node| match &node.kind {
            NodeKind::Stmt(StmtKind::Foreach {
                vars, dimensions, ..
            }) => Some((
                node.line,
                vars.iter().map(Option::is_some).collect::<Vec<_>>(),
                dimensions.clone(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    loops.sort_by_key(|(line, _, _)| *line);
    let slots = loops
        .into_iter()
        .map(|(_, vars, dims)| (vars, dims))
        .collect::<Vec<_>>();
    let both = vec![Some((0, 1)), Some((3, 0))];
    assert_eq!(
        slots,
        [
            (vec![true, true], both.clone()),
            (vec![false, true], both.clone()),
            (vec![true, false], both.clone()),
            (vec![false, false], both),
            (vec![], vec![]),
        ]
    );
}

#[test]
fn foreach_mixed_owned_import_requires_the_iterated_expression_type() {
    let mut snapshot = capture(SOURCE);
    let database = db::Db::from_slang(&snapshot).expect("initial import");
    let index = database
        .nodes()
        .iter()
        .position(|node| matches!(node.kind, NodeKind::Stmt(StmtKind::Foreach { .. })))
        .expect("foreach statement");
    let statement = &snapshot.semantic_nodes[index];
    let start = usize::try_from(statement.edge_start()).unwrap();
    let end = start + usize::try_from(statement.edge_count()).unwrap();
    let base = snapshot.semantic_edges[start..end]
        .iter()
        .find(|edge| edge.role == SemanticEdgeRole::Base)
        .expect("iterated expression edge")
        .target_id;
    snapshot.semantic_nodes[usize::try_from(base).unwrap()].set_type_id(None);
    let error = db::Db::from_slang(&snapshot).expect_err("missing bounds source must fail closed");
    assert!(error
        .to_string()
        .contains("foreach array expression has no type"));
}

#[test]
fn foreach_mixed_owned_import_rejects_an_excessive_slot_count_before_allocation() {
    let mut snapshot = capture(SOURCE);
    let database = db::Db::from_slang(&snapshot).expect("initial import");
    let index = database
        .nodes()
        .iter()
        .position(|node| matches!(node.kind, NodeKind::Stmt(StmtKind::Foreach { .. })))
        .expect("foreach statement");
    snapshot.semantic_nodes[index].auxiliary = u64::MAX;
    let error =
        db::Db::from_slang(&snapshot).expect_err("invalid count must not allocate a slot vector");
    let detail = error.to_string();
    assert!(
        detail.contains("foreach") && (detail.contains("scalar") || detail.contains("too large"))
    );
}
