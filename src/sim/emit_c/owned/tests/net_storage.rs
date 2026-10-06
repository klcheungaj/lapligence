use super::*;
use crate::sim::emit_c::model::render_with_sharing_threshold;

fn render(model: IrModel) -> String {
    let execution = ExecutionModel::lower(model).unwrap();
    render_with_sharing_threshold(&execution, usize::MAX).unwrap()
}

fn lifecycle(source: &str, name: &str) -> String {
    source
        .split_once(&format!("static void {name}(void) {{\n"))
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0
        .to_owned()
}

fn net_model(nets: usize, drivers: usize) -> IrModel {
    let mut model = IrModel::new("net_storage".to_owned(), 1).unwrap();
    for index in 0..nets {
        model.net_groups.push(
            IrNetGroup::new(format!("g_net_{index}"), 65, true, IrNetKind::Wire, drivers).unwrap(),
        );
    }
    model
}

#[test]
fn driver_cells_use_one_array_and_constant_lifecycle_bodies() {
    let small = render(net_model(1, 1));
    let large = render(net_model(1, 10000));
    assert!(large.contains("sv4_t g_net_0__cells[10000] = {0};"));
    assert_eq!(large.matches("sv4_t g_net_0__cells[").count(), 1);
    assert!(large.contains("&g_net_0__cells[0], &g_net_0__cells[1]"));
    assert!(large.contains("&g_net_0__cells[9999]"));
    assert!(!large.contains("g_net_0_d0"));
    for name in ["llg_model_storage_defaults", "llg_model_storage_destroy"] {
        let body = lifecycle(&large, name);
        assert_eq!(body, lifecycle(&small, name));
        assert!(body.contains("for (int slot = 0; slot < net->n_drivers; ++slot)"));
        assert!(!body.contains("__cells["));
    }
    assert!(large.contains("sv4_destroy(net->drivers[slot]);"));
}

#[test]
fn heterogeneous_nets_use_descriptor_rows() {
    let mut model = net_model(0, 0);
    for (index, kind) in [
        IrNetKind::Wire,
        IrNetKind::Wand,
        IrNetKind::Wor,
        IrNetKind::Tri0,
        IrNetKind::Tri1,
        IrNetKind::Supply0,
        IrNetKind::Supply1,
    ]
    .into_iter()
    .enumerate()
    {
        model.net_groups.push(
            IrNetGroup::new(
                format!("g_net_{index}"),
                (index * 32 + 1) as u32,
                index % 2 == 1,
                kind,
                index + 1,
            )
            .unwrap(),
        );
    }
    let source = render(model);
    for (index, fill) in [3, 3, 3, 0, 1, 0, 1].into_iter().enumerate() {
        assert!(source.contains(&format!("{{ &g_net_{index}, {fill} }}")));
    }
    let small = render(net_model(1, 2));
    let large = render(net_model(1024, 2));
    assert_eq!(large.matches(", 3 },\n").count(), 1024);
    for name in ["llg_model_storage_defaults", "llg_model_storage_destroy"] {
        assert_eq!(lifecycle(&small, name), lifecycle(&large, name));
    }
}

fn array_model(width: u32) -> IrModel {
    let mut model = net_model(0, 0);
    for (array_index, elements) in [vec![2, 0], vec![], vec![1]].into_iter().enumerate() {
        let mut array = IrArray::new(
            format!("G_array_{array_index}"),
            String::new(),
            width,
            false,
            vec![(2, 0)],
        )
        .unwrap();
        for element in elements {
            let signal_index = model.signals.len();
            let mut signal = IrSignal::new(
                format!("G_element_{array_index}_{element}"),
                None,
                IrType::Packed {
                    width,
                    signed: false,
                    two_state: false,
                },
                None,
            )
            .unwrap();
            for bit in 0..width {
                let group = model.net_groups.len();
                model.net_groups.push(
                    IrNetGroup::new(format!("g_bit_{group}"), 1, false, IrNetKind::Wire, 2)
                        .unwrap(),
                );
                signal.net_alias.push(IrNetAliasBinding {
                    group,
                    slot: 0,
                    signal_bit: bit,
                    group_bit: 0,
                });
            }
            model.signals.push(signal);
            array.net_elements.push((element, signal_index));
        }
        model.arrays.push(array);
        model.init_steps.push(IrInitStep::FillArrayZ(array_index));
    }
    model
}

#[test]
fn bit_net_arrays_use_rows_and_refresh_only_their_own_aliases() {
    let small = render(array_model(7));
    let large = render(array_model(65));
    assert!(large.contains("&llg_array_net_0_2, &llg_array_net_0_0, &llg_array_net_2_1,"));
    assert!(large.contains("for (size_t _llg_n = 0; _llg_n < 2; ++_llg_n)"));
    assert!(large.contains("for (size_t _llg_n = 2; _llg_n < 3; ++_llg_n)"));
    assert_eq!(
        large
            .matches("sv4_replace(alias->storage, llg_net_alias_read(alias));")
            .count(),
        2
    );
    for name in ["llg_model_storage_defaults", "llg_model_storage_destroy"] {
        let body = lifecycle(&large, name);
        assert_eq!(
            body.lines().count(),
            lifecycle(&small, name).lines().count()
        );
        assert!(!body.contains("g_bit_"));
        assert!(!body.contains("llg_array_net_0_2.visible"));
    }
}

