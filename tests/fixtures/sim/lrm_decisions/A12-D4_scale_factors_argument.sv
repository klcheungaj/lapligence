// Decision A12-D4: the scale_factors argument multiplies the selected
// member of each min:typ:max triple by that member's factor.
//
// IEEE 1800-2009 32.9 (SystemVerilog-1800-2009.txt L53180-53183):
// "scale_factors-is an optional character string argument specifying the
// scale factors to be used while annotating timing values. For example,
// "1.6:1.4:1.2" causes minimum values to be multiplied by 1.6, typical
// values by 1.4, and maximum values by 1.2."
`timescale 1ns/1ps
module BUF (input a, output y);
  buf b (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule
module tb;
  reg a = 1'b0;
  wire y;
  BUF u (a, y);
  initial begin
    #10 $sdf_annotate("A12-D4_scale_factors_argument.sdf", , , , "TYPICAL", "1:2.5:3");
    #10 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
