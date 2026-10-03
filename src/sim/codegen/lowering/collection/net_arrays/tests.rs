use super::*;

#[test]
fn component_record_net_arrays_preserve_cell_and_recursive_member_offsets() {
    let database = {
        let compiled = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "record_net_arrays.sv",
                include_str!("../../../../../../tests/fixtures/sim/feature_completion/rtl_001/record_net_arrays.sv"),
            )],
            &crate::core::compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&compiled.snapshot).unwrap()
    };
    database.validate().unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut codegen = Codegen::new(&semantic);
    codegen.collect_design().unwrap();
    let mut observed = Vec::new();
    for node in codegen.design_nodes() {
        if !matches!(database.node_kind(node), NodeKind::ContAssign { .. }) {
            continue;
        }
        let target = database.node(node).children[0];
        let ((_, element), bits) = codegen.array_net_selection(target).unwrap().unwrap();
        assert!(bits.windows(2).all(|pair| pair[0] == pair[1] + 1));
        observed.push((element, bits.len(), *bits.last().unwrap(), bits[0]));
    }
    observed.sort_unstable();
    assert_eq!(
        observed,
        vec![
            (0, 4, 0, 3),
            (0, 8, 4, 11),
            (0, 8, 12, 19),
            (0, 8, 20, 27),
            (1, 4, 0, 3),
            (1, 8, 4, 11),
            (1, 8, 12, 19),
            (1, 8, 20, 27),
        ]
    );
    codegen.build_net_groups().unwrap();
    codegen.model.validate().unwrap();
}
