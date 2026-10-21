use super::*;
use crate::sim::ir::{
    IrModulePath, IrModulePathDestination, IrModulePathSet, IrNetGroup, IrNetKind, IrPathPolarity,
    IrProcess, IrShape, IrSignal, IrStreamDirection, IrTransitionDelay, IrType,
};

fn constant(value: u64, width: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![value],
            x: vec![0],
            z: vec![0],
            width,
            signed: false,
            real: None,
            fill: None,
        }),
        width,
        false,
        None,
    )
}

fn read(signal: usize, width: u32) -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(signal), width, false, None)
}

/// One net group of `width` bits per entry, each with one driver-slot signal
/// per driver, plus a 1-bit source variable as the last signal.
struct Fixture {
    model: IrModel,
    slots: Vec<Vec<usize>>,
    source: usize,
}

fn fixture(groups: &[(u32, usize, bool)]) -> Fixture {
    let mut model = IrModel::new("net_delay_layout".to_owned(), 1).unwrap();
    let mut slots = Vec::new();
    for (index, &(width, drivers, delayed)) in groups.iter().enumerate() {
        let mut group = IrNetGroup::new(
            format!("g_net_{index}"),
            width,
            false,
            IrNetKind::Wire,
            drivers,
        )
        .unwrap();
        if delayed {
            group.propagation_delay = Some(IrTransitionDelay {
                rise: 2,
                fall: 3,
                turn_off: 4,
            });
        }
        model.net_groups.push(group);
        let mut group_slots = Vec::new();
        for slot in 0..drivers {
            group_slots.push(model.signals.len());
            model.signals.push(
                IrSignal::new(
                    format!("g_net_{index}_d{slot}"),
                    None,
                    IrType::Packed {
                        width,
                        signed: false,
                        two_state: false,
                    },
                    Some((index, slot)),
                )
                .unwrap(),
            );
        }
        slots.push(group_slots);
    }
    let source = model.signals.len();
    model.signals.push(
        IrSignal::new(
            "G_tb_a".to_owned(),
            Some("tb.a".to_owned()),
            IrType::Packed {
                width: 1,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    Fixture {
        model,
        slots,
        source,
    }
}

fn drive(fixture: &mut Fixture, lhs: IrLhs, width: u32) {
    let name = format!("p_drive_{}", fixture.model.processes.len());
    let rhs = read(fixture.source, 1);
    let rhs = if width == 1 {
        rhs
    } else {
        IrExpr::new(IrExprKind::Resize { a: Box::new(rhs) }, width, false, None)
    };
    fixture.model.processes.push(IrProcess::new(
        name.clone(),
        "tb.assign".to_owned(),
        IrShape::RunOnce,
        Vec::new(),
        vec![IrStmt::Assign {
            lhs,
            rhs,
            nba: false,
        }],
    ));
    fixture.model.spawns.push(name);
}

fn layout(fixture: &mut Fixture, group: usize) -> (Vec<u32>, bool) {
    assign_net_delay_layouts(&mut fixture.model);
    let group = &fixture.model.net_groups[group];
    (
        group.propagation_segments().to_vec(),
        group.starts_unknown(),
    )
}

#[test]
fn whole_driver_keeps_one_vector_and_starts_unknown() {
    let mut fixture = fixture(&[(4, 1, true)]);
    let slot = fixture.slots[0][0];
    drive(&mut fixture, IrLhs::Whole(slot), 4);
    assert_eq!(layout(&mut fixture, 0), (Vec::new(), true));
}

#[test]
fn drivers_of_different_ranges_split_the_vector_at_their_edges() {
    // [3:2] and [1] driven separately, [0] undriven: parts [0], [1], [3:2].
    let mut fixture = fixture(&[(4, 2, true)]);
    let (first, second) = (fixture.slots[0][0], fixture.slots[0][1]);
    drive(&mut fixture, IrLhs::Part(first, 3, 2, false), 2);
    drive(
        &mut fixture,
        IrLhs::Bit(second, Box::new(constant(1, 32)), false),
        1,
    );
    assert_eq!(layout(&mut fixture, 0), (vec![1, 2], true));
}

#[test]
fn one_concatenation_target_is_one_part() {
    let mut fixture = fixture(&[(4, 1, true)]);
    let slot = fixture.slots[0][0];
    drive(
        &mut fixture,
        IrLhs::Stream {
            parts: vec![
                (IrLhs::Part(slot, 3, 2, false), 2),
                (IrLhs::Part(slot, 1, 0, false), 2),
            ],
            width: 4,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        },
        4,
    );
    assert_eq!(layout(&mut fixture, 0), (Vec::new(), true));
}

#[test]
fn a_runtime_bit_select_makes_every_bit_its_own_part() {
    let mut fixture = fixture(&[(3, 1, true)]);
    let (slot, source) = (fixture.slots[0][0], fixture.source);
    drive(
        &mut fixture,
        IrLhs::Bit(slot, Box::new(read(source, 1)), false),
        1,
    );
    assert_eq!(layout(&mut fixture, 0), (vec![1, 2], true));
}

#[test]
fn undriven_delayed_nets_and_undelayed_nets_keep_their_defaults() {
    let mut fixture = fixture(&[(2, 1, true), (2, 1, false)]);
    let slot = fixture.slots[1][0];
    drive(&mut fixture, IrLhs::Part(slot, 1, 1, false), 1);
    assert_eq!(layout(&mut fixture, 0), (Vec::new(), false));
    assert_eq!(layout(&mut fixture, 1), (Vec::new(), false));
}

#[test]
fn a_driven_path_destination_starts_unknown_without_parts() {
    let mut fixture = fixture(&[(2, 1, false)]);
    let (slot, source) = (fixture.slots[0][0], fixture.source);
    drive(&mut fixture, IrLhs::Part(slot, 0, 0, false), 1);
    fixture.model.module_paths.push(IrModulePathSet::new(
        "tb.u".to_owned(),
        vec![source],
        vec![IrModulePathDestination::new(
            0,
            vec![IrModulePath::new(
                0,
                0,
                1,
                0,
                1,
                true,
                IrPathPolarity::Unknown,
                [1; 12],
                "t.sv:2:5".to_owned(),
            )],
        )],
    ));
    assert_eq!(layout(&mut fixture, 0), (Vec::new(), true));
}

#[test]
fn initialization_writes_drive_their_non_z_runs() {
    let mut fixture = fixture(&[(4, 1, true)]);
    fixture.model.init_steps.push(IrInitStep::WriteNet {
        group: 0,
        slot: 0,
        value: IrConst {
            bits: vec![0b0001],
            x: vec![0],
            z: vec![0b0110],
            width: 4,
            signed: false,
            real: None,
            fill: None,
        },
    });
    assert_eq!(layout(&mut fixture, 0), (vec![1, 3], true));
}

#[test]
fn undelayed_driven_bits_start_unknown_as_merged_runs() {
    // Decision T0-D3: [3:2] and [1] driven, [0] and [7:4] undriven, no delay.
    let mut fixture = fixture(&[(8, 2, false), (4, 1, false)]);
    let (first, second) = (fixture.slots[0][0], fixture.slots[0][1]);
    drive(&mut fixture, IrLhs::Part(first, 3, 2, false), 2);
    drive(
        &mut fixture,
        IrLhs::Bit(second, Box::new(constant(1, 32)), false),
        1,
    );
    assert_eq!(layout(&mut fixture, 0), (Vec::new(), false));
    assert_eq!(fixture.model.net_groups[0].driven_runs(), &[(1, 3)]);
    // An undriven group keeps its default.
    assert!(fixture.model.net_groups[1].driven_runs().is_empty());
}

#[test]
fn delayed_and_pull_groups_have_no_driven_runs() {
    let mut fixture = fixture(&[(4, 1, true), (4, 1, false)]);
    fixture.model.net_groups[1].kind = IrNetKind::Tri0;
    let (delayed, pulled) = (fixture.slots[0][0], fixture.slots[1][0]);
    drive(&mut fixture, IrLhs::Whole(delayed), 4);
    drive(&mut fixture, IrLhs::Whole(pulled), 4);
    assign_net_delay_layouts(&mut fixture.model);
    assert!(fixture.model.net_groups[0].starts_unknown());
    assert!(fixture.model.net_groups[0].driven_runs().is_empty());
    assert!(fixture.model.net_groups[1].driven_runs().is_empty());
}