#[test]
fn net_storage_restarts_with_empty_cells_indexes_and_alias_lists() {
    let mut model = array_model(7);
    model
        .net_groups
        .push(IrNetGroup::new("g_pull".to_owned(), 129, true, IrNetKind::Tri1, 1).unwrap());
    let mut source = render(model).replacen(
        "#define LLG_MODEL_VALUE_ABI 5",
        "#define LLG_MODEL_VALUE_ABI 5\n#define LLG_MODEL_NO_MAIN 1",
        1,
    );
    source.push_str(
        r#"
int main(void) {
    for (int cycle = 0; cycle < 8; ++cycle) {
        if (llg_model_start(0, NULL)) return 1;
        if (llg_sv4_width(g_pull.resolved) != 129 || !llg_sv4_signed(g_pull.resolved) ||
            llg_sv4_word(g_pull.resolved, 0, LLG_SV4_BITS) != UINT64_MAX ||
            llg_sv4_word(g_pull.resolved, 1, LLG_SV4_BITS) != UINT64_MAX || llg_sv4_word(g_pull.resolved, 2, LLG_SV4_BITS) != 1) return 2;
        if (g_bit_0.n_aliases != 2 || g_bit_0.index_root != -1 ||
            g_bit_0.driver_index[0].active || g_bit_0.driver_index[1].active) return 3;
        if (g_bit_0.drivers[0] != &g_bit_0__cells[0] ||
            g_bit_0.drivers[1] != &g_bit_0__cells[1] ||
            llg_sv4_word(g_bit_0__cells[0], 0, LLG_SV4_Z) != 1 || llg_sv4_word(g_bit_0__cells[1], 0, LLG_SV4_Z) != 1) return 4;
        sv4_t one = sv4_from_u64(1, 1, 0);
        llg_net_write(&g_bit_0, 1, one);
        sv4_destroy(&one);
        if (llg_sv4_word(g_bit_0.resolved, 0, LLG_SV4_BITS) != 1 || !g_bit_0.driver_index[1].active ||
            llg_sv4_word(G_array_0[2], 0, LLG_SV4_BITS) != 1 || llg_sv4_word(llg_array_net_0_2.visible, 0, LLG_SV4_BITS) != 1) return 5;
        if (llg_model_close()) return 6;
        if (llg_sv4_width(g_bit_0__cells[0]) || llg_sv4_width(g_bit_0__cells[1]) ||
            llg_sv4_width(g_bit_0.resolved) || llg_sv4_width(g_pull.resolved) ||
            g_bit_0.aliases || g_bit_0.n_aliases || g_bit_0.alias_capacity ||
            llg_sv4_width(llg_array_net_0_2.visible)) return 7;
    }
    return 0;
}
"#,
    );
    let directory = toolchain::Directory::new("net-storage-restart");
    let binary =
        crate::sim::build::build_model_cmake(directory.path(), &[("model.c", &source)]).unwrap();
    let result = toolchain::execute(&binary);
    assert!(result.status.success(), "{result:?}");
    assert!(result.stdout.is_empty(), "{result:?}");
    assert!(result.stderr.is_empty(), "{result:?}");
}

