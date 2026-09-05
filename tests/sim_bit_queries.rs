//! Packed bit-vector query results, widths, and optimizer read dependencies.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

#[test]
fn bit_queries_preserve_four_state_and_wide_operand_semantics() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/1ps
module tb;
    localparam integer N = $countones(4'b1xz1);
    localparam ONE = $onehot(4'b1xz0);
    localparam ZERO = $onehot0(4'bxxzz);
    localparam UNKNOWN = $isunknown(4'b00z0);
    reg [3:0] v;
    reg [1023:0] wide;
    integer calls;
    integer initialized_count = $countones(4'b1xz1);
    reg initialized_one = $onehot(4'b1xz0);
    reg initialized_zero = $onehot0(4'bxxzz);
    reg initialized_unknown = $isunknown(4'b00z0);
    wire [31:0] count;
    assign count = $countones(v);
    function automatic [3:0] sample;
        begin
            calls = calls + 1;
            sample = v;
        end
    endfunction
    initial begin
        $display("init=%0d %0d %0d %0d", initialized_count, initialized_one, initialized_zero, initialized_unknown);
        $display("param=%0d %0d %0d %0d", N, ONE, ZERO, UNKNOWN);
        v = 4'b0000;
        #1;
        $display("%0d %0d %0d %0d", count, $onehot(v), $onehot0(v), $isunknown(v));
        v = 4'b1xz0;
        #1;
        $display("%0d %0d %0d %0d", count, $onehot(v), $onehot0(v), $isunknown(v));
        v = 4'b11xz;
        #1;
        $display("%0d %0d %0d %0d", count, $onehot(v), $onehot0(v), $isunknown(v));
        v = 4'bxxzz;
        #1;
        $display("%0d %0d %0d %0d", count, $onehot(v), $onehot0(v), $isunknown(v));
        wide = '1;
        $display("wide=%0d %0d %0d %0d", $countones(wide), $onehot(wide), $onehot0(wide), $isunknown(wide));
        wide = '0;
        wide[1023] = 1;
        wide[65] = 1'bx;
        wide[64] = 1'bz;
        $display("high=%0d %0d %0d %0d", $countones(wide), $onehot(wide), $onehot0(wide), $isunknown(wide));
        $display("width=%0d %0d %0d %0d", $bits($countones(v)), $bits($onehot(v)), $bits($onehot0(v)), $bits($isunknown(v)));
        $display("signed=%0d", $countones(v) - 32'sd1);
        $display("literal=%0d %0d %0d %0d", $countones(4'b1xz0), $onehot(4'b1xz0), $onehot0(4'bxxzz), $isunknown(4'b0000));
        calls = 0;
        $display("sample=%0d", $countones(sample()));
        $display("sample=%0d", $onehot(sample()));
        $display("sample=%0d", $onehot0(sample()));
        $display("sample=%0d", $isunknown(sample()));
        $display("calls=%0d", calls);
        $finish;
    end
endmodule
"#;
    let expected = "init=2 1 1 1\nparam=2 1 1 1\n0 0 1 0\n1 1 1 1\n2 0 0 1\n0 0 1 1\nwide=1024 0 0 0\nhigh=1 1 1 1\nwidth=32 1 1 1\nsigned=-1\nliteral=1 1 1 0\nsample=0\nsample=0\nsample=1\nsample=1\ncalls=4\n";
    sim_harness::with_surelog_temp_cwd("bit_queries", |dir| {
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
    .expect("bit query simulations");
}

#[test]
fn bit_queries_reject_real_operands() {
    for name in ["$countones", "$onehot", "$onehot0", "$isunknown"] {
        sim_harness::with_surelog_temp_cwd("bit_query_real", |dir| {
            let path = dir.join("tb.sv");
            std::fs::write(
                &path,
                format!("module tb; real r; initial $display(\"%0d\", {name}(r)); endmodule"),
            )
            .map_err(|error| error.to_string())?;
            let compiled = compile::compile_checked(&compile::CompileOpts {
                files: vec![path.to_string_lossy().into_owned()],
                top: Some("tb".to_owned()),
                ..Default::default()
            })
            .map_err(|error| error.to_string())?;
            let error = match sim::codegen::generate(compiled.uhdm_design().ok_or("no design")?) {
                Ok(_) => return Err("real query unexpectedly succeeded".to_owned()),
                Err(error) => error.to_string(),
            };
            assert!(
                error.contains("requires a packed integral argument"),
                "{name}: {error}"
            );
            Ok(())
        })
        .expect("real operand rejection");
    }
}
