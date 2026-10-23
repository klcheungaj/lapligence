// Decision A12-D3: an SDF value too large for the simulation time range is
// annotated as one unit of the annotated module's precision, with a warning.
// The text does not say what an unrepresentable value becomes.
//
// IEEE 1800-2009 32.3 (SystemVerilog-1800-2009.txt L52627-52628): "It shall
// issue a warning for any data it is unable to annotate."
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
    #10 $sdf_annotate("A12-D3_out_of_range_value.sdf");
    #10 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
