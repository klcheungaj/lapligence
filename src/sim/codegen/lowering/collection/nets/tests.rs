//! Hierarchical contribution identity survives native snapshot destruction.
use super::*;

#[test]
fn hierarchical_sources_keep_distinct_slots_per_instance_and_group() {
    let database = {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "wired_identity.sv",
                include_str!("../../../../../../tests/fixtures/sim/continuation_16_19/wired_driver_identity.sv"),
            )],
            &crate::core::compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
        ).unwrap();
        Db::from_slang(&output.snapshot).unwrap()
    };
    database.validate().unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    cg.collect_timescales();
    cg.build_net_groups().unwrap();
    let mut slots = HashSet::new();
    let mut counts = HashMap::new();
    for ((_, source, group), id) in &cg.structural_driver_sites {
        if matches!(database.node_kind(*source), NodeKind::ContAssign { .. }) {
            let record = &cg.structural_drivers[id.0 as usize];
            let (actual_group, slot) = cg.model.signals[record.signal].net_driver.unwrap();
            assert_eq!(*group, actual_group);
            assert!(slots.insert((actual_group, slot)), "continuous sites must not share a slot");
            *counts.entry(actual_group).or_insert(0usize) += 1;
        }
    }
    assert_eq!(counts.len(), 4, "two independent wired kinds in each of two instances");
    assert!(counts.values().all(|count| *count == 3));
    for group in counts.keys() {
        assert_eq!(cg.model.net_groups[*group].width, 65);
    }
}


#[test]
fn indexed_aliases_use_declaration_order_in_both_directions() {
    let database = {
        let result = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit("indexed_aliases.sv", r#"
module tb;
    wire [0:7] up;
    wire [7:0] down;
    wire [-4:3] negative;
    wire [2:0] u_minus, u_plus, d_minus, d_plus, n_minus;
    wire singleton;
    alias u_minus = up[5 -: 3];
    alias u_plus = up[3 +: 3];
    alias d_minus = down[5 -: 3];
    alias d_plus = down[3 +: 3];
    alias n_minus = negative[1 -: 3];
    alias singleton = negative[0 -: 1];
endmodule
"#)],
            &crate::core::compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
        ).unwrap();
        Db::from_slang(&result.snapshot).unwrap()
    };
    database.validate().unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    let mut count = 0;
    for node in database.node_ids() {
        let NodeKind::NetAlias { nets } = cg.kind(node) else { continue; };
        let left_bits = cg.alias_expression_bits(node, nets[0]).unwrap();
        let AliasBit::Net { net: left, .. } = left_bits[0] else {
            panic!("simple left alias endpoint");
        };
        let (root, physical): (&str, &[u32]) = match cg.node(left).name.as_str() {
            "u_minus" | "u_plus" => ("up", &[4, 3, 2]),
            "d_minus" | "d_plus" => ("down", &[5, 4, 3]),
            "n_minus" => ("negative", &[4, 3, 2]),
            "singleton" => ("negative", &[3]),
            other => panic!("unexpected alias endpoint {other}"),
        };
        let root = database.node_ids().find(|candidate| {
            cg.node(*candidate).name == root && matches!(cg.kind(*candidate), NodeKind::Net { .. })
        }).unwrap();
        let expected: Vec<_> = physical.iter().map(|bit| AliasBit::Net { net: root, bit: *bit }).collect();
        assert_eq!(cg.alias_expression_bits(node, nets[1]).unwrap(), expected);
        count += 1;
    }
    assert_eq!(count, 6);
}
