//! RTL-019: source provenance and exact edition admission.
//!
//! Mapped `` `__FILE__``/`` `__LINE__`` values, runtime and diagnostic
//! locations are counted by hand from the physical fixture lines (see the
//! fixture comments and `readme.md`). Edition negatives each hold one later
//! form inside otherwise legal source; `sv_forms_2009.sv` executes all of them
//! under SystemVerilog-2009.

use super::sim_cli;
use llg::core::{compile, db};
use llg::sim::semantic::SemanticModel;
use std::path::{Path, PathBuf};

const SUITE: &str = "feature_completion/rtl_019";

fn fixture(name: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(name);
    assert!(path.is_file(), "missing fixture input: {}", path.display());
    path.to_string_lossy().into_owned()
}

/// Value-backend lanes: legacy, compact portable and compact GMP (bundled
/// unless `gmp` names an installation).
fn backend_lanes(gmp: &str) -> Vec<Vec<(&'static str, String)>> {
    vec![
        vec![
            ("LLG_DEV_VALUE_BACKEND", "legacy".to_owned()),
            ("LLG_DEV_COMPACT_KERNELS", "portable".to_owned()),
        ],
        vec![
            ("LLG_DEV_VALUE_BACKEND", "compact".to_owned()),
            ("LLG_DEV_COMPACT_KERNELS", "portable".to_owned()),
        ],
        vec![
            ("LLG_DEV_VALUE_BACKEND", "compact".to_owned()),
            ("LLG_DEV_COMPACT_KERNELS", "gmp".to_owned()),
            ("GMP_ROOT", gmp.to_owned()),
        ],
    ]
}

const LINE_MAP_STDERR: &str = "llg: $finish at time 0 at tb:14:5\n";

/// A01: `__FILE__`/`__LINE__` through nested includes, their restoration
/// after each include, `` `line`` in the top file and in headers, and a macro
/// body expanding `__LINE__` at its use site.
#[test]
fn mapped_file_and_line_values_follow_includes_and_line_directives() {
    let gmp = super::sim_harness::test_gmp_root();
    for lane in backend_lanes(&gmp) {
        let envs: Vec<(&str, &str)> = lane.iter().map(|(k, v)| (*k, v.as_str())).collect();
        sim_cli::run_case_with_inputs(
            SUITE,
            "line_map",
            &["line_map_outer.svh", "line_map_inner.svh"],
            include_str!("../fixtures/sim/feature_completion/rtl_019/line_map.out"),
            LINE_MAP_STDERR,
            &[],
            &envs,
        );
    }
    // IEEE 1364-2001 19.7 has `line but not the predefined macros.
    sim_cli::reject_case_with_args(SUITE, "line_map", "`__FILE__`", &["--edition", "v2001"]);
}

/// A01 after snapshot destruction: the mapped values are compiled into the
/// model, which is generated and built after the Db is dropped.
#[test]
fn mapped_values_execute_after_snapshot_and_db_destruction() {
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        "line_map",
        compile::CompileOpts {
            files: vec![fixture("line_map.sv")],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        include_str!("../fixtures/sim/feature_completion/rtl_019/line_map.out"),
        LINE_MAP_STDERR,
    );
}

/// A01: `` `line`` state belongs to its file. The peer file's directive
/// does not reach the next file in either compilation-unit mode.
#[test]
fn line_state_stays_in_its_file_in_both_unit_modes() {
    for mode in ["separate", "merged"] {
        sim_cli::run_case_with_inputs(
            SUITE,
            "line_units",
            &["line_units_peer.sv"],
            include_str!("../fixtures/sim/feature_completion/rtl_019/line_units.out"),
            "llg: $finish at time 0 at tb:9:5\n",
            &["--compilation-units", mode, "line_units_peer.sv"],
            &[],
        );
    }
    sim_cli::reject_case_with_inputs(
        SUITE,
        "line_units",
        &["line_units_peer.sv"],
        "`__FILE__`",
        &["--edition", "v2001", "line_units_peer.sv"],
    );
}

