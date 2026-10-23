// Decision A12-D6: the settings of a configuration file apply to the call
// that names it only; a later call without one uses the defaults (the
// typical member, unscaled).
//
// IEEE 1800-2009 32.9 (SystemVerilog-1800-2009.txt L53158-53159):
// "config_file-is an optional character string argument providing the name
// of a configuration file. Information in this file can be used to provide
// detailed control over many aspects of annotation." The text does not
// say whether the information outlives the call.
`timescale 1ns/1ps
module BUF (input a, output y);
  buf b (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule
module tb;
  reg a = 1'b0;
  wire [1:0] y;
  BUF u0 (a, y[0]);
  BUF u1 (a, y[1]);
  initial begin
    #10 $sdf_annotate("A12-D6_config_file_per_call.sdf", tb.u0, "A12-D6_config_file_per_call.cfg");
    $sdf_annotate("A12-D6_config_file_per_call.sdf", tb.u1);
    #10 a = 1'b1;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
