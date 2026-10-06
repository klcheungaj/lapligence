//! SYN-032 library-map and configuration selection coverage.

use std::path::Path;

use llg::core::{compile, db, model};

use crate::sim_cli;
use crate::sim_harness;

const SUITE: &str = "syn032_library_configs";
const EXPECTED: &str = "cell=22 value=3\ncell=11 value=4\ndefault=11\n";
const EXPECTED_STDERR: &str =
    "llg: simulation ended without $finish (no processes remain) at time 0\n";

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(name);
    assert!(
        path.is_file(),
        "missing library fixture: {}",
        path.display()
    );
    path.to_string_lossy().into_owned()
}

#[test]
fn macro_generated_maps_select_runtime_designs_in_both_editions() {
    for edition in ["2001", "2009"] {
        for (map_name, top, define, expected) in [
            ("macro_root.map", "choose:config", None, "macro=11\n"),
            ("macro_root.map", "whole:config", None, "macro=11\n"),
            ("macro_paths.map", "choose:config", None, "macro=11\n"),
            ("macro_conditional.map", "choose:config", None, "macro=11\n"),
            (
                "macro_conditional.map",
                "choose:config",
                Some("PICK_GATE"),
                "macro=22\n",
            ),
            (
                "macro_included_root.map",
                "choose:config",
                None,
                "macro=22\n",
            ),
            ("macro_sv_only.map", "choose:config", None, "macro=11\n"),
        ] {
            if edition == "2001" && map_name == "macro_sv_only.map" {
                continue;
            }
            let map = fixture(map_name);
            let mut args = vec!["--edition", edition, "--top", top, "--libmap", &map];
            if let Some(define) = define {
                args.extend(["-D", define]);
            }
            sim_cli::run_case_with_args(SUITE, "macro_top", expected, EXPECTED_STDERR, &[], &args);
        }
        if edition == "2001" {
            let map = fixture("macro_sv_only.map");
            sim_cli::reject_case_with_args(
                SUITE,
                "macro_top",
                "`undefineall requires SystemVerilog-2009",
                &["--edition", edition, "--libmap", &map],
            );
        }
        let map = fixture("macro_incdir.map");
        sim_cli::run_case_with_args(
            SUITE,
            "incdir_top",
            "incdir=17,23\n",
            "llg: $finish at time 1000 at incdir_top:10:5\n",
            &[],
            &[
                "--edition",
                edition,
                "--top",
                "incdir_top",
                "--libmap",
                &map,
            ],
        );
    }
}

#[test]
fn macro_generated_map_errors_are_located_and_rejected_in_both_editions() {
    for edition in ["2001", "2009"] {
        for (map_name, diagnostic) in [
            (
                "macro_undefined.map",
                "line 1: undefined macro `MISSING_DECL",
            ),
            (
                "macro_recursive.map",
                "line 2: recursive or over-depth macro `LOOP",
            ),
            ("macro_unbalanced.map", "unbalanced conditional directive"),
            (
                "macro_invalid.map",
                "line 2: unexpected token `not_a_library`",
            ),
            (
                "macro_multiline_invalid.map",
                "line 3: unexpected token `not_a_library`",
            ),
            (
                "macro_reserved.map",
                "compiler directive names cannot be redefined",
            ),
            (
                "macro_bad_define.map",
                "line 1: malformed function-like macro definition",
            ),
        ] {
            let map = fixture(map_name);
            sim_cli::reject_case_with_args(
                SUITE,
                "macro_top",
                diagnostic,
                &["--edition", edition, "--libmap", &map],
            );
        }
    }
}

