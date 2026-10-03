//! R05: declaration identity survives capture and drives directional collapse.
use llg::core::{
    compile,
    db::{Db, Direction, NodeKind},
};
use llg::sim::{codegen, opt::OptConfig};

fn capture(name: &str, source: &str) -> Db {
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(name, source)],
        &compile::CompileOpts {
            top: Some("tb".into()),
            ..Default::default()
        },
    )
    .expect("legal dissimilar port source");
    Db::from_slang(&compiled.snapshot).expect("owned port-net database")
}

#[test]
fn port_net_type_supported_pairs_select_the_table_resolver_after_snapshot_drop() {
    // All 49 pairs of currently implemented resolved-net classes. The pure policy includes the
    // uwire/trireg rows; uwire actuals are covered by RTL-011 and trireg remains rejected.
    let names = ["wire", "wand", "wor", "tri0", "tri1", "supply0", "supply1"];
    let resolvers = [
        "LLG_RESOLVE_WIRE",
        "LLG_RESOLVE_WAND",
        "LLG_RESOLVE_WOR",
        "LLG_RESOLVE_TRI0",
        "LLG_RESOLVE_TRI1",
        "LLG_RESOLVE_SUPPLY0",
        "LLG_RESOLVE_SUPPLY1",
    ];
    // I/E select a declaration, lower-case additionally requires a warning.
    let choices = [
        "EEEEEEE", "IEeeeEE", "IeEeeEE", "IeeEeEE", "IeeeEEE", "IIIIIEe", "IIIIIeE",
    ];
    for (row, internal) in names.iter().enumerate() {
        for (column, external) in names.iter().enumerate() {
            let source = format!(
                "module child(inout {internal} p); endmodule\n\
                 module tb; {external} p; child u(p);\n\
                 initial begin #1 $display(\"%b\", p); $finish; end endmodule\n"
            );
            let database = capture("port-net-pair.sv", &source);
            database.validate().expect("valid imported port roles");
            let choice = choices[row].as_bytes()[column];
            let resolver = resolvers[if choice == b'I' || choice == b'i' {
                row
            } else {
                column
            }];
            for options in [OptConfig::none(), OptConfig::default()] {
                let model = codegen::generate_from_db_with_opts(&database, &options)
                    .expect("dissimilar pair lowers");
                let groups = model
                    .model_c
                    .lines()
                    .filter(|line| line.starts_with("static llg_net_t "))
                    .collect::<Vec<_>>();
                assert_eq!(groups.len(), 1, "{internal}/{external}: {groups:?}");
                assert!(
                    groups[0].contains(resolver),
                    "{internal}/{external}: {groups:?}"
                );
                assert_eq!(
                    model.warnings.len(),
                    usize::from(choice.is_ascii_lowercase()),
                    "{internal}/{external}: {:?}",
                    model.warnings
                );
            }
        }
    }
}

#[test]
fn port_net_type_selected_alias_array_and_ascending_contexts_lower() {
    for (name, source) in [
        (
            "selected.sv",
            include_str!("../fixtures/sim/port_net_types/selected.sv"),
        ),
        (
            "arrays.sv",
            include_str!("../fixtures/sim/port_net_types/arrays.sv"),
        ),
        (
            "aliases.sv",
            include_str!("../fixtures/sim/port_net_types/aliases.sv"),
        ),
        (
            "uwire_alias.sv",
            include_str!("../fixtures/sim/port_net_types/uwire_alias.sv"),
        ),
        (
            "ascending.sv",
            include_str!("../fixtures/sim/port_net_types/ascending.sv"),
        ),
        (
            "hierarchy.sv",
            include_str!("../fixtures/sim/port_net_types/hierarchy.sv"),
        ),
    ] {
        let database = capture(name, source);
        for options in [OptConfig::none(), OptConfig::default()] {
            let model = codegen::generate_from_db_with_opts(&database, &options)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(model.warnings.is_empty(), "{name}: {:?}", model.warnings);
        }
    }
}

#[test]
fn port_net_type_delay_selection_drops_dominated_delays_including_zero() {
    let database = capture(
        "delays.sv",
        include_str!("../fixtures/sim/port_net_types/delays.sv"),
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        let model = codegen::generate_from_db_with_opts(&database, &options)
            .expect("choose delays of dominating declarations, not both declarations");
        // The four input-wire ports have independent WIRE resolver groups.
        // Inspect only the four collapsed WAND networks tested by this fixture.
        let groups = model
            .model_c
            .lines()
            .filter(|line| {
                line.starts_with("static llg_net_t ") && line.contains("LLG_RESOLVE_WAND")
            })
            .collect::<Vec<_>>();
        assert_eq!(groups.len(), 4);
        assert_eq!(
            groups
                .iter()
                .filter(|line| { line.contains(".propagation_enabled = 1, .propagation = NULL") })
                .count(),
            2
        );
        assert_eq!(
            groups
                .iter()
                .filter(|line| {
                    line.contains(".propagation_enabled = 0, .propagation = NULL")
                        && line.contains(".propagation_rise = 0")
                        && line.contains(".propagation_fall = 0")
                        && line.contains(".propagation_turn_off = 0")
                })
                .count(),
            2
        );
    }
}

#[test]
fn port_net_type_expression_only_actual_joins_all_electrical_bits() {
    for (name, source) in [
        (
            "aliases.sv",
            include_str!("../fixtures/sim/port_net_types/aliases.sv"),
        ),
        (
            "concat.sv",
            include_str!("../fixtures/sim/port_net_types/concat_actual.sv"),
        ),
    ] {
        let database = capture(name, source);
        assert!(
            database.nodes().iter().any(|node| matches!(
                node.kind,
                NodeKind::Port {
                    direction: Direction::Inout,
                    high: None,
                    high_expr: Some(_),
                    low: Some(_),
                    ..
                }
            )),
            "{name}: fixture must exercise an expression-only actual"
        );
        for options in [OptConfig::none(), OptConfig::default()] {
            let model = codegen::generate_from_db_with_opts(&database, &options)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            // Partitioning may combine adjacent connected bits. Count their
            // electrical widths, rather than assuming one group per bit.
            let wired_width: u32 = model
                .model_c
                .lines()
                .filter(|line| {
                    line.starts_with("static llg_net_t ") && line.contains("LLG_RESOLVE_WAND")
                })
                .map(|line| {
                    line.split_once(".width = ")
                        .expect("net group width")
                        .1
                        .split_once(',')
                        .expect("width field delimiter")
                        .0
                        .parse::<u32>()
                        .expect("electrical width")
                })
                .sum();
            assert_eq!(
                wired_width, 4,
                "{name}: every connected bit must be wired-AND"
            );
            assert!(model.warnings.is_empty(), "{name}: {:?}", model.warnings);
        }
    }
}
