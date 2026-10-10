// Decision S38-D6: a concurrent assertion that makes up an initial procedure
// begins one evaluation attempt, at the first leading clock event; it does
// not start an attempt on every clock tick like a static assertion.
//
// IEEE 1800-2009 16.15.6 (SystemVerilog-1800-2009.txt L26750-26753): "the
// assertion ... [is] placed in a procedural assertion queue associated with
// the currently executing process", and an instance that matures before its
// clocking event "will cause the assertion to begin an evaluation attempt upon
// the next clocking event" (L26758-26766). An initial procedure executes once.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  initial i1: assert property (@(posedge clk) a ##1 b)
    $display("%0t i1 pass", $time); else $display("%0t i1 fail", $time);
  initial begin
    a = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    b = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    b = 1'b0;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #5 clk = 1'b1;
    #1 $finish;
  end
endmodule