#[test]
fn logical_macro_map_defines_change_selected_owned_design_and_keep_original_text() {
    let map = "`ifdef PICK_GATE\nlibrary chosen gate.sv;\n`else\nlibrary chosen rtl.sv;\n`endif\n`define C config choose; design work.top; default liblist chosen; endconfig\n`C\n";
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        for (defines, expected) in [(Vec::new(), 11), (vec!["PICK_GATE".to_owned()], 22)] {
            let output = compile::compile_sources_checked(
                &[
                    compile::OwnedSource::compilation_unit(
                        "virtual/top.sv",
                        "module top; macro_cell chosen(); endmodule",
                    ),
                    compile::OwnedSource::include(
                        "virtual/rtl.sv",
                        "module macro_cell; localparam integer MARK = 11; endmodule",
                    ),
                    compile::OwnedSource::include(
                        "virtual/gate.sv",
                        "module macro_cell; localparam integer MARK = 22; endmodule",
                    ),
                ],
                &compile::CompileOpts {
                    edition,
                    defines,
                    top: Some("choose:config".to_owned()),
                    library_maps: vec![compile::OwnedSource::include("virtual/root.map", map)],
                    ..Default::default()
                },
            )
            .expect("macro map should select one logical library source");
            let database = db::Db::from_slang(&output.snapshot).expect("owned database");
            assert_eq!(database.source_text("virtual/root.map"), Some(map));
            let chosen = model::DesignModel::from_db(&database)
                .instance("top.chosen")
                .expect("selected instance")
                .params
                .iter()
                .find(|param| param.name == "MARK")
                .and_then(|param| param.value.as_ref())
                .and_then(|value| match value {
                    llg::core::elab::Val::Bits(bits) => bits.to_u64(),
                    _ => None,
                });
            assert_eq!(chosen, Some(expected));
        }
    }
}

#[test]
fn invalid_macro_generated_config_reports_its_original_map_use() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        let error = compile::compile_sources_checked(
            &[
                compile::OwnedSource::compilation_unit(
                    "virtual/top.sv",
                    "module top; macro_cell chosen(); endmodule",
                ),
                compile::OwnedSource::include(
                    "virtual/rtl.sv",
                    "module macro_cell; endmodule",
                ),
            ],
            &compile::CompileOpts {
                edition,
                top: Some("choose:config".to_owned()),
                library_maps: vec![compile::OwnedSource::include(
                    "virtual/root.map",
                    "library chosen rtl.sv;\n`define BAD config choose; design work.top; cell macro_cell use chosen.absent; endconfig\n`BAD\n",
                )],
                ..Default::default()
            },
        )
        .expect_err("invalid macro-produced binding must reject");
        assert!(
            error
                .diagnostics()
                .is_some_and(|diagnostics| diagnostics.iter().any(|diag| {
                    diag.file.as_deref() == Some("virtual/root.map")
                        && diag.line == 3
                        && diag.message.contains("absent")
                })),
            "macro diagnostics must point to the original invocation: {error:?}"
        );
    }
}

#[test]
fn map_local_macros_do_not_enter_design_sources_or_included_maps() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        let output = compile::compile_sources_checked(
            &[
                compile::OwnedSource::compilation_unit(
                    "virtual/top.sv",
                    "`ifdef MAP_ONLY\nmodule top; localparam integer MARK = 99; endmodule\n`else\nmodule top; localparam integer MARK = 11; endmodule\n`endif\n",
                ),
                compile::OwnedSource::include("virtual/rtl.sv", "module spare; endmodule"),
            ],
            &compile::CompileOpts {
                edition,
                top: Some("top".to_owned()),
                library_maps: vec![compile::OwnedSource::include(
                    "virtual/root.map",
                    "`define MAP_ONLY\nlibrary chosen rtl.sv;\n",
                )],
                ..Default::default()
            },
        )
        .expect("map-local define must not enter the design source");
        let database = db::Db::from_slang(&output.snapshot).expect("owned database");
        let model = model::DesignModel::from_db(&database);
        let marker = model
            .instance("top")
            .and_then(|instance| instance.params.iter().find(|param| param.name == "MARK"))
            .and_then(|param| param.value.as_ref())
            .and_then(|value| match value {
                llg::core::elab::Val::Bits(bits) => bits.to_u64(),
                _ => None,
            });
        assert_eq!(marker, Some(11));

        let error = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "virtual/top.sv",
                "module top; endmodule",
            )],
            &compile::CompileOpts {
                edition,
                top: Some("top".to_owned()),
                library_maps: vec![
                    compile::OwnedSource::include(
                        "virtual/root.map",
                        "`define PARENT library chosen rtl.sv;\ninclude \"child.map\";\n",
                    ),
                    compile::OwnedSource::include("virtual/child.map", "`PARENT\n"),
                ],
                ..Default::default()
            },
        )
        .expect_err("a map include starts a fresh macro environment");
        assert!(error
            .to_string()
            .contains("child.map: library map line 1: undefined macro `PARENT"));
    }
}

