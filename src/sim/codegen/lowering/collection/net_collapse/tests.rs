use super::*;

const KINDS: [NetType; 9] = [
    NetType::Wire,
    NetType::Wand,
    NetType::Wor,
    NetType::TriReg,
    NetType::Tri0,
    NetType::Tri1,
    NetType::Uwire,
    NetType::Supply0,
    NetType::Supply1,
];

fn point(node: u32) -> NetPoint {
    NetPoint::Whole(NodeId(node))
}

fn add(plan: &mut NetCollapsePlan, node: u32, kind: NetType) -> NetPoint {
    let point = point(node);
    plan.insert(point, kind).unwrap();
    point
}

#[test]
fn port_net_type_table_matches_every_supplied_entry() {
    // Independent transcription: uppercase selects without a warning;
    // lowercase selects with a warning. Rows internal, columns external.
    let expected = [
        "EEEEEEEEE",
        "IEeeeeeEE",
        "IeEeeeeEE",
        "IeeEEEeEE",
        "IeeIEeeEE",
        "IeeIeEeEE",
        "IiiiiiEEE",
        "IIIIIIIEe",
        "IIIIIIIeE",
    ];
    for (row, internal) in KINDS.iter().enumerate() {
        for (column, external) in KINDS.iter().enumerate() {
            let actual = port_choice(*internal, *external).unwrap();
            let expected = match expected[row].as_bytes()[column] {
                b'E' => PortChoice::External,
                b'I' => PortChoice::Internal,
                b'e' => PortChoice::ExternalWarn,
                b'i' => PortChoice::InternalWarn,
                _ => unreachable!(),
            };
            assert_eq!(actual, expected, "{internal:?} / {external:?}");
        }
    }
}

#[test]
fn port_net_type_equivalent_spellings_keep_uwire_distinct() {
    for (alternate, canonical) in [
        (NetType::Tri, NetType::Wire),
        (NetType::Logic, NetType::Wire),
        (NetType::TriAnd, NetType::Wand),
        (NetType::TriOr, NetType::Wor),
    ] {
        assert_eq!(canonical_net_type(alternate), Some(canonical));
        for kind in KINDS {
            assert_eq!(port_choice(alternate, kind), port_choice(canonical, kind));
            assert_eq!(port_choice(kind, alternate), port_choice(kind, canonical));
        }
    }
    assert_ne!(
        canonical_net_type(NetType::Uwire),
        canonical_net_type(NetType::Wire)
    );
}

#[test]
fn port_net_type_rejects_nonnet_categories() {
    for bad in [NetType::None, NetType::Reg, NetType::Unsupported] {
        assert!(canonical_net_type(bad).is_none());
        assert!(port_choice(NetType::Wire, bad).is_none());
        assert!(port_choice(bad, NetType::Wire).is_none());
    }
}

#[test]
fn port_net_type_all_pairs_choose_the_correct_declaration_not_union_root() {
    for internal in KINDS {
        for external in KINDS {
            let mut plan = NetCollapsePlan::default();
            // Opposite insertion order/ID order cannot reverse the port roles.
            let child = add(&mut plan, 1, internal);
            let parent = add(&mut plan, 99, external);
            let choice = port_choice(internal, external).unwrap();
            let warning = plan.port(child, parent).unwrap();
            let winner = if choice.internal() { child } else { parent };
            let resolved = plan.resolved(child).unwrap();
            assert_eq!(
                resolved.kind,
                if choice.internal() {
                    internal
                } else {
                    external
                }
            );
            assert_eq!(resolved.delay_members, vec![winner.owner()]);
            assert_eq!(plan.resolved(parent), Some(resolved));
            assert_eq!(warning.is_some(), choice.warns());
        }
    }
}

#[test]
fn port_net_type_rank_cannot_override_external_warning_winner() {
    let mut plan = NetCollapsePlan::default();
    let child = add(&mut plan, 1, NetType::Wor);
    let child_alias = add(&mut plan, 2, NetType::Wor);
    let parent = add(&mut plan, 3, NetType::Wand);
    plan.alias(child, child_alias).unwrap();
    assert!(plan.port(child, parent).unwrap().is_some());
    assert_eq!(
        plan.resolved(child_alias).unwrap().delay_members,
        vec![parent.owner()]
    );
    assert_eq!(plan.resolved(parent).unwrap().kind, NetType::Wand);
}

#[test]
fn port_net_type_propagates_dominance_through_a_chain() {
    let mut plan = NetCollapsePlan::default();
    let outer = add(&mut plan, 0, NetType::Wire);
    let middle = add(&mut plan, 1, NetType::Wire);
    let inner = add(&mut plan, 2, NetType::Wand);
    plan.port(middle, outer).unwrap();
    plan.port(inner, middle).unwrap();
    assert_eq!(plan.resolved(outer).unwrap().kind, NetType::Wand);
    assert_eq!(
        plan.resolved(outer).unwrap().delay_members,
        vec![inner.owner()]
    );
}

