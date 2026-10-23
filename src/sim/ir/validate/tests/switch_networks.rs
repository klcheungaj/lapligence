use super::*;
use crate::sim::ir::{
    IrNetGroup, IrNetKind, IrSwitch, IrSwitchControl, IrSwitchNetwork, IrSwitchNode,
};

fn model_with(network: IrSwitchNetwork) -> IrModel {
    let mut model = valid_model();
    model.net_groups = vec![
        IrNetGroup::new("g_net_0".into(), 2, false, IrNetKind::Wire, 1).unwrap(),
        IrNetGroup::new("g_net_1".into(), 1, false, IrNetKind::Wire, 1).unwrap(),
    ];
    model.switch_network = network;
    model
}

fn node(group: usize, bit: u32) -> IrSwitchNode {
    IrSwitchNode::new(group, bit)
}

#[test]
fn switch_networks_partition_their_nodes() {
    // Network 0 is g0[0] and g1[0] joined twice; network 1 is g0[1] alone.
    let two = IrSwitchNetwork::new(
        vec![node(0, 0), node(1, 0), node(0, 1)],
        vec![
            IrSwitch::new(0, 1, false, false),
            IrSwitch::new(1, 0, true, true),
        ],
        vec![0, 2],
    );
    assert_eq!(two.component_of(1), Some(0));
    assert_eq!(two.component_of(2), Some(1));
    assert_eq!(two.component_of(3), None);
    assert!(model_with(two).validate().is_ok());
    assert!(model_with(IrSwitchNetwork::default()).validate().is_ok());
}

#[test]
fn switch_networks_reject_malformed_tables() {
    let check = |nodes, switches, components| {
        model_with(IrSwitchNetwork::new(nodes, switches, components)).validate()
    };
    let plain = |a, b| IrSwitch::new(a, b, false, false);
    // A node bit outside its group and a missing group.
    assert!(check(vec![node(0, 0), node(1, 1)], vec![plain(0, 1)], vec![0]).is_err());
    assert!(check(vec![node(0, 0), node(2, 0)], vec![plain(0, 1)], vec![0]).is_err());
    // A node listed twice.
    assert!(check(vec![node(0, 0), node(0, 0)], vec![plain(0, 1)], vec![0]).is_err());
    // A switch from a node to itself, to a missing node, or across networks.
    assert!(check(vec![node(0, 0), node(1, 0)], vec![plain(0, 0)], vec![0]).is_err());
    assert!(check(vec![node(0, 0), node(1, 0)], vec![plain(0, 2)], vec![0]).is_err());
    assert!(check(vec![node(0, 0), node(1, 0)], vec![plain(0, 1)], vec![0, 1]).is_err());
    // Networks that do not start at node 0 or are out of order.
    assert!(check(vec![node(0, 0), node(1, 0)], vec![plain(0, 1)], vec![1]).is_err());
    assert!(check(
        vec![node(0, 0), node(1, 0), node(0, 1)],
        vec![plain(0, 1)],
        vec![0, 2, 1]
    )
    .is_err());
    // Switches without nodes.
    assert!(check(Vec::new(), vec![plain(0, 1)], Vec::new()).is_err());
}

#[test]
fn switch_controls_name_a_controlled_switch_and_a_signal_bit() {
    // Switch 0 is a tran, switch 1 a tranif; signal 0 is one bit wide.
    let check = |controls: Vec<IrSwitchControl>| {
        model_with(
            IrSwitchNetwork::new(
                vec![node(0, 0), node(1, 0)],
                vec![
                    IrSwitch::new(0, 1, false, false),
                    IrSwitch::new(0, 1, false, true),
                ],
                vec![0],
            )
            .with_controls(controls),
        )
        .validate()
    };
    assert!(check(vec![IrSwitchControl::new(1, 0, 0, true)]).is_ok());
    // An uncontrolled or missing switch, a switch with two controls, a
    // missing signal and a bit outside the signal.
    assert!(check(vec![IrSwitchControl::new(0, 0, 0, false)]).is_err());
    assert!(check(vec![IrSwitchControl::new(2, 0, 0, false)]).is_err());
    assert!(check(vec![
        IrSwitchControl::new(1, 0, 0, false),
        IrSwitchControl::new(1, 0, 0, true)
    ])
    .is_err());
    assert!(check(vec![IrSwitchControl::new(1, 1, 0, false)]).is_err());
    assert!(check(vec![IrSwitchControl::new(1, 0, 1, false)]).is_err());
}
