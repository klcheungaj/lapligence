// IEEE 1800-2009 10.3.3: a continuous assignment delay is inertial, so a new
// evaluation replaces the pending update. The LRM does not say what happens
// when only the left-hand selector changes while an update is pending;
// lapligence re-evaluates the whole assignment (fixture readme), so the newest
// evaluation's selector and value replace the pending update and the
// previously selected element keeps its value.
`timescale 1ns / 1ns
module tb;
  logic [3:0] a;
  logic [1:0] i = 2'd0;
  logic x = 1'b0;
  assign #5 a[i] = x;

  logic [7:0] v;
  logic [2:0] j = 3'd0;
  logic y = 1'b1;
  assign #4 v[j] = y;

  initial begin
    #10 x = 1'b1;
    #2 i = 2'd1;
    #1 $display("%0t a=%b", $time, a);
    #3 $display("%0t a=%b", $time, a);
    #5 $display("%0t a=%b v=%b", $time, a, v);
    j = 3'd2;
    #2 y = 1'b0;
    #1 j = 3'd3;
    #2 $display("%0t v=%b", $time, v);
    #3 $display("%0t v=%b", $time, v);
    $finish(0);
  end
endmodule
