//! SYN-032 library-map and configuration selection coverage.

use std::path::Path;

use llg::core::{compile, db, model};

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "syn032_library_configs";
const EXPECTED: &str = "cell=22 value=3\ncell=11 value=4\ndefault=11\n";
const EXPECTED_STDERR: &str =
    "llg: simulation ended without $finish (no processes remain) at time 0\n";

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn configured_libraries_execute_in_both_editions_and_optimizer_modes() {
    let map = fixture("root.map");
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_source_prefix(
            SUITE,
            "top",
            &["config"],
            EXPECTED,
            EXPECTED_STDERR,
            &[],
            &[
                "--edition",
                edition,
                "--top",
                "choose:config",
                "--libmap",
                map.as_str(),
            ],
        );
    }
}

#[test]
fn explicit_library_files_execute_in_both_editions_and_optimizer_modes() {
    let rtl = fixture("rtl.sv");
    let gate = fixture("gate.sv");
    let rtl_arg = format!("rtl={rtl}");
    let gate_arg = format!("gate={gate}");
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_source_prefix(
            SUITE,
            "top",
            &["config"],
            EXPECTED,
            EXPECTED_STDERR,
            &[],
            &[
                "--edition",
                edition,
                "--top",
                "choose:config",
                "--libfile",
                rtl_arg.as_str(),
                "--libfile",
                gate_arg.as_str(),
            ],
        );
    }
}

#[test]
fn selected_library_identity_reaches_the_owned_instance_model() {
    let output = compile::compile_sources_checked(
        &[
            compile::OwnedSource::compilation_unit(
                "top.sv",
                "module top; logic_cell #(.VALUE(3)) from_cell(); logic_cell #(.VALUE(4)) from_instance(); default_cell from_default(); endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "config.sv",
                "config choose; design custom.top; default liblist rtl; cell logic_cell use gate.gate_cfg:config; instance top.from_instance use rtl.logic_cell; endconfig",
            ),
        ],
        &compile::CompileOpts {
            top: Some("choose:config".to_owned()),
            library_order: vec!["rtl".to_owned()],
            default_library: Some("custom".to_owned()),
            library_sources: vec![
                compile::LibrarySource::new(
                    "rtl.sv",
                    "module logic_cell #(parameter integer VALUE = 0); localparam integer LIB_MARK = 11; endmodule module default_cell; endmodule",
                    "rtl",
                ),
                compile::LibrarySource::new(
                    "gate.sv",
                    "module logic_cell #(parameter integer VALUE = 0); localparam integer LIB_MARK = 22; endmodule config gate_cfg; design gate.logic_cell; endconfig",
                    "gate",
                ),
            ],
            ..Default::default()
        },
    )
    .expect("configured in-memory sources should elaborate");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    let design = model::DesignModel::from_db(&database);

    let from_cell = design
        .instance("top.from_cell")
        .expect("cell override instance");
    let from_instance = design
        .instance("top.from_instance")
        .expect("instance override");
    let from_default = design
        .instance("top.from_default")
        .expect("default liblist instance");
    assert_eq!(from_cell.def_name, "logic_cell");
    assert_eq!(from_instance.def_name, "logic_cell");
    assert_eq!(from_default.def_name, "default_cell");
    assert_eq!(design.modules_in("gate.sv").len(), 1);
    assert_eq!(design.modules_in("rtl.sv").len(), 2);
    assert_eq!(
        from_cell
            .params
            .iter()
            .find(|parameter| parameter.name == "VALUE")
            .and_then(|parameter| parameter.value.as_ref())
            .and_then(|value| match value {
                llg::core::elab::Val::Bits(value) => value.to_u64(),
                _ => None,
            }),
        Some(3)
    );
    assert_eq!(
        from_instance
            .params
            .iter()
            .find(|parameter| parameter.name == "VALUE")
            .and_then(|parameter| parameter.value.as_ref())
            .and_then(|value| match value {
                llg::core::elab::Val::Bits(value) => value.to_u64(),
                _ => None,
            }),
        Some(4)
    );
    assert_eq!(
        from_cell
            .params
            .iter()
            .find(|parameter| parameter.name == "LIB_MARK")
            .and_then(|parameter| parameter.value.as_ref())
            .and_then(|value| match value {
                llg::core::elab::Val::Bits(value) => value.to_u64(),
                _ => None,
            }),
        Some(22)
    );
    assert_eq!(
        from_instance
            .params
            .iter()
            .find(|parameter| parameter.name == "LIB_MARK")
            .and_then(|parameter| parameter.value.as_ref())
            .and_then(|value| match value {
                llg::core::elab::Val::Bits(value) => value.to_u64(),
                _ => None,
            }),
        Some(11)
    );
}

#[test]
fn in_memory_library_map_uses_a_logical_name_and_admitted_source() {
    let output = compile::compile_sources_checked(
        &[
            compile::OwnedSource::compilation_unit(
                "virtual/top.sv",
                "module top; mapped_cell instance_name(); endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "virtual/missing/mapped.sv",
                "module mapped_cell; endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "virtual/config.sv",
                "config choose; design custom.top; cell mapped_cell use rtl.mapped_cell; endconfig",
            ),
        ],
        &compile::CompileOpts {
            top: Some("choose:config".to_owned()),
            default_library: Some("custom".to_owned()),
            library_maps: vec![
                compile::OwnedSource::include(
                    "virtual/missing/root.map",
                    "include \"nested.map\";",
                ),
                compile::OwnedSource::include(
                    "virtual/missing/nested.map",
                    "library rtl mapped.sv;",
                ),
            ],
            ..Default::default()
        },
    )
    .expect("a supplied map buffer must not be opened by name");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    let design = model::DesignModel::from_db(&database);
    assert_eq!(
        design.instance("top.instance_name").unwrap().def_name,
        "mapped_cell"
    );
    assert_eq!(design.modules_in("virtual/missing/mapped.sv").len(), 1);
}

