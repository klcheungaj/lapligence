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
            assert!(
                slots.insert((actual_group, slot)),
                "continuous sites must not share a slot"
            );
            *counts.entry(actual_group).or_insert(0usize) += 1;
        }
    }
    assert_eq!(
        counts.len(),
        4,
        "two independent wired kinds in each of two instances"
    );
    assert!(counts.values().all(|count| *count == 3));
    for group in counts.keys() {
        assert_eq!(cg.model.net_groups[*group].width, 65);
    }
}

#[test]
fn indexed_aliases_use_declaration_order_in_both_directions() {
    let database = {
        let result = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "indexed_aliases.sv",
                r#"
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
"#,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&result.snapshot).unwrap()
    };
    database.validate().unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    let mut count = 0;
    for node in database.node_ids() {
        let NodeKind::NetAlias { nets } = cg.kind(node) else {
            continue;
        };
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
        let root = database
            .node_ids()
            .find(|candidate| {
                cg.node(*candidate).name == root
                    && matches!(cg.kind(*candidate), NodeKind::Net { .. })
            })
            .unwrap();
        let expected: Vec<_> = physical
            .iter()
            .map(|bit| AliasBit::Net {
                net: root,
                bit: *bit,
            })
            .collect();
        assert_eq!(cg.alias_expression_bits(node, nets[1]).unwrap(), expected);
        count += 1;
    }
    assert_eq!(count, 6);
}

