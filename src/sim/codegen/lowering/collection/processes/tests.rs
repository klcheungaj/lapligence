//! Continuous fixed arrays keep RHS dependencies and per-site contribution slots.
use super::*;

#[test]
fn continuous_array_graph_keeps_static_topology_and_rhs_only_dependencies() {
    let database = {
        let result = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "continuous_identity.sv",
                include_str!("../../../../../../tests/fixtures/sim/continuation_20_23/continuous_identity.sv"),
            )],
            &crate::core::compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
        ).unwrap();
        Db::from_slang(&result.snapshot).unwrap()
    };
    database.validate().unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    let tops = cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    cg.collect_timescales();
    cg.build_net_groups().unwrap();
    cg.validate_process_semantics().unwrap();
    for top in tops {
        cg.emit_pass(top, Pass::Comb).unwrap();
    }
    let array = |name: &str| {
        cg.array_globals.iter().find_map(|(node, array)| {
            (database.node(*node).name == name).then_some(cg.reference_array(array.ir))
        }).expect("named source array")
    };
    let left = IrDependency::ArrayContents(array("left"));
    let right = IrDependency::ArrayContents(array("right"));
    let choice = database.nodes().iter().enumerate().find_map(|(index, node)| {
        if node.name == "choice" {
            cg.signal_of(NodeId::from_index(index)).map(|signal| cg.signal_dependency(signal))
        } else { None }
    }).expect("selector signal");
    let expected: HashSet<_> = [left.clone(), right, choice].into_iter().collect();
    let mut constant = 0;
    let mut conditional = 0;
    for process in &cg.model.processes {
        match &process.shape {
            IrShape::RunOnce => constant += 1,
            IrShape::SensLoop { reads } if reads.len() == 3 => {
                assert_eq!(reads.iter().cloned().collect::<HashSet<_>>(), expected);
                conditional += 1;
            }
            IrShape::SensLoop { reads } => {
                assert_eq!(reads.len(), 1);
                assert!(expected.contains(&reads[0]), "no LHS/self-wake dependency");
            }
            _ => panic!("continuous driver is neither run-once nor source-sensitive"),
        }
    }
    assert_eq!(constant, 1);
    assert_eq!(conditional, 1);
    assert_eq!(cg.model.processes.len(), 5);
    let mut counts = HashMap::new();
    let mut slots = HashSet::new();
    for ((_, source, group), id) in &cg.structural_driver_sites {
        if matches!(database.node_kind(*source), NodeKind::ContAssign { .. }) {
            let driver = &cg.structural_drivers[id.0 as usize];
            let (actual_group, slot) = cg.model.signals[driver.signal].net_driver.unwrap();
            assert_eq!(*group, actual_group);
            assert!(slots.insert((actual_group, slot)), "independent continuous sources share storage");
            *counts.entry(actual_group).or_insert(0usize) += 1;
        }
    }
    assert_eq!(counts.len(), 2, "two fixed net elements");
    assert!(counts.values().all(|count| *count == 3));
    for group in counts.keys() {
        assert_eq!(cg.model.net_groups[*group].width, 65);
    }
}

#[test]
fn continuous_variable_conflicts_are_owned_errors_but_overrides_are_not() {
    for (name, source, conflict) in [
        ("mixed.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_20_23/continuous_mixed_writer_error.sv"), true),
        ("initialized.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_20_23/continuous_initialized_writer_error.sv"), true),
        ("force.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_20_23/continuous_force_control.sv"), false),
    ] {
        let database = {
            // The pinned frontend warns about these conflicts; the owned
            // simulator boundary must still refuse to execute the invalid mix.
            let result = crate::core::compile::compile_sources_checked(
                &[crate::core::compile::OwnedSource::compilation_unit(name, source)],
                &crate::core::compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
            ).unwrap();
            Db::from_slang(&result.snapshot).unwrap()
        };
        database.validate().unwrap();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        cg.collect_design().unwrap();
        cg.bind_reference_ports().unwrap();
        cg.collect_timescales();
        cg.build_net_groups().unwrap();
        let result = cg.validate_process_semantics();
        if conflict {
            assert!(result.unwrap_err().contains("has both a continuous assignment"));
        } else {
            result.expect("force/release and disjoint bit assignments are not mixed drivers");
        }
    }
}
