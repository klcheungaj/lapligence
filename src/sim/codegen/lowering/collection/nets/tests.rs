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

fn partition_model(source: &str) -> IrModel {
    let compiled = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            "partition.sv",
            source,
        )],
        &crate::core::compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    let database = Db::from_slang(&compiled.snapshot).unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    cg.collect_timescales();
    cg.build_net_groups().unwrap();
    cg.model
}

#[test]
fn identically_connected_array_elements_have_one_wide_group() {
    for width in [1, 7, 64, 65, 129] {
        let model = partition_model(&format!(
            "module tb; wire [{0}:0] r[3]; assign r = '{{default:'0}}; assign r = '{{default:'1}}; endmodule", width - 1));
        assert_eq!(model.net_groups.len(), 3);
        assert!(model.net_groups.iter().all(|group| group.width == width));
    }
}

#[test]
fn electrical_runs_split_at_driver_and_force_boundaries() {
    let model = partition_model(
        r#"
module tb;
    wire [128:0] r[1];
    wire [128:0] peer;
    alias peer = r[0];
    assign r[0] = '0;
    assign peer[96:32] = '1;
    initial begin force peer[64] = 1'b1; #1; release peer[64]; end
endmodule
"#,
    );
    let mut widths = model
        .net_groups
        .iter()
        .map(|group| group.width)
        .collect::<Vec<_>>();
    widths.sort_unstable();
    assert_eq!(widths, [1, 32, 32, 32, 32]);
    for group in &model.net_groups {
        assert_eq!(
            group.width as usize * 2,
            model
                .signals
                .iter()
                .flat_map(|signal| &signal.net_alias)
                .filter(|binding| model.net_groups[binding.group].c_name == group.c_name)
                .count()
        );
    }
}

#[test]
fn array_contribution_work_grows_by_ranges_instead_of_bit_operations() {
    for width in [7, 129] {
        let source = format!("module tb; logic [{}:0] a = '0, b = '1; wire [{}:0] r[3]; for(genvar i=0;i<3;i++) begin assign r[i]=a; assign r[i]=b; end endmodule", width-1, width-1);
        let compiled = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "array_scale.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        let database = Db::from_slang(&compiled.snapshot).unwrap();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        let tops = cg.collect_design().unwrap();
        cg.bind_reference_ports().unwrap();
        cg.collect_timescales();
        cg.build_net_groups().unwrap();
        for top in tops {
            cg.emit_pass(top, Pass::Comb).unwrap();
        }
        assert_eq!(cg.model.net_groups.len(), 3);
        assert!(cg.model.net_groups.iter().all(|group| group.width == width));
        let mut contributions = HashSet::new();
        let mut writes_per_group = vec![0; cg.model.net_groups.len()];
        for statement in cg.model.processes.iter().flat_map(|process| &process.body) {
            if let IrStmt::Assign {
                lhs: IrLhs::Whole(signal),
                rhs,
                nba: false,
            } = statement
            {
                let driver = cg.model.signals[*signal].net_driver.unwrap();
                assert!(contributions.insert(driver), "each driver writes once");
                writes_per_group[driver.0] += 1;
                assert_eq!(rhs.width(), width);
                // The value reaches the slot whole, not as a concatenation
                // of bit selects.
                assert!(!matches!(rhs.kind(), IrExprKind::Concat { .. }));
            }
        }
        assert_eq!(contributions.len(), 6);
        assert_eq!(writes_per_group, [2, 2, 2]);
    }
}

/// Lower `source`'s net groups and continuous processes with a shared-run
/// bound of `limit` slot bits.
fn shared_run_model(source: &str, limit: u64) -> IrModel {
    shared_run_model_with(source, limit, false)
}

fn shared_run_model_with(source: &str, limit: u64, merge_ports: bool) -> IrModel {
    let compiled = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            "shared_runs.sv",
            source,
        )],
        &crate::core::compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    let database = Db::from_slang(&compiled.snapshot).unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.shared_run_slot_bits = limit;
    cg.merge_ports = merge_ports;
    let tops = cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    cg.collect_timescales();
    cg.build_net_groups().unwrap();
    for top in tops {
        cg.emit_pass(top, Pass::Comb).unwrap();
    }
    cg.model
}

fn group_widths(model: &IrModel) -> Vec<u32> {
    model.net_groups.iter().map(|group| group.width).collect()
}

const PER_BIT_INOUTS: &str = r#"
module leaf(inout wire a, input logic v);
    assign a = v;
endmodule
module tb;
    logic [15:0] r = '0;
    wire [15:0] bus;
    assign bus = r;
    for (genvar i = 0; i < 16; i++) begin : g
        leaf u(.a(bus[i]), .v(r[i]));
    end
endmodule
"#;

#[test]
fn a_vector_split_by_per_bit_connections_stays_one_group() {
    let model = shared_run_model(PER_BIT_INOUTS, MAX_SHARED_RUN_SLOT_BITS);
    assert_eq!(group_widths(&model), [16]);
    // The vector, sixteen formals, the wide driver and sixteen per-bit drivers.
    assert_eq!(model.net_groups[0].n_drivers, 1 + 16 + 1 + 16);
    let mut narrow = Vec::new();
    let mut whole = 0;
    for statement in model.processes.iter().flat_map(|process| &process.body) {
        match statement {
            IrStmt::Assign {
                lhs: IrLhs::Part(signal, left, right, false),
                rhs,
                nba: false,
            } if model.signals[*signal].net_driver.is_some() => {
                assert_eq!(left, right, "each leaf drives its own bit");
                assert_eq!(rhs.width(), 1);
                narrow.push(*left);
            }
            IrStmt::Assign {
                lhs: IrLhs::Whole(signal),
                rhs,
                nba: false,
            } if model.signals[*signal].net_driver.is_some() => {
                assert_eq!(rhs.width(), 16);
                whole += 1;
            }
            _ => {}
        }
    }
    narrow.sort_unstable();
    assert_eq!(narrow, (0..16).collect::<Vec<_>>());
    assert_eq!(whole, 1, "the wide driver writes its whole slot once");
}