fn line_locations_stderr() -> String {
    // Physical line 18 follows `line 40 at line 13, so it is logical 44.
    // Scope-based runtime locations stay physical (tb.u0:4:3 is line 4 of
    // line_task.svh; tb:16:5 is physical although it is logical 42).
    format!(
        "llg: assertion assert failed: {}:18:3 (`line orig_tb.sv:44) (a_never)\n\
         llg: severity error: tb.u0:4:3: late 1\n\
         llg: severity error: tb.u1:4:3: late 2\n\
         llg: severity error: tb:16:5: resumed 3\n\
         llg: $finish at time 6000 at tb:22:8\n",
        super::sim_harness::source_display(Path::new(&fixture("line_locations.sv")))
    )
}

/// A01: errors from tasks resumed after a delay (two instances of an
/// included task, and a forked task in a `` `line`` region) and a concurrent
/// assertion failure keep their HDL locations on every value backend.
#[test]
fn resumed_task_and_assertion_locations_survive_includes_and_line() {
    let stderr = line_locations_stderr();
    let gmp = super::sim_harness::test_gmp_root();
    for lane in backend_lanes(&gmp) {
        let envs: Vec<(&str, &str)> = lane.iter().map(|(k, v)| (*k, v.as_str())).collect();
        sim_cli::run_case_with_inputs(
            SUITE,
            "line_locations",
            &["line_task.svh"],
            "",
            &stderr,
            &[],
            &envs,
        );
    }
    sim_cli::reject_case_with_args(
        SUITE,
        "line_locations",
        "`$error` is not available in IEEE 2001",
        &["--edition", "v2001"],
    );
}

/// A01: the same locations after the snapshot and then the Db are dropped.
#[test]
fn resumed_task_locations_execute_after_snapshot_and_db_destruction() {
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        "line_locations",
        compile::CompileOpts {
            files: vec![fixture("line_locations.sv")],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        "",
        &line_locations_stderr(),
    );
}

/// A01: frontend errors in an included file and in a macro body name the
/// header's and the use site's physical positions in both editions.
#[test]
fn include_and_macro_errors_name_their_physical_source() {
    for edition in ["v2001", "sv2009"] {
        let args = ["--edition", edition];
        sim_cli::reject_case_with_args(
            SUITE,
            "include_error",
            "include_error.svh:3:24 use of undeclared identifier 'undeclared_in_header'",
            &args,
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "macro_error",
            "macro_error.sv:7:3 use of undeclared identifier 'undeclared_in_macro'",
            &args,
        );
    }
}

/// A01: simulator (owned Db) semantic errors keep the physical position of
/// an included header, and append the `` `line`` position when one applies.
#[test]
fn owned_errors_keep_physical_and_mapped_positions() {
    sim_cli::reject_case(SUITE, "owned_error_include", "owned_error.svh:4:5");
    sim_cli::reject_case(
        SUITE,
        "owned_error_line",
        "owned_error_line.sv:7:39 (`line orig_rtl.sv:40)",
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "owned_error_line",
        "use of undeclared identifier 'logic'",
        &["--edition", "v2001"],
    );
}

