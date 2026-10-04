use super::*;
use crate::sim::ir::IrNetKind;

fn database_for(source: &str) -> Db {
    let compiled = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            "net_cells.sv",
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
fn undriven_cells_become_typed_runs_and_peers_share_one_type_point() {
    let database = database_for(
        "module child(inout tri1 [3:0] c [0:63], input wire [3:0] d);\n\
         assign c[5] = d;\n\
         endmodule\n\
         module tb;\n\
         wire [3:0] w [0:63];\n\
         tri0 [1:0] t [0:9];\n\
         reg [3:0] d;\n\
         child u(.c(w), .d(d));\n\
         assign t[3] = 2'b01;\n\
         endmodule\n",
    );
    database.validate().unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut codegen = Codegen::new(&semantic);
    codegen.collect_design().unwrap();
    let nodes = codegen.design_nodes();
    let plan = codegen.plan_net_cells(&nodes).unwrap();
    // The 63 undriven peer pairs share one shape and the nine undriven tri0
    // cells another: only one pair and one cell enter the type plan.
    assert_eq!(plan.representative_points().len(), 3);
    codegen.build_net_groups().unwrap();
    let runs = |name: &str| {
        let array = codegen
            .model
            .arrays
            .iter()
            .find(|array| match name {
                "formal" => !array.c_name.starts_with("G_tb_"),
                name => array.c_name == name,
            })
            .unwrap();
        (
            array
                .net_elements
                .iter()
                .map(|(cell, _)| *cell)
                .collect::<Vec<_>>(),
            array
                .net
                .as_ref()
                .unwrap()
                .constant_cells
                .iter()
                .map(|run| (run.first, run.count, run.kind))
                .collect::<Vec<_>>(),
        )
    };
    let tri1 = IrNetKind::Tri1;
    let expected_peer = (vec![5], vec![(0, 5, tri1), (6, 58, tri1)]);
    assert_eq!(runs("G_tb_w"), expected_peer);
    assert_eq!(runs("formal"), expected_peer);
    let tri0 = IrNetKind::Tri0;
    assert_eq!(runs("G_tb_t"), (vec![3], vec![(0, 3, tri0), (4, 6, tri0)]));
    codegen.model.validate().unwrap();
}