#[test]
fn port_net_type_sibling_tie_is_stable_and_supply_still_dominates() {
    let mut plan = NetCollapsePlan::default();
    let parent = add(&mut plan, 10, NetType::Wire);
    let first = add(&mut plan, 3, NetType::Wor);
    let second = add(&mut plan, 1, NetType::Wand);
    let supply = add(&mut plan, 0, NetType::Supply0);
    plan.port(first, parent).unwrap();
    assert_eq!(
        plan.port(second, parent).unwrap().unwrap().selected,
        NetType::Wor
    );
    plan.port(supply, parent).unwrap();
    assert_eq!(plan.resolved(first).unwrap().kind, NetType::Supply0);
    assert_eq!(
        plan.resolved(second).unwrap().delay_members,
        vec![supply.owner()]
    );
}

#[test]
fn port_net_type_does_not_legalize_mixed_alias_declarations() {
    let mut plan = NetCollapsePlan::default();
    let a = add(&mut plan, 0, NetType::Wand);
    let b = add(&mut plan, 1, NetType::Wor);
    assert!(plan
        .alias(a, b)
        .unwrap_err()
        .contains("incompatible net types"));
    let mut plan = NetCollapsePlan::default();
    let a = add(&mut plan, 0, NetType::Wire);
    let b = add(&mut plan, 1, NetType::Uwire);
    assert!(plan.alias(a, b).is_err());
}

#[test]
fn port_net_type_aliases_keep_delay_checks_until_a_port_selects_another_type() {
    let mut plan = NetCollapsePlan::default();
    let a = add(&mut plan, 8, NetType::Wire);
    let b = add(&mut plan, 2, NetType::Tri);
    plan.alias(a, b).unwrap();
    assert_eq!(
        plan.resolved(a).unwrap().delay_members,
        vec![b.owner(), a.owner()]
    );
    let parent = add(&mut plan, 10, NetType::Tri1);
    plan.port(a, parent).unwrap();
    assert_eq!(
        plan.resolved(b).unwrap().delay_members,
        vec![parent.owner()]
    );
}

#[test]
fn port_net_type_keeps_selected_bits_and_array_cells_independent() {
    let mut plan = NetCollapsePlan::default();
    let a0 = NetPoint::ArrayBit {
        owner: NodeId(0),
        element: 4,
        bit: 0,
    };
    let a1 = NetPoint::ArrayBit {
        owner: NodeId(0),
        element: 5,
        bit: 0,
    };
    let b0 = NetPoint::Bit(AliasBit::Net {
        net: NodeId(1),
        bit: 0,
    });
    let b1 = NetPoint::Bit(AliasBit::Net {
        net: NodeId(1),
        bit: 1,
    });
    for point in [a0, a1, b0, b1] {
        plan.insert(point, NetType::Wire).unwrap();
    }
    let wired = add(&mut plan, 2, NetType::Wand);
    plan.port(b0, a0).unwrap();
    plan.port(wired, b0).unwrap();
    assert_eq!(plan.resolved(a0).unwrap().kind, NetType::Wand);
    assert_eq!(plan.resolved(a1).unwrap().kind, NetType::Wire);
    assert_eq!(plan.resolved(b1).unwrap().kind, NetType::Wire);
    assert_ne!(plan.component(a1), plan.component(a0));
}

#[test]
fn port_net_type_duplicate_connections_preserve_the_winner() {
    let mut plan = NetCollapsePlan::default();
    let child = add(&mut plan, 1, NetType::Wire);
    let parent = add(&mut plan, 2, NetType::Tri0);
    plan.port(child, parent).unwrap();
    for _ in 0..100 {
        assert!(plan.port(child, parent).unwrap().is_none());
        assert_eq!(
            plan.resolved(child).unwrap().delay_members,
            vec![parent.owner()]
        );
    }
}

#[test]
fn port_net_type_missing_points_fail_without_creating_a_component() {
    let mut plan = NetCollapsePlan::default();
    assert!(plan.port(point(1), point(2)).is_err());
    assert!(plan.alias(point(1), point(2)).is_err());
    assert!(plan.resolved(point(1)).is_none());
}

#[test]
fn port_net_type_same_kind_uwire_alias_keeps_its_declaration_contract() {
    let mut plan = NetCollapsePlan::default();
    let a = add(&mut plan, 0, NetType::Uwire);
    let b = add(&mut plan, 1, NetType::Uwire);
    plan.alias(a, b).unwrap();
    assert_eq!(plan.resolved(a).unwrap().kind, NetType::Uwire);
    assert_eq!(plan.component(a), plan.component(b));
}
