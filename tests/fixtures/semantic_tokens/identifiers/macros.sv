`include "defs.svh"
`include "missing.svh"
`define LOCAL_W 8
`define LEVEL_A
`ifdef LEVEL_A
`define PICKED 1
`else
`define PICKED 2
`endif
`ifndef UNDEFINED_GUARD
`endif
`ifdef UNDEFINED_A
`elsif LEVEL_A
`endif
module macro_top;
  logic [`LOCAL_W-1:0] first;
`define LOCAL_W 16
  logic [`LOCAL_W-1:0] second;
`undef LOCAL_W
  logic [`LOCAL_W-1:0] third;
  logic [`HDR_W-1:0] sum;
  logic [`CONFIG_W-1:0] configured;
  localparam int line = `__LINE__;
  assign sum = `SUM(first[0], `SUM(second[1], sum_late));
  logic sum_late;
  int picked = `PICKED;
endmodule
