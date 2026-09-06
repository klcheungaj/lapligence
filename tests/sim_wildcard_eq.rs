//! End-to-end wildcard equality tests (LRM 1800-2009 §11.4.6).
//!
//! Both optimizer configurations run the same four-state cases so constant
//! folding and the generated runtime are required to agree.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};
use std::path::Path;

#[test]
fn wildcard_equality_rhs_wildcards_unknowns_and_coercion() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/1ps
module tb;
    logic [3:0] lhs;
    logic [3:0] rhs;
    logic [3:0] add_a;
    logic [3:0] add_b;
    logic [4:0] add_rhs;
    logic [1023:0] wide_lhs;
    logic [1023:0] wide_rhs;
    logic init_match = 4'b1010 ==? 4'b1?1?;
    integer inside_hit;
    integer selector_calls;

    function automatic [3:0] sample_selector;
        begin
            selector_calls = selector_calls + 1;
            sample_selector = 4'd8;
        end
    endfunction

    initial begin
        lhs = 4'b10xz;
        rhs = 4'b10xz;
        $display("wild=%b neq=%b", lhs ==? rhs, lhs !=? rhs);

        rhs = 4'b10?0;
        $display("unknown=%b neq=%b", lhs ==? rhs, lhs !=? rhs);

        lhs = 4'b11xz;
        $display("mismatch=%b neq=%b", lhs ==? rhs, lhs !=? rhs);

        lhs = 4'b1x01;
        rhs = 4'b1001;
        $display("rhs_only=%b %b", lhs ==? rhs, rhs ==? lhs);

        $display("width_signed=%b %b %b",
                 4'sh9 ==? 8'shf9,
                 4'sh9 ==? 8'h09,
                 4'sh9 ==? 8'sh?9);
        $display("fills=%b %b", 8'ha5 ==? 'x, 'x ==? 8'ha5);

        add_a = 4'd15;
        add_b = 4'd1;
        add_rhs = 5'd16;
        $display("nested=%b init=%b", (add_a + add_b) ==? add_rhs, init_match);

        lhs = 4'b0x0;
        case (lhs) inside
            4'b0?0, [4:7]: inside_hit = 1;
            default: inside_hit = 0;
        endcase
        $display("inside_wild=%0d", inside_hit);
        lhs = 4'd6;
        case (lhs) inside
            4'b0?0, [4:7]: inside_hit = 1;
            default: inside_hit = 0;
        endcase
        $display("inside_range=%0d", inside_hit);
        lhs = 4'd8;
        case (lhs) inside
            4'b0?0, [4:7]: inside_hit = 1;
            default: inside_hit = 0;
        endcase
        $display("inside_default=%0d", inside_hit);
        selector_calls = 0;
        case (sample_selector()) inside
            4'd1: inside_hit = 1;
            [2:3]: inside_hit = 2;
            [7:9]: inside_hit = 3;
            default: inside_hit = 0;
        endcase
        $display("inside_once=%0d calls=%0d", inside_hit, selector_calls);

        wide_lhs = '0;
        wide_rhs = '0;
        wide_lhs[900] = 1'b1;
        wide_lhs[17] = 1'bx;
        $display("wide_mismatch=%b", wide_lhs ==? wide_rhs);
        wide_rhs[900] = 1'b1;
        $display("wide_unknown=%b", wide_lhs ==? wide_rhs);
        wide_rhs[17] = 1'b?;
        $display("wide_wild=%b", wide_lhs ==? wide_rhs);

        $display("constant=%b %b %b",
                 4'b1010 ==? 4'b1?1?,
                 4'b1010 !=? 4'b1?0?,
                 4'b1x10 ==? 4'b1010);
        $finish;
    end
endmodule
"#;
    let expected = "wild=1 neq=0\n\
unknown=x neq=x\n\
mismatch=0 neq=1\n\
rhs_only=x 1\n\
width_signed=1 1 1\n\
fills=1 x\n\
nested=1 init=1\n\
inside_wild=1\n\
inside_range=1\n\
inside_default=0\n\
inside_once=3 calls=1\n\
wide_mismatch=0\n\
wide_unknown=x\n\
wide_wild=1\n\
constant=1 1 x\n";

    sim_harness::with_surelog_temp_cwd("wildcard_eq", |dir| {
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

        for (variant, opts) in [
            ("opt_on", OptConfig::default()),
            ("opt_off", OptConfig::none()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &opts)
                .map_err(|error| error.to_string())?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| error.to_string())?;
            assert_eq!(
                sim_harness::run_executable(&executable)?,
                expected,
                "{variant}"
            );
        }
        Ok(())
    })
    .expect("wildcard equality simulations");
}

#[test]
fn wildcard_equality_accepts_wide_expression_context() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/wildcard_eq/wide_context.sv");
    sim_harness::with_surelog_temp_cwd("wildcard_eq_wide_context", |dir| {
        let path = dir.join("wide_context.sv");
        std::fs::copy(&fixture, &path).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::build_with_source_files(
            compiled.uhdm_design().ok_or("no design")?,
            &compiled.frontend_source_files(),
        )
        .map_err(|error| error.to_string())?;
        for (variant, options) in [
            ("opt_on", OptConfig::default()),
            ("opt_off", OptConfig::none()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &options)
                .map_err(|error| format!("{variant} lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} C model build: {error}"))?;
            assert_eq!(
                sim_harness::run_executable(&executable)?,
                "PASS wildcard_wide_context\n",
                "{variant}"
            );
        }
        Ok(())
    })
    .expect("wide wildcard context simulation");
}
