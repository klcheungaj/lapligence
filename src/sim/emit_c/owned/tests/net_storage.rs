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
        "#define LLG_MODEL_VALUE_ABI 4",
        "#define LLG_MODEL_VALUE_ABI 4\n#define LLG_MODEL_NO_MAIN 1",
        1,
    );
    source.push_str(
        r#"
int main(void) {
    for (int cycle = 0; cycle < 8; ++cycle) {
        if (llg_model_start(0, NULL)) return 1;
        if (g_pull.resolved.width != 129 || !g_pull.resolved.is_signed ||
            g_pull.resolved.bits[0] != UINT64_MAX ||
            g_pull.resolved.bits[1] != UINT64_MAX || g_pull.resolved.bits[2] != 1) return 2;
        if (g_bit_0.n_aliases != 2 || g_bit_0.index_root != -1 ||
            g_bit_0.driver_index[0].active || g_bit_0.driver_index[1].active) return 3;
        if (g_bit_0.drivers[0] != &g_bit_0__cells[0] ||
            g_bit_0.drivers[1] != &g_bit_0__cells[1] ||
            g_bit_0__cells[0].z[0] != 1 || g_bit_0__cells[1].z[0] != 1) return 4;
        sv4_t one = sv4_from_u64(1, 1, 0);
        llg_net_write(&g_bit_0, 1, one);
        sv4_destroy(&one);
        if (g_bit_0.resolved.bits[0] != 1 || !g_bit_0.driver_index[1].active ||
            G_array_0[2].bits[0] != 1 || llg_array_net_0_2.visible.bits[0] != 1) return 5;
        if (llg_model_close()) return 6;
        if (g_bit_0__cells[0].width || g_bit_0__cells[1].width ||
            g_bit_0.resolved.width || g_pull.resolved.width ||
            g_bit_0.aliases || g_bit_0.n_aliases || g_bit_0.alias_capacity ||
            llg_array_net_0_2.visible.width) return 7;
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