/// A01 component check: physical node positions and the separate logical
/// map survive snapshot destruction; semantic origins carry both.
#[test]
fn component_logical_positions_survive_snapshot_destruction() {
    let output = compile::compile_checked(&compile::CompileOpts {
        files: vec![fixture("line_map.sv")],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("line_map compiles");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    drop(output);
    let position = |file: &str, line: u32| {
        let id = database
            .node_ids()
            .find(|id| {
                let node = database.node(*id);
                node.line == line && node.file.as_deref().is_some_and(|f| f.ends_with(file))
            })
            .unwrap_or_else(|| panic!("a node at {file}:{line}"));
        (
            id,
            database
                .logical_position(id)
                .map(|p| (p.file.to_owned(), p.line)),
        )
    };
    assert_eq!(position("line_map.sv", 7).1, None);
    assert_eq!(
        position("line_map.sv", 11).1,
        Some(("mapped_top.sv".to_owned(), 300))
    );
    assert_eq!(
        position("line_map.sv", 13).1,
        Some(("mapped_top.sv".to_owned(), 302))
    );
    assert_eq!(position("line_map_outer.svh", 4).1, None);
    assert_eq!(
        position("line_map_outer.svh", 6).1,
        Some(("mapped_outer.sv".to_owned(), 70))
    );
    assert_eq!(
        position("line_map_inner.svh", 3).1,
        Some(("mapped_inner.sv".to_owned(), 9))
    );
    let (mapped, _) = position("line_map.sv", 11);
    let model = SemanticModel::from_db(&database);
    let origin = model.origins()[mapped.index()].location();
    assert!(
        origin.ends_with(":11:5 (`line mapped_top.sv:300)"),
        "{origin}"
    );
}

/// A02: every form the strict 2001 profile rejects executes under 2009 on
/// every value backend; the same file rejects under 2001.
#[test]
fn later_forms_execute_in_2009_and_reject_in_2001() {
    let gmp = super::sim_harness::test_gmp_root();
    for lane in backend_lanes(&gmp) {
        let envs: Vec<(&str, &str)> = lane.iter().map(|(k, v)| (*k, v.as_str())).collect();
        sim_cli::run_case_with_inputs(
            SUITE,
            "sv_forms_2009",
            &[],
            include_str!("../fixtures/sim/feature_completion/rtl_019/sv_forms_2009.out"),
            "llg: $finish at time 3000 at tb:53:5\n",
            &[],
            &envs,
        );
    }
    sim_cli::reject_case_with_args(
        SUITE,
        "sv_forms_2009",
        "`implicit named port connection` is not available in IEEE 2001",
        &["--edition", "v2001"],
    );
}

/// A02: the nearest legal 2001 neighbours of every new gate, including the
/// memory-storage arguments of `$readmemh` and `$fread` and a `` `line``
/// directive, execute in both editions.
#[test]
fn legacy_neighbours_and_memory_storage_arguments_stay_admitted() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "legacy_forms_2001.v",
            include_str!("../fixtures/sim/feature_completion/rtl_019/legacy_forms_2001.out"),
            "llg: $finish at time 1000 at tb:57:5\n",
            &[],
            &["--edition", edition],
        );
    }
}

/// A02: one later form per fixture rejects under 2001 with a source-located
/// strict-edition diagnostic (`file:line:col `label``).
#[test]
fn single_later_forms_reject_in_2001() {
    let args = ["--edition", "v2001"];
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_queue.v",
        "neg_2001_queue.v:3:20 `dynamic, associative or queue array` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_dynamic.v",
        "neg_2001_dynamic.v:3:20 `dynamic, associative or queue array` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_assoc.v",
        "neg_2001_assoc.v:3:20 `dynamic, associative or queue array` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_packed_dims.v",
        "neg_2001_packed_dims.v:3:27 `multiple packed dimensions` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_size_dim.v",
        "neg_2001_size_dim.v:3:22 `unpacked dimension size` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_end_label.v",
        "neg_2001_end_label.v:3:39 `end label` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_module_label.v",
        "neg_2001_module_label.v:3:29 `end label` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_dot_name.v",
        "neg_2001_dot_name.v:4:29 `implicit named port connection` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_default_arg.v",
        "neg_2001_default_arg.v:3:31 `default subroutine argument` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_function_output.v",
        "neg_2001_function_output.v:3:58 `function output or inout argument` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_function_no_input.v",
        "neg_2001_function_no_input.v:3:27 `function without an input argument` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_task_statements.v",
        "neg_2001_task_statements.v:3:30 `multiple statements in a subroutine body` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_empty_task.v",
        "neg_2001_empty_task.v:3:17 `empty subroutine body` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_empty_parens.v",
        "neg_2001_empty_parens.v:3:17 `empty subroutine argument list` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_empty_call.v",
        "neg_2001_empty_call.v:3:57 `empty subroutine argument list` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_block_init.v",
        "neg_2001_block_init.v:3:40 `procedural declaration initializer` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_unnamed_decl.v",
        "neg_2001_unnamed_decl.v:3:20 `declaration in an unnamed block` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_edge_event.v",
        "neg_2001_edge_event.v:3:30 `edge event control` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_loop_genvar.v",
        "neg_2001_loop_genvar.v:3:26 `genvar declaration in a generate loop` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_generate_region.v",
        "neg_2001_generate_region.v:3:60 `generate construct outside a generate region` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_localparam_port.v",
        "neg_2001_localparam_port.v:3:30 `localparam in a parameter port list` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_cast.v",
        "neg_2001_cast.v:3:66 `cast` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_time_literal.v",
        "neg_2001_time_literal.v:3:27 `time literal` is not available in IEEE 2001",
        &args,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_2001_statement_label.v",
        "neg_2001_statement_label.v:3:20 `statement label` is not available in IEEE 2001",
        &args,
    );
}