#[test]
fn logical_included_map_expands_paths_and_configuration() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        let output = compile::compile_sources_checked(
            &[
                compile::OwnedSource::compilation_unit(
                    "virtual/top.sv",
                    "module top; macro_cell chosen(); endmodule",
                ),
                compile::OwnedSource::include(
                    "virtual/gate.sv",
                    "module macro_cell; localparam integer MARK = 22; endmodule",
                ),
                compile::OwnedSource::include("virtual/extra.sv", "module spare; endmodule"),
            ],
            &compile::CompileOpts {
                edition,
                top: Some("choose:config".to_owned()),
                library_maps: vec![
                    compile::OwnedSource::include(
                        "virtual/root.map",
                        "`define CHILD include \"nested.map\";\n`CHILD\n",
                    ),
                    compile::OwnedSource::include(
                        "virtual/nested.map",
                        "`define PATHS gate.sv, extra.sv\nlibrary chosen `PATHS;\n`define CONF config choose; design work.top; default liblist chosen; endconfig\n`CONF\n",
                    ),
                ],
                ..Default::default()
            },
        )
        .expect("logical included map macros should elaborate");
        let database = db::Db::from_slang(&output.snapshot).expect("owned database");
        let model = model::DesignModel::from_db(&database);
        let marker = model
            .instance("top.chosen")
            .and_then(|instance| instance.params.iter().find(|param| param.name == "MARK"))
            .and_then(|param| param.value.as_ref())
            .and_then(|value| match value {
                llg::core::elab::Val::Bits(bits) => bits.to_u64(),
                _ => None,
            });
        assert_eq!(marker, Some(22));
    }
}

#[test]
fn explicitly_assigned_map_retains_macro_configuration_binding() {
    let map = "`define LIB library chosen gate.sv;\n`LIB\n`define CFG config choose; design work.top; default liblist chosen; endconfig\n`CFG\n";
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        let output = compile::compile_sources_checked(
            &[
                compile::OwnedSource::compilation_unit(
                    "virtual/top.sv",
                    "module top; macro_cell chosen(); endmodule",
                ),
                compile::OwnedSource::include(
                    "virtual/gate.sv",
                    "module macro_cell; localparam integer MARK = 22; endmodule",
                ),
            ],
            &compile::CompileOpts {
                edition,
                top: Some("choose:config".to_owned()),
                library_sources: vec![compile::LibrarySource::new("virtual/root.map", map, "cfg")],
                library_maps: vec![compile::OwnedSource::include("virtual/root.map", map)],
                ..Default::default()
            },
        )
        .expect("explicitly assigned map must still parse as a map");
        let database = db::Db::from_slang(&output.snapshot).expect("owned database");
        assert_eq!(database.source_text("virtual/root.map"), Some(map));
        let model = model::DesignModel::from_db(&database);
        assert!(model.instance("top.chosen").is_some());
    }
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
fn filesystem_map_binding_changes_selected_composition() {
    for edition in ["2001", "2009"] {
        for (map_name, expected) in [
            ("choose_gate.map", EXPECTED),
            (
                "choose_rtl.map",
                "cell=11 value=3\ncell=11 value=4\ndefault=11\n",
            ),
        ] {
            let map = fixture(map_name);
            sim_cli::run_case_with_args(
                SUITE,
                "top",
                expected,
                EXPECTED_STDERR,
                &[],
                &[
                    "--edition",
                    edition,
                    "--top",
                    "choose:config",
                    "--libmap",
                    &map,
                ],
            );
        }
    }
}

