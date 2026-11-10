// Decision K1-D2: an edge input skew (`input negedge x` under
// `@(posedge clk)`) takes the #1step sample at each such edge of the clock
// signal, and the clockvar holds that sample from the edge on: a read
// between the negedge and the next posedge already sees it. Before the first
// skew edge a four-state clockvar is X.
//
// IEEE 1800-2009 14.3 (SystemVerilog-1800-2009.txt L19457-19458): "When the
// clocking event specifies a simple edge, instead of a number, the skew can
// be specified as the specific edge of the signal." 14.13 L19949-19950: "When
// an input or inout clockvar appears in any expression its value is the
// signal's sampled value. That is, the value that the clocking block sampled
// at the most recent clocking event." Read literally, 14.13 would keep the
// previous clocking event's sample until the posedge; llg follows
// industry-standard behaviour, which publishes at the skew edge.
`timescale 1ns/1ns
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  logic [3:0] x = 4'h5;
  logic [3:0] w;
  real r = 1.5;
  clocking cb @(posedge clk);
    input negedge x, w, r;
  endclocking
  clocking cn @(negedge clk);
    input posedge x;
  endclocking
  initial begin
    #2 w = 4'h3;
    #6 x = 4'h6; r = 2.5;
    #10 x = 4'h7; w = 4'h4; r = 3.5;
  end
  initial begin
    #1 $display("%0t cb.x %h cb.w %h cb.r %0.1f cn.x %h", $time, cb.x, cb.w, cb.r, cn.x);
    #11 $display("%0t cb.x %h cb.w %h cb.r %0.1f", $time, cb.x, cb.w, cb.r);
  end
  always @(cb) $display("%0t cb x=%h w=%h r=%0.1f", $time, cb.x, cb.w, cb.r);
  always @(cn) $display("%0t cn x=%h", $time, cn.x);
  initial #36 $finish;
endmodule
