//! RTL-018: library/configuration selection and structural bind.
//!
//! Library and configuration inputs (maps, library sources, configurations)
//! are checked-in companions named through `sim_cli::*_with_inputs`; their
//! expected values are derived by hand in the fixture comments and below.

use super::sim_cli;
use llg::core::{compile, db, model};
use std::path::{Path, PathBuf};

const SUITE: &str = "feature_completion/rtl_018";
const COMPOSE_STDERR: &str = "llg: $finish at time 31000 at tb:50:9\n";
const UNITS_STDERR: &str = "llg: $finish at time 1000 at tb:17:5\n";
const BINDING_STDERR: &str = "llg: $finish at time 1000 at tb:16:5\n";

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
}

fn fixture(name: &str) -> String {
    let path = fixture_dir().join(name);
    assert!(path.is_file(), "missing fixture input: {}", path.display());
    path.to_string_lossy().into_owned()
}

/// Value-backend lanes: legacy, compact portable and, when a GMP root is
/// supplied, compact GMP. A missing GMP root is reported as blocked.
fn backend_lanes(gmp: &str) -> Vec<Vec<(&'static str, String)>> {
    let mut lanes = vec![
        vec![
            ("LLG_VALUE_BACKEND", "legacy".to_owned()),
            ("LLG_COMPACT_KERNELS", "portable".to_owned()),
        ],
        vec![
            ("LLG_VALUE_BACKEND", "compact".to_owned()),
            ("LLG_COMPACT_KERNELS", "portable".to_owned()),
        ],
    ];
    if gmp.is_empty() {
        eprintln!("BLOCKED GMP parity: set LLG_TEST_GMP_ROOT");
    } else {
        lanes.push(vec![
            ("LLG_VALUE_BACKEND", "compact".to_owned()),
            ("LLG_COMPACT_KERNELS", "gmp".to_owned()),
            ("GMP_ROOT", gmp.to_owned()),
        ]);
    }
    lanes
}

/// A01: configuration (library cells in generate scopes, parameter overrides),
/// generate/interface memories, instance and interface binds, and an oversized
/// memory crossing configured and bound ports, in both compilation-unit modes,
/// both optimizer modes and every value backend.
#[test]
fn configured_bound_generate_composition_runs_on_every_backend() {
    let gmp = std::env::var("LLG_TEST_GMP_ROOT").unwrap_or_default();
    for lane in backend_lanes(&gmp) {
        let envs: Vec<(&str, &str)> = lane.iter().map(|(k, v)| (*k, v.as_str())).collect();
        for mode in ["separate", "merged"] {
            sim_cli::run_case_with_inputs(
                SUITE,
                "compose",
                &[
                    "compose_config.sv",
                    "compose.map",
                    "lib/lane_rtl.sv",
                    "lib/lane_gate.sv",
                ],
                include_str!("../fixtures/sim/feature_completion/rtl_018/compose.out"),
                COMPOSE_STDERR,
                &[
                    "--top",
                    "rtl018_cfg:config",
                    "--compilation-units",
                    mode,
                    "--libmap",
                    "compose.map",
                    "compose_config.sv",
                ],
                &envs,
            );
        }
    }
}

fn compose_opts() -> compile::CompileOpts {
    compile::CompileOpts {
        files: vec![fixture("compose_config.sv"), fixture("compose.sv")],
        library_map_files: vec![fixture("compose.map")],
        top: Some("rtl018_cfg:config".to_owned()),
        ..Default::default()
    }
}

/// A01 after snapshot destruction: generation and native builds (O0/O3, both
/// optimizer modes) run after the frontend snapshot and then the Db are gone.
#[test]
fn composition_executes_after_snapshot_and_db_destruction() {
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        "compose",
        compose_opts(),
        include_str!("../fixtures/sim/feature_completion/rtl_018/compose.out"),
        COMPOSE_STDERR,
    );
}

/// The selected library of every instance is owned data in the Db.
#[test]
fn component_library_bindings_survive_snapshot_destruction() {
    let output = compile::compile_checked(&compose_opts()).expect("configured design compiles");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    drop(output);
    let mut bindings: Vec<(String, String)> = database
        .node_ids()
        .filter_map(|id| match database.node_kind(id) {
            db::NodeKind::ModuleInst { def_name, .. } => {
                let library = database.source_library(id)?;
                Some((def_name.clone(), library.to_owned()))
            }
            _ => None,
        })
        .filter(|(name, _)| name.starts_with("rtl018_lane") || name == "tb")
        .collect();
    bindings.sort();
    let expected = [
        ("rtl018_lane", "gate"),
        ("rtl018_lane", "rtl"),
        ("rtl018_lane", "rtl"),
        ("tb", "work"),
    ];
    assert_eq!(
        bindings,
        expected
            .iter()
            .map(|(name, library)| (name.to_string(), library.to_string()))
            .collect::<Vec<_>>()
    );
}