#[test]
fn selected_library_identity_reaches_the_owned_instance_model() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
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
            edition,
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
fn selected_map_composition_changes_owned_instance_identity() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        for (selected, expected_marker) in [("gate", 22), ("rtl", 11)] {
            let map = format!(
                "library rtl rtl.sv; library gate gate.sv; \
                 config choose; design custom.top; default liblist rtl; \
                 cell logic_cell use {selected}.logic_cell; endconfig"
            );
            let database = {
                let output = compile::compile_sources_checked(
                    &[
                        compile::OwnedSource::compilation_unit(
                            "virtual/top.sv",
                            "module top; logic_cell chosen(); endmodule",
                        ),
                        compile::OwnedSource::include(
                            "virtual/rtl.sv",
                            "module logic_cell; localparam integer LIB_MARK = 11; endmodule",
                        ),
                        compile::OwnedSource::include(
                            "virtual/gate.sv",
                            "module logic_cell; localparam integer LIB_MARK = 22; endmodule",
                        ),
                    ],
                    &compile::CompileOpts {
                        edition,
                        top: Some("choose:config".to_owned()),
                        default_library: Some("custom".to_owned()),
                        library_maps: vec![compile::OwnedSource::include(
                            "virtual/root.map",
                            map.clone(),
                        )],
                        ..Default::default()
                    },
                )
                .expect("selected map composition should elaborate");
                let database = db::Db::from_slang(&output.snapshot).expect("owned selected design");
                assert_eq!(database.source_text("virtual/root.map"), Some(map.as_str()));
                database
            };
            let design = model::DesignModel::from_db(&database);
            let chosen = design.instance("top.chosen").expect("selected instance");
            let marker = chosen
                .params
                .iter()
                .find(|parameter| parameter.name == "LIB_MARK")
                .and_then(|parameter| parameter.value.as_ref())
                .and_then(|value| match value {
                    llg::core::elab::Val::Bits(value) => value.to_u64(),
                    _ => None,
                });
            assert_eq!(marker, Some(expected_marker));
        }
    }
}

#[test]
fn missing_configured_library_is_a_frontend_error() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
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
            edition,
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
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("missing")
                    && diagnostic.file.as_deref() == Some("config.sv")
                    && diagnostic.line > 0
            }),
            "diagnostics should name missing library: {diagnostics:?}"
        );
    }
}

#[test]
fn missing_configured_cell_is_a_frontend_error_in_both_editions() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        let error = compile::compile_sources_checked(
            &[
                compile::OwnedSource::compilation_unit(
                    "top.sv",
                    "module top; logic_cell instance_name(); endmodule",
                ),
                compile::OwnedSource::compilation_unit(
                    "config.sv",
                    "config choose; design work.top; cell logic_cell use rtl.absent_cell; endconfig",
                ),
            ],
            &compile::CompileOpts {
                edition,
                top: Some("choose:config".to_owned()),
                library_sources: vec![compile::LibrarySource::new(
                    "rtl.sv",
                    "module logic_cell; endmodule",
                    "rtl",
                )],
                ..Default::default()
            },
        )
        .expect_err("missing selected cell must block elaboration");
        let diagnostics = error.diagnostics().expect("frontend binding diagnostic");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("absent_cell")
                    && diagnostic.file.as_deref() == Some("config.sv")
                    && diagnostic.line > 0
            }),
            "diagnostics should name the missing cell: {diagnostics:?}"
        );
    }
}

#[test]
fn filesystem_map_binding_errors_are_reported_in_both_editions_and_modes() {
    for edition in ["2001", "2009"] {
        for (map_name, diagnostic) in [
            ("missing_library.map", "missing"),
            ("missing_cell.map", "absent_cell"),
            ("ambiguous.map", "ambiguous library mapping"),
            ("unmatched_path.map", "matched no files"),
        ] {
            let map = fixture(map_name);
            sim_cli::reject_case_with_args(
                SUITE,
                "top",
                diagnostic,
                &[
                    "--edition",
                    edition,
                    "--top",
                    "choose:config",
                    "--libmap",
                    &map,
                ],
            );
        }
    }
}

