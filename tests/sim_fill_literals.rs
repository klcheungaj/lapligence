//! End-to-end unbased unsized fill-literal tests (LRM 1800-2009 §5.7.1).
//!
//! The same design runs with optimization enabled and disabled because fill
//! identity must survive until each context-determined expression is sized.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

#[test]
fn fill_literals_follow_expression_contexts() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"// llg-test-fixture: tests/sim_fill_literals.rs/contexts.sv
module tb;
    logic condition;
    logic [7:0] zeros, ones, xs, zs, ternary;
    logic [15:0] dynamic_value, mask, arithmetic, nested;
    logic [1023:0] wide, wide_mask;
    integer hit;

    function automatic [15:0] take16(input [15:0] value);
        take16 = value;
    endfunction

    initial begin
        zeros = '0;
        ones = '1;
        xs = 'x;
        zs = 'z;
        dynamic_value = 16'h0000;
        mask = 16'h5aa5;
        condition = 1'b1;
        $display("direct=%b %b %b %b", zeros, ones, xs, zs);
        $display("self=%b bits=%0d", '1, $bits('z));
        $display("self_expr=%h concat=%b repeat=%b",
                 condition ? '1 : 8'h00, {'1, '0, 'x, 'z}, {4{'1}});

        condition = 1'b1;
        ternary = condition ? '1 : 8'h00;
        $display("ternary_true=%h", ternary);
        condition = 1'b0;
        ternary = condition ? 8'h00 : '1;
        $display("ternary_false=%h", ternary);
        condition = 1'bx;
        ternary = condition ? '1 : 8'hff;
        $display("ternary_merge=%h", ternary);

        arithmetic = dynamic_value + '1;
        nested = dynamic_value | ('1 & mask);
        $display("expr=%h %h fn=%h", arithmetic, nested,
                 take16('1 & 16'h33cc));
        $display("compare=%b %b %b %b %b",
                 (dynamic_value | 16'hffff) == '1,
                 '1 == (dynamic_value | 16'hffff),
                 (dynamic_value | 16'hxxxx) === 'x, zs === 'z,
                 '1 > (dynamic_value | 16'hfffe));

        hit = 0;
        case ('1)
            4'hf: hit = 1;
            8'hff: hit = 2;
            default: hit = -1;
        endcase
        $display("case_selector=%0d", hit);
        case (16'hffff)
            '1: hit = 3;
            default: hit = -1;
        endcase
        $display("case_item=%0d", hit);
        casez (16'hffff)
            '1: hit = 4;
            default: hit = -1;
        endcase
        $display("casez_item=%0d", hit);
        casex (16'hxxxx)
            'x: hit = 5;
            default: hit = -1;
        endcase
        $display("casex_item=%0d", hit);

        wide_mask = {1024{1'b1}};
        wide = '1 & wide_mask;
        $display("wide_ones=%0d ends=%b%b", $countones(wide),
                 wide[1023], wide[0]);
        wide = 1'b0 ? '0 : 'z;
        $display("wide_z=%b case=%b", $isunknown(wide), wide === 'z);
        $finish;
    end
endmodule
"#;
    let expected = concat!(
        "direct=00000000 11111111 xxxxxxxx zzzzzzzz\n",
        "self=1 bits=1\n",
        "self_expr=ff concat=10xz repeat=1111\n",
        "ternary_true=ff\n",
        "ternary_false=ff\n",
        "ternary_merge=ff\n",
        "expr=ffff 5aa5 fn=33cc\n",
        "compare=1 1 1 1 1\n",
        "case_selector=2\n",
        "case_item=3\n",
        "casez_item=4\n",
        "casex_item=5\n",
        "wide_ones=1024 ends=11\n",
        "wide_z=1 case=1\n",
    );

    sim_harness::with_surelog_temp_cwd("fill_literal_contexts", |dir| {
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
            ("opt_off", OptConfig::none()),
            ("opt_on", OptConfig::default()),
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
    .expect("fill-literal context simulations");
}

#[test]
fn wide_context_rejects_nested_limited_operation() {
    let source = r#"// llg-test-fixture: tests/sim_fill_literals.rs/wide_div.sv
module tb;
    logic [7:0] dividend, divisor;
    logic [127:0] comparison_rhs;
    logic result;
    initial result = (dividend / divisor) == comparison_rhs;
endmodule
"#;

    sim_harness::with_surelog_temp_cwd("fill_wide_div_context", |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let error = match sim::codegen::generate(compiled.uhdm_design().ok_or("no design")?) {
            Ok(_) => return Err("wide comparison context was accepted".to_owned()),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("comparison context wider than 64 bits"),
            "unexpected error: {error}"
        );
        Ok(())
    })
    .expect("wide nested limited operation rejection");
}
