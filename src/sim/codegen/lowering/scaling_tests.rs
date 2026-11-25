//! The node lookups of lowering grow with the design: no check walks every
//! node once per construct.
use super::*;
use crate::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use crate::core::db::NODE_READS;

const SMALL: usize = 25;
const LARGE: usize = 400;

/// Lookups that lower `instances` instances of a module with `body`.
fn lookups(body: &str, instances: usize) -> u64 {
    let output = compile_sources_checked(
        &[OwnedSource::compilation_unit(
            "scaling.sv",
            &format!(
                "module m(input logic clk, input logic [7:0] d); {body} endmodule \
                 module tb; logic clk; logic [7:0] d; \
                 for (genvar g = 0; g < {instances}; g++) begin : gen \
                 m u(.clk(clk), .d(d)); end endmodule"
            ),
        )],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    let db = Db::from_slang(&output.snapshot).unwrap();
    NODE_READS.with(|reads| reads.set(0));
    lower_model(&db, &CodegenOptions::default()).unwrap_or_else(|error| panic!("{body}: {error}"));
    NODE_READS.with(std::cell::Cell::get)
}

#[test]
fn lookups_grow_with_the_design() {
    for body in [
        "logic [7:0] q, n; assign n = q + d; always_ff @(posedge clk) q <= n;",
        "function [7:0] f(input [7:0] x); f = x + 1; endfunction logic [7:0] a; assign a = f(d);",
        "typedef struct { int x; string s; int q[$]; } t; t s; initial s.q.push_back(1);",
        "typedef enum logic [1:0] {A, B, C} e; e s; int i; initial if ($cast(s, i)) i = s;",
        "if (1) begin : a if (1) begin : b logic q; always_ff @(posedge clk) q <= d[0]; end end",
        "initial begin : b forever begin @(posedge clk); if (d == 3) disable b; end end",
        "initial begin fork : f #1; #2; join_any disable f; end",
        "global clocking g @(posedge clk); endclocking a: assert property (@(posedge clk) $rose_gclk(d[0]));",
        "logic [7:0] mem[4]; always @(posedge clk) mem[d[1:0]] <= d;",
        "class c; int v; virtual function int g(); return v; endfunction endclass \
         initial begin c h = new; h.v = h.g(); end",
    ] {
        let (small, large) = (lookups(body, SMALL), lookups(body, LARGE));
        let growth = (LARGE / SMALL) as u64;
        assert!(
            large <= small * growth * 5 / 4,
            "{small} lookups for {SMALL} instances, {large} for {LARGE}: {body}"
        );
    }
}
