//! End-to-end simulator coverage for SystemVerilog time literals in value
//! expressions. IEEE 1800-2009 §5.8 defines them as realtime values scaled
//! to the calling design element's unit and rounded to its precision.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::compile;
use llg::core::db::{ConstantSource, ExprKind, NodeKind};
use llg::sim;
use llg::sim::opt::OptConfig;

fn compile_database(dir: &std::path::Path, source: &str) -> Result<llg::core::db::Db, String> {
    let testbench = dir.join("tb.sv");
    std::fs::write(&testbench, source).map_err(|error| format!("write testbench: {error}"))?;
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![testbench.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .map_err(|error| format!("compile: {error}"))?;
    llg::core::db::Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())
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
        sim::build::build_model_cmake(&dir.join(name), &[("model.c", &generated.model_c)])
            .map_err(|error| format!("cmake: {error}"))?;
    sim_harness::run_executable(&executable)
}

fn run_optimized_and_unoptimized(source: &str, tag: &str) -> (String, String) {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let database = compile_database(dir, source)?;
        Ok((
            build_and_run(dir, &database, &OptConfig::default(), "optimized")?,
            build_and_run(dir, &database, &OptConfig::none(), "unoptimized")?,
        ))
    })
    .expect("time-value simulation")
}

#[test]
fn sim_time_literals_scale_round_and_coerce_in_value_expressions() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/100ps
module tb;
    real rounded;
    time packed_time;
    logic [7:0] packed_bits;
    initial begin
        rounded = 2.14ns;
        packed_time = 2.6ns;
        packed_bits = 250ps;
        $display("values=%.2f %0d %0d %.2f", rounded, packed_time,
                 packed_bits, 2.14ns + 400ps);
        $finish;
    end
endmodule
"#;
    // IEEE 1800-2009 §5.8 rounds each time literal to the local precision
    // before ordinary expression use. Assignment to `time` still applies
    // integral conversion after that local rounding.
    let expected = "values=2.10 3 0 2.50\n";
    let (optimized, unoptimized) = run_optimized_and_unoptimized(source, "time-values");
    assert_eq!(optimized, expected);
    assert_eq!(unoptimized, expected);
}

#[test]
fn sim_time_literal_uses_calling_module_unit() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1s/100ms
module tb;
    real scaled;
    real large_value;
    time rounded;
    initial begin
        scaled = 2500ms;
        large_value = 20000000s;
        rounded = 2500ms;
        $display("scaled=%.1f large=%.0f rounded=%0d", scaled, large_value, rounded);
        $finish;
    end
endmodule
"#;
    let expected = "scaled=2.5 large=20000000 rounded=3\n";
    let (optimized, unoptimized) = run_optimized_and_unoptimized(source, "time-value-unit");
    assert_eq!(optimized, expected);
    assert_eq!(unoptimized, expected);
}

#[test]
fn sim_time_literal_source_is_owned_after_frontend_capture() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"module tb;
    real value;
    initial begin
        value = 2.1ns;
        $display("owned=%.1f", value);
        $finish;
    end
endmodule
"#;
    let (optimized, unoptimized) =
        sim_harness::with_frontend_temp_cwd("time-value-owned-source", |dir| {
            let database = compile_database(dir, source)?;
            std::fs::remove_file(dir.join("tb.sv"))
                .map_err(|error| format!("remove captured source: {error}"))?;
            Ok((
                build_and_run(dir, &database, &OptConfig::default(), "optimized")?,
                build_and_run(dir, &database, &OptConfig::none(), "unoptimized")?,
            ))
        })
        .expect("owned time-literal source simulation");
    assert_eq!(optimized, "owned=2.1\n");
    assert_eq!(unoptimized, "owned=2.1\n");
}

#[test]
fn sim_time_literal_parameter_initializer_uses_typed_value() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/100ps
module tb;
    localparam time VALUE = 2.1ns;
    initial begin
        $display("param=%0d", VALUE);
        $finish;
    end
endmodule
"#;
    let (optimized, unoptimized) = run_optimized_and_unoptimized(source, "time-value-param");
    assert_eq!(optimized, "param=2\n");
    assert_eq!(unoptimized, "param=2\n");
}

#[test]
fn sim_time_literal_declaration_initializer_uses_typed_value() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/100ps
module tb;
    real value = 2.1ns;
    initial begin
        $display("initializer=%.1f", value);
        $finish;
    end
endmodule
"#;
    let (optimized, unoptimized) = run_optimized_and_unoptimized(source, "time-value-initializer");
    assert_eq!(optimized, "initializer=2.1\n");
    assert_eq!(unoptimized, "initializer=2.1\n");
}

#[test]
fn sim_macro_time_literal_uses_typed_value() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/100ps
`define VALUE 2.1ns
module tb;
    real value;
    initial begin
        value = `VALUE;
        $display("macro=%.1f", value);
        $finish;
    end
endmodule
"#;
    let (optimized, unoptimized) = run_optimized_and_unoptimized(source, "time-value-macro");
    assert_eq!(optimized, "macro=2.1\n");
    assert_eq!(unoptimized, "macro=2.1\n");
}

#[test]
fn sim_default_database_build_does_not_read_time_literal_source() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let stdout = sim_harness::with_frontend_temp_cwd("time-value-default-db", |dir| {
        let testbench = dir.join("tb.sv");
        std::fs::write(
            &testbench,
            "module tb; real value; initial begin value = 2.1ns; \
             $display(\"default=%.1f\", value); $finish; end endmodule\n",
        )
        .map_err(|error| format!("write testbench: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![testbench.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database =
            llg::core::db::Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        std::fs::remove_file(&testbench)
            .map_err(|error| format!("remove captured source: {error}"))?;
        build_and_run(dir, &database, &OptConfig::default(), "default-db")
    })
    .expect("default database build");
    assert_eq!(stdout, "default=2.1\n");
}

#[test]
fn sim_logical_line_remap_does_not_replace_physical_literal_source() {
    let source = r#"`timescale 1ns/100ps
module tb;
    real value;
`line 100 "/not/an/admitted/source.sv" 0
    initial value = 2.1ns;
endmodule
"#;
    sim_harness::with_frontend_temp_cwd("time-value-line-remap", |dir| {
        let database = compile_database(dir, source)?;
        assert!(database
            .nodes()
            .iter()
            .all(|node| { node.file() != Some("/not/an/admitted/source.sv") }));
        assert!(database.nodes().iter().any(|node| {
            matches!(
                node.kind(),
                NodeKind::Expr(ExprKind::Constant {
                    source: ConstantSource::Exact(source),
                    ..
                }) if source == "2.1ns"
            )
        }));
        sim::codegen::generate_from_db_with_opts(&database, &OptConfig::default())
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .expect("logical line remap should retain admitted physical source");
}