/// A02: library sources follow the compilation-unit mode (separate: each file
/// its own preprocessor; merged: one per library in admission order), the root
/// map's local macro reaches neither design sources nor its included map, and
/// command-line defines seed every unit, in both editions.
#[test]
fn library_units_follow_compilation_mode_and_map_macro_scope() {
    for edition in ["2001", "2009"] {
        for (mode, expected) in [
            ("separate", "mark=17 shared=0 map_only=0 cli=0 late=0\n"),
            ("merged", "mark=17 shared=7 map_only=0 cli=0 late=9\n"),
        ] {
            sim_cli::run_case_with_inputs(
                SUITE,
                "units",
                &[
                    "units_root.map",
                    "units/child.map",
                    "units/unit_a.sv",
                    "units/unit_b.sv",
                    "units/headers/mark.vh",
                    "units/headers/late.vh",
                ],
                expected,
                UNITS_STDERR,
                &[
                    "--edition",
                    edition,
                    "--compilation-units",
                    mode,
                    "--libmap",
                    "units_root.map",
                    "-L",
                    "rtl",
                ],
                &[],
            );
        }
        sim_cli::run_case_with_inputs(
            SUITE,
            "units",
            &["units_root.map", "units/child.map"],
            "mark=17 shared=0 map_only=0 cli=5 late=0\n",
            UNITS_STDERR,
            &[
                "--edition",
                edition,
                "--libmap",
                "units_root.map",
                "-L",
                "rtl",
                "-D",
                "RTL018_CLI=5",
            ],
            &[],
        );
    }
}

/// A02 include precedence: a global include root precedes the library's
/// -incdir directory (the including file's own directory has no header).
#[test]
fn global_include_root_precedes_library_incdir() {
    let global = fixture_dir().join("global");
    assert!(global.join("mark.vh").is_file());
    let global = global.to_string_lossy().into_owned();
    sim_cli::run_case_with_inputs(
        SUITE,
        "units",
        &["units_root.map", "global/mark.vh"],
        "mark=66 shared=0 map_only=0 cli=0 late=0\n",
        UNITS_STDERR,
        &[
            "--libmap",
            "units_root.map",
            "-L",
            "rtl",
            "--include-dir",
            &global,
        ],
        &[],
    );
}

/// A02: without a configuration, the declared library order resolves the
/// implementation-defined tie between same-named cells; a configuration
/// `use` clause overrides it. `%l` reports the bound library.cell (SV §33.7).
#[test]
fn library_order_and_configuration_select_the_bound_cell() {
    for edition in ["2001", "2009"] {
        for (order, expected) in [
            ("rtl,gate", "pick rtl.rtl018_pick\npicked=11 local=40\n"),
            ("gate,rtl", "pick gate.rtl018_pick\npicked=22 local=40\n"),
        ] {
            sim_cli::run_case_with_inputs(
                SUITE,
                "binding",
                &["binding.map", "lib/pick_rtl.sv", "lib/pick_gate.sv"],
                expected,
                BINDING_STDERR,
                &["--edition", edition, "--libmap", "binding.map", "-L", order],
                &[],
            );
        }
        sim_cli::run_case_with_inputs(
            SUITE,
            "binding",
            &["binding.map", "binding_config.sv"],
            "pick gate.rtl018_pick\npicked=22 local=40\n",
            BINDING_STDERR,
            &[
                "--edition",
                edition,
                "--top",
                "rtl018_gate_cfg:config",
                "--libmap",
                "binding.map",
                "-L",
                "rtl,gate",
                "binding_config.sv",
            ],
            &[],
        );
    }
}

/// A02 negatives: a missing library in a use clause, a liblist that omits the
/// parent library, and an equal-rank map ambiguity.
#[test]
fn missing_unreachable_and_ambiguous_bindings_reject() {
    sim_cli::reject_case_with_inputs(
        SUITE,
        "binding",
        &["binding.map", "binding_missing.sv"],
        "unknown library 'nolib'",
        &[
            "--top",
            "rtl018_missing_cfg:config",
            "--libmap",
            "binding.map",
            "binding_missing.sv",
        ],
    );
    sim_cli::reject_case_with_inputs(
        SUITE,
        "binding",
        &["binding.map", "binding_liblist.sv"],
        "unknown module 'rtl018_local'",
        &[
            "--top",
            "rtl018_liblist_cfg:config",
            "--libmap",
            "binding.map",
            "binding_liblist.sv",
        ],
    );
    sim_cli::reject_case_with_inputs(
        SUITE,
        "binding",
        &["binding_ambiguous.map"],
        "rtl and gate have equal precedence",
        &["--libmap", "binding_ambiguous.map"],
    );
}