#[test]
fn filesystem_config_cycle_is_diagnosed_in_both_editions_and_modes() {
    let rtl = format!("rtl={}", fixture("rtl.sv"));
    for edition in ["2001", "2009"] {
        for optimized in [false, true] {
            let output = sim_cli::invoke_with_source_prefix(
                SUITE,
                "cycle_top",
                &["cycle_config"],
                optimized,
                &[
                    "--edition",
                    edition,
                    "--top",
                    "first:config",
                    "--libfile",
                    &rtl,
                ],
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{stderr}");
            assert!(output.stdout.is_empty());
            assert!(
                stderr.contains("cycle")
                    || stderr.contains("recursive")
                    || stderr.contains("maximum depth"),
                "configuration cycle needs a binding diagnostic: {stderr}"
            );
            assert!(stderr.contains("cycle_top.sv:"), "{stderr}");
        }
    }
}

#[test]
fn in_memory_maps_reject_ambiguous_and_unadmitted_sources() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        for (map, expected) in [
            (
                "library rtl rtl.sv; library gate rtl.sv;",
                "ambiguous library mapping",
            ),
            ("library rtl unadmitted.sv;", "matched no admitted buffers"),
        ] {
            let error = compile::compile_sources_checked(
                &[
                    compile::OwnedSource::compilation_unit(
                        "virtual/top.sv",
                        "module top; endmodule",
                    ),
                    compile::OwnedSource::include("virtual/rtl.sv", "module cell; endmodule"),
                ],
                &compile::CompileOpts {
                    edition,
                    top: Some("top".to_owned()),
                    library_maps: vec![compile::OwnedSource::include("virtual/root.map", map)],
                    ..Default::default()
                },
            )
            .expect_err("conflicting or unadmitted library source must reject");
            assert!(
                error.to_string().contains(expected),
                "expected {expected}: {error}"
            );
        }
    }
}

#[test]
fn cyclic_config_binding_is_a_frontend_error() {
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
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
            edition,
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
}

#[test]
fn library_map_incdirs_select_scoped_headers_in_both_editions() {
    let map_path = fixture("incdir.map");
    let wildcard_map = fixture("incdir_wildcard.map");
    let _ = fixture("incdir_top.sv");
    let _ = fixture("incdir_rtl.sv");
    let _ = fixture("incdir_gate.sv");
    let _ = fixture("incdir_headers/rtl_first/value.vh");
    let _ = fixture("incdir_headers/rtl_second/value.vh");
    let _ = fixture("incdir_headers/gate/value.vh");
    let global_header = fixture("incdir_headers/global/value.vh");
    let global = Path::new(&global_header)
        .parent()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    for (edition, language_edition) in [
        ("2001", compile::LanguageEdition::Verilog2001),
        ("2009", compile::LanguageEdition::SystemVerilog2009),
    ] {
        for map in [&map_path, &wildcard_map] {
            for (global_dir, expected) in [(false, "incdir=17,23\n"), (true, "incdir=66,66\n")] {
                let mut args = vec![
                    "--edition",
                    edition,
                    "--top",
                    "incdir_top",
                    "--libmap",
                    map,
                    "--library-order",
                    "rtl,gate",
                ];
                if global_dir {
                    args.extend(["--include-dir", &global]);
                }
                sim_cli::run_case_with_args(
                    SUITE,
                    "incdir_top",
                    expected,
                    "llg: $finish at time 1000 at incdir_top:10:5\n",
                    &[],
                    &args,
                );
            }
        }

        for map_text in [
            "library rtl rtl.sv -incdir headers/rtl;",
            "library rtl rtl.sv - incdir headers/rtl;",
            "`define DIR -incdir headers/rtl\nlibrary rtl rtl.sv `DIR;",
        ] {
            let output = compile::compile_sources_checked(
                &[
                    compile::OwnedSource::compilation_unit("virtual/top.sv", "module top; rtl_cell a(); endmodule"),
                    compile::OwnedSource::compilation_unit("virtual/rtl.sv", "`include \"value.vh\"\nmodule rtl_cell; localparam integer MARK = `VALUE; endmodule"),
                    compile::OwnedSource::include("virtual/headers/rtl/value.vh", "`define VALUE 17\n"),
                ],
                &compile::CompileOpts {
                    edition: language_edition,
                    top: Some("top".to_owned()),
                    library_maps: vec![compile::OwnedSource::include("virtual/root.map", map_text)],
                    library_order: vec!["rtl".to_owned()],
                    ..Default::default()
                },
            )
            .expect("in-memory headers should resolve under the library's logical incdir");
            let database = db::Db::from_slang(&output.snapshot).expect("owned database");
            let design = model::DesignModel::from_db(&database);
            let mark = design
                .instance("top.a")
                .unwrap()
                .params
                .iter()
                .find(|parameter| parameter.name == "MARK")
                .and_then(|parameter| parameter.value.as_ref())
                .and_then(|value| match value {
                    llg::core::elab::Val::Bits(value) => value.to_u64(),
                    _ => None,
                });
            assert_eq!(mark, Some(17));
        }
    }
}

