//! End-to-end simulator coverage for fixed-point and unit-suffixed procedural
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
    let design = out.uhdm_design().ok_or("no UHDM design")?;
    llg::core::db::Db::build(design).map_err(|error| error.to_string())
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
        sim_harness::with_surelog_temp_cwd("time-literal-rounding", |dir| {
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
    sim_harness::with_surelog_temp_cwd(tag, |dir| {
        let database = compile_database(dir, source)?;
        match sim::codegen::generate_from_db_with_opts(&database, &OptConfig::default()) {
            Ok(_) => Err("codegen unexpectedly accepted delay".to_string()),
            Err(error) => Ok(error.to_string()),
        }
    })
    .expect("compile and codegen should complete")
}

/// General arithmetic over real/time literals remains outside this literal
/// slice and is rejected instead of being evaluated with incomplete context.
#[test]
fn sim_time_literal_expression_is_rejected() {
    let source = r#"`timescale 1ns/100ps
module tb;
    initial #(1ns + 1ns) $finish;
endmodule
"#;
    let error = codegen_error(source, "time-literal-expression");
    assert!(
        error.contains("unsupported token"),
        "unexpected codegen error: {error}"
    );
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