/// Adopted FND-002 positive witnesses (L-F03-08-05, L-F03-08-06): an include
/// directory without headers and a macro-generated library name.
#[test]
fn adopted_library_witnesses_execute() {
    sim_cli::run_case_with_inputs(
        SUITE,
        "witness_empty_incdir.v",
        &[
            "witness/empty_incdir.map",
            "witness/cell_display.v",
            "witness/empty/README.txt",
        ],
        "library\n",
        "llg: $finish at time 1000 at tb:5:34\n",
        &[
            "--edition",
            "2001",
            "--libmap",
            "witness/empty_incdir.map",
            "-L",
            "aux",
        ],
        &[],
    );
    sim_cli::run_case_with_inputs(
        SUITE,
        "witness_macro_map.v",
        &["witness/macro_map.map", "witness/cell_display.v"],
        "library\n",
        "llg: $finish at time 1000 at tb:5:34\n",
        &[
            "--edition",
            "2001",
            "--libmap",
            "witness/macro_map.map",
            "-L",
            "aux",
        ],
        &[],
    );
}

/// L-F03-08-05: an in-memory caller can admit an empty logical directory, so a
/// map -incdir may name it; search continues to the next listed directory.
#[test]
fn component_logical_empty_include_directory_is_admitted() {
    let sources = [
        compile::OwnedSource::compilation_unit(
            "virtual/top.sv",
            "module top; rtl_cell a(); endmodule",
        ),
        compile::OwnedSource::compilation_unit(
            "virtual/rtl.sv",
            "`include \"value.vh\"\nmodule rtl_cell; localparam integer MARK = `VALUE; endmodule",
        ),
        compile::OwnedSource::include("virtual/headers/full/value.vh", "`define VALUE 17\n"),
    ];
    let mut opts = compile::CompileOpts {
        top: Some("top".to_owned()),
        library_order: vec!["rtl".to_owned()],
        library_maps: vec![compile::OwnedSource::include(
            "virtual/root.map",
            "library rtl rtl.sv -incdir headers/empty, headers/full;",
        )],
        ..Default::default()
    };
    let error = compile::compile_sources_checked(&sources, &opts)
        .expect_err("an unrepresented empty directory cannot be selected");
    assert!(
        error
            .to_string()
            .contains("matched no admitted directories"),
        "{error}"
    );

    opts.logical_directories = vec!["virtual/headers/empty".to_owned()];
    let output = compile::compile_sources_checked(&sources, &opts)
        .expect("an admitted empty logical directory is a valid -incdir");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    drop(output);
    let design = model::DesignModel::from_db(&database);
    let mark = design
        .instance("top.a")
        .expect("library instance")
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

/// A03: invalid bind target kinds and a repeated bound instance name reject
/// with specific diagnostics; each fixture has a single fault.
#[test]
fn invalid_bind_targets_reject_with_specific_diagnostics() {
    sim_cli::reject_case(SUITE, "bind_package", "unknown module 'rtl018_pkg'");
    sim_cli::reject_case(
        SUITE,
        "bind_generate",
        "is not a valid bind target; only modules and interfaces are allowed",
    );
    sim_cli::reject_case(
        SUITE,
        "bind_class",
        "'rtl018_class' is not a valid bind target; only modules and interfaces are allowed",
    );
    sim_cli::reject_case(
        SUITE,
        "bind_program",
        "cannot instantiate a module in a program",
    );
    sim_cli::reject_case(
        SUITE,
        "bind_module_into_interface",
        "cannot instantiate a module in an interface",
    );
    sim_cli::reject_case(
        SUITE,
        "bind_duplicate_name",
        "duplicate instance name `probe` in `tb.t`",
    );
}

/// A03 nearest legal form: an interface bound into a module target.
#[test]
fn interface_bound_into_module_executes() {
    sim_cli::run_case(
        SUITE,
        "bind_interface_into_module",
        "watch=8\nwatch=14\n",
        "llg: $finish at time 2000 at tb:20:5\n",
        &[],
    );
}

/// A03: unauthorized or missing filesystem inputs reject before compilation:
/// a compiler `include` in a map, a missing -incdir directory and an include
/// outside every admitted root. Symbolic-link escapes are covered by the
/// SYN-032 suite, which may use platform-conditional tests.
#[test]
fn unauthorized_library_and_include_paths_reject() {
    sim_cli::reject_case_with_inputs(
        SUITE,
        "witness_compiler_include.v",
        &[
            "witness/compiler_include.map",
            "witness/other.map",
            "witness/cell.v",
        ],
        "`include in a library map is unsupported; use a map include declaration",
        &[
            "--edition",
            "2001",
            "--libmap",
            "witness/compiler_include.map",
            "-L",
            "aux",
        ],
    );
    sim_cli::reject_case_with_inputs(
        SUITE,
        "witness_missing_incdir.v",
        &["witness/missing_incdir.map", "witness/cell.v"],
        "library -incdir `nonexistent` matched no directories",
        &[
            "--edition",
            "2001",
            "--libmap",
            "witness/missing_incdir.map",
            "-L",
            "aux",
        ],
    );
    sim_cli::reject_case(
        SUITE,
        "include_escape",
        "'../../../../../Cargo.toml': No such file or directory",
    );
}