fn contribution_model(count: usize, delayed: bool) -> IrModel {
    let mut model = IrModel::new("net_contributions".to_owned(), 129).unwrap();
    let mut body = vec![IrStmt::DeclLocal {
        name: "captured".to_owned(),
        width: 129,
        signed: false,
        two_state: false,
        init: Some(Box::new(number(1, 129))),
    }];
    for index in 0..count {
        let group = model.net_groups.len();
        let width = if index % 2 == 0 { 1 } else { 65 };
        model.net_groups.push(
            IrNetGroup::new(format!("g_part_{index}"), width, false, IrNetKind::Wire, 1).unwrap(),
        );
        let mut signal = IrSignal::new(
            format!("g_part_{index}.resolved"),
            None,
            IrType::Packed {
                width,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap();
        signal.net_driver = Some((group, 0));
        let target = model.signals.len();
        model.signals.push(signal);
        let rhs = IrExpr::new(
            IrExprKind::PartSel {
                base: Box::new(IrExpr::new(
                    IrExprKind::LocalRead("captured".to_owned()),
                    129,
                    false,
                    None,
                )),
                left: i64::from(width - 1),
                right: 0,
            },
            width,
            false,
            None,
        );
        body.push(if delayed {
            IrStmt::InertialAssign {
                lhs: IrLhs::Whole(target),
                rhs,
                delay: IrTransitionDelay {
                    rise: 2,
                    fall: 3,
                    turn_off: 4,
                },
            }
        } else {
            IrStmt::Assign {
                lhs: IrLhs::Whole(target),
                rhs,
                nba: false,
            }
        });
    }
    model.processes.push(IrProcess::new(
        "p_contribute".to_owned(),
        "tb.assign".to_owned(),
        IrShape::RunOnce,
        Vec::new(),
        body,
    ));
    model.spawns.push("p_contribute".to_owned());
    model
}

#[test]
fn many_electrical_contributions_have_constant_executable_work_shape() {
    for count in [4, 41, 100] {
        let source = render(contribution_model(count, false));
        assert!(source.contains("uint32_t cast_width; uint8_t cast_signed;"));
        assert_eq!(source.matches("].cast_width").count(), 1);
        assert_eq!(source.matches("].cast_signed").count(), 1);
        assert_eq!(
            source
                .matches("static const llg_net_drive_row_t llg_net_rows_")
                .count(),
            1
        );
        assert_eq!(source.matches("llg_net_write(").count(), 1);
        assert_eq!(source.matches("sv4_part_select_to(").count(), 1);
        assert_eq!(source.matches("for (; _llg_net_i_").count(), 1);
        for index in 0..count {
            let width = if index % 2 == 0 { 1 } else { 65 };
            assert!(source.contains(&format!(
                "{{ &g_part_{index}, 0, {}LL, 0LL, {width}, 0 }}",
                width - 1
            )));
        }
    }
}

#[test]
fn delayed_electrical_contributions_keep_one_inertial_handle_per_row() {
    let source = render(contribution_model(41, true));
    assert!(source.contains("uint32_t cast_width; uint8_t cast_signed;"));
    assert_eq!(source.matches("].cast_width").count(), 1);
    assert_eq!(source.matches("].cast_signed").count(), 1);
    assert_eq!(source.matches("llg_inertial_net(").count(), 1);
    assert!(source.contains("[41] = {0};"));
    assert!(source.contains("2ULL, 3ULL, 4ULL"));
    assert_eq!(source.matches("for (; _llg_net_i_").count(), 1);
}

#[test]
fn electrical_batch_casts_remain_explicit_for_heterogeneous_row_shapes() {
    for delayed in [false, true] {
        let mut model = contribution_model(4, delayed);
        for index in [1, 2] {
            model.net_groups[index].signed = true;
            model.signals[index].ty = IrType::Packed {
                width: model.net_groups[index].width,
                signed: true,
                two_state: false,
            };
        }
        let rhs = match model.processes[0].body.last_mut().unwrap() {
            IrStmt::Assign { rhs, .. } | IrStmt::InertialAssign { rhs, .. } => rhs,
            _ => unreachable!(),
        };
        if let IrExprKind::PartSel { left, .. } = &mut rhs.kind {
            *left = 63;
            rhs.width = 64;
        }
        let source = render(model);
        assert_eq!(source.matches("sv4_cast_to(").count(), 1);
        assert_eq!(source.matches("].cast_width").count(), 1);
        assert_eq!(source.matches("].cast_signed").count(), 1);
        assert!(source.contains("{ &g_part_0, 0, 0LL, 0LL, 1, 0 }"));
        assert!(source.contains("{ &g_part_1, 0, 64LL, 0LL, 65, 1 }"));
        assert!(source.contains("{ &g_part_2, 0, 0LL, 0LL, 1, 1 }"));
        assert!(source.contains("{ &g_part_3, 0, 63LL, 0LL, 65, 0 }"));
        assert_eq!(source.matches("sv4_part_select_to(").count(), 1);
    }
}

#[test]
fn declared_view_bindings_render_contiguous_runs() {
    let mut model = net_model(2, 2);
    let mut array =
        IrArray::new("G_view".to_owned(), String::new(), 16, false, vec![(0, 0)]).unwrap();
    let mut signal = IrSignal::new(
        "G_view_element".to_owned(),
        None,
        IrType::Packed {
            width: 16,
            signed: false,
            two_state: false,
        },
        None,
    )
    .unwrap();
    // Bits 0..10 map onto group 0 from bit 3, bits 10..12 onto group 1 slot 1
    // and bits 12..16 back onto group 0 with a gap; insertion order is
    // shuffled so runs come from the binding coordinates, not their order.
    let mut bindings = (0..10)
        .map(|bit| (0, 0, bit, bit + 3))
        .chain((10..12).map(|bit| (1, 1, bit, bit - 10)))
        .chain((12..16).map(|bit| (0, 0, bit, bit + 8)))
        .collect::<Vec<_>>();
    bindings.reverse();
    for (group, slot, signal_bit, group_bit) in bindings {
        signal.net_alias.push(IrNetAliasBinding {
            group,
            slot,
            signal_bit,
            group_bit,
        });
    }
    let signal_index = model.signals.len();
    model.signals.push(signal);
    array.net_elements.push((0, signal_index));
    model.arrays.push(array);
    model.init_steps.push(IrInitStep::FillArrayZ(0));
    let source = render(model);
    assert!(source.contains(
        "llg_array_net_0_0_parts[] = { { &g_net_0, 0, 0, 3, 10 }, { &g_net_0, 0, 12, 20, 4 }, { &g_net_1, 1, 10, 0, 2 } };"
    ));
    assert!(source.contains(".parts = llg_array_net_0_0_parts, .n_parts = 3,"));
}
