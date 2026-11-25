//! The node lookups of lowering grow with the design: no check walks every
//! node once per construct.
use super::*;
use crate::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use crate::work::{counted, Work};

const SMALL: usize = 25;
const LARGE: usize = 400;

/// Lookups that lower `instances` instances of a module with `body`.
fn lookups(body: &str, instances: usize) -> usize {
    let output = compile_sources_checked(
        &[OwnedSource::compilation_unit(
            "scaling.sv",
            format!(
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
    counted(Work::NodeReads, || {
        lower_model(&db, &CodegenOptions::default())
            .unwrap_or_else(|error| panic!("{body}: {error}"));
    })
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
        let growth = LARGE / SMALL;
        assert!(
            large <= small * growth * 5 / 4,
            "{small} lookups for {SMALL} instances, {large} for {LARGE}: {body}"
        );
    }
}

/// Lookups that lower `levels` subroutines that each call the next one twice,
/// and the process or assignment that calls the first.
fn diamond_lookups(
    subroutine: &dyn Fn(usize, &str) -> String,
    last: &str,
    user: &str,
    levels: usize,
) -> usize {
    let chain: String = (0..levels)
        .map(|level| subroutine(level, &format!("s{}", level + 1)))
        .collect();
    let last = last.replace("LAST", &format!("s{levels}"));
    let output = compile_sources_checked(
        &[OwnedSource::compilation_unit(
            "diamond.sv",
            format!("module tb; logic clk; logic [7:0] d, e, y; {last} {chain} {user} endmodule"),
        )],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    let db = Db::from_slang(&output.snapshot).unwrap();
    counted(Work::NodeReads, || {
        lower_model(&db, &CodegenOptions::default())
            .unwrap_or_else(|error| panic!("{user} of {levels}: {error}"));
    })
}

/// A call graph with two ways from each level to the next has 2^levels call
/// paths; the walks over it visit each body once.
#[test]
fn lookups_do_not_follow_every_call_path() {
    let value = |qualifier: &'static str| {
        move |level: usize, next: &str| {
            format!(
                "function {qualifier} logic [7:0] s{level}(input logic [7:0] x); \
                 s{level} = {next}(x) ^ {next}(x + 1); endfunction\n"
            )
        }
    };
    let reference = |level: usize, next: &str| {
        format!(
            "function automatic void s{level}(ref logic [7:0] x); {next}(x); {next}(x); endfunction\n"
        )
    };
    let swapped = |level: usize, next: &str| {
        format!(
            "function automatic void s{level}(ref logic [7:0] x, ref logic [7:0] z); \
             {next}(x, z); {next}(z, x); endfunction\n"
        )
    };
    let task = |level: usize, next: &str| {
        format!(
            "task automatic s{level}(input logic [7:0] x); {next}(x); #1 {next}(x + 1); endtask\n"
        )
    };
    let waiting = |level: usize, next: &str| {
        format!("task automatic s{level}(ref logic [7:0] x); {next}(x); {next}(x); endtask\n")
    };
    let handle = |level: usize, next: &str| {
        format!(
            "function automatic int s{level}(input c x); return {next}(x) + {next}(x); endfunction\n"
        )
    };
    let leaf = "function automatic logic [7:0] LAST(input logic [7:0] x); \
                LAST = x ^ e; endfunction";
    let guarded = "function automatic logic [7:0] LAST(input logic [7:0] x); \
                   LAST = x > 3 ? s0(x - 1) : e; endfunction";
    let read = "class c; int v; endclass c h = new; \
                function automatic int LAST(input c x); return x.v; endfunction";
    let store = "function automatic void LAST(ref logic [7:0] x); x = e; endfunction";
    let swap = "function automatic void LAST(ref logic [7:0] x, ref logic [7:0] z); \
                x = z ^ e; endfunction";
    let wait = "task automatic LAST(ref logic [7:0] x); @(x); endtask";
    let again = "task automatic LAST(ref logic [7:0] x); @(x); if (d == 3) s0(x); endtask";
    let write = "task automatic LAST(input logic [7:0] x); y = x ^ e; endtask";
    type Level<'a> = &'a dyn Fn(usize, &str) -> String;
    let cases: [(&str, Level, &str, &str); 15] = [
        ("assign", &value("automatic"), leaf, "assign y = s0(d);"),
        (
            "always_comb",
            &value("automatic"),
            leaf,
            "always_comb y = s0(d);",
        ),
        ("static functions", &value(""), leaf, "always @* y = s0(d);"),
        (
            "recursion",
            &value("automatic"),
            guarded,
            "always_comb y = s0(d);",
        ),
        (
            "wait",
            &value("automatic"),
            leaf,
            "initial begin wait (s0(d) == 3); y = 1; end",
        ),
        (
            "event control",
            &value("automatic"),
            leaf,
            "initial begin @(s0(d)) y = 1; end",
        ),
        (
            "initializer",
            &value("automatic"),
            leaf,
            "logic [7:0] v = s0(8'd3); initial y = v;",
        ),
        ("reference", &reference, store, "always_comb s0(y);"),
        ("swapped references", &swapped, swap, "initial s0(y, d);"),
        ("tasks", &task, write, "initial s0(d);"),
        ("waiting tasks", &waiting, wait, "initial s0(y);"),
        ("recursive waiting tasks", &waiting, again, "initial s0(y);"),
        (
            "monitor",
            &value("automatic"),
            leaf,
            "initial $monitor(s0(d));",
        ),
        (
            "waited handles",
            &handle,
            read,
            "initial begin wait (s0(h) == 3); y = 1; end",
        ),
        (
            "monitored handles",
            &handle,
            read,
            "initial $monitor(s0(h));",
        ),
    ];
    let mut failures = Vec::new();
    for (name, subroutine, last, user) in cases {
        let counts: Vec<usize> = [4, 8, 12]
            .iter()
            .map(|levels| diamond_lookups(subroutine, last, user, *levels))
            .collect();
        println!("{name}: {counts:?}");
        if counts[2] > counts[0] * 4 {
            failures.push(format!("{name}: {counts:?} lookups for 4, 8 and 12 levels"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
