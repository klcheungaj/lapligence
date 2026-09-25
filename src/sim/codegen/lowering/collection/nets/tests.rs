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
