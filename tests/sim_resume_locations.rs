use llg::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use llg::core::db::Db;
use llg::sim::codegen::{generate_from_db_with_codegen_options, CodegenOptions};
use llg::sim::execution::ExecutionAnalysisOptions;
use llg::sim::opt::OptConfig;

#[path = "support/sim.rs"]
mod sim_harness;

fn database(sources: &[OwnedSource]) -> Db {
    let compiled = compile_sources_checked(
        sources,
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("resume-location source compiles");
    Db::from_slang(&compiled.snapshot).expect("owned resume-location database")
}

fn render(database: &Db, optimization: OptConfig) -> String {
    generate_from_db_with_codegen_options(
        database,
        &CodegenOptions {
            optimization,
            ..Default::default()
        },
    )
    .expect("resume-location source emits C")
    .model_c
}

fn site_tables(c: &str) -> Vec<Vec<String>> {
    c.split("static const llg_co_site_t ")
        .skip(1)
        .map(|table| {
            table
                .split_once("};")
                .expect("site table terminator")
                .0
                .lines()
                .filter_map(|line| line.trim().strip_suffix(" },"))
                .map(|line| {
                    serde_json::from_str(line.rsplit_once(", ").expect("site location field").1)
                        .expect("plain source-location string")
                })
                .collect()
        })
        .collect()
}

fn assert_table(c: &str, expected: &[&str]) {
    assert!(
        site_tables(c).iter().any(|table| table
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())),
        "missing site locations {expected:?}: {:?}",
        site_tables(c)
    );
}

#[test]
fn waits_calls_forks_and_intra_assignment_delays_keep_individual_locations() {
    let database = database(&[OwnedSource::compilation_unit(
        "sites.sv",
        "module tb;\n\
task automatic pause;\n\
    #1;\n\
    #2;\n\
endtask\n\
logic ready;\n\
initial begin\n\
    #3;\n\
    pause();\n\
    @(ready);\n\
    pause();\n\
    fork\n\
        #4;\n\
        #5;\n\
    join\n\
    ready = #6 1;\n\
end\n\
endmodule\n",
    )]);
    for optimization in [OptConfig::none(), OptConfig::default()] {
        let c = render(&database, optimization);
        assert_table(&c, &["sites.sv:3:1", "sites.sv:4:1"]);
        assert_table(
            &c,
            &[
                "sites.sv:8:1",
                "sites.sv:9:1",
                "sites.sv:10:1",
                "sites.sv:11:1",
                "sites.sv:12:1",
                "sites.sv:16:1",
            ],
        );
        assert_table(&c, &["sites.sv:13:1"]);
        assert_table(&c, &["sites.sv:14:1"]);
    }
}

#[test]
fn pruning_and_inline_event_tasks_keep_definition_site_locations() {
    let database = database(&[OwnedSource::compilation_unit(
        "inline.sv",
        "module tb;\n\
event ready;\n\
task automatic await_event(inout event e);\n\
    @(e);\n\
    #2;\n\
endtask\n\
initial begin\n\
    if (1) begin\n\
        #3;\n\
        await_event(ready);\n\
    end else #99;\n\
    #4;\n\
    await_event(ready);\n\
end\n\
endmodule\n",
    )]);
    let unoptimized = render(&database, OptConfig::none());
    assert_table(
        &unoptimized,
        &[
            "inline.sv:9:1",
            "inline.sv:4:1",
            "inline.sv:5:1",
            "inline.sv:11:10",
            "inline.sv:12:1",
            "inline.sv:4:1",
            "inline.sv:5:1",
        ],
    );
    let optimized = render(&database, OptConfig::default());
    assert_table(
        &optimized,
        &[
            "inline.sv:9:1",
            "inline.sv:4:1",
            "inline.sv:5:1",
            "inline.sv:12:1",
            "inline.sv:4:1",
            "inline.sv:5:1",
        ],
    );
}

#[test]
fn included_and_macro_expanded_waits_use_owned_physical_locations() {
    let database = database(&[
        OwnedSource::compilation_unit(
            "/virtual/top.sv",
            "module tb;\ninitial begin\n`include \"body.svh\"\nend\nendmodule\n",
        ),
        OwnedSource::include("/virtual/body.svh", "`define PAUSE #1;\n`PAUSE\n#2;\n"),
    ]);
    for optimization in [OptConfig::none(), OptConfig::default()] {
        assert_table(
            &render(&database, optimization),
            &["/virtual/body.svh:2:1", "/virtual/body.svh:3:1"],
        );
    }
}

#[test]
fn shared_instance_bodies_keep_distinct_site_tables() {
    let database = database(&[OwnedSource::compilation_unit(
        "instances.sv",
        "module worker;\n\
initial begin\n\
    #1;\n\
    #2;\n\
end\n\
endmodule\n\
module tb;\n\
worker a(), b(), c(), d();\n\
endmodule\n",
    )]);
    for optimization in [OptConfig::none(), OptConfig::default()] {
        let c = render(&database, optimization);
        assert_eq!(c.matches("LLG_CO_DISPATCH_BEGIN").count(), 1, "{c}");
        let tables = site_tables(&c);
        assert_eq!(tables.len(), 4, "{c}");
        for table in tables {
            assert_eq!(table, ["instances.sv:3:1", "instances.sv:4:1"]);
        }
    }
}

#[test]
fn generated_runtime_backtrace_reports_the_resumed_wait_and_call_sites() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("tests/fixtures/sim/coroutine_semantics/resume_locations.sv");
    let source = std::fs::read_to_string(&path).expect("checked-in diagnostic source");
    let database = database(&[OwnedSource::compilation_unit("resume_locations.sv", source)]);
    sim_harness::with_temp_cwd("resume-locations", |directory| {
        for (name, optimization, poll_depth_max) in [
            ("default-polled", OptConfig::default(), 3),
            ("no-opt-anchored", OptConfig::none(), 0),
        ] {
            let mut c = generate_from_db_with_codegen_options(
                &database,
                &CodegenOptions {
                    optimization,
                    execution: ExecutionAnalysisOptions {
                        poll_depth_max,
                        ..Default::default()
                    },
                },
            )
            .map_err(|error| error.to_string())?
            .model_c;
            let arm = c
                .lines()
                .find(|line| line.contains("LLG_CO_AWAIT(co, ch, 2, llg_arm_time("))
                .expect("second task delay await")
                .to_owned();
            assert_eq!(c.matches(&arm).count(), 1, "{c}");
            c = c.replace(
                &arm,
                &format!("{arm}\nllg_rt_co_bad_state(co, \"resume location probe\");"),
            );
            let executable = llg::sim::build::build_model_cmake(
                &directory.join(name),
                &[("model.c", c.as_str())],
            )
            .map_err(|error| error.to_string())?;
            let output = sim_harness::run_command(
                &mut std::process::Command::new(executable),
                std::time::Duration::from_secs(60),
            )?;
            assert!(!output.status.success(), "{name}: {output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("coroutine HDL backtrace:"), "{stderr}");
            assert!(
                stderr.contains("tb.initial at resume_locations.sv:8:5"),
                "{stderr}"
            );
            assert!(
                stderr.contains("tb.pause at resume_locations.sv:4:5"),
                "{stderr}"
            );
        }
        Ok(())
    })
    .expect("generated diagnostic model runs");
}
