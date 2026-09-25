//! Continuation coverage for value ports, structural connectivity and callbacks.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn fixed_inputs_retain_nested_conversions_at_every_limb_width() {
    for width in [1, 7, 65, 129] {
        let define = format!("CONTINUATION_PORT_W={width}");
        sim_cli::run_case_with_args(
            "continuation_24_27", "input_casts", &format!("INPUT_CASTS_PASS W={width}\n"),
            "", &[], &["--edition", "2009", "--define", &define],
        );
    }
}

#[test]
fn fixed_inputs_track_values_rows_selectors_and_single_evaluations() {
    sim_cli::run_case_with_args(
        "continuation_24_27", "input_values", "INPUT_VALUES_PASS\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_input_graph_lowers_after_native_teardown() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    for (name, source) in [
        ("input_casts.sv", include_str!("fixtures/sim/continuation_24_27/input_casts.sv")),
        ("input_values.sv", include_str!("fixtures/sim/continuation_24_27/input_values.sv")),
    ] {
        let database = {
            let result = compile::compile_sources_checked(
                &[compile::OwnedSource::compilation_unit(name, source)],
                &compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
            ).expect("legal fixed input values");
            db::Db::from_slang(&result.snapshot).unwrap()
        };
        database.validate().unwrap();
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("input conversions and projections retain owned type data");
        }
    }
}


#[test]
fn fixed_output_ref_and_primitive_shapes_keep_distinct_contracts() {
    for width in [1, 7, 65, 129] {
        let define = format!("CONTINUATION_PORT_W={width}");
        let args = ["--edition", "2009", "--define", &define];
        sim_cli::run_case_with_args(
            "continuation_24_27", "port_shapes", &format!("PORT_SHAPES_PASS W={width}\n"),
            "", &[], &args,
        );
        sim_cli::run_case_with_args(
            "continuation_24_27", "terminal_shapes", &format!("TERMINALS_PASS W={width}\n"),
            "", &[], &args,
        );
    }
}

#[test]
fn port_value_permissiveness_does_not_weaken_output_or_reference_legality() {
    sim_cli::reject_case_with_args(
        "rtl_completion", "syn_008_output_expression_rejected", "expression is not assignable",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "rtl_completion", "syn_008_ref_shape_rejected", "inequivalent type",
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_structural_port_graphs_lower_after_native_teardown() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    for (name, source) in [
        ("port_shapes.sv", include_str!("fixtures/sim/continuation_24_27/port_shapes.sv")),
        ("terminal_shapes.sv", include_str!("fixtures/sim/continuation_24_27/terminal_shapes.sv")),
    ] {
        let database = {
            let result = compile::compile_sources_checked(
                &[compile::OwnedSource::compilation_unit(name, source)],
                &compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
            ).expect("legal structural port shapes");
            db::Db::from_slang(&result.snapshot).unwrap()
        };
        database.validate().unwrap();
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options).unwrap();
        }
    }
}


#[test]
fn indexed_fixed_aliases_preserve_both_directions_and_release() {
    for width in [1, 7, 65, 129] {
        let define = format!("CONTINUATION_ALIAS_W={width}");
        sim_cli::run_case_with_args(
            "continuation_24_27", "alias_indexed", &format!("ALIASES_PASS W={width}\n"),
            "", &[], &["--edition", "2009", "--define", &define],
        );
    }
}

#[test]
fn static_alias_legality_remains_stricter_than_port_matching() {
    sim_cli::reject_case_with_args(
        "net_resolution", "syn_010_self_alias", "cannot alias a net to itself", &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "net_resolution", "syn_010_duplicate_alias", "same bits of the same nets more than once",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "net_resolution", "syn_010_incompatible_alias", "common nettype", &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "net_resolution", "syn_010_variable_alias", "is not a net", &["--edition", "2009"],
    );
}

#[test]
fn indexed_alias_graph_lowers_without_native_storage() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    let database = {
        let result = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit("alias_indexed.sv",
                include_str!("fixtures/sim/continuation_24_27/alias_indexed.sv"))],
            &compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
        ).unwrap();
        db::Db::from_slang(&result.snapshot).unwrap()
    };
    database.validate().unwrap();
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options).unwrap();
    }
}