/// A02 component check: each single-form 2001 negative is otherwise legal
/// SystemVerilog-2009, so the rejection is the edition gate alone.
#[test]
fn component_single_later_forms_compile_in_2009() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE);
    let mut checked = 0;
    for entry in std::fs::read_dir(&directory).expect("fixture directory") {
        let path = entry.expect("fixture entry").path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !(name.starts_with("neg_2001_") && name.ends_with(".v")) {
            continue;
        }
        let file = path.to_string_lossy().into_owned();
        for (edition, accepted) in [
            (compile::LanguageEdition::SystemVerilog2009, true),
            (compile::LanguageEdition::Verilog2001, false),
        ] {
            let result = compile::compile_checked(&compile::CompileOpts {
                files: vec![file.clone()],
                top: Some("tb".to_owned()),
                edition,
                ..Default::default()
            });
            assert_eq!(result.is_ok(), accepted, "{name} under {edition}");
        }
        checked += 1;
    }
    assert_eq!(checked, 24);
}

/// A02: post-2009 covergroup bins forms reject under 2009; the 2009 bins
/// forms beside them stay admitted and execute.
#[test]
fn post_2009_bins_forms_reject_and_2009_bins_execute() {
    sim_cli::reject_case(
        SUITE,
        "neg_2012_bins_with",
        "neg_2012_bins_with.sv:7:20 `covergroup bins with or matches clause` is not available in IEEE 2009",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_2012_cross_with",
        "neg_2012_cross_with.sv:9:23 `covergroup bins with or matches clause` is not available in IEEE 2009",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_2012_bins_set",
        "neg_2012_bins_set.sv:8:20 `covergroup set-expression bins` is not available in IEEE 2009",
    );
    sim_cli::run_case(
        SUITE,
        "cover_bins_2009",
        "bins admitted\n",
        "llg: $finish at time 0 at tb:20:5\n",
        &[],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "cover_bins_2009",
        "use of undeclared identifier 'bit'",
        &["--edition", "v2001"],
    );
}

/// Adopted FND-002 witnesses: `$assertcontrol` (IEEE 1800-2012) and a ref
/// formal of a static task (IEEE 1800-2009 13.5.2) block compilation.
#[test]
fn adopted_post_2009_and_static_ref_witnesses_reject() {
    sim_cli::reject_case(
        SUITE,
        "witness_assertcontrol",
        "witness_assertcontrol.sv:8:1 `$assertcontrol` is not available in IEEE 2009",
    );
    sim_cli::reject_case(
        SUITE,
        "witness_ref_static",
        "witness_ref_static.sv:6:15 'ref' arguments can only be used in 'automatic' subroutines",
    );
}