#[test]
fn library_incdir_failures_are_reported_in_both_editions_and_modes() {
    let no_leak = fixture("incdir_no_leak.map");
    let missing_dir = fixture("incdir_missing_dir.map");
    let missing_header = fixture("incdir_missing_header.map");
    let _ = fixture("incdir_missing_header.sv");
    for edition in ["2001", "2009"] {
        for (map, diagnostic) in [
            (&no_leak, "value.vh"),
            (&missing_dir, "incdir_headers/absent"),
            (&missing_header, "absent.vh"),
        ] {
            sim_cli::reject_case_with_args(
                SUITE,
                "incdir_top",
                diagnostic,
                &[
                    "--edition",
                    edition,
                    "--top",
                    "incdir_top",
                    "--libmap",
                    map,
                    "--library-order",
                    "rtl,gate",
                ],
            );
        }
    }
}

#[test]
fn logical_incdir_choice_is_part_of_compile_input_identity() {
    for (directory, expected) in [("first", 17), ("second", 99), ("*", 17)] {
        let output = compile::compile_sources_checked(
            &[
                compile::OwnedSource::compilation_unit("virtual/top.sv", "module top; rtl_cell a(); endmodule"),
                compile::OwnedSource::compilation_unit("virtual/rtl.sv", "`include \"value.vh\"\nmodule rtl_cell; localparam integer MARK = `VALUE; endmodule"),
                compile::OwnedSource::include("virtual/headers/first/value.vh", "`define VALUE 17\n"),
                compile::OwnedSource::include("virtual/headers/second/value.vh", "`define VALUE 99\n"),
            ],
            &compile::CompileOpts {
                top: Some("top".to_owned()),
                library_order: vec!["rtl".to_owned()],
                library_maps: vec![compile::OwnedSource::include(
                    "virtual/root.map",
                    format!("library rtl rtl.sv -incdir headers/{directory};"),
                )],
                ..Default::default()
            },
        ).expect("logical map compiles with selected include directory");
        let database = db::Db::from_slang(&output.snapshot).expect("owned database");
        let design = model::DesignModel::from_db(&database);
        let mark = design
            .instance("top.a")
            .unwrap()
            .params
            .iter()
            .find(|parameter| parameter.name == "MARK")
            .and_then(|parameter| parameter.value.as_ref())
            .and_then(|value| match value {
                llg::core::elab::Val::Bits(value) => value.to_u64(),
                _ => None,
            });
        assert_eq!(mark, Some(expected));
    }
}

#[test]
fn including_file_directory_precedes_library_incdir() {
    let output = compile::compile_sources_checked(
        &[
            compile::OwnedSource::compilation_unit("virtual/top.sv", "module top; rtl_cell a(); endmodule"),
            compile::OwnedSource::compilation_unit("virtual/rtl.sv", "`include \"value.vh\"\nmodule rtl_cell; localparam integer MARK = `VALUE; endmodule"),
            compile::OwnedSource::include("virtual/value.vh", "`define VALUE 42\n"),
            compile::OwnedSource::include("virtual/headers/rtl/value.vh", "`define VALUE 17\n"),
        ],
        &compile::CompileOpts {
            top: Some("top".to_owned()),
            library_order: vec!["rtl".to_owned()],
            library_maps: vec![compile::OwnedSource::include(
                "virtual/root.map",
                "library rtl rtl.sv -incdir headers/rtl;",
            )],
            ..Default::default()
        },
    ).expect("local include should resolve before library search");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    let design = model::DesignModel::from_db(&database);
    let mark = design
        .instance("top.a")
        .unwrap()
        .params
        .iter()
        .find(|parameter| parameter.name == "MARK")
        .and_then(|parameter| parameter.value.as_ref())
        .and_then(|value| match value {
            llg::core::elab::Val::Bits(value) => value.to_u64(),
            _ => None,
        });
    assert_eq!(mark, Some(42));
}

