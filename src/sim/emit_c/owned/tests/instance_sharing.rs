use super::*;
use crate::sim::emit_c::model::render_with_sharing_threshold;

fn instance_model(count: usize) -> IrModel {
    let mut model = IrModel::new("sharing".to_owned(), 1).unwrap();
    for index in 0..count {
        model.signals.push(
            IrSignal::new(
                format!("G_v{index}"),
                None,
                IrType::Packed {
                    width: 8,
                    signed: false,
                    two_state: false,
                },
                None,
            )
            .unwrap(),
        );
        let mut process = IrProcess::new(
            format!("p_v{index}"),
            format!("tb.u{index}"),
            IrShape::RunOnce,
            Vec::new(),
            vec![
                IrStmt::Delay {
                    ticks: IrDelay::Constant(1),
                },
                IrStmt::Assign {
                    lhs: IrLhs::Whole(index),
                    rhs: number(7, 8),
                    nba: false,
                },
            ],
        );
        process.origin = crate::sim::semantic::Origin::Synthetic {
            reason: "one source process".to_owned(),
        };
        model.spawns.push(process.c_name.clone());
        model.processes.push(process);
    }
    model
}

fn instances(count: usize) -> ExecutionModel {
    ExecutionModel::lower(instance_model(count)).unwrap()
}

#[test]
fn one_body_has_one_record_per_instance_and_spawn_initializes_root() {
    let execution = instances(8);
    let source = render_with_sharing_threshold(&execution, 4).unwrap();
    // One record row per instance in one array, one class descriptor and
    // one spawn row for the run of members.
    assert!(source.contains("static const llg_body_record_0_t llg_body_instances_0[8] = {"));
    assert_eq!(source.matches("static const llg_co_desc_t").count(), 1);
    assert!(source.contains(
        "static const llg_co_desc_t llg_shared_body_0_desc = { llg_shared_body_0, NULL, "
    ));
    assert!(
        source.contains("    { 0, 0, 8 },\n") || source.contains("llg_model_spawn_class(0, 0, 8);")
    );
    assert_eq!(
        source
            .matches("static LLG_MODEL_SHARED llg_co_status_t llg_shared_body_0(")
            .count(),
        2
    );
    assert_eq!(source.matches("LLG_CO_DISPATCH_BEGIN").count(), 1);
    assert!(source.contains("const void* _llg_instance;"));
    assert!(source.contains("llg_spawn_instance_in_region"));
    for index in 0..8 {
        assert!(source.contains(&format!("{{ &G_v{index} }}")));
    }
    assert_eq!(
        source,
        render_with_sharing_threshold(&execution, 4).unwrap()
    );
}

#[test]
fn threshold_and_unlimited_leave_bodies_independent() {
    for threshold in [4, usize::MAX] {
        let source = render_with_sharing_threshold(&instances(3), threshold).unwrap();
        assert!(!source.contains("llg_shared_body"));
        assert_eq!(source.matches("LLG_CO_DISPATCH_BEGIN").count(), 3);
    }
}

#[test]
fn width_and_parameter_payload_differences_split_classes() {
    let mut model = instance_model(8);
    for index in 4..8 {
        model.signals[index].ty = IrType::Packed {
            width: 16,
            signed: false,
            two_state: false,
        };
        if let IrStmt::Assign { rhs, .. } = &mut model.processes[index].body[1] {
            *rhs = number(9, 16);
        }
    }
    let source = render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
    assert!(source.contains("llg_shared_body_0("));
    assert!(source.contains("llg_shared_body_1("));
    assert!(!source.contains("llg_shared_body_2("));
    let mut model = instance_model(8);
    for index in 4..8 {
        if let IrStmt::Assign { rhs, .. } = &mut model.processes[index].body[1] {
            *rhs = number(9, 8);
        }
    }
    let source = render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
    assert!(source.contains("llg_shared_body_1("));
}

#[test]
fn activation_and_disable_identities_are_record_operands() {
    let mut model = instance_model(4);
    for (index, process) in model.processes.iter_mut().enumerate() {
        let target = IrActivationTarget::new(100 + index as u32, 200 + index as u32);
        process.body = vec![IrStmt::ActivationScope {
            target,
            exit: format!("exit_{index}"),
            body: vec![
                IrStmt::Delay {
                    ticks: IrDelay::Constant(1),
                },
                IrStmt::DisableTarget { target },
            ],
        }];
    }
    let source = render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
    assert!(source.contains("llg_activation_enter(I->"), "{source}");
    assert!(source.contains("llg_disable_target(LLG_CO_OWNER(ch, llg_proc_t), I->"));
    for index in 0..4 {
        assert!(source.contains(&format!("{}u, {}u", 100 + index, 200 + index)));
    }
}

