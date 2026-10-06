//! SYN-013 finite zero-time subroutine, lifetime, and reference evidence.

use crate::sim_cli;

#[test]
fn zero_time_calls_preserve_fixed_values_lifetimes_and_references() {
    sim_cli::run_case(
        "syn013_zero_time_calls",
        "zero_time_calls",
        "default=4,9,10 row=5,6,7 snapshot=4,6,7 forwarded=17 selected=7 workers=6,33\n",
        "",
        &[],
    );
}

#[test]
fn verilog_2001_zero_time_calls_preserve_automatic_activations() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            "syn013_zero_time_calls",
            "legacy_calls",
            "legacy value=40 result=42 calls=2\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn static_local_array_nba_publishes_after_task_return() {
    sim_cli::run_case_with_args(
        "syn013_zero_time_calls",
        "static_array_nba",
        "value=a5\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn legacy_static_task_memory_nba_runs_in_both_editions() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            "syn013_zero_time_calls",
            "legacy_static_array_nba",
            "legacy=5a\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn static_array_rows_slices_instances_and_wakeups() {
    sim_cli::run_case_with_args(
        "syn013_zero_time_calls",
        "static_array_matrix",
        "first=23/21 second=43 wakeups=4/2\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn static_array_delayed_nbas_publish_in_time_and_issue_order() {
    sim_cli::run_case_with_args(
        "syn013_zero_time_calls",
        "static_array_delayed_nba",
        "ordered=22 delayed=44\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn static_function_arrays_keep_issued_nbas_and_previous_values() {
    sim_cli::run_case_with_args(
        "syn013_zero_time_calls",
        "static_function_array_nba",
        "static=46 explicit=64 old=35/53\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn explicit_static_array_in_automatic_task_publishes_after_return() {
    sim_cli::run_case_with_args(
        "syn013_zero_time_calls",
        "explicit_static_array_nba",
        "value=67\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn automatic_array_and_formal_nbas_remain_rejected() {
    for fixture in [
        "automatic_array_nba_rejected",
        "automatic_formal_nba_rejected",
    ] {
        sim_cli::reject_case_with_args(
            "syn013_zero_time_calls",
            fixture,
            "nonblocking assignment to automatic variable",
            &["--edition", "sv2009"],
        );
    }
}

#[test]
fn static_array_lifetime_survives_owned_import_and_native_snapshot_drop() {
    use llg::core::{
        compile,
        db::{Db, NodeKind, VariableLifetime},
    };
    use llg::sim::{codegen, opt::OptConfig};

    let database = {
        let compiled = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "static_array_nba.sv",
                include_str!("fixtures/sim/syn013_zero_time_calls/static_array_nba.sv"),
            )],
            &compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .expect("static task array is legal");
        Db::from_slang(&compiled.snapshot).expect("owned static array capture")
    };
    database.validate().expect("owned relationships");
    let arrays = database
        .node_ids()
        .filter(|node| {
            matches!(database.node_kind(*node), NodeKind::Array { .. })
                && database.node(*node).name == "data"
        })
        .collect::<Vec<_>>();
    assert_eq!(arrays.len(), 1, "one task-local array declaration");
    assert_eq!(
        database.variable_lifetime(arrays[0]),
        VariableLifetime::Static
    );

    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("static array NBA lowers after native snapshot destruction");
    }
}
