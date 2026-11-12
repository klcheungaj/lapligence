// SIM-043 T1-D11: a value-change callback (value_changes.c) sees one change
// per check: a value changed and changed back within one process turn or
// one NBA batch calls nothing; a print call, a #0 delay and a later time
// step each end a turn.
`timescale 1ns / 1ns
module tb;
  logic g, h, k, v;
  logic [7:0] w;
  real r;
  initial begin
    k <= 1'b0;
    #3 k <= 1'b1;
    k <= 1'b0;
  end
  initial begin
    g = 1'b0;
    h = 1'b0;
    v = 1'b0;
    w = 8'h00;
    r = 0.0;
    #1 g = 1'b1;
    g = 1'b0;
    #1 h = 1'b1;
    #0 h = 1'b0;
    #2 v = 1'b1;
    $display("%0t print", $time);
    v = 1'b0;
    #1 w = 8'h01;
    w = 8'h02;
    #1 r = 1.5;
    r = 0.0;
    #1 r = 2.5;
    #1 g = 1'b1;
    #1 $finish;
  end
endmodule