/// Scattered bit-select ports on wide vectors (the `mixed_perf` shape): each
/// stage's delayed gates drive one bit of a shared vector group through a
/// merged output formal.
const SCATTERED_DELAYED_BITS: &str = r#"
module stage(input d, input en, output q, output l, output n);
    buf #2 b(q, d);
    bufif1 #1 t(l, q, en);
    not x(n, d);
endmodule
module tb;
    logic d = 0, en = 1;
    wire [7:0] q, l, n;
    for (genvar i = 0; i < 8; i++) begin : g
        stage s(.d(d), .en(en), .q(q[i]), .l(l[(i + 3) % 8]), .n(n[i]));
    end
endmodule
"#;

#[test]
fn delayed_per_bit_drivers_of_a_shared_vector_write_only_their_bits() {
    let model = shared_run_model_with(SCATTERED_DELAYED_BITS, MAX_SHARED_RUN_SLOT_BITS, true);
    // q, l and n stay one group each; the rest are the stages' one-bit nets.
    let widths = group_widths(&model);
    assert_eq!(widths.iter().filter(|width| **width == 8).count(), 3);
    assert!(widths.iter().all(|width| *width == 8 || *width == 1));
    let mut delayed = Vec::new();
    let mut undelayed = Vec::new();
    for mut statement in model.processes.iter().flat_map(|process| &process.body) {
        while let IrStmt::Located {
            statement: inner, ..
        } = statement
        {
            statement = inner;
        }
        match statement {
            IrStmt::InertialAssign {
                lhs: IrLhs::Part(signal, left, right, false),
                rhs,
                ..
            } if model.signals[*signal].net_driver.is_some() => {
                assert_eq!((left, rhs.width()), (right, 1), "one bit per gate");
                delayed.push(*left);
            }
            IrStmt::InertialEnable(enable) => {
                let IrLhs::Part(signal, left, right, false) = &enable.zero_lhs else {
                    panic!("an enable gate's driver writes one bit");
                };
                assert!(model.signals[*signal].net_driver.is_some());
                assert_eq!(
                    (left, enable.zero_rhs.width()),
                    (right, 1),
                    "one bit per gate"
                );
                delayed.push(*left);
            }
            IrStmt::Assign {
                lhs: IrLhs::Part(signal, left, right, false),
                rhs,
                nba: false,
            } if model.signals[*signal].net_driver.is_some() => {
                assert_eq!((left, rhs.width()), (right, 1), "one bit per gate");
                undelayed.push(*left);
            }
            // A gate's value reaches its one contribution directly: a local
            // would cost a value scope per evaluation.
            IrStmt::DeclLocal { name, .. } => {
                assert!(!name.starts_with("_alias_gate_value"), "{name}")
            }
            _ => {}
        }
    }
    delayed.sort_unstable();
    undelayed.sort_unstable();
    assert_eq!(
        delayed,
        (0..8).flat_map(|bit| [bit, bit]).collect::<Vec<_>>()
    );
    assert_eq!(undelayed, (0..8).collect::<Vec<_>>());
}

#[test]
fn a_shared_run_stops_at_its_slot_bound() {
    // Slots after w bits: the vector, w formals, the wide driver and w
    // per-bit drivers; 2w + 2 slots times w bits stay within 160 up to w = 8.
    let model = shared_run_model(PER_BIT_INOUTS, 160);
    assert_eq!(group_widths(&model), [8, 8]);
    let model = shared_run_model(PER_BIT_INOUTS, 143);
    assert_eq!(group_widths(&model), [7, 7, 2]);
}

#[test]
fn shared_runs_split_where_a_member_returns_or_reverses() {
    // `a` covers bits 0 and 2: it may not return after bit 1 left it.
    let model = shared_run_model(
        r#"
module pair(inout wire [1:0] a);
endmodule
module tb;
    wire [3:0] bus;
    assign bus = '0;
    pair u(.a({bus[2], bus[0]}));
endmodule
"#,
        MAX_SHARED_RUN_SLOT_BITS,
    );
    assert_eq!(group_widths(&model), [2, 2]);
    // A member whose bits descend against the vector's cannot share either.
    let model = shared_run_model(
        r#"
module pair(inout wire [1:0] a);
endmodule
module tb;
    wire [3:0] bus;
    assign bus = '0;
    pair u(.a({bus[0], bus[1]}));
endmodule
"#,
        MAX_SHARED_RUN_SLOT_BITS,
    );
    assert_eq!(group_widths(&model), [1, 3]);
}

#[test]
fn driver_boundaries_alone_still_split_a_run() {
    // Every column has the same members; only the drivers change.
    let model = shared_run_model(
        r#"
module tb;
    wire [7:0] a, b;
    alias a = b;
    assign a = '0;
    assign b[5:2] = '1;
endmodule
"#,
        MAX_SHARED_RUN_SLOT_BITS,
    );
    assert_eq!(group_widths(&model), [2, 4, 2]);
}