fn checked_database(name: &str, source: &str, top: &str) -> Db {
    let output = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            name, source,
        )],
        &crate::core::compile::CompileOpts {
            top: Some(top.to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    let database = Db::from_slang(&output.snapshot).unwrap();
    database.validate().unwrap();
    database
}

fn prepare_nets(cg: &mut Codegen<'_>) {
    cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    cg.collect_timescales();
    cg.build_net_groups().unwrap();
}

/// `alias_bit` resolutions spent resolving the alias bindings of every
/// one-bit continuous assignment to a `width`-bit net, with the assignment
/// count.
fn per_bit_assignment_alias_work(width: usize) -> (usize, usize) {
    let source = format!(
        "module tb;\n\
         \x20   wire [{high}:0] d;\n\
         \x20   genvar i;\n\
         \x20   for (i = 0; i < {width}; i = i + 1) begin : bits\n\
         \x20       assign d[i] = i[0];\n\
         \x20   end\n\
         endmodule\n",
        high = width - 1
    );
    let database = checked_database("per_bit_assignments.sv", &source, "tb");
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    prepare_nets(&mut cg);
    let assignments: Vec<_> = database
        .node_ids()
        .filter(|node| matches!(cg.kind(*node), NodeKind::ContAssign { .. }))
        .collect();
    let before = ALIAS_BIT_RESOLUTIONS.with(std::cell::Cell::get);
    for assignment in &assignments {
        if let Some(instance) = cg.owning_inst(*assignment) {
            cg.inst = instance;
        }
        let lhs = cg.node(*assignment).children[0];
        // No net here participates in an alias, so the ordinary path is kept.
        assert_eq!(cg.alias_lvalue_bindings(*assignment, lhs).unwrap(), None);
    }
    let work = ALIAS_BIT_RESOLUTIONS.with(std::cell::Cell::get) - before;
    (assignments.len(), work)
}

#[test]
fn per_bit_assignment_alias_work_does_not_grow_with_net_width() {
    // Resolving `d[i]` used to materialize every bit of `d`, making N
    // one-bit assignments to an N-bit net quadratic. Each assignment now
    // checks the net's two end bits and resolves the selected bit.
    const MAX_RESOLUTIONS_PER_ASSIGNMENT: usize = 3;
    let (narrow_count, narrow_work) = per_bit_assignment_alias_work(32);
    let (wide_count, wide_work) = per_bit_assignment_alias_work(512);
    assert_eq!((narrow_count, wide_count), (32, 512));
    assert!(
        narrow_work <= narrow_count * MAX_RESOLUTIONS_PER_ASSIGNMENT,
        "{narrow_work} resolutions for {narrow_count} assignments"
    );
    assert!(
        wide_work <= wide_count * MAX_RESOLUTIONS_PER_ASSIGNMENT,
        "{wide_work} resolutions for {wide_count} assignments"
    );
    assert_eq!(narrow_work * wide_count, wide_work * narrow_count);
}

#[test]
fn bit_and_part_selects_of_whole_nets_resolve_declared_bits() {
    let database = checked_database(
        "selected_aliases.sv",
        r#"
module tb;
    wire [0:7] up;
    wire [7:0] down;
    wire [-4:3] negative;
    wire [5:2] offset;
    wire up_bit, down_bit, offset_bit;
    wire [2:0] up_part, down_part, negative_part, offset_part;
    wire [2:0] up_plus, up_minus, down_plus, down_minus;
    wire [2:0] negative_plus, offset_minus;
    alias up_bit = up[2];
    alias down_bit = down[6];
    alias offset_bit = offset[3];
    alias up_part = up[1:3];
    alias down_part = down[6:4];
    alias negative_part = negative[-1:1];
    alias offset_part = offset[5:3];
    alias up_plus = up[1 +: 3];
    alias up_minus = up[3 -: 3];
    alias down_plus = down[4 +: 3];
    alias down_minus = down[6 -: 3];
    alias negative_plus = negative[-1 +: 3];
    alias offset_minus = offset[5 -: 3];
endmodule
"#,
        "tb",
    );
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    let net = |name: &str| {
        database
            .node_ids()
            .find(|node| {
                cg.node(*node).name == name && matches!(cg.kind(*node), NodeKind::Net { .. })
            })
            .unwrap()
    };
    // Independent oracle: physical bit 0 is the declared right bound, so a
    // label's bit is its distance from that bound.
    let physical = |left: i128, right: i128, label: i128| -> u32 {
        let bit = if left < right {
            right - label
        } else {
            label - right
        };
        u32::try_from(bit).unwrap()
    };
    let expected: &[(&str, &str, i128, i128, &[i128])] = &[
        ("up_bit", "up", 0, 7, &[2]),
        ("down_bit", "down", 7, 0, &[6]),
        ("offset_bit", "offset", 5, 2, &[3]),
        ("up_part", "up", 0, 7, &[1, 2, 3]),
        ("down_part", "down", 7, 0, &[6, 5, 4]),
        ("negative_part", "negative", -4, 3, &[-1, 0, 1]),
        ("offset_part", "offset", 5, 2, &[5, 4, 3]),
        ("up_plus", "up", 0, 7, &[1, 2, 3]),
        ("up_minus", "up", 0, 7, &[1, 2, 3]),
        ("down_plus", "down", 7, 0, &[6, 5, 4]),
        ("down_minus", "down", 7, 0, &[6, 5, 4]),
        ("negative_plus", "negative", -4, 3, &[-1, 0, 1]),
        ("offset_minus", "offset", 5, 2, &[5, 4, 3]),
    ];
    let mut checked = 0;
    for node in database.node_ids() {
        let NodeKind::NetAlias { nets } = cg.kind(node) else {
            continue;
        };
        let left = cg.alias_expression_bits(node, nets[0]).unwrap();
        let AliasBit::Net { net: endpoint, .. } = left[0] else {
            panic!("simple left alias endpoint");
        };
        let endpoint = cg.node(endpoint).name.clone();
        let (_, root, high, low, labels) = expected
            .iter()
            .find(|(name, ..)| *name == endpoint)
            .unwrap_or_else(|| panic!("unexpected alias endpoint {endpoint}"));
        let root = net(root);
        let bits: Vec<_> = labels
            .iter()
            .map(|label| AliasBit::Net {
                net: root,
                bit: physical(*high, *low, *label),
            })
            .collect();
        assert_eq!(cg.alias_expression_bits(node, nets[1]).unwrap(), bits);
        checked += 1;
    }
    assert_eq!(checked, expected.len());
}

#[test]
fn signal_reverse_index_returns_the_lowest_node_for_each_signal() {
    let database = checked_database(
        "reverse_index.sv",
        r#"
module child(input wire a, output wire y);
    assign y = a;
endmodule
module tb;
    wire x, z;
    logic r;
    child u0(.a(x), .y(z));
    child u1(.a(z), .y(x));
endmodule
"#,
        "tb",
    );
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    prepare_nets(&mut cg);
    let scan = |cg: &Codegen<'_>, ir: usize| {
        sorted_node_ids(&cg.sig_globals)
            .into_iter()
            .find(|node| cg.sig_globals[node].ir == ir)
    };
    assert!(!cg.sig_globals.is_empty());
    for info in cg.sig_globals.values() {
        assert_eq!(cg.sig_global_for_ir(info.ir), scan(&cg, info.ir));
    }
    // Re-pointing a node moves it between index buckets.
    let mut nodes = sorted_node_ids(&cg.sig_globals);
    let first = nodes.remove(0);
    let moved_from = cg.sig_globals[&first].ir;
    let target = cg.sig_globals[&nodes[0]].clone();
    cg.insert_sig_global(first, target.clone());
    assert_eq!(cg.sig_global_for_ir(target.ir), Some(first));
    assert_eq!(cg.sig_global_for_ir(moved_from), scan(&cg, moved_from));
    // In-place rewrites are picked up by a rebuild.
    for info in cg.sig_globals.values_mut() {
        info.ir += 1_000;
    }
    cg.rebuild_sig_global_index();
    assert_eq!(cg.sig_global_for_ir(target.ir), None);
    assert_eq!(cg.sig_global_for_ir(target.ir + 1_000), Some(first));
}

#[test]
fn structural_driver_lookup_matches_recorded_sites() {
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
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    prepare_nets(&mut cg);
    let mut drivers = 0;
    for node in database.node_ids() {
        let owner = cg.structural_site_owner(node);
        let recorded = cg
            .structural_driver_sites
            .keys()
            .any(|(site_owner, source, _)| *site_owner == owner && *source == node);
        assert_eq!(cg.has_structural_driver(node), recorded);
        drivers += usize::from(recorded);
    }
    assert!(drivers > 0);
}

#[test]
fn lazy_alias_view_keeps_the_first_eager_error_for_invalid_ranges() {
    let database = checked_database(
        "alias_range_error.sv",
        "module tb; wire [7:0] n; endmodule",
        "tb",
    );
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    let net = database
        .node_ids()
        .find(|node| matches!(cg.kind(*node), NodeKind::Net { .. }))
        .unwrap();
    // A malformed view reaches bit 8 after eight valid positions. Endpoint
    // failure must preserve the diagnostic at the first failing eager bit.
    let expected = cg.alias_bit(net, net, 8).unwrap_err();
    let actual = cg.whole_net_alias_bits(net, net, 0, 1, 9).err().unwrap();
    assert_eq!(actual, expected);
}
