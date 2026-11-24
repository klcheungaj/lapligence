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
    for process in &mut model.processes[4..8] {
        if let IrStmt::Assign { rhs, .. } = &mut process.body[1] {
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

#[test]
fn only_initial_roots_carry_the_blockable_descriptor_flag() {
    for kind in [IrProcessKind::Initial, IrProcessKind::Always] {
        let mut model = instance_model(8);
        model.vpi_catalog = false;
        for process in &mut model.processes {
            process.kind = kind;
        }
        let source =
            render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
        let blockable = source.contains("LLG_DESC_BLOCKABLE };");
        assert_eq!(
            blockable,
            kind == IrProcessKind::Initial,
            "{kind:?}: {source}"
        );
    }
    // Members that disagree keep their own descriptors, so a shared one can
    // never hand an always root the initial flag.
    let mut model = instance_model(8);
    model.vpi_catalog = false;
    for process in &mut model.processes[..4] {
        process.kind = IrProcessKind::Initial;
    }
    for process in &mut model.processes[4..] {
        process.kind = IrProcessKind::Always;
    }
    let source = render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
    assert!(!source.contains("llg_shared_body_0_desc"), "{source}");
    assert_eq!(
        source.matches("LLG_DESC_BLOCKABLE };").count(),
        4,
        "{source}"
    );
    assert_eq!(source.matches(", 2, 0 };").count(), 4, "{source}");
}

#[test]
fn catalog_models_mark_procedure_roots_as_vpi_threads() {
    // A model built with the VPI catalog marks initial and always roots as
    // threads (37.40) and keeps the blockable flag of the initial ones.
    let mut model = instance_model(8);
    model.vpi_catalog = true;
    for process in &mut model.processes[..4] {
        process.kind = IrProcessKind::Initial;
    }
    for process in &mut model.processes[4..] {
        process.kind = IrProcessKind::Always;
    }
    let source = render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
    assert_eq!(
        source
            .matches("LLG_DESC_BLOCKABLE | LLG_DESC_PROCEDURAL };")
            .count(),
        4,
        "{source}"
    );
    assert_eq!(
        source.matches(", 2, LLG_DESC_PROCEDURAL };").count(),
        4,
        "{source}"
    );
}

fn redrive_model(count: usize) -> IrModel {
    let mut model = instance_model(count);
    for index in 0..count {
        let source = model.signals.len();
        let mut signal = model.signals[index].clone();
        signal.c_name = format!("G_s{index}");
        model.signals.push(signal);
        model.redrives.push(crate::sim::ir::IrRedrive {
            label: format!("tb.u{index}.assign"),
            port_link: false,
            targets: vec![crate::sim::ir::IrRedriveTarget::Signal(index)],
            action: crate::sim::ir::IrRedriveAction::Evaluate(vec![IrStmt::Assign {
                lhs: IrLhs::Whole(index),
                rhs: IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::BitNeg,
                        a: Box::new(IrExpr::new(IrExprKind::SigRead(source), 8, false, None)),
                    },
                    8,
                    false,
                    None,
                ),
                nba: false,
            }]),
        });
    }
    model
}

#[test]
fn redrive_functions_of_one_module_share_one_body() {
    let execution = ExecutionModel::lower(redrive_model(8)).unwrap();
    let source = render_with_sharing_threshold(&execution, 4).unwrap();
    // One cold body for the eight drivers; each row passes its record.
    assert_eq!(
        source.matches("LLG_COLD void llg_shared_body_").count(),
        2,
        "a prototype and a definition: {source}"
    );
    assert!(!source.contains("llg_redrive_0("), "{source}");
    let table = |source: &str, name: &str| {
        let start = source
            .find(&format!("static const void* const {name}[8] = {{\n"))
            .unwrap_or_else(|| panic!("{name}: {source}"));
        let end = start + source[start..].find("};").unwrap();
        source[start..end]
            .lines()
            .skip(1)
            .flat_map(|line| line.split(','))
            .map(|item| item.trim().to_owned())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        table(&source, "llg_redrive_storage"),
        (0..8)
            .map(|index| format!("&G_v{index}"))
            .collect::<Vec<_>>()
    );
    let records = table(&source, "llg_redrive_operand");
    assert_eq!(records.len(), 8);
    assert!(
        records
            .iter()
            .all(|record| record.starts_with("&llg_body_instances_")),
        "{records:?}"
    );
    assert_eq!(
        records
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        8
    );
    // One class for the eight rows of one shape.
    assert!(source.contains("static const llg_redrive_class_t llg_redrive_classes[1] = {\n"));
    assert!(source.contains(
        "    { 0u, 1u, (uint32_t)sizeof(sv4_t), LLG_REDRIVE_EVALUATE, 0, llg_shared_body_"
    ));
    assert!(source.contains("static uint32_t llg_redrive_order[8];"));
    assert!(source.contains("static uint8_t llg_redrive_busy[8];"));
    assert!(source.contains("llg_redrives_register(&llg_redrives);"));
    // Below the threshold every driver keeps its own function and class.
    let own = render_with_sharing_threshold(&execution, 16).unwrap();
    assert_eq!(own.matches("static LLG_COLD void llg_redrive_").count(), 8);
    assert!(own.contains("static const llg_redrive_class_t llg_redrive_classes[8] = {\n"));
    assert!(own.contains(
        "    { 3u, 1u, (uint32_t)sizeof(sv4_t), LLG_REDRIVE_EVALUATE, 0, llg_redrive_3, NULL },\n"
    ));
    assert!(table(&own, "llg_redrive_operand")
        .iter()
        .all(|operand| operand == "NULL"));
    // A model without rows registers nothing.
    let plain = render_with_sharing_threshold(&instances(8), 4).unwrap();
    assert!(!plain.contains("llg_redrive"));
}

