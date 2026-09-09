//! End-to-end simulator coverage for real and unit-suffixed procedural
//! delays. IEEE 1800-2009 §3.14.1 requires delay values to be rounded to the
//! calling design element's time precision before simulation; §5.8 applies
//! that rule to time literals. Statement and intra-assignment forms are run
//! with optimization enabled and disabled against one owned frontend model.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::compile;
use llg::sim;
use llg::sim::opt::OptConfig;

fn compile_database(dir: &std::path::Path, source: &str) -> Result<llg::core::db::Db, String> {
    let tb = dir.join("tb.sv");
    let precision_anchor = dir.join("precision_anchor.sv");
    std::fs::write(&tb, source).map_err(|error| format!("write testbench: {error}"))?;
    std::fs::write(
        &precision_anchor,
        r#"`timescale 1ps/1ps
module precision_anchor(input [7:0] a, input [2:0] marker);
    always @(marker) begin
        if (marker != 0)
            $strobe("global=%0t marker=%0d a=%0d", $time, marker, a);
    end
endmodule
"#,
    )
    .map_err(|error| format!("write precision anchor: {error}"))?;
    let out = compile::compile_checked(&compile::CompileOpts {
        files: vec![
            tb.to_string_lossy().into_owned(),
            precision_anchor.to_string_lossy().into_owned(),
        ],
        top: Some("tb".to_string()),
        ..Default::default()
    })
    .map_err(|error| format!("compile: {error}"))?;
    llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| error.to_string())
}

fn build_and_run(
    dir: &std::path::Path,
    database: &llg::core::db::Db,
    config: &OptConfig,
    name: &str,
) -> Result<String, String> {
    let generated = sim::codegen::generate_from_db_with_opts(database, config)
        .map_err(|error| error.to_string())?;
    let executable =
        sim::build::build_model_cmake(&dir.join(name), &[("model.c", generated.model_c.as_str())])
            .map_err(|error| format!("cmake: {error}"))?;
    sim_harness::run_executable(&executable)
}

/// Local precision is 100ps while another design file makes the scheduler
/// precision 1ps. Accumulating several rounded delays distinguishes required
/// local rounding from incorrectly rounding only to the global tick.
#[test]
fn sim_fractional_and_time_literal_delays_round_locally() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/100ps
module tb;
    reg [7:0] a;
    reg [2:0] marker;
    precision_anchor observer(.a(a), .marker(marker));

    initial begin
        a = 0;
        marker = 0;
        #((0.54)) marker = 1;
        a = #(2.1ns) 7;
        marker = 2;
        a = #2.75 9;
        marker = 3;
        #0.05 marker = 4;
        #0.1;
        $finish;
    end
endmodule
"#;
    let expected = concat!(
        "global=500 marker=1 a=0\n",
        "global=2600 marker=2 a=7\n",
        "global=5400 marker=3 a=9\n",
        "global=5500 marker=4 a=9\n"
    );

    let (optimized, unoptimized) =
        sim_harness::with_frontend_temp_cwd("time-literal-rounding", |dir| {
            let database = compile_database(dir, source)?;
            Ok((
                build_and_run(dir, &database, &OptConfig::default(), "optimized"),
                build_and_run(dir, &database, &OptConfig::none(), "unoptimized"),
            ))
        })
        .expect("time-literal setup");
    assert_eq!(optimized.expect("optimized run"), expected);
    assert_eq!(unoptimized.expect("unoptimized run"), expected);
}

fn codegen_error(source: &str, tag: &str) -> String {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let database = compile_database(dir, source)?;
        match sim::codegen::generate_from_db_with_opts(&database, &OptConfig::default()) {
            Ok(_) => Err("codegen unexpectedly accepted delay".to_string()),
            Err(error) => Ok(error.to_string()),
        }
    })
    .expect("compile and codegen should complete")
}

#[test]
fn sim_scientific_and_real_parameter_delays_round_locally() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/100ps
module tb;
    parameter real P = 0.25;
    parameter real _1e3 = 0.25;
    reg [7:0] a;
    reg [2:0] marker;
    precision_anchor observer(.a(a), .marker(marker));
    initial begin
        a = 0;
        marker = 0;
        #P marker = 1;
        #(1.25e-1) marker = 2;
        #(2E-1) marker = 3;
        a = #(1e0_0) 9;
        marker = 4;
        #((_1e3)) marker = 5;
        #1 $finish;
    end
endmodule
"#;
    let expected = concat!(
        "global=300 marker=1 a=0\n",
        "global=400 marker=2 a=0\n",
        "global=600 marker=3 a=0\n",
        "global=1600 marker=4 a=9\n",
        "global=1900 marker=5 a=9\n",
    );
    sim_harness::with_frontend_temp_cwd("scientific-real-delay", |dir| {
        let database = compile_database(dir, source)?;
        for (name, options) in [("off", OptConfig::none()), ("on", OptConfig::default())] {
            assert_eq!(
                build_and_run(dir, &database, &options, name)?,
                expected,
                "{name}"
            );
        }
        Ok(())
    })
    .expect("scientific and real parameter delays");
}

#[test]
fn sim_negative_real_parameter_delay_is_rejected() {
    let source = r#"module tb;
    parameter real P = -0.25;
    initial #P $finish;
endmodule
"#;
    let error = codegen_error(source, "negative-real-delay");
    assert!(error.contains("finite and nonnegative"), "{error}");
}

#[test]
fn sim_local_variable_shadows_real_delay_parameter() {
    let source = include_str!("fixtures/sim/time_literals/shadowed_real_delay_local.sv");
    let error = codegen_error(source, "shadowed-real-delay");
    assert!(
        error.contains("real/shortreal procedural variable `P` is not supported in `tb`"),
        "{error}"
    );
}

#[test]
fn sim_task_argument_delay_is_rejected_as_runtime_valued() {
    let source = r#"module tb;
    parameter real P = 0.25;
    task t(input integer P);
        #P;
    endtask
    initial begin
        t(1);
        $finish;
    end
endmodule
"#;
    let error = codegen_error(source, "shadowed-task-delay");
    assert!(
        error.contains("procedural delay") && error.contains("runtime-valued"),
        "{error}"
    );
}

/// A typed expression containing multiple unit-suffixed literals lowers
/// without source-text reconstruction.
#[test]
fn sim_time_literal_expression_is_supported() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/100ps
module tb;
    initial begin
        #(1ns + 1ns) $display("time=%0t", $time);
        $finish;
    end
endmodule
"#;
    sim_harness::with_frontend_temp_cwd("time-literal-expression", |dir| {
        let database = compile_database(dir, source)?;
        assert_eq!(
            build_and_run(dir, &database, &OptConfig::default(), "optimized")?,
            "time=2\n"
        );
        assert_eq!(
            build_and_run(dir, &database, &OptConfig::none(), "unoptimized")?,
            "time=2\n"
        );
        Ok(())
    })
    .expect("typed time-literal expression must execute");
}

/// Scaling a syntactically valid time literal must fail cleanly when its
/// exact physical value cannot fit the bounded compile-time representation.
#[test]
fn sim_time_literal_overflow_is_rejected() {
    let source = r#"`timescale 1ns/1ps
module tb;
    initial #18446744073709551615s $finish;
endmodule
"#;
    let error = codegen_error(source, "time-literal-overflow");
    assert!(
        error.contains("exceeds the supported range")
            || error.contains("exceeds the 64-bit tick range"),
        "unexpected codegen error: {error}"
    );
}