#[test]
fn generated_pooled_constants_vary_per_site_without_splitting_shared_bodies() {
    for width in [8, 65, 128] {
        let mut model = instance_model(8);
        for (index, process) in model.processes.iter_mut().enumerate() {
            model.signals[index].ty = IrType::packed(width, false).unwrap();
            process.label = format!("tb.u[{index}]");
            process.body[1] = IrStmt::Assign {
                lhs: IrLhs::Whole(index),
                rhs: add(number(index as u64, width), number(0, width), width),
                nba: false,
            };
        }
        let source =
            render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
        assert_eq!(source.matches("LLG_CO_DISPATCH_BEGIN").count(), 1);
        assert_eq!(source.matches("sv4_add_to(").count(), 1);
        if width > 64 {
            assert!(source.contains("const sv4_t* v"));
            assert_eq!(source.matches("static sv4_t llg_constant_").count(), 8);
        } else {
            assert!(!source.contains("static sv4_t llg_constant_"));
        }
    }
}

#[test]
fn module_paths_are_loaded_from_each_instances_record() {
    let mut model = instance_model(4);
    for (index, process) in model.processes.iter_mut().enumerate() {
        process.body.push(IrStmt::DisplayTyped {
            fmt: "%m".to_owned(),
            args: Vec::new(),
            scope: format!("tb.u{index}"),
            newline: true,
            default_radix: IrDisplayRadix::Decimal,
            descriptor: None,
            time_unit_fs: 1,
        });
    }
    let source = render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
    for index in 0..4 {
        assert!(source.contains(&format!("\"tb.u{index}\"")));
    }
    assert_eq!(source.matches("LLG_CO_DISPATCH_BEGIN").count(), 1);
    assert!(source.contains("const char* v"));
}

#[test]
fn shared_net_writers_keep_distinct_array_backed_contributions() {
    let mut model = instance_model(8);
    model
        .net_groups
        .push(IrNetGroup::new("g_net".to_owned(), 8, false, IrNetKind::Wire, 8).unwrap());
    for (slot, signal) in model.signals.iter_mut().enumerate() {
        signal.net_driver = Some((0, slot));
    }
    let source = render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
    assert_eq!(source.matches("LLG_CO_DISPATCH_BEGIN").count(), 1);
    assert!(source.contains("static sv4_t llg_net_cell_pool[8];"));
    assert!(source.contains("llg_net_write(&g_net, I->"), "{source}");
    for slot in 0..8 {
        assert!(source.contains(&format!("    {{ {slot} }},\n")), "{source}");
    }
}

#[test]
fn unchanged_arena_analysis_renders_without_cloning_the_execution_model() {
    for count in [32, 512] {
        let execution = instances(count);
        let before = super::super::super::model::PREPARE_MODEL_CLONES.with(std::cell::Cell::get);
        let source = render_with_sharing_threshold(&execution, 4).unwrap();
        let clones =
            super::super::super::model::PREPARE_MODEL_CLONES.with(std::cell::Cell::get) - before;
        assert_eq!(clones, 0);
        assert!(source.contains("llg_shared_body_0"));
        execution.validate().unwrap();
    }
}

/// Port links that read selected bits of one bus, as a netlist's
/// `.a(bus[i])` connections do; `port_link` marks them as links.
fn bus_link_model(count: usize, port_link: bool) -> IrModel {
    let mut model = instance_model(count);
    for signal in &mut model.signals {
        signal.ty = IrType::packed(1, false).unwrap();
    }
    let bus = model.signals.len();
    model.signals.push(
        IrSignal::new(
            "G_bus".to_owned(),
            None,
            IrType::packed(count as u32, false).unwrap(),
            None,
        )
        .unwrap(),
    );
    for (index, process) in std::mem::take(&mut model.processes).into_iter().enumerate() {
        let read = IrExpr::new(
            IrExprKind::BitSel {
                base: Box::new(IrExpr::new(
                    IrExprKind::SigRead(bus),
                    count as u32,
                    false,
                    None,
                )),
                idx: Box::new(number(index as u64, 32)),
            },
            1,
            false,
            None,
        );
        let mut link = IrProcess::new_with_origin(
            process.c_name.clone(),
            format!("tb.u{index}.link"),
            IrShape::SensLoop {
                reads: vec![IrDependency::PackedRange {
                    storage: Box::new(IrDependency::scalar("G_bus")),
                    lsb: index as u32,
                    width: 1,
                }],
            },
            Vec::new(),
            vec![IrStmt::Assign {
                lhs: IrLhs::Whole(index),
                rhs: read,
                nba: false,
            }],
            process.origin.clone(),
        );
        if port_link {
            link = link.into_port_link();
        }
        model.processes.push(link);
    }
    model
}

#[test]
fn port_link_bit_selections_are_record_operands_of_one_body() {
    let source =
        render_with_sharing_threshold(&ExecutionModel::lower(bus_link_model(8, true)).unwrap(), 4)
            .unwrap();
    assert_eq!(
        source.matches("LLG_CO_DISPATCH_BEGIN").count(),
        1,
        "{source}"
    );
    assert!(source.contains(".lsb = I->"), "{source}");
    for index in 0..8 {
        assert!(source.contains(&format!("{index}ULL")), "{source}");
    }
    // Ordinary bodies keep their literal payloads structural.
    let source =
        render_with_sharing_threshold(&ExecutionModel::lower(bus_link_model(8, false)).unwrap(), 4)
            .unwrap();
    assert_eq!(source.matches("LLG_CO_DISPATCH_BEGIN").count(), 8);
    assert!(!source.contains("llg_shared_body"));
}