/// Eight evaluate-then-wait drivers of one source process, `G_v{i} = ~G_s{i}`.
fn driver_model(count: usize, catalog: bool) -> IrModel {
    let mut model = IrModel::new("drivers".to_owned(), 1).unwrap();
    model.vpi_catalog = catalog;
    let packed = |name: String| {
        IrSignal::new(
            name,
            None,
            IrType::Packed {
                width: 8,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap()
    };
    for index in 0..count {
        model.signals.push(packed(format!("G_v{index}")));
    }
    for index in 0..count {
        model.signals.push(packed(format!("G_s{index}")));
    }
    for index in 0..count {
        let source = count + index;
        let mut process = IrProcess::new(
            format!("p_v{index}"),
            format!("tb.u{index}.assign"),
            IrShape::SensLoop {
                reads: vec![IrDependency::scalar(format!("G_s{index}"))],
            },
            Vec::new(),
            vec![IrStmt::Assign {
                lhs: IrLhs::Whole(index),
                rhs: IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::BitNeg,
                        a: Box::new(IrExpr::new(IrExprKind::SigRead(source), 8, false, None)),
                    },
                    8,
                    false,
                    None,
                ),
                nba: false,
            }],
        );
        process.origin = crate::sim::semantic::Origin::Synthetic {
            reason: "one source process".to_owned(),
        };
        model.spawns.push(process.c_name.clone());
        model.processes.push(process);
    }
    model
}

#[test]
fn a_catalog_model_names_the_driver_process_of_each_variable() {
    let execution = ExecutionModel::lower(driver_model(8, true)).unwrap();
    let source = render_with_sharing_threshold(&execution, 4).unwrap();
    // No function: one class of process rows with the shared descriptor,
    // each row with the instance record its process holds.
    assert!(!source.contains("LLG_COLD"), "{source}");
    assert!(source.contains("static const llg_redrive_class_t llg_redrive_classes[1] = {\n"));
    assert!(
        source.contains(
            "    { 0u, 1u, (uint32_t)sizeof(sv4_t), LLG_REDRIVE_PROCESS, \
             (uint32_t)offsetof(llg_body_frame_0_t, _llg_instance), NULL, \
             &llg_shared_body_0_desc },\n"
        ),
        "{source}"
    );
    for index in 0..8 {
        assert!(source.contains(&format!("&G_v{index}")));
        assert!(source.contains(&format!("&llg_body_instances_0[{index}]")));
    }
    assert!(source.contains("llg_redrives_register(&llg_redrives);"));
    // Unshared, each process has its own descriptor and no record.
    let own = render_with_sharing_threshold(&execution, 16).unwrap();
    assert!(own.contains(
        "    { 3u, 1u, (uint32_t)sizeof(sv4_t), LLG_REDRIVE_PROCESS, 0, NULL, &p_v3_desc },\n"
    ));
    // Without the catalog no VPI application can release anything.
    let plain = ExecutionModel::lower(driver_model(8, false)).unwrap();
    let plain = render_with_sharing_threshold(&plain, 4).unwrap();
    assert!(!plain.contains("llg_redrive"), "{plain}");
}