#[test]
fn logical_library_incdir_missing_inputs_have_precise_diagnostics() {
    let sources = [
        compile::OwnedSource::compilation_unit(
            "virtual/top.sv",
            "module top; rtl_cell a(); endmodule",
        ),
        compile::OwnedSource::compilation_unit(
            "virtual/rtl.sv",
            "`include \"absent.vh\"\nmodule rtl_cell; endmodule",
        ),
        compile::OwnedSource::include(
            "virtual/headers/rtl/other.vh",
            "// admitted directory witness\n",
        ),
    ];
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        let opts = compile::CompileOpts {
            edition,
            top: Some("top".to_owned()),
            library_order: vec!["rtl".to_owned()],
            library_maps: vec![compile::OwnedSource::include(
                "virtual/root.map",
                "library rtl rtl.sv -incdir headers/rtl;",
            )],
            ..Default::default()
        };
        let error = compile::compile_sources_checked(&sources, &opts)
            .expect_err("missing include must fail at its directive");
        let compile::CompileError::FrontendDiagnostics(diagnostics) = error else {
            panic!("expected frontend diagnostics")
        };
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.file.as_deref() == Some("virtual/rtl.sv")
                    && diag.line == 1
                    && diag.message.contains("absent.vh")),
            "{diagnostics:?}"
        );

        let mut missing_dir = opts;
        missing_dir.library_maps = vec![compile::OwnedSource::include(
            "virtual/root.map",
            "library rtl rtl.sv -incdir headers/missing;",
        )];
        let error = compile::compile_sources_checked(&sources, &missing_dir)
            .expect_err("logical directory without admitted buffers must reject");
        let compile::CompileError::Startup(error) = error else {
            panic!("expected admission diagnostic")
        };
        assert_eq!(error.kind(), compile::StartupErrorKind::Input);
        assert!(error
            .to_string()
            .contains("matched no admitted directories"));
    }
}

#[cfg(unix)]
#[test]
fn library_incdir_symlink_escape_is_rejected_before_compilation() {
    use std::os::unix::fs::symlink;

    let map_dir = sim_harness::TempDir::new("incdir_map").expect("map directory");
    let outside = sim_harness::TempDir::new("incdir_outside").expect("outside directory");
    let map = map_dir.path().join("root.map");
    std::fs::write(&map, "library rtl rtl.sv -incdir escape;\n").expect("map");
    std::fs::copy(fixture("incdir_rtl.sv"), map_dir.path().join("rtl.sv")).expect("library source");
    symlink(outside.path(), map_dir.path().join("escape")).expect("escape link");
    let map = map.to_string_lossy().into_owned();
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(
            SUITE,
            "incdir_top",
            "library -incdir `escape` cannot be admitted",
            &["--edition", edition, "--libmap", &map],
        );
    }
}

#[cfg(unix)]
#[test]
fn library_pattern_symlink_escape_is_rejected_before_compilation() {
    use std::os::unix::fs::symlink;

    let map_dir = sim_harness::TempDir::new("pattern_map").expect("map directory");
    let outside = sim_harness::TempDir::new("pattern_outside").expect("outside directory");
    std::fs::copy(fixture("incdir_rtl.sv"), outside.path().join("rtl.sv")).expect("outside source");
    symlink(outside.path(), map_dir.path().join("escape")).expect("escape link");
    let map = map_dir.path().join("root.map");
    std::fs::write(&map, "library rtl escape/rtl.sv;\n").expect("map");
    let map = map.to_string_lossy().into_owned();
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(
            SUITE,
            "incdir_top",
            "escapes admitted root",
            &["--edition", edition, "--libmap", &map],
        );
    }
}
