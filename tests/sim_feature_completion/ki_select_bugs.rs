use super::sim_cli;

const SUITE: &str = "feature_completion/ki_select_bugs";

#[test]
fn packed_array_element_members_read_and_write_one_element() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_select_bugs/element_members.out");
    sim_cli::run_case(SUITE, "element_members", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "element_members", expected, &[], &[]);
}

#[test]
fn neg_member_forms_stay_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_member_of_slice",
        "invalid member access for type 'pair_t[2:1]'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_member_of_vector_element",
        "invalid member access for type 'logic[7:0]'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_net_member_runtime_select",
        "reference to non-constant variable 'i' is not allowed in a constant expression",
    );
}

#[test]
fn tagged_union_element_members_are_an_explicit_boundary() {
    sim_cli::reject_case(
        SUITE,
        "unsupported_tagged_element_member",
        "tagged-union member `a` of a packed-array element is not supported",
    );
}

/// Capture keeps the element select as a member path's first reference, so
/// lowering selects the element instead of resolving the declaration name.
#[test]
fn packed_array_element_member_paths_keep_their_element_select() {
    use llg::core::compile;
    use llg::core::db::{Db, ExprKind, NodeKind};

    let source = "module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  typedef struct packed { pair_t [1:0] arr; logic [3:0] tag; } holder_t;
  pair_t [3:0] ps;
  pair_t [1:0][1:0] w;
  holder_t h;
  logic [7:0] v;
  integer i;
  initial v = {ps[i].hi, w[1][i].lo};
  initial v[3:0] = h.arr[i].hi;
endmodule
";
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit("members.sv", source)],
        &compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("member paths compile");
    let database = Db::from_slang(&compiled.snapshot).expect("owned database");
    database.validate().expect("database validates");
    drop(compiled);

    let mut paths = database
        .node_ids()
        .filter_map(|id| {
            let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = database.node_kind(id) else {
                return None;
            };
            let select = refs.first().copied().flatten()?;
            let indices = match database.node_kind(select) {
                NodeKind::Expr(ExprKind::BitSelect { .. }) => 1,
                NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => indices.len(),
                _ => return None,
            };
            Some((parts.join("."), indices))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(
        paths,
        [
            ("arr.hi".to_owned(), 1),
            ("ps.hi".to_owned(), 1),
            ("w.lo".to_owned(), 2),
        ]
    );
}

#[test]
fn packed_array_element_members_drive_ports_nets_and_wake_readers() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_select_bugs/element_member_views.out");
    sim_cli::run_case(SUITE, "element_member_views", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "element_member_views", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "element_member_views", expected);
}

#[test]
fn runtime_selected_member_targets_wait_on_their_selectors() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_select_bugs/selector_retarget.out");
    sim_cli::run_case(SUITE, "selector_retarget", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "selector_retarget", expected, &[], &[]);
}
