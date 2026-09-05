//! End-to-end IEEE 1800-2009 §20.5 real/integer conversion tests.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

#[test]
fn real_conversion_functions_match_truncation_and_ieee_bits() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"module tb;
    integer signed_value;
    reg [31:0] unsigned_value;
    reg [63:0] wide_unsigned;
    reg signed [63:0] wide_signed;
    reg [63:0] bits;
    real value;
    shortreal short_value;

    initial begin
        signed_value = -7;
        unsigned_value = 32'hffffffff;
        wide_unsigned = 64'hffffffffffffffff;
        wide_signed = 64'hffffffffffffffff;
        $display("rtoi=%0d %0d %0d %0d", $rtoi(3.9), $rtoi(-3.9), $rtoi(2.5), $rtoi(-2.5));
        $display("itor=%.1f %.1f", $itor(signed_value), $itor(unsigned_value));
        $display("itor-wide=%.1f %.1f", $itor(wide_unsigned), $itor(wide_signed));
        $display("coerce=%0d %.1f %h %h", $rtoi(9), $itor(1.9),
                 $realtobits(1), $shortrealtobits(1));

        value = 0.1;
        bits = $realtobits(value);
        $display("bits=%h width=%0d", bits, $bits($realtobits(value)));
        value = $bitstoreal(64'h400921fb54442d18);
        $display("pi=%.15f roundtrip=%h", value, $realtobits($bitstoreal(bits)));
        short_value = $bitstoshortreal(32'h3f800001);
        $display("short=%.9f bits=%h", short_value, $shortrealtobits(short_value));
        $finish;
    end
endmodule
"#;
    let expected = "rtoi=3 -3 2 -2\nitor=-7.0 4294967295.0\nitor-wide=18446744073709551616.0 -1.0\ncoerce=9 2.0 3ff0000000000000 3f800000\nbits=3fb999999999999a width=64\npi=3.141592653589793 roundtrip=3fb999999999999a\nshort=1.000000119 bits=3f800001\n";

    sim_harness::with_surelog_temp_cwd("real_conversions", |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::build(compiled.uhdm_design().ok_or("no design")?)
            .map_err(|error| error.to_string())?;

        for (variant, opts) in [("on", OptConfig::default()), ("off", OptConfig::none())] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &opts)
                .map_err(|error| error.to_string())?;
            let exe =
                sim::build::build_model_cmake(&dir.join(variant), &[("model.c", &model.model_c)])
                    .map_err(|error| error.to_string())?;
            assert_eq!(sim_harness::run_executable(&exe)?, expected, "{variant}");
        }
        Ok(())
    })
    .expect("real conversion simulations");
}

#[test]
fn real_conversion_functions_reject_wrong_argument_shapes() {
    let cases = [
        (
            "bitstoreal_narrow",
            "$bitstoreal(32'd1)",
            "$bitstoreal requires an exactly 64-bit packed argument",
        ),
        (
            "bitstoreal_real",
            "$bitstoreal(1.0)",
            "$bitstoreal requires an exactly 64-bit packed argument",
        ),
        (
            "bitstoshortreal_wide",
            "$bitstoshortreal(64'd1)",
            "$bitstoshortreal requires an exactly 32-bit packed argument",
        ),
        (
            "bitstoshortreal_real",
            "$bitstoshortreal(1.0)",
            "$bitstoshortreal requires an exactly 32-bit packed argument",
        ),
        (
            "rtoi_missing",
            "$rtoi()",
            "$rtoi requires exactly one argument",
        ),
        (
            "itor_extra",
            "$itor(1, 2)",
            "$itor requires exactly one argument",
        ),
    ];

    for (case, expression, expected) in cases {
        sim_harness::with_surelog_temp_cwd(case, |dir| {
            let path = dir.join("tb.sv");
            std::fs::write(
                &path,
                format!("module tb; initial $display(\"%0d\", {expression}); endmodule\n"),
            )
            .map_err(|error| error.to_string())?;
            let compiled = compile::compile_checked(&compile::CompileOpts {
                files: vec![path.to_string_lossy().into_owned()],
                top: Some("tb".to_owned()),
                ..Default::default()
            })
            .map_err(|error| error.to_string())?;
            let error = match sim::codegen::generate(compiled.uhdm_design().ok_or("no design")?) {
                Ok(_) => return Err("invalid conversion unexpectedly succeeded".to_owned()),
                Err(error) => error.to_string(),
            };
            assert!(error.contains(expected), "{case}: {error}");
            Ok(())
        })
        .expect("real conversion rejection");
    }
}

#[test]
fn real_conversion_constant_declaration_initializers() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"module tb;
    integer from_real = $rtoi(7);
    real from_integer = $itor(1.9);
    reg [63:0] bits = $realtobits(1);
    real from_bits = $bitstoreal(64'h4000000000000000);
    reg [31:0] short_bits = $shortrealtobits(1);
    shortreal from_short_bits = $bitstoshortreal(32'h40400000);

    initial begin
        $display("init=%0d %.1f %h %.1f %h %.1f", from_real, from_integer,
                 bits, from_bits, short_bits, from_short_bits);
        $finish;
    end
endmodule
"#;

    sim_harness::with_surelog_temp_cwd("real_conversion_init", |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let model = sim::codegen::generate(compiled.uhdm_design().ok_or("no design")?)
            .map_err(|error| error.to_string())?;
        let exe = sim::build::build_model_cmake(dir, &[("model.c", &model.model_c)])
            .map_err(|error| error.to_string())?;
        assert_eq!(
            sim_harness::run_executable(&exe)?,
            "init=7 2.0 3ff0000000000000 2.0 3f800000 3.0\n"
        );
        Ok(())
    })
    .expect("real conversion declaration initializers");
}

#[test]
fn real_conversion_typed_localparams_resolve_in_shared_elaboration() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"module tb;
    localparam integer FROM_REAL = $rtoi(7);
    localparam real FROM_INTEGER = $itor(1.9);
    localparam logic [63:0] REAL_BITS = $realtobits(1);
    localparam real FROM_BITS = $bitstoreal(64'h4000000000000000);
    localparam logic [31:0] SHORT_BITS = $shortrealtobits(1);
    localparam shortreal FROM_SHORT_BITS = $bitstoshortreal(32'h40400000);

    initial begin
        $display("params=%0d %.1f %h %.1f %h %.1f", FROM_REAL, FROM_INTEGER,
                 REAL_BITS, FROM_BITS, SHORT_BITS, FROM_SHORT_BITS);
        $finish;
    end
endmodule
"#;

    sim_harness::with_surelog_temp_cwd("real_conversion_params", |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let model = sim::codegen::generate(compiled.uhdm_design().ok_or("no design")?)
            .map_err(|error| error.to_string())?;
        let exe = sim::build::build_model_cmake(dir, &[("model.c", &model.model_c)])
            .map_err(|error| error.to_string())?;
        assert_eq!(
            sim_harness::run_executable(&exe)?,
            "params=7 2.0 3ff0000000000000 2.0 3f800000 3.0\n"
        );
        Ok(())
    })
    .expect("real conversion localparams");
}