#[test]
fn compile_checked_applies_in_memory_map_to_compile_opts_sources() {
    let output = compile::compile_checked(&compile::CompileOpts {
        sources: vec![
            compile::OwnedSource::compilation_unit(
                "virtual/top.sv",
                "module top; mapped_cell instance_name(); endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "virtual/missing/mapped.sv",
                "module mapped_cell; endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "virtual/config.sv",
                "config choose; design custom.top; cell mapped_cell use rtl.mapped_cell; endconfig",
            ),
        ],
        top: Some("choose:config".to_owned()),
        default_library: Some("custom".to_owned()),
        library_maps: vec![compile::OwnedSource::include(
            "virtual/missing/root.map",
            "library rtl mapped.sv;",
        )],
        ..Default::default()
    })
    .expect("compile_checked should apply in-memory map assignments");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    let design = model::DesignModel::from_db(&database);
    assert_eq!(
        design.instance("top.instance_name").unwrap().def_name,
        "mapped_cell"
    );
    assert_eq!(design.modules_in("virtual/missing/mapped.sv").len(), 1);
}

#[test]
fn compile_sources_resolves_configured_library_from_an_in_memory_map() {
    let output = compile::compile_sources_checked(
        &[
            compile::OwnedSource::compilation_unit(
                "virtual/top.sv",
                "module top; logic_cell from_cell(); default_cell from_default(); endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "virtual/config.sv",
                "config choose; design custom.top; default liblist rtl; cell logic_cell use rtl.logic_cell; endconfig",
            ),
            compile::OwnedSource::compilation_unit(
                "virtual/rtl.sv",
                "module logic_cell; endmodule module default_cell; endmodule",
            ),
        ],
        &compile::CompileOpts {
            top: Some("choose:config".to_owned()),
            default_library: Some("custom".to_owned()),
            library_order: vec!["rtl".to_owned()],
            library_maps: vec![compile::OwnedSource::include(
                "virtual/maps/root.map",
                "library rtl ../rtl.sv;",
            )],
            ..Default::default()
        },
    )
    .expect("compile_sources should apply in-memory map library assignments");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    let design = model::DesignModel::from_db(&database);
    assert_eq!(
        design.instance("top.from_cell").unwrap().def_name,
        "logic_cell"
    );
    assert_eq!(
        design.instance("top.from_default").unwrap().def_name,
        "default_cell"
    );
    assert_eq!(design.modules_in("virtual/rtl.sv").len(), 2);
}

#[test]
fn library_order_selects_an_unconfigured_definition() {
    let output = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(
            "top.sv",
            "module top; default_cell instance_name(); endmodule",
        )],
        &compile::CompileOpts {
            top: Some("top".to_owned()),
            library_order: vec!["rtl".to_owned()],
            default_library: Some("custom".to_owned()),
            library_sources: vec![compile::LibrarySource::new(
                "rtl.sv",
                "module default_cell; endmodule",
                "rtl",
            )],
            ..Default::default()
        },
    )
    .expect("library order should resolve an unconfigured definition");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    let design = model::DesignModel::from_db(&database);
    assert_eq!(
        design
            .instance("top.instance_name")
            .expect("library-selected instance")
            .def_name,
        "default_cell"
    );
}

#[test]
fn missing_configured_library_is_a_frontend_error() {
    let error = compile::compile_sources_checked(
        &[
            compile::OwnedSource::compilation_unit(
                "top.sv",
                "module top; logic_cell instance_name(); endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "config.sv",
                "config choose; design work.top; cell logic_cell use missing.logic_cell; endconfig",
            ),
        ],
        &compile::CompileOpts {
            top: Some("choose:config".to_owned()),
            library_sources: vec![compile::LibrarySource::new(
                "rtl.sv",
                "module logic_cell; endmodule",
                "rtl",
            )],
            ..Default::default()
        },
    )
    .expect_err("unknown config library must block elaboration");
    let diagnostics = error
        .diagnostics()
        .expect("frontend diagnostics for missing library");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("missing")),
        "diagnostics should name missing library: {diagnostics:?}"
    );
}

#[test]
fn cyclic_config_binding_is_a_frontend_error() {
    let error = compile::compile_sources_checked(
        &[
            compile::OwnedSource::compilation_unit(
                "top.sv",
                "module top; logic_cell instance_name(); endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "configs.sv",
                "config first; design work.top; cell logic_cell use work.second:config; endconfig config second; design work.top; cell logic_cell use work.first:config; endconfig",
            ),
        ],
        &compile::CompileOpts {
            top: Some("first:config".to_owned()),
            library_sources: vec![compile::LibrarySource::new(
                "rtl.sv",
                "module logic_cell; endmodule",
                "rtl",
            )],
            ..Default::default()
        },
    )
    .expect_err("cyclic config binding must block elaboration");
    let diagnostics = error
        .diagnostics()
        .expect("frontend diagnostics for cyclic config");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.to_ascii_lowercase().contains("cycle")
                || diagnostic
                    .message
                    .to_ascii_lowercase()
                    .contains("recursive")
                || diagnostic
                    .message
                    .to_ascii_lowercase()
                    .contains("maximum depth")
        }),
        "diagnostics should identify the config cycle: {diagnostics:?}"
    );
}
